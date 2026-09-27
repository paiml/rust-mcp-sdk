//! AST-based JavaScript execution for Code Mode.
//!
//! This module provides secure execution of validated JavaScript code by:
//! 1. Compiling the SWC AST to an ExecutionPlan
//! 2. Executing the plan in pure Rust (no JS runtime)
//!
//! ## Security Model
//!
//! Only operations that can be represented in the ExecutionPlan are allowed.
//! The plan is a simple tree structure that's fully auditable before execution.
//!
//! ## Supported Operations
//!
//! - API calls: `api.get()`, `api.post()`, `api.put()`, `api.delete()`, `api.patch()`
//! - Variable assignment: `const x = ...`
//! - Property access: `user.id`, `response.data`
//! - Array methods: `.map()`, `.filter()`, `.slice()`, `.find()`, `.length`
//! - Template literals: `` `/users/${id}` ``
//! - Object literals: `{ name: "test", id: 123 }`
//! - Array literals: `[1, 2, 3]`
//! - Conditionals: `if/else`
//! - Bounded loops: `for (const x of arr.slice(0, N))`
//! - Return statements

use crate::javascript::HttpMethod;
use crate::types::ExecutionError;
use serde::Serialize;
use serde_json::Value as JsonValue;
use std::collections::{HashMap, HashSet};

// Import shared evaluation functions
use crate::eval::{
    evaluate as shared_evaluate, is_truthy as shared_is_truthy,
    json_to_string_with_mode as shared_json_to_string_with_mode, JsonStringMode,
};
use swc_common::{FileName, SourceMap};
use swc_ecma_ast::*;
use swc_ecma_parser::{lexer::Lexer, Parser, StringInput, Syntax};

/// Internal control flow outcome from executing a single plan step.
///
/// Replaces the previous pattern of abusing `ExecutionError::LoopContinue`
/// and `ExecutionError::LoopBreak` for non-error control flow.
pub(crate) enum StepOutcome {
    /// Step completed, no value produced.
    None,
    /// Step produced a return value (exits function/plan).
    Return(serde_json::Value),
    /// Loop continue signal (skip to next iteration).
    Continue,
    /// Loop break signal (exit current loop).
    Break,
}

/// Configuration for execution.
#[derive(Debug, Clone)]
pub struct ExecutionConfig {
    /// Maximum number of API calls allowed
    pub max_api_calls: usize,
    /// Maximum execution time in seconds
    pub timeout_seconds: u64,
    /// Maximum loop iterations
    pub max_loop_iterations: usize,
    /// Fields that should be filtered from API responses (internal blocklist).
    /// These fields are stripped from responses before scripts can access them.
    /// Field names are case-sensitive and matched at any nesting level.
    pub blocked_fields: HashSet<String>,
    /// Fields that cannot appear in script output (output blocklist).
    /// These fields can be used internally but cannot be returned by the script.
    /// Field names are case-sensitive and matched at any nesting level.
    pub output_blocked_fields: HashSet<String>,
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            max_api_calls: 50,
            timeout_seconds: 30,
            max_loop_iterations: 100,
            blocked_fields: HashSet::new(),
            output_blocked_fields: HashSet::new(),
        }
    }
}

impl ExecutionConfig {
    /// Create a new config with blocked fields for API response filtering.
    pub fn with_blocked_fields(
        mut self,
        fields: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.blocked_fields = fields.into_iter().map(Into::into).collect();
        self
    }

    /// Create a new config with output blocked fields for return value validation.
    pub fn with_output_blocked_fields(
        mut self,
        fields: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.output_blocked_fields = fields.into_iter().map(Into::into).collect();
        self
    }
}

/// Recursively filter blocked fields from a JSON value.
///
/// This removes any fields whose names are in the blocklist from objects,
/// and recursively processes nested objects and arrays.
///
/// # Arguments
///
/// * `value` - The JSON value to filter
/// * `blocked_fields` - Set of field names to remove
///
/// # Returns
///
/// A new JSON value with blocked fields removed.
pub fn filter_blocked_fields(value: JsonValue, blocked_fields: &HashSet<String>) -> JsonValue {
    if blocked_fields.is_empty() {
        return value;
    }

    match value {
        JsonValue::Object(mut map) => {
            // Remove blocked fields at this level
            map.retain(|key, _| !blocked_fields.contains(key));

            // Recursively filter remaining values
            let filtered: serde_json::Map<String, JsonValue> = map
                .into_iter()
                .map(|(k, v)| (k, filter_blocked_fields(v, blocked_fields)))
                .collect();

            JsonValue::Object(filtered)
        },
        JsonValue::Array(arr) => {
            // Recursively filter each element
            let filtered: Vec<JsonValue> = arr
                .into_iter()
                .map(|v| filter_blocked_fields(v, blocked_fields))
                .collect();

            JsonValue::Array(filtered)
        },
        // Non-container values pass through unchanged
        other => other,
    }
}

/// Find blocked fields that appear in a JSON value (output validation).
///
/// Unlike `filter_blocked_fields` which silently removes fields, this function
/// identifies blocked fields without modifying the value. Used for output
/// blocklist validation where blocked fields can be used internally but
/// cannot appear in script output.
///
/// # Arguments
///
/// * `value` - The JSON value to check
/// * `blocked_fields` - Set of field names that are not allowed in output
///
/// # Returns
///
/// A vector of blocked field names found in the value, along with their paths.
pub fn find_blocked_fields_in_output(
    value: &JsonValue,
    blocked_fields: &HashSet<String>,
) -> Vec<String> {
    if blocked_fields.is_empty() {
        return Vec::new();
    }

    let mut violations = Vec::new();
    find_blocked_fields_recursive(value, blocked_fields, "", &mut violations);
    violations
}

/// Recursively find blocked fields in a JSON value.
fn find_blocked_fields_recursive(
    value: &JsonValue,
    blocked_fields: &HashSet<String>,
    path: &str,
    violations: &mut Vec<String>,
) {
    match value {
        JsonValue::Object(map) => visit_object(map, blocked_fields, path, violations),
        JsonValue::Array(arr) => visit_array(arr, blocked_fields, path, violations),
        // Non-container values don't need checking.
        _ => {},
    }
}

/// Walk a JSON object: record direct blocked-key matches, then recurse into
/// each value with an extended dotted path.
fn visit_object(
    map: &serde_json::Map<String, JsonValue>,
    blocked_fields: &HashSet<String>,
    path: &str,
    violations: &mut Vec<String>,
) {
    for (key, v) in map {
        let key_path = join_field_path(path, key);
        if blocked_fields.contains(key) {
            violations.push(key_path.clone());
        }
        find_blocked_fields_recursive(v, blocked_fields, &key_path, violations);
    }
}

/// Walk a JSON array: recurse into each element with an indexed path.
fn visit_array(
    arr: &[JsonValue],
    blocked_fields: &HashSet<String>,
    path: &str,
    violations: &mut Vec<String>,
) {
    for (i, v) in arr.iter().enumerate() {
        let elem_path = join_index_path(path, i);
        find_blocked_fields_recursive(v, blocked_fields, &elem_path, violations);
    }
}

/// Append a dotted field segment to a path: `""` + `"k"` → `"k"`, `"a.b"` + `"k"` → `"a.b.k"`.
fn join_field_path(path: &str, key: &str) -> String {
    if path.is_empty() {
        key.to_string()
    } else {
        format!("{}.{}", path, key)
    }
}

/// Append an indexed segment to a path: `""` + `0` → `"[0]"`, `"a"` + `1` → `"a[1]"`.
fn join_index_path(path: &str, idx: usize) -> String {
    if path.is_empty() {
        format!("[{}]", idx)
    } else {
        format!("{}[{}]", path, idx)
    }
}

/// An execution plan compiled from JavaScript AST.
#[derive(Debug, Clone, Serialize)]
pub struct ExecutionPlan {
    /// The steps to execute
    pub steps: Vec<PlanStep>,
    /// Metadata about the plan
    pub metadata: PlanMetadata,
}

/// Metadata about the execution plan.
#[derive(Debug, Clone, Serialize)]
pub struct PlanMetadata {
    /// Total number of API calls in the plan
    pub api_call_count: usize,
    /// Whether any mutations (POST/PUT/DELETE/PATCH) are present
    pub has_mutations: bool,
    /// List of all endpoints accessed
    pub endpoints: Vec<String>,
    /// HTTP methods used
    pub methods_used: Vec<String>,
}

/// A single step in the execution plan.
#[derive(Debug, Clone, Serialize)]
pub enum PlanStep {
    /// API call: `const result = await api.get('/path')`
    ApiCall {
        result_var: String,
        method: String,
        path: PathTemplate,
        body: Option<ValueExpr>,
    },

    /// Variable assignment: `const x = expr`
    Assign { var: String, expr: ValueExpr },

    /// Conditional: `if (cond) { ... } else { ... }`
    Conditional {
        condition: ValueExpr,
        then_steps: Vec<PlanStep>,
        else_steps: Vec<PlanStep>,
    },

    /// Bounded loop: `for (const x of arr.slice(0, N)) { ... }`
    BoundedLoop {
        item_var: String,
        collection: ValueExpr,
        max_iterations: usize,
        body: Vec<PlanStep>,
    },

    /// Return statement
    Return { value: ValueExpr },

    /// Try/catch statement: `try { ... } catch (e) { ... }`
    TryCatch {
        try_steps: Vec<PlanStep>,
        catch_var: Option<String>,
        catch_steps: Vec<PlanStep>,
        finally_steps: Vec<PlanStep>,
    },

    /// Parallel API calls: `const [a, b] = await Promise.all([api.get(...), api.get(...)])`
    ParallelApiCalls {
        result_var: String,
        calls: Vec<(String, String, PathTemplate, Option<ValueExpr>)>, // (temp_var, method, path, body)
    },

    /// Continue statement: skip to next loop iteration
    Continue,

    /// Break statement: exit the current loop
    Break,

    /// MCP tool call: `const result = await mcp.call('server', 'tool', { ... })`
    #[cfg(feature = "mcp-code-mode")]
    McpCall {
        result_var: String,
        server_id: String,
        tool_name: String,
        args: Option<ValueExpr>,
    },

    /// SDK call: `const result = await api.getCostAndUsage({ start_date: '...' })`
    SdkCall {
        result_var: String,
        operation: String,       // camelCase SDK operation name
        args: Option<ValueExpr>, // Single object argument
    },
}

/// A path template that may contain interpolations.
#[derive(Debug, Clone, Serialize)]
pub struct PathTemplate {
    /// Parts of the path
    pub parts: Vec<PathPart>,
}

impl PathTemplate {
    /// Create a static path.
    pub fn static_path(path: String) -> Self {
        Self {
            parts: vec![PathPart::Literal(path)],
        }
    }

    /// Check if this path has any dynamic parts.
    pub fn is_dynamic(&self) -> bool {
        self.parts
            .iter()
            .any(|p| matches!(p, PathPart::Variable(_) | PathPart::Expression(_)))
    }
}

/// A part of a path template.
#[derive(Debug, Clone, Serialize)]
pub enum PathPart {
    /// Literal string
    Literal(String),
    /// Variable reference: `${id}`
    Variable(String),
    /// Expression: `${user.id}`
    Expression(ValueExpr),
}

/// An expression that produces a value.
#[derive(Debug, Clone, Serialize)]
pub enum ValueExpr {
    /// Literal value (null, bool, number, string)
    Literal(JsonValue),

    /// Variable reference
    Variable(String),

    /// Property access: `obj.prop`
    PropertyAccess {
        object: Box<ValueExpr>,
        property: String,
    },

    /// Array index: `arr[0]`
    ArrayIndex {
        array: Box<ValueExpr>,
        index: Box<ValueExpr>,
    },

    /// Object literal: `{ key: value, ...spread }`
    ObjectLiteral { fields: Vec<ObjectField> },

    /// Array literal: `[1, 2, 3]`
    ArrayLiteral { items: Vec<ValueExpr> },

    /// Array method call
    ArrayMethod {
        array: Box<ValueExpr>,
        method: ArrayMethodCall,
    },

    /// Number method call: `num.toFixed()`, etc.
    NumberMethod {
        number: Box<ValueExpr>,
        method: NumberMethodCall,
    },

    /// Binary operation: `a + b`, `a === b`, etc.
    BinaryOp {
        left: Box<ValueExpr>,
        op: BinaryOperator,
        right: Box<ValueExpr>,
    },

    /// Unary operation: `!a`
    UnaryOp {
        op: UnaryOperator,
        operand: Box<ValueExpr>,
    },

    /// Ternary: `cond ? a : b`
    Ternary {
        condition: Box<ValueExpr>,
        consequent: Box<ValueExpr>,
        alternate: Box<ValueExpr>,
    },

    /// Optional chaining: `obj?.prop`
    OptionalChain {
        object: Box<ValueExpr>,
        property: String,
    },

    /// Nullish coalescing: `a ?? b`
    NullishCoalesce {
        left: Box<ValueExpr>,
        right: Box<ValueExpr>,
    },

    /// Await expression (for Promise.all, etc.)
    Await { expr: Box<ValueExpr> },

    /// Promise.all: `Promise.all([...])`
    PromiseAll { items: Vec<ValueExpr> },

    /// API call expression (when used inline, not as a statement)
    ApiCall {
        method: String,
        path: PathTemplate,
        body: Option<Box<ValueExpr>>,
    },

    /// Block expression with local variable bindings and a final result.
    /// Used for arrow function block bodies: `x => { const a = x.foo; return a; }`
    Block {
        /// Local variable bindings: `[(name, expr), ...]`
        bindings: Vec<(String, ValueExpr)>,
        /// The final expression to evaluate and return
        result: Box<ValueExpr>,
    },

    /// MCP tool call expression: `mcp.call('server', 'tool', args)`
    #[cfg(feature = "mcp-code-mode")]
    McpCall {
        server_id: String,
        tool_name: String,
        args: Option<Box<ValueExpr>>,
    },

    /// SDK call expression (used in non-assignment contexts): `api.getCostAndUsage({ ... })`
    SdkCall {
        operation: String,
        args: Option<Box<ValueExpr>>,
    },

    /// Built-in function call: `parseFloat(x)`, `Math.abs(x)`, `Object.keys(obj)`
    BuiltinCall {
        func: BuiltinFunction,
        args: Vec<ValueExpr>,
    },
}

/// A field within an object literal, either a regular key-value pair or a spread.
#[derive(Debug, Clone, Serialize)]
pub enum ObjectField {
    /// Regular key-value pair: `key: value`
    KeyValue { key: String, value: ValueExpr },
    /// Spread: `...expr`
    Spread { expr: ValueExpr },
}

/// Array method calls.
#[derive(Debug, Clone, Serialize)]
pub enum ArrayMethodCall {
    /// `.map(x => expr)`
    Map {
        item_var: String,
        body: Box<ValueExpr>,
    },
    /// `.filter(x => expr)`
    Filter {
        item_var: String,
        predicate: Box<ValueExpr>,
    },
    /// `.find(x => expr)`
    Find {
        item_var: String,
        predicate: Box<ValueExpr>,
    },
    /// `.slice(start, end)`
    Slice { start: usize, end: Option<usize> },
    /// `.length`
    Length,
    /// `.some(x => expr)`
    Some {
        item_var: String,
        predicate: Box<ValueExpr>,
    },
    /// `.every(x => expr)`
    Every {
        item_var: String,
        predicate: Box<ValueExpr>,
    },
    /// `.reduce((acc, x) => expr, init)`
    Reduce {
        acc_var: String,
        item_var: String,
        body: Box<ValueExpr>,
        initial: Box<ValueExpr>,
    },
    /// `.push(item)` - returns new array (pure)
    Push { item: Box<ValueExpr> },
    /// `.concat(other)`
    Concat { other: Box<ValueExpr> },
    /// `.includes(item)`
    Includes { item: Box<ValueExpr> },
    /// `.indexOf(item)`
    IndexOf { item: Box<ValueExpr> },
    /// `.join(separator)`
    Join { separator: Option<String> },
    /// `.reverse()` - returns new array (pure)
    Reverse,
    /// `.sort()` or `.sort((a, b) => expr)` - returns new array (pure)
    Sort {
        comparator: Option<(String, String, Box<ValueExpr>)>,
    },
    /// `.flat()` - flatten nested arrays
    Flat,
    /// `.flatMap(x => expr)`
    FlatMap {
        item_var: String,
        body: Box<ValueExpr>,
    },
    /// Get first element: `[0]` or `.at(0)`
    First,
    /// Get last element: `.at(-1)`
    Last,
    /// `.toLowerCase()` (string-only)
    ToLowerCase,
    /// `.toUpperCase()` (string-only)
    ToUpperCase,
    /// `.startsWith(searchString)`
    StartsWith { search: Box<ValueExpr> },
    /// `.endsWith(searchString)`
    EndsWith { search: Box<ValueExpr> },
    /// `.trim()`
    Trim,
    /// `.replace(search, replacement)` — first occurrence only
    Replace {
        search: Box<ValueExpr>,
        replacement: Box<ValueExpr>,
    },
    /// `.split(separator)`
    Split { separator: Box<ValueExpr> },
    /// `.substring(start, end?)`
    Substring {
        start: Box<ValueExpr>,
        end: Option<Box<ValueExpr>>,
    },
    /// `.toString()` — works on strings (identity), numbers, arrays, objects
    ToString,
}

/// Number method calls.
#[derive(Debug, Clone, Serialize)]
pub enum NumberMethodCall {
    /// `.toFixed(digits)`
    ToFixed { digits: usize },
    /// `.toString()`
    ToString,
}

/// Built-in global functions and static methods.
///
/// These mirror JavaScript built-in functions (parseFloat, parseInt, Number)
/// and static methods (Math.abs, Object.keys, etc.) that are commonly used
/// in cost analysis scripts.
#[derive(Debug, Clone, Serialize)]
pub enum BuiltinFunction {
    // Type conversion
    ParseFloat,
    ParseInt,
    NumberCast,
    // Math static methods
    MathAbs,
    MathMax,
    MathMin,
    MathRound,
    MathFloor,
    MathCeil,
    // Object static methods
    ObjectKeys,
    ObjectValues,
    ObjectEntries,
}

/// Binary operators.
#[derive(Debug, Clone, Copy, Serialize)]
pub enum BinaryOperator {
    // Arithmetic
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    // Bitwise (integer truncation)
    BitwiseOr,
    // Comparison
    Eq,
    NotEq,
    StrictEq,
    StrictNotEq,
    Lt,
    Lte,
    Gt,
    Gte,
    // Logical
    And,
    Or,
    // String
    Concat,
}

/// Unary operators.
#[derive(Debug, Clone, Copy, Serialize)]
pub enum UnaryOperator {
    Not,
    Neg,
    Plus,
    TypeOf,
}

/// Error during plan compilation.
#[derive(Debug, thiserror::Error)]
pub enum CompileError {
    #[error("Unsupported statement type: {0}")]
    UnsupportedStatement(String),

    #[error("Unsupported expression type: {0}")]
    UnsupportedExpression(String),

    #[error("Invalid API call: {0}")]
    InvalidApiCall(String),

    #[error("Unbounded loop detected")]
    UnboundedLoop,

    #[error("Invalid path template: {0}")]
    InvalidPath(String),

    #[error("Too many API calls in plan: {count} (max: {max})")]
    TooManyApiCalls { count: usize, max: usize },

    #[error("Unsupported array method: {0}")]
    UnsupportedArrayMethod(String),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Missing variable name")]
    MissingVariableName,
}

/// Result of extracting an API call from an AST expression.
///
/// Used by `try_extract_api_call()` to return either an HTTP-mode or SDK-mode call,
/// enabling downstream code to create the appropriate `PlanStep` or `ValueExpr`.
enum ExtractedCall {
    /// HTTP call: `api.get('/path', body)` → `PlanStep::ApiCall` / `ValueExpr::ApiCall`
    Http {
        method: String,
        path: PathTemplate,
        body: Option<ValueExpr>,
    },
    /// SDK call: `api.getCostAndUsage({ ... })` → `PlanStep::SdkCall` / `ValueExpr::SdkCall`
    Sdk {
        operation: String,
        args: Option<ValueExpr>,
    },
}

/// Compiler that converts SWC AST to ExecutionPlan.
pub struct PlanCompiler {
    api_call_count: usize,
    endpoints: Vec<String>,
    methods_used: Vec<String>,
    has_mutations: bool,
    /// Set of allowed SDK operation names (camelCase). When non-empty, enables SDK mode.
    sdk_operations: HashSet<String>,
    /// Counter for generating unique temp variable names during destructuring.
    destructure_counter: usize,
}

impl PlanCompiler {
    /// Create a new compiler with default settings.
    pub fn new() -> Self {
        Self::with_config(&ExecutionConfig::default())
    }

    /// Create a new compiler with custom config.
    pub fn with_config(_config: &ExecutionConfig) -> Self {
        Self {
            api_call_count: 0,
            endpoints: Vec::new(),
            methods_used: Vec::new(),
            has_mutations: false,
            sdk_operations: HashSet::new(),
            destructure_counter: 0,
        }
    }

    /// Set the allowed SDK operation names. When non-empty, the compiler operates in SDK mode:
    /// `api.<operation>(args)` is compiled to `SdkCall` instead of HTTP `ApiCall`.
    pub fn with_sdk_operations(mut self, operations: HashSet<String>) -> Self {
        self.sdk_operations = operations;
        self
    }

    /// Compile JavaScript code to an execution plan.
    ///
    /// This is a convenience method that parses the code and compiles it.
    pub fn compile_code(&mut self, code: &str) -> Result<ExecutionPlan, CompileError> {
        // Parse the JavaScript code using SWC
        let cm = SourceMap::default();
        let fm = cm.new_source_file(FileName::Anon.into(), code.to_string());

        let lexer = Lexer::new(
            Syntax::Es(Default::default()),
            EsVersion::Es2022,
            StringInput::from(&*fm),
            None,
        );

        let mut parser = Parser::new_from(lexer);

        let module = parser.parse_module().map_err(|e| {
            CompileError::ParseError(format!("JavaScript parse error: {:?}", e.into_kind()))
        })?;

        // Compile the parsed module
        self.compile(&module)
    }

    /// Compile a module to an execution plan.
    pub fn compile(&mut self, module: &Module) -> Result<ExecutionPlan, CompileError> {
        let mut steps = Vec::new();

        for item in &module.body {
            match item {
                ModuleItem::Stmt(stmt) => {
                    self.compile_statement(stmt, &mut steps)?;
                },
                _ => {
                    return Err(CompileError::UnsupportedStatement(
                        "import/export not allowed".into(),
                    ));
                },
            }
        }

        // Deduplicate methods
        self.methods_used.sort();
        self.methods_used.dedup();

        Ok(ExecutionPlan {
            steps,
            metadata: PlanMetadata {
                api_call_count: self.api_call_count,
                has_mutations: self.has_mutations,
                endpoints: self.endpoints.clone(),
                methods_used: self.methods_used.clone(),
            },
        })
    }

    fn compile_statement(
        &mut self,
        stmt: &Stmt,
        steps: &mut Vec<PlanStep>,
    ) -> Result<(), CompileError> {
        match stmt {
            // Variable declaration: const x = ... or const { a, b } = ...
            Stmt::Decl(Decl::Var(var_decl)) => {
                for decl in &var_decl.decls {
                    if let Some(init) = &decl.init {
                        match &decl.name {
                            Pat::Ident(ident) => {
                                let var_name = ident.id.sym.to_string();
                                self.compile_var_init(&var_name, init, steps)?;
                            },
                            Pat::Object(obj_pat) => {
                                self.compile_object_destructuring(obj_pat, init, steps)?;
                            },
                            Pat::Array(arr_pat) => {
                                self.compile_array_destructuring(arr_pat, init, steps)?;
                            },
                            _ => {
                                return Err(CompileError::UnsupportedExpression(
                                    "complex destructuring pattern".into(),
                                ));
                            },
                        }
                    }
                }
            },

            // Expression statement: await api.get(...), items.push(x), x = expr, etc.
            Stmt::Expr(expr_stmt) => {
                // Handle assignment expressions: `x = expr` or `x = await mcp.call(...)`
                if let Expr::Assign(assign) = expr_stmt.expr.as_ref() {
                    if assign.op == swc_ecma_ast::AssignOp::Assign {
                        if let Some(ident) = assign.left.as_ident() {
                            let var_name = ident.sym.to_string();
                            self.compile_var_init(&var_name, &assign.right, steps)?;
                            return Ok(());
                        }
                    }
                }

                let expr = self.compile_expr(&expr_stmt.expr)?;
                match expr {
                    // API calls at statement level are tracked as side effects
                    ValueExpr::ApiCall { method, path, body } => {
                        self.record_api_call(&method, &path);
                        steps.push(PlanStep::ApiCall {
                            result_var: "_".into(), // Discarded result
                            method,
                            path,
                            body: body.map(|b| *b),
                        });
                    },
                    // SDK calls at statement level (discarded result)
                    ValueExpr::SdkCall { operation, args } => {
                        steps.push(PlanStep::SdkCall {
                            result_var: "_".into(), // Discarded result
                            operation,
                            args: args.map(|a| *a),
                        });
                    },
                    // Mutating array methods (push, concat) as statements:
                    // `items.push(x)` → assign the new array back to the variable
                    ValueExpr::ArrayMethod {
                        ref array,
                        ref method,
                    } if matches!(
                        method,
                        ArrayMethodCall::Push { .. } | ArrayMethodCall::Concat { .. }
                    ) =>
                    {
                        if let ValueExpr::Variable(var_name) = array.as_ref() {
                            steps.push(PlanStep::Assign {
                                var: var_name.clone(),
                                expr,
                            });
                        }
                        // If array is not a simple variable (e.g., obj.arr.push(x)),
                        // silently discard — same as before.
                    },
                    // Other expressions at statement level are discarded
                    _ => {},
                }
            },

            // If statement
            Stmt::If(if_stmt) => {
                let condition = self.compile_expr(&if_stmt.test)?;
                let mut then_steps = Vec::new();
                self.compile_statement(&if_stmt.cons, &mut then_steps)?;

                let mut else_steps = Vec::new();
                if let Some(alt) = &if_stmt.alt {
                    self.compile_statement(alt, &mut else_steps)?;
                }

                steps.push(PlanStep::Conditional {
                    condition,
                    then_steps,
                    else_steps,
                });
            },

            // For-of statement: for (const x of arr) { ... } or for (const { a, b } of arr) { ... }
            Stmt::ForOf(for_of) => {
                let (item_var, destructure_bindings) = match &for_of.left {
                    ForHead::VarDecl(decl) => {
                        if let Some(first) = decl.decls.first() {
                            self.extract_loop_var(&first.name)?
                        } else {
                            return Err(CompileError::MissingVariableName);
                        }
                    },
                    ForHead::Pat(pat) => self.extract_loop_var(pat)?,
                    _ => return Err(CompileError::MissingVariableName),
                };

                let collection = self.compile_expr(&for_of.right)?;

                // Check if collection is bounded (has .slice())
                let max_iterations = self.extract_bound(&collection).unwrap_or(100);

                let mut body = destructure_bindings;
                self.compile_statement(&for_of.body, &mut body)?;

                steps.push(PlanStep::BoundedLoop {
                    item_var,
                    collection,
                    max_iterations,
                    body,
                });
            },

            // Block statement: { ... }
            Stmt::Block(block) => {
                for stmt in &block.stmts {
                    self.compile_statement(stmt, steps)?;
                }
            },

            // Return statement
            Stmt::Return(ret) => {
                let value = if let Some(arg) = &ret.arg {
                    self.compile_expr(arg)?
                } else {
                    ValueExpr::Literal(JsonValue::Null)
                };
                steps.push(PlanStep::Return { value });
            },

            // Empty statement
            Stmt::Empty(_) => {},

            // Try/catch statement
            Stmt::Try(try_stmt) => {
                let mut try_steps = Vec::new();
                for stmt in &try_stmt.block.stmts {
                    self.compile_statement(stmt, &mut try_steps)?;
                }

                let (catch_var, catch_steps) = if let Some(handler) = &try_stmt.handler {
                    let var_name = handler.param.as_ref().map(|p| match p {
                        swc_ecma_ast::Pat::Ident(ident) => ident.sym.to_string(),
                        _ => "error".to_string(),
                    });
                    let mut catch_stmts = Vec::new();
                    for stmt in &handler.body.stmts {
                        self.compile_statement(stmt, &mut catch_stmts)?;
                    }
                    (var_name, catch_stmts)
                } else {
                    (None, Vec::new())
                };

                let finally_steps = if let Some(finalizer) = &try_stmt.finalizer {
                    let mut finally_stmts = Vec::new();
                    for stmt in &finalizer.stmts {
                        self.compile_statement(stmt, &mut finally_stmts)?;
                    }
                    finally_stmts
                } else {
                    Vec::new()
                };

                steps.push(PlanStep::TryCatch {
                    try_steps,
                    catch_var,
                    catch_steps,
                    finally_steps,
                });
            },

            // Continue statement: skip to next loop iteration
            Stmt::Continue(_) => {
                steps.push(PlanStep::Continue);
            },

            // Break statement: exit the current loop
            Stmt::Break(_) => {
                steps.push(PlanStep::Break);
            },

            Stmt::Decl(decl) => {
                let msg = match decl {
                    Decl::Fn(_) => "Function declarations are not supported. Use arrow functions inside array methods (.map, .filter) instead",
                    Decl::Class(_) => "Class declarations are not supported",
                    _ => "This declaration type is not supported",
                };
                return Err(CompileError::UnsupportedStatement(msg.into()));
            },
            Stmt::Switch(_) => {
                return Err(CompileError::UnsupportedStatement(
                    "'switch' statements are not supported. Use if/else if/else instead".into(),
                ));
            },
            Stmt::Throw(_) => {
                return Err(CompileError::UnsupportedStatement(
                    "'throw' statements are not supported. Use try/catch for error handling".into(),
                ));
            },
            Stmt::While(_) => {
                return Err(CompileError::UnsupportedStatement(
                    "'while' loops are not supported. Use for-of with .slice() instead: for (const item of array.slice(0, N)) { }".into(),
                ));
            },
            Stmt::DoWhile(_) => {
                return Err(CompileError::UnsupportedStatement(
                    "'do-while' loops are not supported. Use for-of with .slice() instead: for (const item of array.slice(0, N)) { }".into(),
                ));
            },
            Stmt::For(_) => {
                return Err(CompileError::UnsupportedStatement(
                    "'for(;;)' loops are not supported. Use for-of with .slice() instead: for (const item of array.slice(0, N)) { }".into(),
                ));
            },
            Stmt::ForIn(_) => {
                return Err(CompileError::UnsupportedStatement(
                    "'for-in' loops are not supported. Use for-of with .slice() instead".into(),
                ));
            },
            Stmt::Labeled(_) => {
                return Err(CompileError::UnsupportedStatement(
                    "Labeled statements are not supported".into(),
                ));
            },
            Stmt::With(_) => {
                return Err(CompileError::UnsupportedStatement(
                    "'with' statements are not supported".into(),
                ));
            },
            Stmt::Debugger(_) => {
                return Err(CompileError::UnsupportedStatement(
                    "'debugger' statements are not supported".into(),
                ));
            },
        }

        Ok(())
    }

    fn compile_var_init(
        &mut self,
        var_name: &str,
        init: &Expr,
        steps: &mut Vec<PlanStep>,
    ) -> Result<(), CompileError> {
        // Check if this is an await expression
        if let Expr::Await(await_expr) = init {
            // Check if awaiting an API call or SDK call
            if let Some(extracted) = self.try_extract_api_call(&await_expr.arg)? {
                match extracted {
                    ExtractedCall::Http { method, path, body } => {
                        self.record_api_call(&method, &path);
                        steps.push(PlanStep::ApiCall {
                            result_var: var_name.into(),
                            method,
                            path,
                            body,
                        });
                    },
                    ExtractedCall::Sdk { operation, args } => {
                        steps.push(PlanStep::SdkCall {
                            result_var: var_name.into(),
                            operation,
                            args,
                        });
                    },
                }
                return Ok(());
            }

            // Check if awaiting an MCP call: const x = await mcp.call(...)
            #[cfg(feature = "mcp-code-mode")]
            if let Some((server_id, tool_name, args)) =
                self.try_extract_mcp_call(&await_expr.arg)?
            {
                steps.push(PlanStep::McpCall {
                    result_var: var_name.into(),
                    server_id,
                    tool_name,
                    args,
                });
                return Ok(());
            }

            // Check if awaiting Promise.all([api calls...])
            if let Expr::Call(call) = await_expr.arg.as_ref() {
                let inner = self.compile_call(call)?;
                if let ValueExpr::PromiseAll { items } = inner {
                    return self.compile_promise_all(var_name, items, steps);
                }
            }
        }

        // Regular assignment
        let expr = self.compile_expr(init)?;
        steps.push(PlanStep::Assign {
            var: var_name.into(),
            expr,
        });
        Ok(())
    }

    /// Compile `await Promise.all([api.get(...), api.get(...)])` into a ParallelApiCalls step.
    /// Falls back to sequential execution if any item is not an API call.
    fn compile_promise_all(
        &mut self,
        result_var: &str,
        items: Vec<ValueExpr>,
        steps: &mut Vec<PlanStep>,
    ) -> Result<(), CompileError> {
        let mut calls = Vec::new();
        let mut all_api_calls = true;

        for (i, item) in items.iter().enumerate() {
            match item {
                ValueExpr::ApiCall { method, path, body } => {
                    let temp_var = format!("__promise_all_{}_{}", result_var, i);
                    calls.push((
                        temp_var,
                        method.clone(),
                        path.clone(),
                        body.as_ref().map(|b| *b.clone()),
                    ));
                },
                _ => {
                    all_api_calls = false;
                    break;
                },
            }
        }

        if all_api_calls && !calls.is_empty() {
            // Record all API calls for metadata
            for (_, method, path, _) in &calls {
                self.record_api_call(method, path);
            }
            steps.push(PlanStep::ParallelApiCalls {
                result_var: result_var.into(),
                calls,
            });
            Ok(())
        } else {
            // Fallback: execute items sequentially as individual API calls and collect results
            // This handles mixed expressions in Promise.all
            Err(CompileError::UnsupportedExpression(
                "Promise.all with non-API-call expressions".into(),
            ))
        }
    }

    fn compile_expr(&mut self, expr: &Expr) -> Result<ValueExpr, CompileError> {
        match expr {
            // Literal values
            Expr::Lit(lit) => Ok(ValueExpr::Literal(self.lit_to_json(lit))),

            // Variable reference
            Expr::Ident(ident) => Ok(ValueExpr::Variable(ident.sym.to_string())),

            // Property access: obj.prop
            Expr::Member(member) => {
                let object = Box::new(self.compile_expr(&member.obj)?);

                // Check if this is an array method call
                if let MemberProp::Ident(prop) = &member.prop {
                    let prop_name = prop.sym.to_string();
                    if prop_name == "length" {
                        return Ok(ValueExpr::ArrayMethod {
                            array: object,
                            method: ArrayMethodCall::Length,
                        });
                    }
                }

                match &member.prop {
                    MemberProp::Ident(ident) => Ok(ValueExpr::PropertyAccess {
                        object,
                        property: ident.sym.to_string(),
                    }),
                    MemberProp::Computed(computed) => {
                        let index = Box::new(self.compile_expr(&computed.expr)?);
                        Ok(ValueExpr::ArrayIndex {
                            array: object,
                            index,
                        })
                    }
                    _ => Err(CompileError::UnsupportedExpression("private property".into())),
                }
            }

            // Call expression: fn(), api.get(), arr.map(), etc.
            Expr::Call(call) => self.compile_call(call),

            // Object literal: { key: value, ...spread }
            Expr::Object(obj) => {
                let mut fields = Vec::new();
                for prop in &obj.props {
                    match prop {
                        PropOrSpread::Prop(prop) => {
                            if let Prop::KeyValue(kv) = prop.as_ref() {
                                let key = self.prop_name_to_string(&kv.key)?;
                                let value = self.compile_expr(&kv.value)?;
                                fields.push(ObjectField::KeyValue { key, value });
                            } else if let Prop::Shorthand(ident) = prop.as_ref() {
                                let name = ident.sym.to_string();
                                fields.push(ObjectField::KeyValue {
                                    key: name.clone(),
                                    value: ValueExpr::Variable(name),
                                });
                            }
                        }
                        PropOrSpread::Spread(spread) => {
                            let expr = self.compile_expr(&spread.expr)?;
                            fields.push(ObjectField::Spread { expr });
                        }
                    }
                }
                Ok(ValueExpr::ObjectLiteral { fields })
            }

            // Array literal: [1, 2, 3]
            Expr::Array(arr) => {
                let mut items = Vec::new();
                for elem in arr.elems.iter().flatten() {
                    if elem.spread.is_some() {
                        return Err(CompileError::UnsupportedExpression("spread".into()));
                    }
                    items.push(self.compile_expr(&elem.expr)?);
                }
                Ok(ValueExpr::ArrayLiteral { items })
            }

            // Template literal: `Hello ${name}`
            Expr::Tpl(tpl) => {
                // For now, treat as string concatenation
                // This is used primarily for path templates
                let mut parts = Vec::new();
                for (i, quasi) in tpl.quasis.iter().enumerate() {
                    let raw = quasi.raw.to_string();
                    if !raw.is_empty() {
                        parts.push(ValueExpr::Literal(JsonValue::String(raw)));
                    }
                    if i < tpl.exprs.len() {
                        parts.push(self.compile_expr(&tpl.exprs[i])?);
                    }
                }

                // If single part, return it directly
                if parts.len() == 1 {
                    return Ok(parts.remove(0));
                }

                // Otherwise, build concatenation
                let mut result = parts.remove(0);
                for part in parts {
                    result = ValueExpr::BinaryOp {
                        left: Box::new(result),
                        op: BinaryOperator::Concat,
                        right: Box::new(part),
                    };
                }
                Ok(result)
            }

            // Binary expression: a + b, a === b
            Expr::Bin(bin) => {
                let left = Box::new(self.compile_expr(&bin.left)?);
                let right = Box::new(self.compile_expr(&bin.right)?);
                let op = self.compile_bin_op(bin.op)?;
                Ok(ValueExpr::BinaryOp { left, op, right })
            }

            // Unary expression: !a, -a
            Expr::Unary(unary) => {
                let operand = Box::new(self.compile_expr(&unary.arg)?);
                let op = match unary.op {
                    UnaryOp::Bang => UnaryOperator::Not,
                    UnaryOp::Minus => UnaryOperator::Neg,
                    UnaryOp::TypeOf => UnaryOperator::TypeOf,
                    UnaryOp::Plus => UnaryOperator::Plus,
                    _ => return Err(CompileError::UnsupportedExpression("unary op".into())),
                };
                Ok(ValueExpr::UnaryOp { op, operand })
            }

            // Conditional/ternary: cond ? a : b
            Expr::Cond(cond) => {
                let condition = Box::new(self.compile_expr(&cond.test)?);
                let consequent = Box::new(self.compile_expr(&cond.cons)?);
                let alternate = Box::new(self.compile_expr(&cond.alt)?);
                Ok(ValueExpr::Ternary {
                    condition,
                    consequent,
                    alternate,
                })
            }

            // Await expression
            Expr::Await(await_expr) => {
                // Check if it's an API call or SDK call
                if let Some(extracted) = self.try_extract_api_call(&await_expr.arg)? {
                    return match extracted {
                        ExtractedCall::Http { method, path, body } => {
                            self.record_api_call(&method, &path);
                            Ok(ValueExpr::ApiCall {
                                method,
                                path,
                                body: body.map(Box::new),
                            })
                        }
                        ExtractedCall::Sdk { operation, args } => {
                            Ok(ValueExpr::SdkCall {
                                operation,
                                args: args.map(Box::new),
                            })
                        }
                    };
                }
                // Check if it's an MCP call
                #[cfg(feature = "mcp-code-mode")]
                if let Some((server_id, tool_name, args)) = self.try_extract_mcp_call(&await_expr.arg)? {
                    return Ok(ValueExpr::McpCall {
                        server_id,
                        tool_name,
                        args: args.map(Box::new),
                    });
                }
                // Otherwise, just await the inner expression
                let inner = self.compile_expr(&await_expr.arg)?;
                Ok(ValueExpr::Await {
                    expr: Box::new(inner),
                })
            }

            // Arrow function (for use in .map(), .filter(), etc.)
            Expr::Arrow(_) => {
                // Arrow functions are handled specially in array method compilation
                Err(CompileError::UnsupportedExpression(
                    "arrow function outside array method".into(),
                ))
            }

            // Parenthesized expression
            Expr::Paren(paren) => self.compile_expr(&paren.expr),

            // Optional chaining: obj?.prop
            Expr::OptChain(opt) => {
                match opt.base.as_ref() {
                    OptChainBase::Member(member) => {
                        let object = Box::new(self.compile_expr(&member.obj)?);
                        if let MemberProp::Ident(ident) = &member.prop {
                            Ok(ValueExpr::OptionalChain {
                                object,
                                property: ident.sym.to_string(),
                            })
                        } else {
                            Err(CompileError::UnsupportedExpression("computed optional chain".into()))
                        }
                    }
                    _ => Err(CompileError::UnsupportedExpression("optional call".into())),
                }
            }

            Expr::This(_) => Err(CompileError::UnsupportedExpression(
                "'this' keyword is not supported".into(),
            )),
            Expr::Fn(_) => Err(CompileError::UnsupportedExpression(
                "Function expressions are not supported. Use arrow functions inside array methods (.map, .filter) instead".into(),
            )),
            Expr::Update(_) => Err(CompileError::UnsupportedExpression(
                "Increment/decrement operators (++, --) are not supported. Use 'x = x + 1' instead".into(),
            )),
            Expr::New(_) => Err(CompileError::UnsupportedExpression(
                "'new' keyword is not supported".into(),
            )),
            Expr::Seq(_) => Err(CompileError::UnsupportedExpression(
                "Sequence expressions (comma operator) are not supported. Use separate statements instead".into(),
            )),
            Expr::TaggedTpl(_) => Err(CompileError::UnsupportedExpression(
                "Tagged template literals are not supported. Use regular template literals instead".into(),
            )),
            Expr::Class(_) => Err(CompileError::UnsupportedExpression(
                "Class expressions are not supported".into(),
            )),
            Expr::Yield(_) => Err(CompileError::UnsupportedExpression(
                "Generator yield is not supported".into(),
            )),
            Expr::SuperProp(_) => Err(CompileError::UnsupportedExpression(
                "'super' is not supported".into(),
            )),
            Expr::Assign(_) => Err(CompileError::UnsupportedExpression(
                "Assignment expressions are not supported here. Use a separate variable declaration instead".into(),
            )),
            _ => Err(CompileError::UnsupportedExpression(
                "This expression type is not supported in the JavaScript subset".into(),
            )),
        }
    }

    fn compile_call(&mut self, call: &CallExpr) -> Result<ValueExpr, CompileError> {
        // Check if this is an API call (HTTP) or SDK call
        if let Some(extracted) = self.try_extract_api_call(&Expr::Call(call.clone()))? {
            return match extracted {
                ExtractedCall::Http { method, path, body } => {
                    self.record_api_call(&method, &path);
                    Ok(ValueExpr::ApiCall {
                        method,
                        path,
                        body: body.map(Box::new),
                    })
                },
                ExtractedCall::Sdk { operation, args } => Ok(ValueExpr::SdkCall {
                    operation,
                    args: args.map(Box::new),
                }),
            };
        }

        // Check if this is an MCP call: mcp.call('server', 'tool', args)
        #[cfg(feature = "mcp-code-mode")]
        if let Some((server_id, tool_name, args)) =
            self.try_extract_mcp_call(&Expr::Call(call.clone()))?
        {
            return Ok(ValueExpr::McpCall {
                server_id,
                tool_name,
                args: args.map(Box::new),
            });
        }

        // Check if this is Promise.all
        if let Callee::Expr(callee) = &call.callee {
            if let Expr::Member(member) = callee.as_ref() {
                if let Expr::Ident(obj) = member.obj.as_ref() {
                    if obj.sym.as_ref() == "Promise" {
                        if let MemberProp::Ident(prop) = &member.prop {
                            if prop.sym.as_ref() == "all" {
                                if let Some(arg) = call.args.first() {
                                    if let Expr::Array(arr) = arg.expr.as_ref() {
                                        let mut items = Vec::new();
                                        for elem in arr.elems.iter().flatten() {
                                            items.push(self.compile_expr(&elem.expr)?);
                                        }
                                        return Ok(ValueExpr::PromiseAll { items });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Check if this is an array method: arr.map(), arr.filter(), etc.
        if let Callee::Expr(callee) = &call.callee {
            if let Expr::Member(member) = callee.as_ref() {
                let array = Box::new(self.compile_expr(&member.obj)?);

                if let MemberProp::Ident(method_ident) = &member.prop {
                    let method_name = method_ident.sym.as_ref();

                    match method_name {
                        "map" => {
                            let (item_var, body) = self.extract_arrow_callback(call)?;
                            return Ok(ValueExpr::ArrayMethod {
                                array,
                                method: ArrayMethodCall::Map {
                                    item_var,
                                    body: Box::new(body),
                                },
                            });
                        },
                        "filter" => {
                            let (item_var, predicate) = self.extract_arrow_callback(call)?;
                            return Ok(ValueExpr::ArrayMethod {
                                array,
                                method: ArrayMethodCall::Filter {
                                    item_var,
                                    predicate: Box::new(predicate),
                                },
                            });
                        },
                        "find" => {
                            let (item_var, predicate) = self.extract_arrow_callback(call)?;
                            return Ok(ValueExpr::ArrayMethod {
                                array,
                                method: ArrayMethodCall::Find {
                                    item_var,
                                    predicate: Box::new(predicate),
                                },
                            });
                        },
                        "some" => {
                            let (item_var, predicate) = self.extract_arrow_callback(call)?;
                            return Ok(ValueExpr::ArrayMethod {
                                array,
                                method: ArrayMethodCall::Some {
                                    item_var,
                                    predicate: Box::new(predicate),
                                },
                            });
                        },
                        "every" => {
                            let (item_var, predicate) = self.extract_arrow_callback(call)?;
                            return Ok(ValueExpr::ArrayMethod {
                                array,
                                method: ArrayMethodCall::Every {
                                    item_var,
                                    predicate: Box::new(predicate),
                                },
                            });
                        },
                        "flatMap" => {
                            let (item_var, body) = self.extract_arrow_callback(call)?;
                            return Ok(ValueExpr::ArrayMethod {
                                array,
                                method: ArrayMethodCall::FlatMap {
                                    item_var,
                                    body: Box::new(body),
                                },
                            });
                        },
                        "slice" => {
                            let start = self.extract_number_arg(call, 0)?.unwrap_or(0) as usize;
                            let end = self.extract_number_arg(call, 1)?.map(|n| n as usize);
                            return Ok(ValueExpr::ArrayMethod {
                                array,
                                method: ArrayMethodCall::Slice { start, end },
                            });
                        },
                        "push" => {
                            if let Some(arg) = call.args.first() {
                                let item = Box::new(self.compile_expr(&arg.expr)?);
                                return Ok(ValueExpr::ArrayMethod {
                                    array,
                                    method: ArrayMethodCall::Push { item },
                                });
                            }
                        },
                        "concat" => {
                            if let Some(arg) = call.args.first() {
                                let other = Box::new(self.compile_expr(&arg.expr)?);
                                return Ok(ValueExpr::ArrayMethod {
                                    array,
                                    method: ArrayMethodCall::Concat { other },
                                });
                            }
                        },
                        "includes" => {
                            if let Some(arg) = call.args.first() {
                                let item = Box::new(self.compile_expr(&arg.expr)?);
                                return Ok(ValueExpr::ArrayMethod {
                                    array,
                                    method: ArrayMethodCall::Includes { item },
                                });
                            }
                        },
                        "indexOf" => {
                            if let Some(arg) = call.args.first() {
                                let item = Box::new(self.compile_expr(&arg.expr)?);
                                return Ok(ValueExpr::ArrayMethod {
                                    array,
                                    method: ArrayMethodCall::IndexOf { item },
                                });
                            }
                        },
                        "join" => {
                            let separator = if let Some(arg) = call.args.first() {
                                if let Expr::Lit(Lit::Str(s)) = arg.expr.as_ref() {
                                    Some(s.value.to_string_lossy().into_owned())
                                } else {
                                    None
                                }
                            } else {
                                None
                            };
                            return Ok(ValueExpr::ArrayMethod {
                                array,
                                method: ArrayMethodCall::Join { separator },
                            });
                        },
                        "reverse" => {
                            return Ok(ValueExpr::ArrayMethod {
                                array,
                                method: ArrayMethodCall::Reverse,
                            });
                        },
                        "sort" => {
                            let comparator = if !call.args.is_empty() {
                                let (a_var, b_var, body) = self.extract_reduce_callback(call)?;
                                Some((a_var, b_var, Box::new(body)))
                            } else {
                                None
                            };
                            return Ok(ValueExpr::ArrayMethod {
                                array,
                                method: ArrayMethodCall::Sort { comparator },
                            });
                        },
                        "flat" => {
                            return Ok(ValueExpr::ArrayMethod {
                                array,
                                method: ArrayMethodCall::Flat,
                            });
                        },
                        "at" => {
                            if let Some(n) = self.extract_number_arg(call, 0)? {
                                if n == 0 {
                                    return Ok(ValueExpr::ArrayMethod {
                                        array,
                                        method: ArrayMethodCall::First,
                                    });
                                } else if n == -1 {
                                    return Ok(ValueExpr::ArrayMethod {
                                        array,
                                        method: ArrayMethodCall::Last,
                                    });
                                }
                            }
                        },
                        // reduce((acc, item) => expr, initialValue)
                        "reduce" if call.args.len() >= 2 => {
                            let (acc_var, item_var, body) = self.extract_reduce_callback(call)?;
                            let initial = Box::new(self.compile_expr(&call.args[1].expr)?);
                            return Ok(ValueExpr::ArrayMethod {
                                array,
                                method: ArrayMethodCall::Reduce {
                                    acc_var,
                                    item_var,
                                    body: Box::new(body),
                                    initial,
                                },
                            });
                        },
                        "toFixed" => {
                            // Number.toFixed(digits) - treat as a number method
                            let digits = self.extract_number_arg(call, 0)?.unwrap_or(0) as usize;
                            return Ok(ValueExpr::NumberMethod {
                                number: array, // The "array" here is actually the number
                                method: NumberMethodCall::ToFixed { digits },
                            });
                        },
                        "toLowerCase" => {
                            return Ok(ValueExpr::ArrayMethod {
                                array,
                                method: ArrayMethodCall::ToLowerCase,
                            });
                        },
                        "toUpperCase" => {
                            return Ok(ValueExpr::ArrayMethod {
                                array,
                                method: ArrayMethodCall::ToUpperCase,
                            });
                        },
                        "startsWith" => {
                            let arg = call.args.first().ok_or_else(|| {
                                CompileError::UnsupportedExpression(
                                    "startsWith() requires a search argument".into(),
                                )
                            })?;
                            let search = Box::new(self.compile_expr(&arg.expr)?);
                            return Ok(ValueExpr::ArrayMethod {
                                array,
                                method: ArrayMethodCall::StartsWith { search },
                            });
                        },
                        "endsWith" => {
                            let arg = call.args.first().ok_or_else(|| {
                                CompileError::UnsupportedExpression(
                                    "endsWith() requires a search argument".into(),
                                )
                            })?;
                            let search = Box::new(self.compile_expr(&arg.expr)?);
                            return Ok(ValueExpr::ArrayMethod {
                                array,
                                method: ArrayMethodCall::EndsWith { search },
                            });
                        },
                        "trim" => {
                            return Ok(ValueExpr::ArrayMethod {
                                array,
                                method: ArrayMethodCall::Trim,
                            });
                        },
                        "replace" => {
                            if call.args.len() < 2 {
                                return Err(CompileError::UnsupportedExpression(
                                    "replace() requires search and replacement arguments".into(),
                                ));
                            }
                            let search = Box::new(self.compile_expr(&call.args[0].expr)?);
                            let replacement = Box::new(self.compile_expr(&call.args[1].expr)?);
                            return Ok(ValueExpr::ArrayMethod {
                                array,
                                method: ArrayMethodCall::Replace {
                                    search,
                                    replacement,
                                },
                            });
                        },
                        "split" => {
                            let arg = call.args.first().ok_or_else(|| {
                                CompileError::UnsupportedExpression(
                                    "split() requires a separator argument".into(),
                                )
                            })?;
                            let separator = Box::new(self.compile_expr(&arg.expr)?);
                            return Ok(ValueExpr::ArrayMethod {
                                array,
                                method: ArrayMethodCall::Split { separator },
                            });
                        },
                        "substring" => {
                            let arg = call.args.first().ok_or_else(|| {
                                CompileError::UnsupportedExpression(
                                    "substring() requires a start argument".into(),
                                )
                            })?;
                            let start = Box::new(self.compile_expr(&arg.expr)?);
                            let end = if call.args.len() >= 2 {
                                Some(Box::new(self.compile_expr(&call.args[1].expr)?))
                            } else {
                                None
                            };
                            return Ok(ValueExpr::ArrayMethod {
                                array,
                                method: ArrayMethodCall::Substring { start, end },
                            });
                        },
                        "toString" => {
                            return Ok(ValueExpr::ArrayMethod {
                                array,
                                method: ArrayMethodCall::ToString,
                            });
                        },
                        _ => {},
                    }
                }
            }
        }

        // Check for built-in global functions: parseFloat(), parseInt(), Number()
        if let Callee::Expr(callee) = &call.callee {
            if let Expr::Ident(ident) = callee.as_ref() {
                let func = match ident.sym.as_ref() {
                    "parseFloat" => Some(BuiltinFunction::ParseFloat),
                    "parseInt" => Some(BuiltinFunction::ParseInt),
                    "Number" => Some(BuiltinFunction::NumberCast),
                    _ => None,
                };
                if let Some(func) = func {
                    let args = call
                        .args
                        .iter()
                        .map(|a| self.compile_expr(&a.expr))
                        .collect::<Result<Vec<_>, _>>()?;
                    return Ok(ValueExpr::BuiltinCall { func, args });
                }
            }

            // Check for static method calls: Math.abs(), Object.keys(), etc.
            if let Expr::Member(member) = callee.as_ref() {
                if let Expr::Ident(obj) = member.obj.as_ref() {
                    if let MemberProp::Ident(prop) = &member.prop {
                        let func = match (obj.sym.as_ref(), prop.sym.as_ref()) {
                            ("Math", "abs") => Some(BuiltinFunction::MathAbs),
                            ("Math", "max") => Some(BuiltinFunction::MathMax),
                            ("Math", "min") => Some(BuiltinFunction::MathMin),
                            ("Math", "round") => Some(BuiltinFunction::MathRound),
                            ("Math", "floor") => Some(BuiltinFunction::MathFloor),
                            ("Math", "ceil") => Some(BuiltinFunction::MathCeil),
                            ("Object", "keys") => Some(BuiltinFunction::ObjectKeys),
                            ("Object", "values") => Some(BuiltinFunction::ObjectValues),
                            ("Object", "entries") => Some(BuiltinFunction::ObjectEntries),
                            _ => None,
                        };
                        if let Some(func) = func {
                            let args = call
                                .args
                                .iter()
                                .map(|a| self.compile_expr(&a.expr))
                                .collect::<Result<Vec<_>, _>>()?;
                            return Ok(ValueExpr::BuiltinCall { func, args });
                        }
                    }
                }
            }
        }

        Err(CompileError::UnsupportedExpression("function call".into()))
    }

    fn try_extract_api_call(&mut self, expr: &Expr) -> Result<Option<ExtractedCall>, CompileError> {
        let call = match expr {
            Expr::Call(c) => c,
            _ => return Ok(None),
        };

        if let Callee::Expr(callee) = &call.callee {
            if let Expr::Member(member) = callee.as_ref() {
                if let Expr::Ident(obj) = member.obj.as_ref() {
                    if obj.sym.as_ref() == "api" {
                        if let MemberProp::Ident(method_ident) = &member.prop {
                            let method_name = method_ident.sym.as_ref();

                            if !self.sdk_operations.is_empty() {
                                // SDK mode: validate against allowed operation names
                                if !self.sdk_operations.contains(method_name) {
                                    return Err(CompileError::InvalidApiCall(format!(
                                        "Unknown SDK operation: api.{}(). Check the code mode schema resource for available operations.",
                                        method_name
                                    )));
                                }
                                let args = if let Some(arg) = call.args.first() {
                                    Some(self.compile_expr(&arg.expr)?)
                                } else {
                                    None
                                };
                                self.api_call_count += 1;
                                let op_endpoint = format!("sdk:{}", method_name);
                                if !self.endpoints.contains(&op_endpoint) {
                                    self.endpoints.push(op_endpoint);
                                }
                                if !self.methods_used.contains(&method_name.to_string()) {
                                    self.methods_used.push(method_name.to_string());
                                }
                                return Ok(Some(ExtractedCall::Sdk {
                                    operation: method_name.to_string(),
                                    args,
                                }));
                            }

                            // HTTP mode: validate it's a known HTTP method
                            if HttpMethod::from_str(method_name).is_none() {
                                return Err(CompileError::InvalidApiCall(format!(
                                    "Unknown method: api.{}",
                                    method_name
                                )));
                            }

                            // Extract path from first argument
                            let path = if let Some(arg) = call.args.first() {
                                self.extract_path_template(&arg.expr)?
                            } else {
                                return Err(CompileError::InvalidApiCall(
                                    "API call requires path".into(),
                                ));
                            };

                            // Extract body from second argument (for POST, PUT, PATCH)
                            let body = if let Some(arg) = call.args.get(1) {
                                Some(self.compile_expr(&arg.expr)?)
                            } else {
                                None
                            };

                            return Ok(Some(ExtractedCall::Http {
                                method: method_name.to_uppercase(),
                                path,
                                body,
                            }));
                        }
                    }
                }
            }
        }

        Ok(None)
    }

    /// Try to extract an MCP call: `mcp.call('server', 'tool', { args })`
    ///
    /// Returns `(server_id, tool_name, args)` if the expression is an MCP call.
    #[cfg(feature = "mcp-code-mode")]
    fn try_extract_mcp_call(
        &mut self,
        expr: &Expr,
    ) -> Result<Option<(String, String, Option<ValueExpr>)>, CompileError> {
        let call = match expr {
            Expr::Call(c) => c,
            _ => return Ok(None),
        };

        if let Callee::Expr(callee) = &call.callee {
            if let Expr::Member(member) = callee.as_ref() {
                if let Expr::Ident(obj) = member.obj.as_ref() {
                    if obj.sym.as_ref() == "mcp" {
                        if let MemberProp::Ident(method_ident) = &member.prop {
                            if method_ident.sym.as_ref() == "call" {
                                // Extract server_id from first arg (string literal)
                                let server_id = call.args.first()
                                    .and_then(|a| {
                                        if let Expr::Lit(Lit::Str(s)) = a.expr.as_ref() {
                                            Some(s.value.to_string_lossy().into_owned())
                                        } else {
                                            None
                                        }
                                    })
                                    .ok_or_else(|| CompileError::UnsupportedExpression(
                                        "mcp.call() first argument must be a string literal (server_id)".into(),
                                    ))?;

                                // Extract tool_name from second arg (string literal)
                                let tool_name = call.args.get(1)
                                    .and_then(|a| {
                                        if let Expr::Lit(Lit::Str(s)) = a.expr.as_ref() {
                                            Some(s.value.to_string_lossy().into_owned())
                                        } else {
                                            None
                                        }
                                    })
                                    .ok_or_else(|| CompileError::UnsupportedExpression(
                                        "mcp.call() second argument must be a string literal (tool_name)".into(),
                                    ))?;

                                // Extract args from third arg (optional object expression)
                                let args = call
                                    .args
                                    .get(2)
                                    .map(|a| self.compile_expr(&a.expr))
                                    .transpose()?;

                                return Ok(Some((server_id, tool_name, args)));
                            }
                        }
                    }
                }
            }
        }

        Ok(None)
    }

    fn extract_path_template(&mut self, expr: &Expr) -> Result<PathTemplate, CompileError> {
        match expr {
            // Simple string: '/users'
            Expr::Lit(Lit::Str(s)) => Ok(PathTemplate::static_path(
                s.value.to_string_lossy().into_owned(),
            )),

            // Template literal: `/users/${id}`
            Expr::Tpl(tpl) => {
                let mut parts = Vec::new();
                for (i, quasi) in tpl.quasis.iter().enumerate() {
                    let raw = quasi.raw.to_string();
                    if !raw.is_empty() {
                        parts.push(PathPart::Literal(raw));
                    }
                    if i < tpl.exprs.len() {
                        // Check if it's a simple variable
                        if let Expr::Ident(ident) = tpl.exprs[i].as_ref() {
                            parts.push(PathPart::Variable(ident.sym.to_string()));
                        } else {
                            // Complex expression
                            let expr = self.compile_expr(&tpl.exprs[i])?;
                            parts.push(PathPart::Expression(expr));
                        }
                    }
                }
                Ok(PathTemplate { parts })
            },

            _ => Err(CompileError::InvalidPath(
                "Path must be a string or template literal".into(),
            )),
        }
    }

    fn extract_arrow_callback(
        &mut self,
        call: &CallExpr,
    ) -> Result<(String, ValueExpr), CompileError> {
        let arg = call
            .args
            .first()
            .ok_or_else(|| CompileError::UnsupportedExpression("missing callback".into()))?;

        if let Expr::Arrow(arrow) = arg.expr.as_ref() {
            // Get parameter name
            let param_name = if let Some(Pat::Ident(ident)) = arrow.params.first() {
                ident.id.sym.to_string()
            } else {
                return Err(CompileError::UnsupportedExpression(
                    "complex callback parameter".into(),
                ));
            };

            // Compile body
            let body = match &*arrow.body {
                BlockStmtOrExpr::Expr(expr) => self.compile_expr(expr)?,
                BlockStmtOrExpr::BlockStmt(block) => {
                    // For block bodies, collect variable bindings and find return statement
                    let mut bindings: Vec<(String, ValueExpr)> = Vec::new();
                    let mut return_expr: Option<ValueExpr> = None;

                    for stmt in &block.stmts {
                        match stmt {
                            // Variable declaration: const x = ...; or let x = ...;
                            Stmt::Decl(Decl::Var(var_decl)) => {
                                for decl in &var_decl.decls {
                                    let var_name = self.get_var_name(&decl.name)?;
                                    if let Some(init) = &decl.init {
                                        let expr = self.compile_expr(init)?;
                                        bindings.push((var_name, expr));
                                    }
                                }
                            },
                            // Return statement
                            Stmt::Return(ret) => {
                                if let Some(arg) = &ret.arg {
                                    return_expr = Some(self.compile_expr(arg)?);
                                }
                                break; // Stop processing after return
                            },
                            // Expression statement (e.g., a side effect)
                            Stmt::Expr(_) => {
                                // Ignore expression statements in arrow body for now
                            },
                            _ => {},
                        }
                    }

                    match return_expr {
                        Some(result) if bindings.is_empty() => result,
                        Some(result) => ValueExpr::Block {
                            bindings,
                            result: Box::new(result),
                        },
                        None => {
                            return Err(CompileError::UnsupportedExpression(
                                "callback block without return".into(),
                            ));
                        },
                    }
                },
            };

            Ok((param_name, body))
        } else {
            Err(CompileError::UnsupportedExpression(
                "callback must be arrow function".into(),
            ))
        }
    }

    /// Extract reduce callback: (acc, item) => expr
    fn extract_reduce_callback(
        &mut self,
        call: &CallExpr,
    ) -> Result<(String, String, ValueExpr), CompileError> {
        let arg = call
            .args
            .first()
            .ok_or_else(|| CompileError::UnsupportedExpression("missing callback".into()))?;

        if let Expr::Arrow(arrow) = arg.expr.as_ref() {
            // Reduce callback should have 2 parameters: (acc, item)
            if arrow.params.len() < 2 {
                return Err(CompileError::UnsupportedExpression(
                    "reduce callback must have 2 parameters".into(),
                ));
            }

            let acc_name = if let Pat::Ident(ident) = &arrow.params[0] {
                ident.id.sym.to_string()
            } else {
                return Err(CompileError::UnsupportedExpression(
                    "complex callback parameter".into(),
                ));
            };

            let item_name = if let Pat::Ident(ident) = &arrow.params[1] {
                ident.id.sym.to_string()
            } else {
                return Err(CompileError::UnsupportedExpression(
                    "complex callback parameter".into(),
                ));
            };

            // Compile body
            let body = match &*arrow.body {
                BlockStmtOrExpr::Expr(expr) => self.compile_expr(expr)?,
                BlockStmtOrExpr::BlockStmt(block) => {
                    // For block bodies, look for return statement
                    for stmt in &block.stmts {
                        if let Stmt::Return(ret) = stmt {
                            if let Some(arg) = &ret.arg {
                                return Ok((acc_name, item_name, self.compile_expr(arg)?));
                            }
                        }
                    }
                    return Err(CompileError::UnsupportedExpression(
                        "callback block without return".into(),
                    ));
                },
            };

            Ok((acc_name, item_name, body))
        } else {
            Err(CompileError::UnsupportedExpression(
                "callback must be arrow function".into(),
            ))
        }
    }

    fn extract_number_arg(
        &self,
        call: &CallExpr,
        index: usize,
    ) -> Result<Option<i64>, CompileError> {
        if let Some(arg) = call.args.get(index) {
            if let Expr::Lit(Lit::Num(n)) = arg.expr.as_ref() {
                return Ok(Some(n.value as i64));
            }
            if let Expr::Unary(unary) = arg.expr.as_ref() {
                if unary.op == UnaryOp::Minus {
                    if let Expr::Lit(Lit::Num(n)) = unary.arg.as_ref() {
                        return Ok(Some(-(n.value as i64)));
                    }
                }
            }
        }
        Ok(None)
    }

    fn extract_bound(&self, expr: &ValueExpr) -> Option<usize> {
        if let ValueExpr::ArrayMethod {
            method: ArrayMethodCall::Slice { end, .. },
            ..
        } = expr
        {
            return *end;
        }
        None
    }

    fn get_var_name(&self, pat: &Pat) -> Result<String, CompileError> {
        match pat {
            Pat::Ident(ident) => Ok(ident.id.sym.to_string()),
            _ => Err(CompileError::UnsupportedExpression(
                "complex destructuring".into(),
            )),
        }
    }

    /// Generate a unique temp variable name for destructuring.
    fn next_temp_var(&mut self) -> String {
        let name = format!("__destructure_{}", self.destructure_counter);
        self.destructure_counter += 1;
        name
    }

    /// Extract loop variable and destructuring steps for for-of loops.
    /// Returns (item_var, steps_to_prepend_to_body).
    fn extract_loop_var(&mut self, pat: &Pat) -> Result<(String, Vec<PlanStep>), CompileError> {
        match pat {
            Pat::Ident(ident) => Ok((ident.id.sym.to_string(), Vec::new())),
            Pat::Object(obj_pat) => {
                let temp_var = self.next_temp_var();
                let bindings = Self::extract_object_bindings(obj_pat)?;
                let steps = bindings
                    .into_iter()
                    .map(|(var_name, property)| PlanStep::Assign {
                        var: var_name,
                        expr: ValueExpr::PropertyAccess {
                            object: Box::new(ValueExpr::Variable(temp_var.clone())),
                            property,
                        },
                    })
                    .collect();
                Ok((temp_var, steps))
            },
            Pat::Array(arr_pat) => {
                let temp_var = self.next_temp_var();
                let mut steps = Vec::new();
                for (i, elem) in arr_pat.elems.iter().enumerate() {
                    if let Some(p) = elem {
                        let var_name = self.get_var_name(p)?;
                        steps.push(PlanStep::Assign {
                            var: var_name,
                            expr: ValueExpr::ArrayIndex {
                                array: Box::new(ValueExpr::Variable(temp_var.clone())),
                                index: Box::new(ValueExpr::Literal(JsonValue::Number(
                                    (i as i64).into(),
                                ))),
                            },
                        });
                    }
                }
                Ok((temp_var, steps))
            },
            _ => Err(CompileError::UnsupportedExpression(
                "complex loop variable pattern".into(),
            )),
        }
    }

    /// Extract (var_name, property_key) bindings from an object destructuring pattern.
    /// `{ a, b }` → [("a", "a"), ("b", "b")]
    /// `{ id: userId }` → [("userId", "id")]
    fn extract_object_bindings(obj_pat: &ObjectPat) -> Result<Vec<(String, String)>, CompileError> {
        let mut bindings = Vec::new();
        for prop in &obj_pat.props {
            match prop {
                ObjectPatProp::Assign(assign) => {
                    // Shorthand: `{ x }` — reject `{ x = default }` explicitly
                    if assign.value.is_some() {
                        return Err(CompileError::UnsupportedExpression(
                            "default values in destructuring".into(),
                        ));
                    }
                    let name = assign.key.sym.to_string();
                    bindings.push((name.clone(), name));
                },
                ObjectPatProp::KeyValue(kv) => {
                    // Renamed: `{ id: userId }`
                    let key = match &kv.key {
                        PropName::Ident(ident) => ident.sym.to_string(),
                        PropName::Str(s) => s.value.to_string_lossy().into_owned(),
                        _ => {
                            return Err(CompileError::UnsupportedExpression(
                                "computed destructuring key".into(),
                            ));
                        },
                    };
                    let var_name = match kv.value.as_ref() {
                        Pat::Ident(ident) => ident.id.sym.to_string(),
                        _ => {
                            return Err(CompileError::UnsupportedExpression(
                                "nested destructuring".into(),
                            ));
                        },
                    };
                    bindings.push((var_name, key));
                },
                ObjectPatProp::Rest(_) => {
                    return Err(CompileError::UnsupportedExpression(
                        "rest pattern in destructuring".into(),
                    ));
                },
            }
        }
        Ok(bindings)
    }

    /// Compile object destructuring: `const { a, b } = expr`
    /// Generates a temp var assignment + property access assignments.
    fn compile_object_destructuring(
        &mut self,
        obj_pat: &ObjectPat,
        init: &Expr,
        steps: &mut Vec<PlanStep>,
    ) -> Result<(), CompileError> {
        let bindings = Self::extract_object_bindings(obj_pat)?;
        let temp_var = self.next_temp_var();

        // Compile the RHS into the temp var
        self.compile_var_init(&temp_var, init, steps)?;

        // Generate property access assignments for each binding
        for (var_name, property) in bindings {
            steps.push(PlanStep::Assign {
                var: var_name,
                expr: ValueExpr::PropertyAccess {
                    object: Box::new(ValueExpr::Variable(temp_var.clone())),
                    property,
                },
            });
        }
        Ok(())
    }

    /// Compile array destructuring: `const [a, b] = expr`
    /// Generates a temp var assignment + index access assignments.
    fn compile_array_destructuring(
        &mut self,
        arr_pat: &ArrayPat,
        init: &Expr,
        steps: &mut Vec<PlanStep>,
    ) -> Result<(), CompileError> {
        let temp_var = self.next_temp_var();

        // Compile the RHS into the temp var
        self.compile_var_init(&temp_var, init, steps)?;

        // Generate index access assignments for each element
        for (i, elem) in arr_pat.elems.iter().enumerate() {
            if let Some(pat) = elem {
                let var_name = self.get_var_name(pat)?;
                steps.push(PlanStep::Assign {
                    var: var_name,
                    expr: ValueExpr::ArrayIndex {
                        array: Box::new(ValueExpr::Variable(temp_var.clone())),
                        index: Box::new(ValueExpr::Literal(JsonValue::Number((i as i64).into()))),
                    },
                });
            }
            // None elements (holes) are skipped: `const [, b] = arr`
        }
        Ok(())
    }

    fn lit_to_json(&self, lit: &Lit) -> JsonValue {
        match lit {
            Lit::Str(s) => JsonValue::String(s.value.to_string_lossy().into_owned()),
            Lit::Num(n) => {
                if n.value.fract() == 0.0 {
                    JsonValue::Number((n.value as i64).into())
                } else {
                    serde_json::Number::from_f64(n.value)
                        .map(JsonValue::Number)
                        .unwrap_or(JsonValue::Null)
                }
            },
            Lit::Bool(b) => JsonValue::Bool(b.value),
            Lit::Null(_) => JsonValue::Null,
            _ => JsonValue::Null,
        }
    }

    fn prop_name_to_string(&self, prop: &PropName) -> Result<String, CompileError> {
        match prop {
            PropName::Ident(ident) => Ok(ident.sym.to_string()),
            PropName::Str(s) => Ok(s.value.to_string_lossy().into_owned()),
            PropName::Num(n) => Ok(n.value.to_string()),
            _ => Err(CompileError::UnsupportedExpression(
                "computed property".into(),
            )),
        }
    }

    fn compile_bin_op(&self, op: BinaryOp) -> Result<BinaryOperator, CompileError> {
        match op {
            BinaryOp::Add => Ok(BinaryOperator::Add),
            BinaryOp::Sub => Ok(BinaryOperator::Sub),
            BinaryOp::Mul => Ok(BinaryOperator::Mul),
            BinaryOp::Div => Ok(BinaryOperator::Div),
            BinaryOp::Mod => Ok(BinaryOperator::Mod),
            BinaryOp::BitOr => Ok(BinaryOperator::BitwiseOr),
            BinaryOp::EqEq => Ok(BinaryOperator::Eq),
            BinaryOp::NotEq => Ok(BinaryOperator::NotEq),
            BinaryOp::EqEqEq => Ok(BinaryOperator::StrictEq),
            BinaryOp::NotEqEq => Ok(BinaryOperator::StrictNotEq),
            BinaryOp::Lt => Ok(BinaryOperator::Lt),
            BinaryOp::LtEq => Ok(BinaryOperator::Lte),
            BinaryOp::Gt => Ok(BinaryOperator::Gt),
            BinaryOp::GtEq => Ok(BinaryOperator::Gte),
            BinaryOp::LogicalAnd => Ok(BinaryOperator::And),
            BinaryOp::LogicalOr => Ok(BinaryOperator::Or),
            BinaryOp::NullishCoalescing => {
                // Handled separately as NullishCoalesce expr
                Err(CompileError::UnsupportedExpression(
                    "nullish coalescing".into(),
                ))
            },
            _ => Err(CompileError::UnsupportedExpression(format!(
                "binary operator {:?}",
                op
            ))),
        }
    }

    fn record_api_call(&mut self, method: &str, path: &PathTemplate) {
        self.api_call_count += 1;

        // Track methods used
        if !self.methods_used.contains(&method.to_string()) {
            self.methods_used.push(method.to_string());
        }

        // Track if mutations
        if method != "GET" && method != "HEAD" && method != "OPTIONS" {
            self.has_mutations = true;
        }

        // Track endpoints (simplified path for static paths)
        let endpoint = if !path.is_dynamic() {
            path.parts
                .iter()
                .filter_map(|p| match p {
                    PathPart::Literal(s) => Some(s.clone()),
                    _ => None,
                })
                .collect::<String>()
        } else {
            "{dynamic}".to_string()
        };
        if !self.endpoints.contains(&endpoint) {
            self.endpoints.push(endpoint);
        }
    }
}

impl Default for PlanCompiler {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// PLAN EXECUTOR - Executes the compiled execution plan
// ============================================================================

/// A request path that has already been fully resolved and checked.
///
/// # What the value guarantees
///
/// Every value of this type was produced by [`ResolvedPath::from_checked`], which
/// is the only constructor. So, for any `ResolvedPath` an implementor receives:
///
/// - Every `${var}` template-literal interpolation (LAYER 1) and every `{key}`
///   placeholder (LAYER 2) has already been substituted. **There is nothing left
///   to resolve** and an implementor must not attempt its own placeholder
///   resolution.
/// - Each substituted contribution passed
///   [`validate_path_placeholder`](crate::validate_path_placeholder) where it was
///   produced.
/// - The COMPOSED string passed
///   [`validate_resolved_path`](crate::validate_resolved_path) — so it carries no
///   parent-directory sequence, no residual `{`/`}`, no `#`, no backslash, no
///   ASCII control byte, no over-cap segment and no empty interior segment, on
///   EITHER side of an author-written `?`. None of those is visible to a per-value
///   check, because a composition belongs to no single value.
/// - **At most one `?`,** and only one an author wrote into a
///   `PathPart::Literal`. See [`ResolvedPath::from_checked`] for why that single
///   exemption is safe.
/// - The `body` passed alongside has already had the path-consumed keys removed.
///
/// # What the value does NOT guarantee
///
/// It does not prove that a *spec-declared* narrowing (an OpenAPI `pattern` or
/// `maxLength`) was applied — that depends on the implementor's
/// [`HttpExecutor::placeholder_rules`] override, and an implementor that keeps the
/// default still gets the unconditional floor and the always-on cap but no
/// narrowing. It is also a **migration marker** as much as a proof carrier: its
/// job is to make a stale implementor written against the old `&str` parameter
/// fail to COMPILE rather than silently receive already-resolved data and
/// double-resolve it (Phase 128, D-09).
///
/// # Constructor shape
///
/// There is deliberately no `new` and no `new_unchecked`. A public unchecked
/// constructor whose rustdoc claims an invariant is exactly the
/// documented-but-absent class Phase 128 exists to correct, so the check lives
/// *inside* the one constructor and the type cannot be forged from outside.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedPath<'a>(&'a str);

impl<'a> ResolvedPath<'a> {
    /// Check `path` as a composed request path and wrap it on success.
    ///
    /// This is the only constructor: it runs
    /// [`validate_resolved_path`](crate::validate_resolved_path), so the
    /// invariant documented on the type is established here rather than
    /// asserted.
    ///
    /// # The one narrowing: an author-written `?` is permitted
    ///
    /// `validate_resolved_path` refuses a query separator ANYWHERE, and core keeps
    /// that strict rule — it is a general-purpose composed-path checker and other
    /// callers want it. This constructor splits at the FIRST `?` and applies the
    /// full rule set to each side, which exempts exactly that one separator and
    /// nothing else.
    ///
    /// Why that is safe rather than a hole. Both per-value floors already refuse
    /// `?` in a substituted value, in literal AND percent-encoded form, with a
    /// decode-once pass so `%253F`-style regress cannot slip through. So a `?`
    /// surviving into the composed string can only have come from a
    /// `PathPart::Literal` — script text the operator authored and shipped, not
    /// caller data. The asymmetry with traversal is the whole point: refusing `..`
    /// from a literal catches a traversal bug, while refusing `?` from a literal
    /// rejects legitimate authoring. Same rule, different work.
    ///
    /// What the split does NOT relax, because a narrowing must not become a hole:
    ///
    /// - Traversal, control bytes, backslash, `#`, residual `{`/`}`, over-cap
    ///   segments and empty interior segments are checked on **both** sides. So
    ///   `/a/../b?x=1` is still refused for the traversal, and `/a?x=%00` is still
    ///   refused for the control byte.
    /// - A SECOND `?` is still refused: only the first is split off, so the query
    ///   portion is checked by the unmodified rule, which denies `?`.
    /// - An empty query portion is still refused — a dangling `/x?` is a doubled
    ///   or trailing separator, which is the same class as a trailing `/`.
    /// - A `?` reaching the composed string from a VALUE never gets here: the
    ///   per-value floor has already refused it.
    ///
    /// One inherited conservatism, stated so it is not a surprise: `%25` is
    /// refused outright (it is what bounds the decode to a single pass), so a query
    /// carrying a percent-encoded percent sign is refused. That is unchanged from
    /// the path portion's long-standing behaviour, not new here.
    ///
    /// # Errors
    ///
    /// Returns the [`PlaceholderRefusal`](crate::PlaceholderRefusal) from
    /// `validate_resolved_path`. The refusal is value-free: it names the rule and
    /// the declared expectation, never any byte of the path it refused.
    pub fn from_checked(path: &'a str) -> Result<Self, crate::PlaceholderRefusal> {
        match path.split_once('?') {
            // No author-written separator: the whole string is a path.
            None => crate::validate_resolved_path(path)?,
            // Exactly one author-written `?`. The separator itself is permitted;
            // both sides still face the full, unmodified rule set.
            Some((path_part, query_part)) => {
                crate::validate_resolved_path(path_part)?;
                crate::validate_resolved_path(query_part)?;
            },
        }
        Ok(Self(path))
    }

    /// The resolved path as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &'a str {
        self.0
    }
}

impl std::fmt::Display for ResolvedPath<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

/// Trait for making HTTP requests during execution.
///
/// This abstraction allows the executor to be used with different HTTP clients
/// and enables easy testing with mock implementations.
///
/// # The D-09 contract change (Phase 128)
///
/// `execute_request`'s path parameter used to be a bare `&str` carrying the
/// TEMPLATE the script wrote, and each implementor resolved its own `{key}`
/// placeholders inside its own impl. That made the public trait a blind seam: a
/// decorator wrapping `HttpExecutor` to inspect outbound requests saw only the
/// template and never the values, so a placeholder carrying a query separator
/// became a different endpoint with nothing in a position to notice.
///
/// Resolution now happens in [`PlanExecutor`] BEFORE dispatch, and the parameter
/// is a [`ResolvedPath`] rather than a `&str` so the change is a **compile error**
/// for a stale implementor instead of a silent semantic shift. A stale
/// implementor that kept compiling would quietly double-resolve an
/// already-resolved path, which for a security fix is the worst available
/// outcome.
#[async_trait::async_trait]
pub trait HttpExecutor: Send + Sync {
    /// Execute an HTTP request.
    ///
    /// `path` is already fully resolved and checked — see [`ResolvedPath`]. Do
    /// NOT perform placeholder substitution here; `body` has already had the
    /// path-consumed keys removed.
    async fn execute_request(
        &self,
        method: &str,
        path: ResolvedPath<'_>,
        body: Option<JsonValue>,
    ) -> Result<JsonValue, ExecutionError>;

    /// The placeholder rules to apply to one LAYER-2 `{param}` value.
    ///
    /// This is the D4(b) seam on the Code Mode surface. [`PlanExecutor`] has no
    /// access to an OpenAPI document; the only component that does is the
    /// executor implementation, so the narrowing has to be asked for here.
    ///
    /// `method` is part of the signature and not decoration: an OpenAPI schema
    /// indexes operations by `(path, METHOD)`, so `GET /things/{id}` and
    /// `DELETE /things/{id}` are two operations that may declare different
    /// constraints for the same `id`. A `(path_template, param)` signature could
    /// not disambiguate them and would have to either scan linearly or narrow
    /// from the wrong operation.
    ///
    /// `path_template` is the path as it reaches layer 2 — after layer-1
    /// `${var}` interpolation and before `{key}` substitution. For the ordinary
    /// case of a string-literal path that is byte-identical to the OpenAPI path
    /// template, which is what makes a spec lookup work.
    ///
    /// # The default is safe
    ///
    /// The default returns [`PlaceholderRules::default()`](crate::PlaceholderRules),
    /// which is FLOOR-PLUS-CAP-WITH-NO-NARROWING. An implementor that keeps the
    /// default still gets the unconditional character floor and the always-on
    /// 256-code-point cap on every value; what is absent is only the
    /// spec-declared narrowing. The floor is never what a missing override costs.
    fn placeholder_rules(
        &self,
        method: &str,
        path_template: &str,
        param: &str,
    ) -> crate::PlaceholderRules<'_> {
        let _ = (method, path_template, param);
        crate::PlaceholderRules::default()
    }
}

/// Executor for MCP foundation server calls.
///
/// Analogous to `HttpExecutor` for API calls, this trait abstracts MCP tool
/// invocation for use in the AST-based executor. Implementations delegate to
/// actual foundation clients (e.g., `CompositionClient`).
#[cfg(feature = "mcp-code-mode")]
#[async_trait::async_trait]
pub trait McpExecutor: Send + Sync {
    /// Call a tool on a foundation server.
    async fn call_tool(
        &self,
        server_id: &str,
        tool_name: &str,
        args: JsonValue,
    ) -> Result<JsonValue, ExecutionError>;
}

/// Executor for SDK-backed API calls.
///
/// Analogous to `HttpExecutor` for HTTP calls, this trait abstracts named SDK
/// operation invocation. Implementations route `api.<operation>(args)` to actual
/// SDK calls (e.g., AWS Cost Explorer).
#[async_trait::async_trait]
pub trait SdkExecutor: Send + Sync {
    /// Execute a named SDK operation.
    ///
    /// `operation` is the camelCase method name (e.g., "getCostAndUsage").
    /// `args` is the optional JSON object argument from the script.
    async fn execute_operation(
        &self,
        operation: &str,
        args: Option<JsonValue>,
    ) -> Result<JsonValue, ExecutionError>;
}

// ============================================================================
// MOCK HTTP EXECUTOR - For dry-run, testing, and development
// ============================================================================

/// Execution mode for the mock executor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MockExecutionMode {
    /// Dry-run: Returns mock responses, records calls for preview.
    #[default]
    DryRun,
    /// Testing: Returns configured mock responses for assertions.
    Testing,
    /// Record: Passes through to a real executor and records responses.
    Record,
}

/// Mock HTTP executor for dry-run validation and testing.
///
/// This executor doesn't make real HTTP calls. Instead, it:
/// - Records all API calls that would be made
/// - Returns configurable mock responses
/// - Enables dry-run validation showing what a script would do
///
/// # Example
///
/// ```ignore
/// use mcp_server_common::code_mode::executor::{MockHttpExecutor, PlanExecutor};
///
/// // Create a mock executor for dry-run
/// let mock = MockHttpExecutor::new_dry_run();
///
/// // Or with custom responses for testing
/// let mock = MockHttpExecutor::new_testing()
///     .with_response("/users", json!({"users": [{"id": 1, "name": "Test"}]}))
///     .with_response("/orders/*", json!({"orders": []}));
///
/// // Execute the plan
/// let executor = PlanExecutor::new(mock, config);
/// let result = executor.execute(plan).await?;
///
/// // Check what calls would be made
/// for call in mock.recorded_calls() {
///     println!("Would call: {} {}", call.method, call.path);
/// }
/// ```
///
/// # Migration note for downstream test authors (Phase 128, D-09)
///
/// The recorded `path` is now the RESOLVED path, and any direct
/// `execute_request` call must pass a [`ResolvedPath`] built with
/// [`ResolvedPath::from_checked`] rather than a bare `&str` — so a mock
/// expectation written against a `{key}` TEMPLATE must be rewritten against the
/// substituted path it expects to see.
pub struct MockHttpExecutor {
    /// Mock responses by path pattern (exact match or glob pattern with *)
    responses: std::sync::RwLock<HashMap<String, JsonValue>>,
    /// Default response for unmatched paths
    default_response: JsonValue,
    /// Record of all calls made (method, path, body, response)
    recorded_calls: std::sync::RwLock<Vec<MockedCall>>,
}

/// A recorded mock call with request and response.
#[derive(Debug, Clone, Serialize)]
pub struct MockedCall {
    /// HTTP method (GET, POST, etc.)
    pub method: String,
    /// Request path
    pub path: String,
    /// Request body if any
    pub body: Option<JsonValue>,
    /// Response returned
    pub response: JsonValue,
}

impl MockHttpExecutor {
    /// Create a new mock executor for dry-run mode.
    /// Returns empty objects `{}` for all calls.
    pub fn new_dry_run() -> Self {
        Self {
            responses: std::sync::RwLock::new(HashMap::new()),
            default_response: JsonValue::Object(serde_json::Map::new()),
            recorded_calls: std::sync::RwLock::new(Vec::new()),
        }
    }

    /// Create a new mock executor for testing mode.
    /// Configure responses with `with_response()`.
    pub fn new_testing() -> Self {
        Self {
            responses: std::sync::RwLock::new(HashMap::new()),
            default_response: JsonValue::Object(serde_json::Map::new()),
            recorded_calls: std::sync::RwLock::new(Vec::new()),
        }
    }

    /// Set the default response for unmatched paths.
    pub fn with_default_response(mut self, response: JsonValue) -> Self {
        self.default_response = response;
        self
    }

    /// Add a mock response for a specific path pattern.
    /// Supports exact matches and simple glob patterns with `*`.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// mock.with_response("/users", json!({"users": []}))
    ///     .with_response("/users/*", json!({"id": 1, "name": "Test"}))
    ///     .with_response("/orders/*/items", json!({"items": []}));
    /// ```
    pub fn with_response(self, path_pattern: &str, response: JsonValue) -> Self {
        self.responses
            .write()
            .unwrap()
            .insert(path_pattern.to_string(), response);
        self
    }

    /// Add a mock response (non-builder version).
    pub fn add_response(&self, path_pattern: &str, response: JsonValue) {
        self.responses
            .write()
            .unwrap()
            .insert(path_pattern.to_string(), response);
    }

    /// Get all recorded calls.
    pub fn recorded_calls(&self) -> Vec<MockedCall> {
        self.recorded_calls.read().unwrap().clone()
    }

    /// Clear all recorded calls.
    pub fn clear_calls(&self) {
        self.recorded_calls.write().unwrap().clear();
    }

    /// Get the number of calls made.
    pub fn call_count(&self) -> usize {
        self.recorded_calls.read().unwrap().len()
    }

    /// Check if a specific path was called.
    pub fn was_called(&self, path: &str) -> bool {
        self.recorded_calls
            .read()
            .unwrap()
            .iter()
            .any(|c| c.path == path)
    }

    /// Check if a path was called with a specific method.
    pub fn was_called_with_method(&self, method: &str, path: &str) -> bool {
        self.recorded_calls
            .read()
            .unwrap()
            .iter()
            .any(|c| c.method == method && c.path == path)
    }

    /// Find the response for a path, checking patterns.
    fn find_response(&self, path: &str) -> JsonValue {
        let responses = self.responses.read().unwrap();

        // First try exact match
        if let Some(response) = responses.get(path) {
            return response.clone();
        }

        // Then try pattern matching
        for (pattern, response) in responses.iter() {
            if Self::matches_pattern(pattern, path) {
                return response.clone();
            }
        }

        // Return default
        self.default_response.clone()
    }

    /// Simple glob pattern matching (supports * as wildcard for path segments).
    fn matches_pattern(pattern: &str, path: &str) -> bool {
        if !pattern.contains('*') {
            return pattern == path;
        }

        let pattern_parts: Vec<&str> = pattern.split('/').collect();
        let path_parts: Vec<&str> = path.split('/').collect();

        if pattern_parts.len() != path_parts.len() {
            // Check for trailing * that matches multiple segments
            if pattern.ends_with("*") && path_parts.len() >= pattern_parts.len() - 1 {
                // Allow trailing wildcard to match remaining segments
            } else {
                return false;
            }
        }

        for (p, s) in pattern_parts.iter().zip(path_parts.iter()) {
            if *p != "*" && *p != *s {
                return false;
            }
        }

        true
    }
}

#[async_trait::async_trait]
impl HttpExecutor for MockHttpExecutor {
    async fn execute_request(
        &self,
        method: &str,
        path: ResolvedPath<'_>,
        body: Option<JsonValue>,
    ) -> Result<JsonValue, ExecutionError> {
        let path = path.as_str();
        let response = self.find_response(path);

        // Record the call
        let call = MockedCall {
            method: method.to_string(),
            path: path.to_string(),
            body,
            response: response.clone(),
        };
        self.recorded_calls.write().unwrap().push(call);

        Ok(response)
    }
}

// Implement Send + Sync (safe because we use RwLock)
unsafe impl Send for MockHttpExecutor {}
unsafe impl Sync for MockHttpExecutor {}

/// Result of executing a plan.
#[derive(Debug, Clone, Serialize)]
pub struct ExecutionResult {
    /// The final return value
    pub value: JsonValue,
    /// Log of all API calls made
    pub api_calls: Vec<ApiCallLog>,
    /// Total execution time in milliseconds
    pub execution_time_ms: u64,
}

/// Log entry for an API call.
#[derive(Debug, Clone, Serialize)]
pub struct ApiCallLog {
    /// HTTP method
    pub method: String,
    /// The fully resolved request path.
    ///
    /// **Disclosure note (Phase 128, T-128-23 — an ACCEPTED residual.)** This is
    /// an internal execution log and not a client-facing message, which is why it
    /// keeps the resolved path: redacting it would remove the diagnostic signal
    /// the log exists for. But any caller-facing surface that exposes
    /// [`ExecutionResult::api_calls`] inherits the disclosure — a refused or
    /// attacker-shaped path that was deliberately kept out of the error message
    /// is still present here. Such a surface MUST redact this field.
    pub path: String,
    /// Request body (if any)
    pub body: Option<JsonValue>,
    /// Response value
    pub response: JsonValue,
    /// Time taken in milliseconds
    pub duration_ms: u64,
}

/// Map a value-free [`PlaceholderRefusal`](crate::PlaceholderRefusal) into the
/// executor's error type.
///
/// `PlaceholderRefusal`'s `Display` names the parameter and the DECLARED
/// expectation and never any byte of the value it refused, so the resulting
/// message is safe to surface to an MCP client as-is. Kept a free function so the
/// two `ApiCall` arms cannot drift into two different renderings.
fn refusal_to_execution_error(refusal: crate::PlaceholderRefusal) -> ExecutionError {
    ExecutionError::RuntimeError {
        message: refusal.to_string(),
    }
}

/// Apply the LAYER-1 floor to one rendered `${var}` / `${expr}` contribution.
///
/// `PlaceholderRules::default()` and deliberately NOT
/// [`HttpExecutor::placeholder_rules`]: a layer-1 part is not a spec-declared
/// path parameter. It can appear anywhere in the template, including mid-segment,
/// so there is no OpenAPI `Parameter` to narrow from — the unconditional
/// character floor plus the always-on 256-code-point cap is exactly the right
/// level here. A reader who expects a `placeholder_rules` consultation at this
/// layer is looking for something that has no well-defined answer.
fn floor_layer_one_contribution(param: &str, rendered: &str) -> Result<(), ExecutionError> {
    crate::validate_path_placeholder(param, rendered, &crate::PlaceholderRules::default())
        .map_err(refusal_to_execution_error)
}

/// Render a JSON scalar for a LAYER-2 `{key}` substitution, REJECTING non-scalars
/// (WR-03 / GAP 4).
///
/// Moved up from `pmcp-server-toolkit`'s `HttpCodeExecutor::scalar_str` by D-09,
/// with the rule preserved byte for byte: a scalar (`String`, `Number`, `Bool`,
/// `Null`) renders to a bare string (`Null` -> `"null"`, preserving prior
/// behaviour); an `Object` or `Array` is rejected rather than silently
/// JSON-stringified into the URL.
///
/// # Errors
///
/// Returns [`ExecutionError::RuntimeError`] naming `key`. Per Pitfall 5 the
/// message names the KEY only — never the value.
fn render_path_scalar(key: &str, value: &JsonValue) -> Result<String, ExecutionError> {
    match value {
        JsonValue::String(s) => Ok(s.clone()),
        JsonValue::Null => Ok("null".to_string()),
        JsonValue::Number(n) => Ok(n.to_string()),
        JsonValue::Bool(b) => Ok(b.to_string()),
        JsonValue::Object(_) | JsonValue::Array(_) => Err(ExecutionError::RuntimeError {
            message: format!("path/query param '{key}' must be a scalar"),
        }),
    }
}

/// LAYER-2 `{key}` placeholder resolution, moved ahead of
/// [`HttpExecutor::execute_request`] by Phase 128 D-09.
///
/// Returns the substituted path plus the body with the path-consumed keys
/// removed. An implementor used to do this inside its own impl, which is what
/// made the public trait a blind seam (T-128-20).
///
/// # Ordering
///
/// Two passes on purpose. Pass one renders and CHECKS every contribution; pass
/// two applies them. So a refusal on any one placeholder aborts with no
/// substitution having been applied at all, rather than leaving a
/// half-substituted path one `?` away from being dispatched. Pass one also tests
/// containment against the ORIGINAL template rather than a progressively
/// substituted copy, so a value that itself contains `{`/`}` cannot manufacture a
/// placeholder for a later key to fill. Body iteration order is
/// `serde_json::Map`'s, which is deterministic (insertion order under this
/// workspace's `preserve_order`, key order otherwise).
///
/// # Errors
///
/// Returns [`ExecutionError::RuntimeError`] on a non-scalar value or a
/// [`PlaceholderRefusal`](crate::PlaceholderRefusal). Both messages are
/// value-free.
fn resolve_layer_two_placeholders<H: HttpExecutor + ?Sized>(
    http: &H,
    method: &str,
    template: &str,
    body: Option<JsonValue>,
) -> Result<(String, Option<JsonValue>), ExecutionError> {
    let Some(JsonValue::Object(obj)) = body.as_ref() else {
        return Ok((template.to_string(), body));
    };

    // PASS 1 — render + check, mutating nothing.
    let mut substitutions: Vec<(String, String)> = Vec::new();
    let mut remaining = serde_json::Map::new();
    for (key, value) in obj {
        let placeholder = format!("{{{key}}}");
        if template.contains(&placeholder) {
            let rendered = render_path_scalar(key, value)?;
            let rules = http.placeholder_rules(method, template, key);
            crate::validate_path_placeholder(key, &rendered, &rules)
                .map_err(refusal_to_execution_error)?;
            substitutions.push((placeholder, rendered));
        } else {
            remaining.insert(key.clone(), value.clone());
        }
    }

    // PASS 2 — apply.
    let mut resolved = template.to_string();
    for (placeholder, rendered) in substitutions {
        resolved = resolved.replace(&placeholder, &rendered);
    }

    let remaining = if remaining.is_empty() {
        None
    } else {
        Some(JsonValue::Object(remaining))
    };
    Ok((resolved, remaining))
}

/// Executes a compiled execution plan.
pub struct PlanExecutor<H: HttpExecutor> {
    http: H,
    config: ExecutionConfig,
    variables: HashMap<String, JsonValue>,
    api_calls: Vec<ApiCallLog>,
    api_call_count: usize,
    #[cfg(feature = "mcp-code-mode")]
    mcp: Option<Box<dyn McpExecutor>>,
    /// Optional SDK executor for SDK-backed servers (e.g., aws-billing).
    sdk: Option<Box<dyn SdkExecutor>>,
}

impl<H: HttpExecutor> PlanExecutor<H> {
    /// Create a new executor with the given HTTP client.
    pub fn new(http: H, config: ExecutionConfig) -> Self {
        Self {
            http,
            config,
            variables: HashMap::new(),
            api_calls: Vec::new(),
            api_call_count: 0,
            #[cfg(feature = "mcp-code-mode")]
            mcp: None,
            sdk: None,
        }
    }

    /// Set the MCP executor for foundation server calls.
    #[cfg(feature = "mcp-code-mode")]
    pub fn set_mcp_executor(&mut self, executor: impl McpExecutor + 'static) {
        self.mcp = Some(Box::new(executor));
    }

    /// Set the SDK executor for SDK-backed servers.
    pub fn set_sdk_executor(&mut self, executor: impl SdkExecutor + 'static) {
        self.sdk = Some(Box::new(executor));
    }

    /// Pre-bind a variable before execution (e.g., `args` for script tools).
    pub fn set_variable(&mut self, name: impl Into<String>, value: JsonValue) {
        self.variables.insert(name.into(), value);
    }

    /// Execute a plan and return the result.
    pub async fn execute(
        &mut self,
        plan: &ExecutionPlan,
    ) -> Result<ExecutionResult, ExecutionError> {
        let start = std::time::Instant::now();

        let mut return_value = JsonValue::Null;

        for step in &plan.steps {
            match self.execute_step(step).await? {
                StepOutcome::Return(value) => {
                    return_value = value;
                    break; // Early return — stop executing further steps
                },
                StepOutcome::None | StepOutcome::Continue | StepOutcome::Break => {},
            }
        }

        // Validate output against output blocklist.
        // These are fields that can be used internally but cannot be returned.
        let blocked_in_output =
            find_blocked_fields_in_output(&return_value, &self.config.output_blocked_fields);

        if !blocked_in_output.is_empty() {
            return Err(ExecutionError::RuntimeError {
                message: format!(
                    "Script output contains blocked fields: {}",
                    blocked_in_output.join(", ")
                ),
            });
        }

        Ok(ExecutionResult {
            value: return_value,
            api_calls: std::mem::take(&mut self.api_calls),
            execution_time_ms: start.elapsed().as_millis() as u64,
        })
    }

    /// Execute a single step, returning a `StepOutcome` for control flow.
    /// Uses Box::pin for recursive calls to avoid infinite future size.
    fn execute_step<'a>(
        &'a mut self,
        step: &'a PlanStep,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<StepOutcome, ExecutionError>> + Send + 'a>,
    > {
        Box::pin(async move {
            match step {
                PlanStep::ApiCall {
                    result_var,
                    method,
                    path,
                    body,
                } => {
                    self.api_call_count += 1;
                    if self.api_call_count > self.config.max_api_calls {
                        return Err(ExecutionError::RuntimeError {
                            message: format!(
                                "Too many API calls: {} (max: {})",
                                self.api_call_count, self.config.max_api_calls
                            ),
                        });
                    }

                    // LAYER 1 — `${var}` / `${expr}` interpolation, each
                    // contribution floored inside `resolve_path` (FORK 2).
                    let templated_path = self.resolve_path(path)?;
                    let evaluated_body = match body {
                        Some(expr) => Some(self.evaluate(expr)?),
                        None => None,
                    };
                    // LAYER 2 — `{key}` resolution, moved ahead of dispatch (D-09).
                    let (resolved_path, resolved_body) = resolve_layer_two_placeholders(
                        &self.http,
                        method,
                        &templated_path,
                        evaluated_body,
                    )?;
                    // THE COMPOSED CHECK — the only check that can see an
                    // adjacency (T-128-20b). `.` + `.` composes to a traversal and
                    // 180 + 200 code points compose over the cap, from values that
                    // each passed both per-value checks above; a residual `{`/`}`
                    // from an unsubstituted placeholder is refused here too.
                    let checked_path = ResolvedPath::from_checked(&resolved_path)
                        .map_err(refusal_to_execution_error)?;

                    let call_start = std::time::Instant::now();
                    let raw_response = self
                        .http
                        .execute_request(method, checked_path, resolved_body.clone())
                        .await
                        .map_err(|e| ExecutionError::RuntimeError {
                            // The resolved path is deliberately NOT formatted in
                            // (RESEARCH Pitfall 7 / SC-7): a D4 refusal is
                            // value-free where it is raised, and re-attaching the
                            // path here is what would deliver the exact injected
                            // path to the client.
                            message: format!("{method} api call '{result_var}' failed: {e}"),
                        })?;
                    let duration_ms = call_start.elapsed().as_millis() as u64;

                    // Filter blocked fields from API response before scripts can access them.
                    // This implements the "internal blocklist" - fields that are never accessible.
                    let response = filter_blocked_fields(raw_response, &self.config.blocked_fields);

                    self.api_calls.push(ApiCallLog {
                        method: method.clone(),
                        path: resolved_path,
                        body: resolved_body,
                        response: response.clone(),
                        duration_ms,
                    });

                    if result_var != "_" {
                        self.variables.insert(result_var.clone(), response);
                    }
                    Ok(StepOutcome::None)
                },

                PlanStep::Assign { var, expr } => {
                    let value = self.evaluate(expr)?;
                    self.variables.insert(var.clone(), value);
                    Ok(StepOutcome::None)
                },

                PlanStep::Conditional {
                    condition,
                    then_steps,
                    else_steps,
                } => {
                    let cond_value = self.evaluate(condition)?;
                    let steps = if shared_is_truthy(&cond_value) {
                        then_steps
                    } else {
                        else_steps
                    };

                    for step in steps {
                        match self.execute_step(step).await? {
                            StepOutcome::None => {},
                            outcome => return Ok(outcome),
                        }
                    }
                    Ok(StepOutcome::None)
                },

                PlanStep::BoundedLoop {
                    item_var,
                    collection,
                    max_iterations,
                    body,
                } => {
                    let collection_value = self.evaluate(collection)?;
                    let items = match collection_value {
                        JsonValue::Array(arr) => arr,
                        _ => {
                            return Err(ExecutionError::RuntimeError {
                                message: "Loop collection must be an array".into(),
                            })
                        },
                    };

                    let limit = (*max_iterations).min(self.config.max_loop_iterations);
                    'outer: for item in items.into_iter().take(limit) {
                        self.variables.insert(item_var.clone(), item);

                        for step in body {
                            match self.execute_step(step).await? {
                                StepOutcome::Return(value) => {
                                    return Ok(StepOutcome::Return(value))
                                },
                                StepOutcome::None => {},
                                StepOutcome::Continue => continue 'outer,
                                StepOutcome::Break => break 'outer,
                            }
                        }
                    }
                    Ok(StepOutcome::None)
                },

                PlanStep::Return { value } => {
                    let result = self.evaluate(value)?;
                    Ok(StepOutcome::Return(result))
                },

                PlanStep::TryCatch {
                    try_steps,
                    catch_var,
                    catch_steps,
                    finally_steps,
                } => {
                    // Execute try block
                    let try_result = async {
                        for step in try_steps {
                            match self.execute_step(step).await? {
                                StepOutcome::None => {},
                                outcome => return Ok::<StepOutcome, ExecutionError>(outcome),
                            }
                        }
                        Ok(StepOutcome::None)
                    }
                    .await;

                    // If try succeeded, just run finally
                    let result = match try_result {
                        Ok(outcome) => {
                            // Try block succeeded
                            outcome
                        },
                        Err(error) => {
                            // Try block failed, run catch
                            if let Some(var) = catch_var {
                                // Store the error in the catch variable
                                let error_obj = JsonValue::Object(serde_json::Map::from_iter([(
                                    "message".to_string(),
                                    JsonValue::String(format!("{}", error)),
                                )]));
                                self.variables.insert(var.clone(), error_obj);
                            }

                            // Execute catch block
                            let mut catch_outcome = StepOutcome::None;
                            for step in catch_steps {
                                match self.execute_step(step).await? {
                                    StepOutcome::None => {},
                                    outcome => {
                                        catch_outcome = outcome;
                                        break;
                                    },
                                }
                            }
                            catch_outcome
                        },
                    };

                    // Execute finally block (always runs)
                    for step in finally_steps {
                        match self.execute_step(step).await? {
                            StepOutcome::None => {},
                            outcome => return Ok(outcome),
                        }
                    }

                    Ok(result)
                },

                // Parallel API calls: await Promise.all([api.get(...), ...])
                // Executed sequentially (true parallelism isn't needed for correctness),
                // results collected into an array assigned to result_var.
                PlanStep::ParallelApiCalls { result_var, calls } => {
                    let mut results = Vec::with_capacity(calls.len());
                    for (temp_var, method, path, body) in calls {
                        self.api_call_count += 1;
                        if self.api_call_count > self.config.max_api_calls {
                            return Err(ExecutionError::RuntimeError {
                                message: format!(
                                    "Maximum API calls exceeded ({})",
                                    self.config.max_api_calls
                                ),
                            });
                        }

                        // LAYER 1, then LAYER 2, then the COMPOSED check — the same
                        // three steps as the single `ApiCall` arm. See that arm's
                        // comments; both arms must carry all three or the class is
                        // closed on only one of them.
                        let templated_path = self.resolve_path(path)?;
                        let evaluated_body = body.as_ref().map(|b| self.evaluate(b)).transpose()?;
                        let (resolved_path, resolved_body) = resolve_layer_two_placeholders(
                            &self.http,
                            method,
                            &templated_path,
                            evaluated_body,
                        )?;
                        let checked_path = ResolvedPath::from_checked(&resolved_path)
                            .map_err(refusal_to_execution_error)?;
                        let call_start = std::time::Instant::now();
                        let raw_response = self
                            .http
                            .execute_request(method, checked_path, resolved_body.clone())
                            .await
                            .map_err(|e| ExecutionError::RuntimeError {
                                // No resolved path here either (Pitfall 7 / SC-7).
                                message: format!("{method} api call '{temp_var}' failed: {e}"),
                            })?;
                        let duration_ms = call_start.elapsed().as_millis() as u64;
                        let response =
                            filter_blocked_fields(raw_response, &self.config.blocked_fields);

                        self.api_calls.push(ApiCallLog {
                            method: method.clone(),
                            path: resolved_path,
                            body: resolved_body,
                            response: response.clone(),
                            duration_ms,
                        });

                        results.push(response);
                    }
                    self.variables
                        .insert(result_var.clone(), JsonValue::Array(results));
                    Ok(StepOutcome::None)
                },

                // Continue: signal to skip to next loop iteration
                PlanStep::Continue => Ok(StepOutcome::Continue),

                // Break: signal to exit the current loop
                PlanStep::Break => Ok(StepOutcome::Break),

                // MCP tool call: await mcp.call('server', 'tool', { args })
                #[cfg(feature = "mcp-code-mode")]
                PlanStep::McpCall {
                    result_var,
                    server_id,
                    tool_name,
                    args,
                } => {
                    self.api_call_count += 1;
                    if self.api_call_count > self.config.max_api_calls {
                        return Err(ExecutionError::RuntimeError {
                            message: format!(
                                "Too many calls: {} (max: {})",
                                self.api_call_count, self.config.max_api_calls
                            ),
                        });
                    }

                    let resolved_args = match args {
                        Some(expr) => self.evaluate(expr)?,
                        None => JsonValue::Object(Default::default()),
                    };

                    let mcp_executor =
                        self.mcp
                            .as_ref()
                            .ok_or_else(|| ExecutionError::RuntimeError {
                                message: "MCP executor not configured".into(),
                            })?;

                    let call_start = std::time::Instant::now();
                    let result = mcp_executor
                        .call_tool(server_id, tool_name, resolved_args.clone())
                        .await?;
                    let duration_ms = call_start.elapsed().as_millis() as u64;

                    self.api_calls.push(ApiCallLog {
                        method: format!("MCP:{}.{}", server_id, tool_name),
                        path: format!("{}/{}", server_id, tool_name),
                        body: Some(resolved_args),
                        response: result.clone(),
                        duration_ms,
                    });

                    if result_var != "_" {
                        self.variables.insert(result_var.clone(), result);
                    }
                    Ok(StepOutcome::None)
                },

                // SDK call: await api.getCostAndUsage({ ... })
                PlanStep::SdkCall {
                    result_var,
                    operation,
                    args,
                } => {
                    self.api_call_count += 1;
                    if self.api_call_count > self.config.max_api_calls {
                        return Err(ExecutionError::RuntimeError {
                            message: format!(
                                "Too many calls: {} (max: {})",
                                self.api_call_count, self.config.max_api_calls
                            ),
                        });
                    }

                    let resolved_args =
                        args.as_ref().map(|expr| self.evaluate(expr)).transpose()?;

                    let sdk_executor =
                        self.sdk
                            .as_ref()
                            .ok_or_else(|| ExecutionError::RuntimeError {
                                message: "SDK executor not configured".into(),
                            })?;

                    let call_start = std::time::Instant::now();
                    let result = sdk_executor
                        .execute_operation(operation, resolved_args.clone())
                        .await?;
                    let duration_ms = call_start.elapsed().as_millis() as u64;

                    self.api_calls.push(ApiCallLog {
                        method: operation.clone(),
                        path: format!("sdk:{}", operation),
                        body: resolved_args,
                        response: result.clone(),
                        duration_ms,
                    });

                    if result_var != "_" {
                        self.variables.insert(result_var.clone(), result);
                    }
                    Ok(StepOutcome::None)
                },
            }
        })
    }

    /// Resolve a path template to a concrete path string — LAYER 1.
    ///
    /// # The FORK-2 floor (Phase 128, T-128-20a)
    ///
    /// Every DYNAMIC contribution is passed through
    /// [`validate_path_placeholder`](crate::validate_path_placeholder) before it is
    /// pushed, and the first refusal returns so nothing reaches `result`. This arm
    /// used to push the stringified value straight in, which meant a script
    /// writing `` api.get(`/search/${v}`) `` never touched a `{key}` placeholder,
    /// never entered layer 2, and was never floored at all. Code Mode scripts are
    /// model-authored, so that was the untrusted route.
    ///
    /// `PathPart::Literal` parts are deliberately NOT checked: they are the
    /// script's own literal text from the compiled template, not
    /// caller-substituted data, and flooring them would refuse every legitimate
    /// template whose literal segments contain `/`.
    ///
    /// # Errors
    ///
    /// Returns [`ExecutionError::RuntimeError`] on an undefined variable, an
    /// expression-evaluation failure, or a placeholder refusal. A refusal's
    /// message comes from the refusal's own value-free `Display`.
    fn resolve_path(&self, path: &PathTemplate) -> Result<String, ExecutionError> {
        let mut result = String::new();
        for (index, part) in path.parts.iter().enumerate() {
            match part {
                PathPart::Literal(s) => result.push_str(s),
                PathPart::Variable(var) => {
                    let value =
                        self.variables
                            .get(var)
                            .ok_or_else(|| ExecutionError::RuntimeError {
                                message: format!("Undefined variable in path: {}", var),
                            })?;
                    let rendered = shared_json_to_string_with_mode(value, JsonStringMode::Json);
                    // The refusal names the variable IDENTIFIER, which the part
                    // carries. A script-chosen identifier is operator-shipped
                    // content, unlike the value.
                    floor_layer_one_contribution(var, &rendered)?;
                    result.push_str(&rendered);
                },
                PathPart::Expression(expr) => {
                    let value = self.evaluate(expr)?;
                    let rendered = shared_json_to_string_with_mode(&value, JsonStringMode::Json);
                    // An expression has NO name, and a rendering of the expression
                    // body could itself contain caller text — so the refusal
                    // carries a fixed positional descriptor and never interpolates
                    // either the body or the evaluated value.
                    floor_layer_one_contribution(&format!("path expression #{index}"), &rendered)?;
                    result.push_str(&rendered);
                },
            }
        }
        Ok(result)
    }

    /// Evaluate an expression to a JSON value.
    /// Delegates to the shared evaluation module.
    fn evaluate(&self, expr: &ValueExpr) -> Result<JsonValue, ExecutionError> {
        shared_evaluate(expr, &self.variables)
    }
}

// ============================================================================
// LEGACY COMPATIBILITY - Types for backward compatibility
// ============================================================================

/// Legacy JsExecutor type alias for backward compatibility.
pub type JsExecutor = PlanCompiler;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execution_config_default() {
        let config = ExecutionConfig::default();
        assert_eq!(config.max_api_calls, 50);
        assert_eq!(config.timeout_seconds, 30);
        assert_eq!(config.max_loop_iterations, 100);
    }

    #[test]
    fn test_path_template_static() {
        let path = PathTemplate::static_path("/users".into());
        assert!(!path.is_dynamic());
    }

    #[test]
    fn test_path_template_dynamic() {
        let path = PathTemplate {
            parts: vec![
                PathPart::Literal("/users/".into()),
                PathPart::Variable("id".into()),
            ],
        };
        assert!(path.is_dynamic());
    }

    #[test]
    fn test_plan_metadata() {
        let metadata = PlanMetadata {
            api_call_count: 2,
            has_mutations: false,
            endpoints: vec!["/users".into(), "/products".into()],
            methods_used: vec!["GET".into()],
        };
        assert_eq!(metadata.api_call_count, 2);
        assert!(!metadata.has_mutations);
    }

    #[test]
    fn test_compile_simple_api_call() {
        let code = r#"
            const user = await api.get('/users/1');
            return user;
        "#;

        let mut compiler = PlanCompiler::new();
        let plan = compiler.compile_code(code).expect("Should compile");

        assert_eq!(plan.metadata.api_call_count, 1);
        assert!(!plan.metadata.has_mutations);
        assert_eq!(plan.steps.len(), 2); // ApiCall + Return
    }

    #[test]
    fn test_compile_multiple_api_calls() {
        let code = r#"
            const users = await api.get('/users');
            const products = await api.get('/products');
            return { users, products };
        "#;

        let mut compiler = PlanCompiler::new();
        let plan = compiler.compile_code(code).expect("Should compile");

        assert_eq!(plan.metadata.api_call_count, 2);
        assert!(!plan.metadata.has_mutations);
    }

    #[test]
    fn test_compile_mutation() {
        let code = r#"
            const result = await api.post('/users', { name: 'Test' });
            return result;
        "#;

        let mut compiler = PlanCompiler::new();
        let plan = compiler.compile_code(code).expect("Should compile");

        assert_eq!(plan.metadata.api_call_count, 1);
        assert!(plan.metadata.has_mutations);
    }

    #[test]
    fn test_compile_dynamic_path() {
        let code = r#"
            const id = 123;
            const user = await api.get(`/users/${id}`);
            return user;
        "#;

        let mut compiler = PlanCompiler::new();
        let plan = compiler.compile_code(code).expect("Should compile");

        assert_eq!(plan.metadata.api_call_count, 1);
    }

    #[test]
    fn test_compile_bounded_loop() {
        let code = r#"
            const items = [];
            const users = [{ id: 1 }, { id: 2 }, { id: 3 }];
            for (const user of users.slice(0, 2)) {
                const detail = await api.get(`/users/${user.id}`);
                items.push(detail);
            }
            return items;
        "#;

        let mut compiler = PlanCompiler::new();
        let plan = compiler.compile_code(code).expect("Should compile");

        // The loop is bounded, so it should compile
        assert!(plan
            .steps
            .iter()
            .any(|s| matches!(s, PlanStep::BoundedLoop { .. })));
    }

    #[test]
    fn test_compile_unbounded_loop_detection() {
        // Note: The current compiler allows for-of loops without explicit .slice() bounds
        // as long as the loop body doesn't exceed iteration limits at runtime.
        // This test documents the current behavior.
        let code = r#"
            const users = [{ id: 1 }, { id: 2 }, { id: 3 }];
            for (const user of users) {
                const detail = await api.get(`/users/${user.id}`);
            }
            return users;
        "#;

        let mut compiler = PlanCompiler::new();
        let result = compiler.compile_code(code);

        // Currently this compiles - runtime will enforce iteration limits
        // See #249 — investigate compile-time loop-bounds checking.
        assert!(result.is_ok(), "Loop compiled: {:?}", result);
    }

    #[test]
    fn test_compile_conditional() {
        let code = r#"
            const user = await api.get('/users/1');
            if (user.active) {
                const orders = await api.get(`/users/${user.id}/orders`);
                return orders;
            } else {
                return [];
            }
        "#;

        let mut compiler = PlanCompiler::new();
        let plan = compiler.compile_code(code).expect("Should compile");

        assert!(plan
            .steps
            .iter()
            .any(|s| matches!(s, PlanStep::Conditional { .. })));
    }

    // Mock HTTP executor for testing
    struct MockHttpExecutor {
        responses: std::collections::HashMap<String, JsonValue>,
    }

    impl MockHttpExecutor {
        fn new() -> Self {
            Self {
                responses: std::collections::HashMap::new(),
            }
        }

        fn add_response(&mut self, path: &str, response: JsonValue) {
            self.responses.insert(path.to_string(), response);
        }
    }

    #[async_trait::async_trait]
    impl HttpExecutor for MockHttpExecutor {
        async fn execute_request(
            &self,
            _method: &str,
            path: ResolvedPath<'_>,
            _body: Option<JsonValue>,
        ) -> Result<JsonValue, ExecutionError> {
            let path = path.as_str();
            self.responses
                .get(path)
                .cloned()
                .ok_or_else(|| ExecutionError::RuntimeError {
                    message: format!("No mock response for path: {}", path),
                })
        }
    }

    #[tokio::test]
    async fn test_execute_simple_api_call() {
        let code = r#"
            const user = await api.get('/users/1');
            return user;
        "#;

        let mut compiler = PlanCompiler::new();
        let plan = compiler.compile_code(code).expect("Should compile");

        let mut mock_http = MockHttpExecutor::new();
        mock_http.add_response("/users/1", serde_json::json!({ "id": 1, "name": "Alice" }));

        let mut executor = PlanExecutor::new(mock_http, ExecutionConfig::default());
        let result = executor.execute(&plan).await.expect("Should execute");

        assert_eq!(result.value["id"], 1);
        assert_eq!(result.value["name"], "Alice");
        assert_eq!(result.api_calls.len(), 1);
    }

    #[tokio::test]
    async fn test_execute_multiple_api_calls() {
        let code = r#"
            const users = await api.get('/users');
            const products = await api.get('/products');
            return { users, products };
        "#;

        let mut compiler = PlanCompiler::new();
        let plan = compiler.compile_code(code).expect("Should compile");

        let mut mock_http = MockHttpExecutor::new();
        mock_http.add_response("/users", serde_json::json!([{ "id": 1, "name": "Alice" }]));
        mock_http.add_response(
            "/products",
            serde_json::json!([{ "id": 100, "name": "Widget" }]),
        );

        let mut executor = PlanExecutor::new(mock_http, ExecutionConfig::default());
        let result = executor.execute(&plan).await.expect("Should execute");

        assert!(result.value["users"].is_array());
        assert!(result.value["products"].is_array());
        assert_eq!(result.api_calls.len(), 2);
    }

    #[tokio::test]
    async fn test_execute_with_template_path() {
        let code = r#"
            const userId = 42;
            const user = await api.get(`/users/${userId}`);
            return user;
        "#;

        let mut compiler = PlanCompiler::new();
        let plan = compiler.compile_code(code).expect("Should compile");

        let mut mock_http = MockHttpExecutor::new();
        mock_http.add_response("/users/42", serde_json::json!({ "id": 42, "name": "Bob" }));

        let mut executor = PlanExecutor::new(mock_http, ExecutionConfig::default());
        let result = executor.execute(&plan).await.expect("Should execute");

        assert_eq!(result.value["id"], 42);
        assert_eq!(result.value["name"], "Bob");
    }

    #[tokio::test]
    async fn test_execute_conditional_true_branch() {
        let code = r#"
            const user = await api.get('/users/1');
            if (user.active) {
                return { status: "active", user: user };
            } else {
                return { status: "inactive" };
            }
        "#;

        let mut compiler = PlanCompiler::new();
        let plan = compiler.compile_code(code).expect("Should compile");

        let mut mock_http = MockHttpExecutor::new();
        mock_http.add_response("/users/1", serde_json::json!({ "id": 1, "active": true }));

        let mut executor = PlanExecutor::new(mock_http, ExecutionConfig::default());
        let result = executor.execute(&plan).await.expect("Should execute");

        assert_eq!(result.value["status"], "active");
    }

    #[tokio::test]
    async fn test_execute_conditional_false_branch() {
        let code = r#"
            const user = await api.get('/users/1');
            if (user.active) {
                return { status: "active" };
            } else {
                return { status: "inactive", user: user };
            }
        "#;

        let mut compiler = PlanCompiler::new();
        let plan = compiler.compile_code(code).expect("Should compile");

        let mut mock_http = MockHttpExecutor::new();
        mock_http.add_response("/users/1", serde_json::json!({ "id": 1, "active": false }));

        let mut executor = PlanExecutor::new(mock_http, ExecutionConfig::default());
        let result = executor.execute(&plan).await.expect("Should execute");

        assert_eq!(result.value["status"], "inactive");
    }

    #[tokio::test]
    async fn test_compile_and_execute_reduce() {
        let code = r#"
            const products = await api.get('/products');
            const totalPrice = products.reduce((sum, p) => sum + p.price, 0);
            return { total: totalPrice };
        "#;

        let mut compiler = PlanCompiler::new();
        let plan = compiler.compile_code(code).expect("Should compile reduce");

        let mut mock_http = MockHttpExecutor::new();
        mock_http.add_response(
            "/products",
            serde_json::json!([
                { "id": 1, "name": "Widget", "price": 10 },
                { "id": 2, "name": "Gadget", "price": 25 },
                { "id": 3, "name": "Gizmo", "price": 15 }
            ]),
        );

        let mut executor = PlanExecutor::new(mock_http, ExecutionConfig::default());
        let result = executor.execute(&plan).await.expect("Should execute");

        // Result is f64, compare as number
        assert_eq!(result.value["total"].as_f64().unwrap(), 50.0);
    }

    #[tokio::test]
    async fn test_compile_and_execute_to_fixed() {
        let code = r#"
            const products = await api.get('/products');
            const totalPrice = products.reduce((sum, p) => sum + p.price, 0);
            const averagePrice = products.length > 0 ? totalPrice / products.length : 0;
            return { averagePrice: averagePrice.toFixed(2) };
        "#;

        let mut compiler = PlanCompiler::new();
        let plan = compiler.compile_code(code).expect("Should compile toFixed");

        let mut mock_http = MockHttpExecutor::new();
        mock_http.add_response(
            "/products",
            serde_json::json!([
                { "id": 1, "name": "Widget", "price": 10 },
                { "id": 2, "name": "Gadget", "price": 25 },
                { "id": 3, "name": "Gizmo", "price": 15 }
            ]),
        );

        let mut executor = PlanExecutor::new(mock_http, ExecutionConfig::default());
        let result = executor.execute(&plan).await.expect("Should execute");

        // 50 / 3 = 16.666... toFixed(2) = "16.67"
        assert_eq!(result.value["averagePrice"], "16.67");
    }

    // =========================================================================
    // Field Filtering Tests
    // =========================================================================

    #[test]
    fn test_filter_blocked_fields_simple() {
        let value = serde_json::json!({
            "id": 1,
            "name": "Alice",
            "password": "secret123",
            "email": "alice@example.com"
        });

        let blocked: HashSet<String> = ["password"].iter().map(|s| s.to_string()).collect();
        let filtered = filter_blocked_fields(value, &blocked);

        assert_eq!(filtered["id"], 1);
        assert_eq!(filtered["name"], "Alice");
        assert_eq!(filtered["email"], "alice@example.com");
        assert!(filtered.get("password").is_none());
    }

    #[test]
    fn test_filter_blocked_fields_multiple() {
        let value = serde_json::json!({
            "id": 1,
            "name": "Alice",
            "password": "secret123",
            "ssn": "123-45-6789",
            "apiKey": "key-abc123"
        });

        let blocked: HashSet<String> = ["password", "ssn", "apiKey"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let filtered = filter_blocked_fields(value, &blocked);

        assert_eq!(filtered["id"], 1);
        assert_eq!(filtered["name"], "Alice");
        assert!(filtered.get("password").is_none());
        assert!(filtered.get("ssn").is_none());
        assert!(filtered.get("apiKey").is_none());
    }

    #[test]
    fn test_filter_blocked_fields_nested() {
        let value = serde_json::json!({
            "user": {
                "id": 1,
                "profile": {
                    "name": "Alice",
                    "password": "secret123"
                }
            }
        });

        let blocked: HashSet<String> = ["password"].iter().map(|s| s.to_string()).collect();
        let filtered = filter_blocked_fields(value, &blocked);

        assert_eq!(filtered["user"]["id"], 1);
        assert_eq!(filtered["user"]["profile"]["name"], "Alice");
        assert!(filtered["user"]["profile"].get("password").is_none());
    }

    #[test]
    fn test_filter_blocked_fields_in_array() {
        let value = serde_json::json!([
            { "id": 1, "name": "Alice", "password": "secret1" },
            { "id": 2, "name": "Bob", "password": "secret2" }
        ]);

        let blocked: HashSet<String> = ["password"].iter().map(|s| s.to_string()).collect();
        let filtered = filter_blocked_fields(value, &blocked);

        let arr = filtered.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["id"], 1);
        assert_eq!(arr[0]["name"], "Alice");
        assert!(arr[0].get("password").is_none());
        assert_eq!(arr[1]["id"], 2);
        assert_eq!(arr[1]["name"], "Bob");
        assert!(arr[1].get("password").is_none());
    }

    #[test]
    fn test_filter_blocked_fields_empty_blocklist() {
        let value = serde_json::json!({
            "id": 1,
            "password": "secret123"
        });

        let blocked: HashSet<String> = HashSet::new();
        let filtered = filter_blocked_fields(value.clone(), &blocked);

        // Should be unchanged
        assert_eq!(filtered, value);
    }

    #[test]
    fn test_filter_blocked_fields_primitive_values() {
        // Primitives should pass through unchanged
        let blocked: HashSet<String> = ["password"].iter().map(|s| s.to_string()).collect();

        assert_eq!(
            filter_blocked_fields(JsonValue::String("test".into()), &blocked),
            JsonValue::String("test".into())
        );
        assert_eq!(
            filter_blocked_fields(JsonValue::Number(42.into()), &blocked),
            JsonValue::Number(42.into())
        );
        assert_eq!(
            filter_blocked_fields(JsonValue::Bool(true), &blocked),
            JsonValue::Bool(true)
        );
        assert_eq!(
            filter_blocked_fields(JsonValue::Null, &blocked),
            JsonValue::Null
        );
    }

    #[tokio::test]
    async fn test_execute_with_blocked_fields() {
        let code = r#"
            const user = await api.get('/users/1');
            return user;
        "#;

        let mut compiler = PlanCompiler::new();
        let plan = compiler.compile_code(code).expect("Should compile");

        let mut mock_http = MockHttpExecutor::new();
        mock_http.add_response(
            "/users/1",
            serde_json::json!({
                "id": 1,
                "name": "Alice",
                "password": "secret123",
                "apiKey": "key-abc"
            }),
        );

        // Create config with blocked fields
        let config = ExecutionConfig::default().with_blocked_fields(["password", "apiKey"]);

        let mut executor = PlanExecutor::new(mock_http, config);
        let result = executor.execute(&plan).await.expect("Should execute");

        // Blocked fields should be filtered out
        assert_eq!(result.value["id"], 1);
        assert_eq!(result.value["name"], "Alice");
        assert!(result.value.get("password").is_none());
        assert!(result.value.get("apiKey").is_none());
    }

    #[tokio::test]
    async fn test_execute_nested_blocked_fields() {
        let code = r#"
            const data = await api.get('/data');
            return data;
        "#;

        let mut compiler = PlanCompiler::new();
        let plan = compiler.compile_code(code).expect("Should compile");

        let mut mock_http = MockHttpExecutor::new();
        mock_http.add_response(
            "/data",
            serde_json::json!({
                "users": [
                    { "id": 1, "name": "Alice", "secret": "hidden1" },
                    { "id": 2, "name": "Bob", "secret": "hidden2" }
                ],
                "config": {
                    "setting": "value",
                    "secret": "also-hidden"
                }
            }),
        );

        // Create config with blocked fields
        let config = ExecutionConfig::default().with_blocked_fields(["secret"]);

        let mut executor = PlanExecutor::new(mock_http, config);
        let result = executor.execute(&plan).await.expect("Should execute");

        // Secret should be filtered from all nested locations
        let users = result.value["users"].as_array().unwrap();
        assert_eq!(users[0]["name"], "Alice");
        assert!(users[0].get("secret").is_none());
        assert_eq!(users[1]["name"], "Bob");
        assert!(users[1].get("secret").is_none());

        assert_eq!(result.value["config"]["setting"], "value");
        assert!(result.value["config"].get("secret").is_none());
    }

    // =========================================================================
    // Output Validation Tests
    // =========================================================================

    #[test]
    fn test_find_blocked_fields_in_output_simple() {
        let value = serde_json::json!({
            "id": 1,
            "name": "Alice",
            "ssn": "123-45-6789"
        });

        let blocked: HashSet<String> = ["ssn"].iter().map(|s| s.to_string()).collect();
        let violations = find_blocked_fields_in_output(&value, &blocked);

        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0], "ssn");
    }

    #[test]
    fn test_find_blocked_fields_in_output_nested() {
        let value = serde_json::json!({
            "user": {
                "profile": {
                    "name": "Alice",
                    "salary": 100000
                }
            }
        });

        let blocked: HashSet<String> = ["salary"].iter().map(|s| s.to_string()).collect();
        let violations = find_blocked_fields_in_output(&value, &blocked);

        assert_eq!(violations.len(), 1);
        assert!(violations[0].contains("salary"));
    }

    #[test]
    fn test_find_blocked_fields_in_output_array() {
        let value = serde_json::json!([
            { "id": 1, "ssn": "111" },
            { "id": 2, "ssn": "222" }
        ]);

        let blocked: HashSet<String> = ["ssn"].iter().map(|s| s.to_string()).collect();
        let violations = find_blocked_fields_in_output(&value, &blocked);

        // Should find ssn in both array elements
        assert_eq!(violations.len(), 2);
    }

    #[test]
    fn test_find_blocked_fields_in_output_empty_blocklist() {
        let value = serde_json::json!({
            "id": 1,
            "ssn": "123-45-6789"
        });

        let blocked: HashSet<String> = HashSet::new();
        let violations = find_blocked_fields_in_output(&value, &blocked);

        assert!(violations.is_empty());
    }

    #[test]
    fn test_find_blocked_fields_in_output_no_violations() {
        let value = serde_json::json!({
            "id": 1,
            "name": "Alice"
        });

        let blocked: HashSet<String> = ["ssn", "salary"].iter().map(|s| s.to_string()).collect();
        let violations = find_blocked_fields_in_output(&value, &blocked);

        assert!(violations.is_empty());
    }

    #[tokio::test]
    async fn test_execute_output_blocked_fields_rejected() {
        let code = r#"
            const user = await api.get('/users/1');
            return { name: user.name, ssn: user.ssn };
        "#;

        let mut compiler = PlanCompiler::new();
        let plan = compiler.compile_code(code).expect("Should compile");

        let mut mock_http = MockHttpExecutor::new();
        mock_http.add_response(
            "/users/1",
            serde_json::json!({
                "id": 1,
                "name": "Alice",
                "ssn": "123-45-6789"
            }),
        );

        // Note: internal blocklist is empty, so ssn gets through to the script
        // But output blocklist should catch it in the return value
        let config = ExecutionConfig::default().with_output_blocked_fields(["ssn"]);

        let mut executor = PlanExecutor::new(mock_http, config);
        let result = executor.execute(&plan).await;

        // Should fail because output contains blocked field
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(format!("{:?}", err).contains("ssn"));
    }

    #[tokio::test]
    async fn test_execute_output_blocked_fields_internal_use_allowed() {
        // Script reads user data but only returns safe fields - should succeed
        let code = r#"
            const user = await api.get('/users/1');
            return { id: user.id, name: user.name };
        "#;

        let mut compiler = PlanCompiler::new();
        let plan = compiler.compile_code(code).expect("Should compile");

        let mut mock_http = MockHttpExecutor::new();
        mock_http.add_response(
            "/users/1",
            serde_json::json!({
                "id": 1,
                "name": "Alice",
                "ssn": "123-45-6789"
            }),
        );

        // Output blocklist - ssn can be read but not returned
        // Note: This doesn't prevent script from accessing ssn, just returning it
        let config = ExecutionConfig::default().with_output_blocked_fields(["ssn"]);

        let mut executor = PlanExecutor::new(mock_http, config);
        let result = executor.execute(&plan).await.expect("Should succeed");

        // Script read user data but only returned safe fields
        assert_eq!(result.value["id"], 1);
        assert_eq!(result.value["name"], "Alice");
        assert!(result.value.get("ssn").is_none());
    }

    #[tokio::test]
    async fn test_execute_both_blocklists() {
        // Test that internal blocklist AND output blocklist work together
        let code = r#"
            const user = await api.get('/users/1');
            return { name: user.name, dateOfBirth: user.dateOfBirth };
        "#;

        let mut compiler = PlanCompiler::new();
        let plan = compiler.compile_code(code).expect("Should compile");

        let mut mock_http = MockHttpExecutor::new();
        mock_http.add_response(
            "/users/1",
            serde_json::json!({
                "id": 1,
                "name": "Alice",
                "password": "secret123",
                "dateOfBirth": "1990-01-01"
            }),
        );

        // Internal blocklist: password is stripped from API response
        // Output blocklist: dateOfBirth can be used but not returned
        let config = ExecutionConfig::default()
            .with_blocked_fields(["password"])
            .with_output_blocked_fields(["dateOfBirth"]);

        let mut executor = PlanExecutor::new(mock_http, config);
        let result = executor.execute(&plan).await;

        // Should fail because output contains dateOfBirth
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(format!("{:?}", err).contains("dateOfBirth"));
    }

    // ========================================================================
    // Tests for pre-bound variables (args) and conditionals
    // ========================================================================

    #[tokio::test]
    async fn test_prebound_args_comparison() {
        // Test that `if (args.k > args.n)` works with pre-bound args
        let code = r#"
            if (args.k > args.n) {
                return { error: 'k must be <= n' };
            }
            return { ok: true };
        "#;

        let mut compiler = PlanCompiler::new();
        let plan = compiler.compile_code(code).expect("Should compile");

        let mock_http = MockHttpExecutor::new();
        let mut executor = PlanExecutor::new(mock_http, ExecutionConfig::default());
        executor.set_variable("args", serde_json::json!({"n": 3, "k": 5}));

        let result = executor.execute(&plan).await.expect("Should execute");
        assert_eq!(
            result.value["error"], "k must be <= n",
            "Expected error for k > n, got: {:?}",
            result.value
        );
    }

    #[tokio::test]
    async fn test_prebound_args_strict_equality() {
        // Test that `args.k === 0` works
        let code = r#"
            if (args.k === 0) {
                return { result: 1 };
            }
            return { result: 'not zero' };
        "#;

        let mut compiler = PlanCompiler::new();
        let plan = compiler.compile_code(code).expect("Should compile");

        let mock_http = MockHttpExecutor::new();
        let mut executor = PlanExecutor::new(mock_http, ExecutionConfig::default());
        executor.set_variable("args", serde_json::json!({"k": 0}));

        let result = executor.execute(&plan).await.expect("Should execute");
        assert_eq!(result.value["result"], 1);
    }

    #[tokio::test]
    async fn test_assignment_expression_in_statement() {
        // Test that `k = newValue` works as a statement (Expr::Assign)
        let code = r#"
            let k = 5;
            k = 2;
            return { k: k };
        "#;

        let mut compiler = PlanCompiler::new();
        let plan = compiler.compile_code(code).expect("Should compile");

        let mock_http = MockHttpExecutor::new();
        let mut executor = PlanExecutor::new(mock_http, ExecutionConfig::default());

        let result = executor.execute(&plan).await.expect("Should execute");
        assert_eq!(result.value["k"], 2);
    }

    #[tokio::test]
    async fn test_assignment_swap_variables() {
        // Test swapping two let-bound variables
        let code = r#"
            let a = 3;
            let b = 7;
            if (a < b) {
                const old_a = a;
                a = b;
                b = old_a;
            }
            return { a: a, b: b };
        "#;

        let mut compiler = PlanCompiler::new();
        let plan = compiler.compile_code(code).expect("Should compile");

        let mock_http = MockHttpExecutor::new();
        let mut executor = PlanExecutor::new(mock_http, ExecutionConfig::default());

        let result = executor.execute(&plan).await.expect("Should execute");
        assert_eq!(result.value["a"], 7);
        assert_eq!(result.value["b"], 3);
    }

    // ========================================================================
    // MCP call tests (require mcp-code-mode feature)
    // ========================================================================

    #[cfg(feature = "mcp-code-mode")]
    mod mcp_tests {
        use super::*;

        /// Mock MCP executor that simulates a calculator server.
        struct MockCalculatorExecutor;

        #[async_trait::async_trait]
        impl McpExecutor for MockCalculatorExecutor {
            async fn call_tool(
                &self,
                _server_id: &str,
                tool_name: &str,
                args: JsonValue,
            ) -> Result<JsonValue, ExecutionError> {
                match tool_name {
                    "add" => {
                        let a = args["a"].as_f64().unwrap_or(0.0);
                        let b = args["b"].as_f64().unwrap_or(0.0);
                        Ok(serde_json::json!({"result": a + b}))
                    },
                    "subtract" => {
                        let a = args["a"].as_f64().unwrap_or(0.0);
                        let b = args["b"].as_f64().unwrap_or(0.0);
                        Ok(serde_json::json!({"result": a - b}))
                    },
                    "multiply" => {
                        let a = args["a"].as_f64().unwrap_or(0.0);
                        let b = args["b"].as_f64().unwrap_or(0.0);
                        Ok(serde_json::json!({"result": a * b}))
                    },
                    "divide" => {
                        let a = args["a"].as_f64().unwrap_or(0.0);
                        let b = args["b"].as_f64().unwrap_or(1.0);
                        Ok(serde_json::json!({"result": a / b}))
                    },
                    "power" => {
                        let base = args["base"].as_f64().unwrap_or(0.0);
                        let exponent = args["exponent"].as_f64().unwrap_or(1.0);
                        Ok(serde_json::json!({"result": base.powf(exponent)}))
                    },
                    "sqrt" => {
                        let n = args["n"].as_f64().unwrap_or(0.0);
                        Ok(serde_json::json!({"result": n.sqrt()}))
                    },
                    _ => Err(ExecutionError::RuntimeError {
                        message: format!("Unknown tool: {}", tool_name),
                    }),
                }
            }
        }

        #[tokio::test]
        async fn test_mcp_call_simple() {
            let code = r#"
                const result = await mcp.call('calculator', 'add', { a: 5, b: 3 });
                return result;
            "#;

            let mut compiler = PlanCompiler::new();
            let plan = compiler.compile_code(code).expect("Should compile");

            let mock_http = MockHttpExecutor::new();
            let mut executor = PlanExecutor::new(mock_http, ExecutionConfig::default());
            executor.set_mcp_executor(MockCalculatorExecutor);

            let result = executor.execute(&plan).await.expect("Should execute");
            assert_eq!(result.value["result"], 8.0);
        }

        #[tokio::test]
        async fn test_mcp_call_with_args() {
            // Test mcp.call using pre-bound args variable
            let code = r#"
                const result = await mcp.call('calculator', 'add', { a: args.x, b: args.y });
                return { sum: result.result };
            "#;

            let mut compiler = PlanCompiler::new();
            let plan = compiler.compile_code(code).expect("Should compile");

            let mock_http = MockHttpExecutor::new();
            let mut executor = PlanExecutor::new(mock_http, ExecutionConfig::default());
            executor.set_mcp_executor(MockCalculatorExecutor);
            executor.set_variable("args", serde_json::json!({"x": 10, "y": 20}));

            let result = executor.execute(&plan).await.expect("Should execute");
            assert_eq!(result.value["sum"], 30.0);
        }

        #[tokio::test]
        async fn test_mcp_assignment_in_loop() {
            // Test `result = await mcp.call(...)` assignment inside a loop
            let code = r#"
                let result = { result: 1 };
                for (const i of [2, 3, 4, 5]) {
                    const mul = await mcp.call('calculator', 'multiply', { a: result.result, b: i });
                    result = mul;
                }
                return { factorial: result.result };
            "#;

            let mut compiler = PlanCompiler::new();
            let plan = compiler.compile_code(code).expect("Should compile");

            let mock_http = MockHttpExecutor::new();
            let mut executor = PlanExecutor::new(mock_http, ExecutionConfig::default());
            executor.set_mcp_executor(MockCalculatorExecutor);

            let result = executor.execute(&plan).await.expect("Should execute");
            // 1 * 2 * 3 * 4 * 5 = 120
            assert_eq!(result.value["factorial"], 120.0);
        }

        #[tokio::test]
        async fn test_combinations_c_5_3() {
            // Full combinations script: C(5,3) = 10
            let code = r#"
if (args.k > args.n) {
  return { error: 'k must be <= n', n: args.n, k: args.k };
}
if (args.k === 0 || args.k === args.n) {
  return { n: args.n, k: args.k, result: 1 };
}
let k = args.k;
const complement = await mcp.call('calculator', 'subtract', { a: args.n, b: args.k });
let nmk = complement.result;
if (nmk < k) {
  const old_k = k;
  k = nmk;
  nmk = old_k;
}
let result = { result: 1 };
for (const i of [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20]) {
  if (i > k) { break; }
  const nki = await mcp.call('calculator', 'add', { a: nmk, b: i });
  const num = await mcp.call('calculator', 'multiply', { a: result.result, b: nki.result });
  result = await mcp.call('calculator', 'divide', { a: num.result, b: i });
}
return { n: args.n, k: args.k, result: result.result };
            "#;

            let mut compiler = PlanCompiler::new();
            let plan = compiler.compile_code(code).expect("Should compile");

            let mock_http = MockHttpExecutor::new();
            let mut executor = PlanExecutor::new(mock_http, ExecutionConfig::default());
            executor.set_mcp_executor(MockCalculatorExecutor);
            executor.set_variable("args", serde_json::json!({"n": 5, "k": 3}));

            let result = executor.execute(&plan).await.expect("Should execute");
            assert_eq!(
                result.value["result"], 10.0,
                "C(5,3) should be 10, got: {:?}",
                result.value
            );
        }

        #[tokio::test]
        async fn test_combinations_k_greater_than_n() {
            // C(3,5) should return error
            let code = r#"
if (args.k > args.n) {
  return { error: 'k must be <= n', n: args.n, k: args.k };
}
return { result: 'should not reach here' };
            "#;

            let mut compiler = PlanCompiler::new();
            let plan = compiler.compile_code(code).expect("Should compile");

            let mock_http = MockHttpExecutor::new();
            let mut executor = PlanExecutor::new(mock_http, ExecutionConfig::default());
            executor.set_mcp_executor(MockCalculatorExecutor);
            executor.set_variable("args", serde_json::json!({"n": 3, "k": 5}));

            let result = executor.execute(&plan).await.expect("Should execute");
            assert_eq!(
                result.value["error"], "k must be <= n",
                "C(3,5) should return error, got: {:?}",
                result.value
            );
        }

        #[tokio::test]
        async fn test_combinations_c_5_2() {
            // C(5,2) = 10 — no swap needed
            let code = r#"
if (args.k > args.n) {
  return { error: 'k must be <= n', n: args.n, k: args.k };
}
if (args.k === 0 || args.k === args.n) {
  return { n: args.n, k: args.k, result: 1 };
}
let k = args.k;
const complement = await mcp.call('calculator', 'subtract', { a: args.n, b: args.k });
let nmk = complement.result;
if (nmk < k) {
  const old_k = k;
  k = nmk;
  nmk = old_k;
}
let result = { result: 1 };
for (const i of [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20]) {
  if (i > k) { break; }
  const nki = await mcp.call('calculator', 'add', { a: nmk, b: i });
  const num = await mcp.call('calculator', 'multiply', { a: result.result, b: nki.result });
  result = await mcp.call('calculator', 'divide', { a: num.result, b: i });
}
return { n: args.n, k: args.k, result: result.result };
            "#;

            let mut compiler = PlanCompiler::new();
            let plan = compiler.compile_code(code).expect("Should compile");

            let mock_http = MockHttpExecutor::new();
            let mut executor = PlanExecutor::new(mock_http, ExecutionConfig::default());
            executor.set_mcp_executor(MockCalculatorExecutor);
            executor.set_variable("args", serde_json::json!({"n": 5, "k": 2}));

            let result = executor.execute(&plan).await.expect("Should execute");
            assert_eq!(
                result.value["result"], 10.0,
                "C(5,2) should be 10, got: {:?}",
                result.value
            );
        }

        #[tokio::test]
        async fn test_combinations_edge_cases() {
            let code = r#"
if (args.k === 0 || args.k === args.n) {
  return { result: 1 };
}
return { result: 'not edge case' };
            "#;

            let mut compiler = PlanCompiler::new();
            let plan = compiler.compile_code(code).expect("Should compile");

            // Test k=0
            let mock_http = MockHttpExecutor::new();
            let mut executor = PlanExecutor::new(mock_http, ExecutionConfig::default());
            executor.set_variable("args", serde_json::json!({"n": 5, "k": 0}));
            let result = executor.execute(&plan).await.expect("Should execute");
            assert_eq!(result.value["result"], 1, "C(5,0) should be 1");

            // Test k=n
            let mock_http = MockHttpExecutor::new();
            let mut executor = PlanExecutor::new(mock_http, ExecutionConfig::default());
            executor.set_variable("args", serde_json::json!({"n": 5, "k": 5}));
            let result = executor.execute(&plan).await.expect("Should execute");
            assert_eq!(result.value["result"], 1, "C(5,5) should be 1");
        }

        #[tokio::test]
        async fn test_solve_quadratic() {
            // x² - 3x + 2 = 0 → roots [2, 1]
            let code = r#"
const b_sq = await mcp.call('calculator', 'power', { base: args.b, exponent: 2 });
const four_a = await mcp.call('calculator', 'multiply', { a: 4, b: args.a });
const four_ac = await mcp.call('calculator', 'multiply', { a: four_a.result, b: args.c });
const discriminant = await mcp.call('calculator', 'subtract', { a: b_sq.result, b: four_ac.result });
const root_type = discriminant.result > 0 ? 'two_real'
  : discriminant.result === 0 ? 'one_real' : 'complex';
if (discriminant.result < 0) {
  return { discriminant: discriminant.result, root_type: root_type, roots: [] };
}
const sqrt_disc = await mcp.call('calculator', 'sqrt', { n: discriminant.result });
const neg_b = await mcp.call('calculator', 'multiply', { a: -1, b: args.b });
const two_a = await mcp.call('calculator', 'multiply', { a: 2, b: args.a });
const x1_num = await mcp.call('calculator', 'add', { a: neg_b.result, b: sqrt_disc.result });
const x2_num = await mcp.call('calculator', 'subtract', { a: neg_b.result, b: sqrt_disc.result });
const x1 = await mcp.call('calculator', 'divide', { a: x1_num.result, b: two_a.result });
const x2 = await mcp.call('calculator', 'divide', { a: x2_num.result, b: two_a.result });
return { discriminant: discriminant.result, root_type: root_type, roots: [x1.result, x2.result] };
            "#;

            let mut compiler = PlanCompiler::new();
            let plan = compiler.compile_code(code).expect("Should compile");

            let mock_http = MockHttpExecutor::new();
            let mut executor = PlanExecutor::new(mock_http, ExecutionConfig::default());
            executor.set_mcp_executor(MockCalculatorExecutor);
            executor.set_variable("args", serde_json::json!({"a": 1, "b": -3, "c": 2}));

            let result = executor.execute(&plan).await.expect("Should execute");
            assert_eq!(result.value["root_type"], "two_real");
            assert_eq!(result.value["discriminant"], 1.0);
            let roots = result.value["roots"]
                .as_array()
                .expect("roots should be array");
            assert_eq!(roots.len(), 2);
            assert_eq!(roots[0], 2.0);
            assert_eq!(roots[1], 1.0);
        }
    }

    // =========================================================================
    // String method integration tests (compile + execute)
    // =========================================================================

    #[tokio::test]
    async fn test_string_includes() {
        let code = r#"
            const text = "hello world";
            return { found: text.includes("world"), miss: text.includes("xyz") };
        "#;

        let mut compiler = PlanCompiler::new();
        let plan = compiler.compile_code(code).expect("Should compile");

        let mock_http = MockHttpExecutor::new();
        let mut executor = PlanExecutor::new(mock_http, ExecutionConfig::default());
        let result = executor.execute(&plan).await.expect("Should execute");

        assert_eq!(result.value["found"], true);
        assert_eq!(result.value["miss"], false);
    }

    #[tokio::test]
    async fn test_string_index_of() {
        let code = r#"
            const text = "abcdef";
            return { idx: text.indexOf("cd"), miss: text.indexOf("xyz") };
        "#;

        let mut compiler = PlanCompiler::new();
        let plan = compiler.compile_code(code).expect("Should compile");

        let mock_http = MockHttpExecutor::new();
        let mut executor = PlanExecutor::new(mock_http, ExecutionConfig::default());
        let result = executor.execute(&plan).await.expect("Should execute");

        assert_eq!(result.value["idx"], 2);
        assert_eq!(result.value["miss"], -1);
    }

    #[tokio::test]
    async fn test_string_length() {
        let code = r#"
            const text = "hello";
            return { len: text.length };
        "#;

        let mut compiler = PlanCompiler::new();
        let plan = compiler.compile_code(code).expect("Should compile");

        let mock_http = MockHttpExecutor::new();
        let mut executor = PlanExecutor::new(mock_http, ExecutionConfig::default());
        let result = executor.execute(&plan).await.expect("Should execute");

        assert_eq!(result.value["len"], 5);
    }

    #[tokio::test]
    async fn test_string_slice() {
        let code = r#"
            const text = "hello world";
            return { first: text.slice(0, 5), rest: text.slice(6, 11) };
        "#;

        let mut compiler = PlanCompiler::new();
        let plan = compiler.compile_code(code).expect("Should compile");

        let mock_http = MockHttpExecutor::new();
        let mut executor = PlanExecutor::new(mock_http, ExecutionConfig::default());
        let result = executor.execute(&plan).await.expect("Should execute");

        assert_eq!(result.value["first"], "hello");
        assert_eq!(result.value["rest"], "world");
    }

    #[tokio::test]
    async fn test_string_concat() {
        let code = r#"
            const greeting = "hello";
            return { result: greeting.concat(" world") };
        "#;

        let mut compiler = PlanCompiler::new();
        let plan = compiler.compile_code(code).expect("Should compile");

        let mock_http = MockHttpExecutor::new();
        let mut executor = PlanExecutor::new(mock_http, ExecutionConfig::default());
        let result = executor.execute(&plan).await.expect("Should execute");

        assert_eq!(result.value["result"], "hello world");
    }

    #[tokio::test]
    async fn test_string_includes_in_filter() {
        // Real-world pattern: filter array items by string content
        let code = r#"
            const items = [
                { name: "TIMESTAMP_2024", desc: "A timestamped record" },
                { name: "PERSON_1", desc: "A person entity" },
                { name: "TIMESTAMP_2025", desc: "Another timestamped record" }
            ];
            const timestamped = items.filter(item => item.name.includes("TIMESTAMP"));
            return { count: timestamped.length, names: timestamped.map(t => t.name) };
        "#;

        let mut compiler = PlanCompiler::new();
        let plan = compiler.compile_code(code).expect("Should compile");

        let mock_http = MockHttpExecutor::new();
        let mut executor = PlanExecutor::new(mock_http, ExecutionConfig::default());
        let result = executor.execute(&plan).await.expect("Should execute");

        assert_eq!(result.value["count"], 2);
        let names = result.value["names"].as_array().unwrap();
        assert_eq!(names[0], "TIMESTAMP_2024");
        assert_eq!(names[1], "TIMESTAMP_2025");
    }

    #[tokio::test]
    async fn test_array_includes_still_works() {
        // Regression: array .includes() must still work
        let code = r#"
            const ids = ["alice", "bob", "charlie"];
            return { has_bob: ids.includes("bob"), has_dave: ids.includes("dave") };
        "#;

        let mut compiler = PlanCompiler::new();
        let plan = compiler.compile_code(code).expect("Should compile");

        let mock_http = MockHttpExecutor::new();
        let mut executor = PlanExecutor::new(mock_http, ExecutionConfig::default());
        let result = executor.execute(&plan).await.expect("Should execute");

        assert_eq!(result.value["has_bob"], true);
        assert_eq!(result.value["has_dave"], false);
    }

    // =========================================================================
    // Built-in function compilation tests
    // =========================================================================

    #[test]
    fn test_compile_parse_float() {
        let code = r#"
            const x = parseFloat("3.14");
            return x;
        "#;
        let mut compiler = PlanCompiler::new();
        let plan = compiler
            .compile_code(code)
            .expect("parseFloat should compile");
        assert_eq!(plan.steps.len(), 2); // Assign + Return
    }

    #[test]
    fn test_compile_parse_int() {
        let code = r#"
            const x = parseInt("42");
            return x;
        "#;
        let mut compiler = PlanCompiler::new();
        compiler
            .compile_code(code)
            .expect("parseInt should compile");
    }

    #[test]
    fn test_compile_math_abs() {
        let code = r#"
            const x = Math.abs(-5);
            return x;
        "#;
        let mut compiler = PlanCompiler::new();
        compiler
            .compile_code(code)
            .expect("Math.abs should compile");
    }

    #[test]
    fn test_compile_math_max() {
        let code = r#"
            const x = Math.max(1, 2, 3);
            return x;
        "#;
        let mut compiler = PlanCompiler::new();
        compiler
            .compile_code(code)
            .expect("Math.max should compile");
    }

    #[test]
    fn test_compile_object_keys() {
        let code = r#"
            const obj = { a: 1, b: 2 };
            const keys = Object.keys(obj);
            return keys;
        "#;
        let mut compiler = PlanCompiler::new();
        compiler
            .compile_code(code)
            .expect("Object.keys should compile");
    }

    #[test]
    fn test_compile_object_entries() {
        let code = r#"
            const obj = { x: 10 };
            const entries = Object.entries(obj);
            return entries;
        "#;
        let mut compiler = PlanCompiler::new();
        compiler
            .compile_code(code)
            .expect("Object.entries should compile");
    }

    #[test]
    fn test_compile_unary_plus() {
        let code = r#"
            const x = +"42";
            return x;
        "#;
        let mut compiler = PlanCompiler::new();
        compiler.compile_code(code).expect("unary + should compile");
    }

    #[test]
    fn test_compile_sort_with_comparator() {
        let code = r#"
            const arr = [3, 1, 2];
            const sorted = arr.sort((a, b) => a - b);
            return sorted;
        "#;
        let mut compiler = PlanCompiler::new();
        compiler
            .compile_code(code)
            .expect("sort with comparator should compile");
    }

    #[test]
    fn test_compile_sort_without_comparator() {
        let code = r#"
            const arr = ["b", "a", "c"];
            const sorted = arr.sort();
            return sorted;
        "#;
        let mut compiler = PlanCompiler::new();
        compiler
            .compile_code(code)
            .expect("sort without comparator should compile");
    }

    // =========================================================================
    // End-to-end execution tests for new features
    // =========================================================================

    #[tokio::test]
    async fn test_execute_parse_float() {
        let code = r#"
            const x = parseFloat("3.14");
            return x;
        "#;
        let mut compiler = PlanCompiler::new();
        let plan = compiler.compile_code(code).unwrap();
        let mock_http = MockHttpExecutor::new();
        let mut executor = PlanExecutor::new(mock_http, ExecutionConfig::default());
        let result = executor.execute(&plan).await.unwrap();
        // Why: test fixture uses 3.14 as a representative non-integer parse target,
        // not the mathematical PI constant — clippy::approx_constant is a false positive here.
        #[allow(clippy::approx_constant)]
        let expected = serde_json::json!(3.14);
        assert_eq!(result.value, expected);
    }

    #[tokio::test]
    async fn test_execute_math_abs_and_sort() {
        let code = r#"
            const items = [
                { name: "a", val: -5 },
                { name: "b", val: 3 },
                { name: "c", val: -1 }
            ];
            const sorted = items.sort((a, b) => Math.abs(b.val) - Math.abs(a.val));
            return sorted.map(x => x.name);
        "#;
        let mut compiler = PlanCompiler::new();
        let plan = compiler.compile_code(code).unwrap();
        let mock_http = MockHttpExecutor::new();
        let mut executor = PlanExecutor::new(mock_http, ExecutionConfig::default());
        let result = executor.execute(&plan).await.unwrap();
        assert_eq!(result.value, serde_json::json!(["a", "b", "c"]));
    }

    #[tokio::test]
    async fn test_execute_object_keys() {
        let code = r#"
            const obj = { x: 1, y: 2, z: 3 };
            return Object.keys(obj).length;
        "#;
        let mut compiler = PlanCompiler::new();
        let plan = compiler.compile_code(code).unwrap();
        let mock_http = MockHttpExecutor::new();
        let mut executor = PlanExecutor::new(mock_http, ExecutionConfig::default());
        let result = executor.execute(&plan).await.unwrap();
        assert_eq!(result.value, serde_json::json!(3));
    }

    #[tokio::test]
    async fn test_execute_unary_plus() {
        let code = r#"
            const x = +"42";
            return x;
        "#;
        let mut compiler = PlanCompiler::new();
        let plan = compiler.compile_code(code).unwrap();
        let mock_http = MockHttpExecutor::new();
        let mut executor = PlanExecutor::new(mock_http, ExecutionConfig::default());
        let result = executor.execute(&plan).await.unwrap();
        assert_eq!(result.value, serde_json::json!(42.0));
    }

    #[tokio::test]
    async fn test_execute_number_cast() {
        let code = r#"
            const x = Number("99.5");
            return x;
        "#;
        let mut compiler = PlanCompiler::new();
        let plan = compiler.compile_code(code).unwrap();
        let mock_http = MockHttpExecutor::new();
        let mut executor = PlanExecutor::new(mock_http, ExecutionConfig::default());
        let result = executor.execute(&plan).await.unwrap();
        assert_eq!(result.value, serde_json::json!(99.5));
    }

    #[tokio::test]
    async fn test_execute_math_round_floor_ceil() {
        let code = r#"
            return {
                round: Math.round(3.7),
                floor: Math.floor(3.7),
                ceil: Math.ceil(3.2)
            };
        "#;
        let mut compiler = PlanCompiler::new();
        let plan = compiler.compile_code(code).unwrap();
        let mock_http = MockHttpExecutor::new();
        let mut executor = PlanExecutor::new(mock_http, ExecutionConfig::default());
        let result = executor.execute(&plan).await.unwrap();
        assert_eq!(result.value["round"], serde_json::json!(4.0));
        assert_eq!(result.value["floor"], serde_json::json!(3.0));
        assert_eq!(result.value["ceil"], serde_json::json!(4.0));
    }

    // =========================================================================
    // Object Spread Tests
    // =========================================================================

    #[test]
    fn test_compile_object_spread_basic() {
        let code = r#"
            const base = { id: 1, name: "Alice" };
            const extended = { ...base, age: 30 };
            return extended;
        "#;
        let mut compiler = PlanCompiler::new();
        let plan = compiler
            .compile_code(code)
            .expect("Object spread should compile");
        assert!(plan.steps.len() >= 2);
    }

    #[tokio::test]
    async fn test_execute_object_spread_basic() {
        let code = r#"
            const base = { id: 1, name: "Alice" };
            const extended = { ...base, age: 30 };
            return extended;
        "#;
        let mut compiler = PlanCompiler::new();
        let plan = compiler.compile_code(code).unwrap();
        let mock_http = MockHttpExecutor::new();
        let mut executor = PlanExecutor::new(mock_http, ExecutionConfig::default());
        let result = executor.execute(&plan).await.unwrap();
        assert_eq!(result.value["id"], serde_json::json!(1));
        assert_eq!(result.value["name"], serde_json::json!("Alice"));
        assert_eq!(result.value["age"], serde_json::json!(30));
    }

    #[tokio::test]
    async fn test_execute_object_spread_override() {
        // Later properties should override spread properties (JS semantics)
        let code = r#"
            const obj = { id: 1, name: "old" };
            const updated = { ...obj, name: "new" };
            return updated;
        "#;
        let mut compiler = PlanCompiler::new();
        let plan = compiler.compile_code(code).unwrap();
        let mock_http = MockHttpExecutor::new();
        let mut executor = PlanExecutor::new(mock_http, ExecutionConfig::default());
        let result = executor.execute(&plan).await.unwrap();
        assert_eq!(result.value["id"], serde_json::json!(1));
        assert_eq!(result.value["name"], serde_json::json!("new"));
    }

    #[tokio::test]
    async fn test_execute_object_spread_multiple() {
        let code = r#"
            const a = { x: 1 };
            const b = { y: 2 };
            const merged = { ...a, ...b, z: 3 };
            return merged;
        "#;
        let mut compiler = PlanCompiler::new();
        let plan = compiler.compile_code(code).unwrap();
        let mock_http = MockHttpExecutor::new();
        let mut executor = PlanExecutor::new(mock_http, ExecutionConfig::default());
        let result = executor.execute(&plan).await.unwrap();
        assert_eq!(result.value["x"], serde_json::json!(1));
        assert_eq!(result.value["y"], serde_json::json!(2));
        assert_eq!(result.value["z"], serde_json::json!(3));
    }

    #[tokio::test]
    async fn test_execute_object_spread_with_api_result() {
        // Primary use case: spread API result into a new object
        let code = r#"
            const config = await api.get('/config');
            const result = { ...config, extra: "added" };
            return result;
        "#;
        let mut compiler = PlanCompiler::new();
        let plan = compiler.compile_code(code).unwrap();
        let mut mock_http = MockHttpExecutor::new();
        mock_http.add_response(
            "/config",
            serde_json::json!({ "key": "value", "enabled": true }),
        );
        let mut executor = PlanExecutor::new(mock_http, ExecutionConfig::default());
        let result = executor.execute(&plan).await.unwrap();
        assert_eq!(result.value["key"], serde_json::json!("value"));
        assert_eq!(result.value["enabled"], serde_json::json!(true));
        assert_eq!(result.value["extra"], serde_json::json!("added"));
    }

    #[tokio::test]
    async fn test_execute_object_spread_non_object_noop() {
        // Spreading a non-object should be a no-op (matches JS behavior)
        let code = r#"
            const x = 42;
            const obj = { ...x, name: "test" };
            return obj;
        "#;
        let mut compiler = PlanCompiler::new();
        let plan = compiler.compile_code(code).unwrap();
        let mock_http = MockHttpExecutor::new();
        let mut executor = PlanExecutor::new(mock_http, ExecutionConfig::default());
        let result = executor.execute(&plan).await.unwrap();
        assert_eq!(result.value["name"], serde_json::json!("test"));
        // x (number) should not add any properties
        assert!(result.value.as_object().unwrap().len() == 1);
    }

    #[tokio::test]
    async fn test_execute_object_spread_preserves_order() {
        // Spread before explicit property: explicit wins
        // Explicit before spread: spread wins
        let code = r#"
            const obj = { a: 1, b: 2 };
            const result = { b: 99, ...obj, a: 100 };
            return result;
        "#;
        let mut compiler = PlanCompiler::new();
        let plan = compiler.compile_code(code).unwrap();
        let mock_http = MockHttpExecutor::new();
        let mut executor = PlanExecutor::new(mock_http, ExecutionConfig::default());
        let result = executor.execute(&plan).await.unwrap();
        // { b: 99 } then { ...obj } → b overridden to 2, then { a: 100 } → a overridden to 100
        assert_eq!(result.value["a"], serde_json::json!(100));
        assert_eq!(result.value["b"], serde_json::json!(2));
    }

    // =========================================================================
    // Object Destructuring Tests
    // =========================================================================

    #[test]
    fn test_compile_object_destructuring_simple() {
        let code = r#"
            const obj = { id: 1, name: "Alice" };
            const { id, name } = obj;
            return { id, name };
        "#;
        let mut compiler = PlanCompiler::new();
        let plan = compiler
            .compile_code(code)
            .expect("Object destructuring should compile");
        // Should have: Assign(obj), Assign(__destructure_0), Assign(id), Assign(name), Return
        assert!(plan.steps.len() >= 4);
    }

    #[tokio::test]
    async fn test_execute_object_destructuring_simple() {
        let code = r#"
            const obj = { id: 1, name: "Alice", extra: "ignored" };
            const { id, name } = obj;
            return { id, name };
        "#;
        let mut compiler = PlanCompiler::new();
        let plan = compiler.compile_code(code).unwrap();
        let mock_http = MockHttpExecutor::new();
        let mut executor = PlanExecutor::new(mock_http, ExecutionConfig::default());
        let result = executor.execute(&plan).await.unwrap();
        assert_eq!(result.value["id"], serde_json::json!(1));
        assert_eq!(result.value["name"], serde_json::json!("Alice"));
        // "extra" should not be in output since we only destructured id and name
        assert!(result.value.get("extra").is_none());
    }

    #[tokio::test]
    async fn test_execute_object_destructuring_renamed() {
        let code = r#"
            const user = { id: 1, name: "Alice" };
            const { id: userId, name: userName } = user;
            return { userId, userName };
        "#;
        let mut compiler = PlanCompiler::new();
        let plan = compiler.compile_code(code).unwrap();
        let mock_http = MockHttpExecutor::new();
        let mut executor = PlanExecutor::new(mock_http, ExecutionConfig::default());
        let result = executor.execute(&plan).await.unwrap();
        assert_eq!(result.value["userId"], serde_json::json!(1));
        assert_eq!(result.value["userName"], serde_json::json!("Alice"));
    }

    #[tokio::test]
    async fn test_execute_object_destructuring_with_api_call() {
        // The primary use case: destructure API response
        let code = r#"
            const { data, status } = await api.get('/users');
            return { data, status };
        "#;
        let mut compiler = PlanCompiler::new();
        let plan = compiler.compile_code(code).unwrap();
        let mut mock_http = MockHttpExecutor::new();
        mock_http.add_response(
            "/users",
            serde_json::json!({ "data": [{"id": 1}], "status": "ok", "meta": "hidden" }),
        );
        let mut executor = PlanExecutor::new(mock_http, ExecutionConfig::default());
        let result = executor.execute(&plan).await.unwrap();
        assert_eq!(result.value["data"], serde_json::json!([{"id": 1}]));
        assert_eq!(result.value["status"], serde_json::json!("ok"));
    }

    #[tokio::test]
    async fn test_execute_object_destructuring_missing_property() {
        // Missing properties should be null (matches JS behavior)
        let code = r#"
            const obj = { id: 1 };
            const { id, name } = obj;
            return { id, name };
        "#;
        let mut compiler = PlanCompiler::new();
        let plan = compiler.compile_code(code).unwrap();
        let mock_http = MockHttpExecutor::new();
        let mut executor = PlanExecutor::new(mock_http, ExecutionConfig::default());
        let result = executor.execute(&plan).await.unwrap();
        assert_eq!(result.value["id"], serde_json::json!(1));
        assert_eq!(result.value["name"], serde_json::json!(null));
    }

    // =========================================================================
    // Array Destructuring Tests
    // =========================================================================

    #[tokio::test]
    async fn test_execute_array_destructuring_simple() {
        let code = r#"
            const arr = [10, 20, 30];
            const [a, b] = arr;
            return { a, b };
        "#;
        let mut compiler = PlanCompiler::new();
        let plan = compiler.compile_code(code).unwrap();
        let mock_http = MockHttpExecutor::new();
        let mut executor = PlanExecutor::new(mock_http, ExecutionConfig::default());
        let result = executor.execute(&plan).await.unwrap();
        assert_eq!(result.value["a"], serde_json::json!(10));
        assert_eq!(result.value["b"], serde_json::json!(20));
    }

    #[tokio::test]
    async fn test_execute_array_destructuring_with_promise_all() {
        // Common pattern: destructure Promise.all results
        let code = r#"
            const [users, products] = await Promise.all([
                api.get('/users'),
                api.get('/products')
            ]);
            return { users, products };
        "#;
        let mut compiler = PlanCompiler::new();
        let plan = compiler.compile_code(code).unwrap();
        let mut mock_http = MockHttpExecutor::new();
        mock_http.add_response("/users", serde_json::json!([{"id": 1}]));
        mock_http.add_response("/products", serde_json::json!([{"sku": "A"}]));
        let mut executor = PlanExecutor::new(mock_http, ExecutionConfig::default());
        let result = executor.execute(&plan).await.unwrap();
        assert_eq!(result.value["users"], serde_json::json!([{"id": 1}]));
        assert_eq!(result.value["products"], serde_json::json!([{"sku": "A"}]));
    }

    // =========================================================================
    // For-of Loop Destructuring Tests
    // =========================================================================

    #[test]
    fn test_compile_for_of_destructuring() {
        let code = r#"
            const items = [{ id: 1, name: "A" }, { id: 2, name: "B" }];
            const results = [];
            for (const { id, name } of items.slice(0, 10)) {
                results.push({ id, name });
            }
            return results;
        "#;
        let mut compiler = PlanCompiler::new();
        compiler
            .compile_code(code)
            .expect("For-of with destructuring should compile");
    }

    #[tokio::test]
    async fn test_execute_for_of_destructuring() {
        let code = r#"
            const items = [{ id: 1, name: "A" }, { id: 2, name: "B" }];
            const results = [];
            for (const { id, name } of items.slice(0, 10)) {
                results.push({ label: name, num: id });
            }
            return results;
        "#;
        let mut compiler = PlanCompiler::new();
        let plan = compiler.compile_code(code).unwrap();
        let mock_http = MockHttpExecutor::new();
        let mut executor = PlanExecutor::new(mock_http, ExecutionConfig::default());
        let result = executor.execute(&plan).await.unwrap();
        let arr = result.value.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["label"], serde_json::json!("A"));
        assert_eq!(arr[0]["num"], serde_json::json!(1));
        assert_eq!(arr[1]["label"], serde_json::json!("B"));
        assert_eq!(arr[1]["num"], serde_json::json!(2));
    }

    #[tokio::test]
    async fn test_execute_for_of_destructuring_with_api_calls() {
        // Real-world pattern: destructure loop items, use properties in API calls
        let code = r#"
            const users = [{ id: 1, role: "admin" }, { id: 2, role: "user" }];
            const results = [];
            for (const { id, role } of users.slice(0, 10)) {
                const detail = await api.get(`/users/${id}`);
                results.push({ role, detail });
            }
            return results;
        "#;
        let mut compiler = PlanCompiler::new();
        let plan = compiler.compile_code(code).unwrap();
        let mut mock_http = MockHttpExecutor::new();
        mock_http.add_response("/users/1", serde_json::json!({ "name": "Alice" }));
        mock_http.add_response("/users/2", serde_json::json!({ "name": "Bob" }));
        let mut executor = PlanExecutor::new(mock_http, ExecutionConfig::default());
        let result = executor.execute(&plan).await.unwrap();
        let arr = result.value.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["role"], serde_json::json!("admin"));
        assert_eq!(arr[0]["detail"]["name"], serde_json::json!("Alice"));
        assert_eq!(arr[1]["role"], serde_json::json!("user"));
        assert_eq!(arr[1]["detail"]["name"], serde_json::json!("Bob"));
    }

    // =========================================================================
    // Combined Spread + Destructuring Tests
    // =========================================================================

    #[tokio::test]
    async fn test_execute_spread_and_destructuring_combined() {
        // Realistic pattern: destructure API response, spread into new request
        let code = r#"
            const { data, token } = await api.get('/auth');
            const result = await api.post('/action', { ...data, token });
            return result;
        "#;
        let mut compiler = PlanCompiler::new();
        let plan = compiler.compile_code(code).unwrap();
        let mut mock_http = MockHttpExecutor::new();
        mock_http.add_response(
            "/auth",
            serde_json::json!({ "data": { "user": "alice" }, "token": "abc123" }),
        );
        mock_http.add_response("/action", serde_json::json!({ "success": true }));
        let mut executor = PlanExecutor::new(mock_http, ExecutionConfig::default());
        let result = executor.execute(&plan).await.unwrap();
        assert_eq!(result.value["success"], serde_json::json!(true));
    }
}

// ============================================================================
// PHASE 128 D-09 — resolve-and-check-before-dispatch test support
// ============================================================================

/// Shared fixtures for the `layer_one` / `layer_two` / `composition` /
/// `error_does_not_echo_path` modules below.
///
/// Those four modules sit at THIS level (siblings of `tests`) rather than inside
/// it on purpose: the plan's verify selections are `executor::layer_one`,
/// `executor::composition` and `executor::error_does_not_echo_path`, and libtest
/// matches the FULL test path. A test nested in `tests` would be
/// `executor::tests::layer_one_…`, which those filters do NOT match — a
/// zero-selection that exits 0 and measures nothing.
#[cfg(test)]
mod d09_support {
    use super::*;
    use std::sync::{Arc, Mutex};

    /// One observation of an `execute_request` call.
    #[derive(Debug, Clone)]
    pub(super) struct Seen {
        pub(super) method: String,
        pub(super) path: String,
        pub(super) body: Option<JsonValue>,
    }

    /// A recording `HttpExecutor` whose log stays observable after the executor
    /// has been moved into a `PlanExecutor`.
    pub(super) struct RecordingHttp {
        seen: Arc<Mutex<Vec<Seen>>>,
        response: JsonValue,
    }

    impl RecordingHttp {
        /// Returns the executor plus a handle onto its call log.
        pub(super) fn new() -> (Self, Arc<Mutex<Vec<Seen>>>) {
            let seen = Arc::new(Mutex::new(Vec::new()));
            (
                Self {
                    seen: Arc::clone(&seen),
                    response: JsonValue::Object(serde_json::Map::new()),
                },
                seen,
            )
        }
    }

    #[async_trait::async_trait]
    impl HttpExecutor for RecordingHttp {
        async fn execute_request(
            &self,
            method: &str,
            path: ResolvedPath<'_>,
            body: Option<JsonValue>,
        ) -> Result<JsonValue, ExecutionError> {
            self.seen.lock().unwrap().push(Seen {
                method: method.to_string(),
                path: path.as_str().to_string(),
                body,
            });
            Ok(self.response.clone())
        }
    }

    /// An `HttpExecutor` that always fails, for the Pitfall-7 wrap tests.
    pub(super) struct FailingHttp;

    #[async_trait::async_trait]
    impl HttpExecutor for FailingHttp {
        async fn execute_request(
            &self,
            _method: &str,
            _path: ResolvedPath<'_>,
            _body: Option<JsonValue>,
        ) -> Result<JsonValue, ExecutionError> {
            Err(ExecutionError::RuntimeError {
                message: "simulated transport failure".to_string(),
            })
        }
    }

    pub(super) fn plan(steps: Vec<PlanStep>) -> ExecutionPlan {
        ExecutionPlan {
            steps,
            metadata: PlanMetadata {
                api_call_count: 0,
                has_mutations: false,
                endpoints: Vec::new(),
                methods_used: Vec::new(),
            },
        }
    }

    /// A GET `ApiCall` step over the given path parts and optional body literal.
    pub(super) fn get_step(parts: Vec<PathPart>, body: Option<JsonValue>) -> PlanStep {
        PlanStep::ApiCall {
            result_var: "out".to_string(),
            method: "GET".to_string(),
            path: PathTemplate { parts },
            body: body.map(ValueExpr::Literal),
        }
    }

    /// `literal` is the common case: a string-literal path template.
    pub(super) fn literal(path: &str) -> Vec<PathPart> {
        vec![PathPart::Literal(path.to_string())]
    }
}

/// FORK 2 — LAYER-1 `${var}` template-literal interpolation is floored.
///
/// `PathPart::Variable` and `PathPart::Expression` are what the JS compiler emits
/// for `` api.get(`/search/${v}`) ``, and their rendered values used to be pushed
/// into the path unchecked. A script taking that route never touches a `{key}`
/// placeholder, so before this phase it was never floored at all (T-128-20a).
#[cfg(test)]
mod layer_one {
    use super::d09_support::{get_step, literal, plan, RecordingHttp};
    use super::*;

    async fn run_with_var(
        var: &str,
        value: JsonValue,
    ) -> (Result<ExecutionResult, ExecutionError>, usize) {
        let (http, seen) = RecordingHttp::new();
        let step = get_step(
            vec![
                PathPart::Literal("/search/".to_string()),
                PathPart::Variable(var.to_string()),
            ],
            None,
        );
        let mut executor = PlanExecutor::new(http, ExecutionConfig::default());
        executor.set_variable(var, value);
        let result = executor.execute(&plan(vec![step])).await;
        let count = seen.lock().unwrap().len();
        (result, count)
    }

    #[tokio::test]
    async fn layer_one_refuses_a_query_separator_in_a_variable_part() {
        let payload = format!("2026AA?string={}", "z".repeat(60));
        let (result, calls) = run_with_var("lookupKey", JsonValue::String(payload.clone())).await;
        let err = result.expect_err("a query separator in a ${var} part must be refused");
        let rendered = err.to_string();
        assert_eq!(
            calls, 0,
            "no upstream request may be dispatched: {rendered}"
        );
        assert!(
            rendered.contains("lookupKey"),
            "a Variable part's refusal names its identifier, which is what proves \
             the refusal came from LAYER 1 and not from the composed check: {rendered}"
        );
        assert!(
            !rendered.contains("2026AA") && !rendered.contains('?'),
            "the refusal must carry no byte of the value: {rendered}"
        );
    }

    #[tokio::test]
    async fn layer_one_refuses_parent_traversal_in_a_variable_part() {
        let (result, calls) =
            run_with_var("lookupKey", JsonValue::String("../etc".to_string())).await;
        let rendered = result
            .expect_err("parent traversal in a ${var} part must be refused")
            .to_string();
        assert_eq!(
            calls, 0,
            "no upstream request may be dispatched: {rendered}"
        );
        assert!(rendered.contains("lookupKey"), "{rendered}");
    }

    #[tokio::test]
    async fn layer_one_refuses_an_over_cap_variable_part() {
        let over = "a".repeat(crate::PLACEHOLDER_MAX_LENGTH + 1);
        let (result, calls) = run_with_var("lookupKey", JsonValue::String(over)).await;
        let rendered = result
            .expect_err("an over-cap ${var} part must be refused")
            .to_string();
        assert_eq!(
            calls, 0,
            "no upstream request may be dispatched: {rendered}"
        );
        assert!(rendered.contains("lookupKey"), "{rendered}");
    }

    #[tokio::test]
    async fn layer_one_refuses_a_query_separator_in_an_expression_part() {
        // An Expression part has no name, and a rendering of the expression body
        // could itself contain caller text — so the refusal must use a FIXED
        // positional descriptor and never interpolate either one.
        let (http, seen) = RecordingHttp::new();
        let step = get_step(
            vec![
                PathPart::Literal("/search/".to_string()),
                PathPart::Expression(ValueExpr::PropertyAccess {
                    object: Box::new(ValueExpr::Variable("holder".to_string())),
                    property: "secretField".to_string(),
                }),
            ],
            None,
        );
        let mut executor = PlanExecutor::new(http, ExecutionConfig::default());
        executor.set_variable(
            "holder",
            serde_json::json!({ "secretField": "2026AA?string=payload" }),
        );
        let rendered = executor
            .execute(&plan(vec![step]))
            .await
            .expect_err("a query separator in an ${expr} part must be refused")
            .to_string();
        assert_eq!(seen.lock().unwrap().len(), 0, "{rendered}");
        assert!(
            rendered.contains("path expression"),
            "an Expression part uses a fixed positional descriptor: {rendered}"
        );
        for forbidden in ["2026AA", "payload", "secretField", "holder", "?"] {
            assert!(
                !rendered.contains(forbidden),
                "the refusal must carry neither the evaluated value nor a rendering \
                 of the expression body; found {forbidden:?} in {rendered:?}"
            );
        }
    }

    #[tokio::test]
    async fn layer_one_accepts_a_literal_only_template_including_slashes() {
        // Literal parts are the SCRIPT's own compiled text, not caller-substituted
        // data. Flooring them would refuse every legitimate multi-segment template.
        let (http, seen) = RecordingHttp::new();
        let step = get_step(literal("/Line/Mode/tube/Status"), None);
        let mut executor = PlanExecutor::new(http, ExecutionConfig::default());
        executor
            .execute(&plan(vec![step]))
            .await
            .expect("a literal-only template with slashes must still resolve");
        let seen = seen.lock().unwrap();
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].path, "/Line/Mode/tube/Status");
    }
}

/// LAYER-2 `{key}` placeholder resolution, moved ahead of dispatch by D-09.
#[cfg(test)]
mod layer_two {
    use super::d09_support::{get_step, literal, plan, RecordingHttp};
    use super::*;

    async fn run(
        path: &str,
        body: JsonValue,
    ) -> (
        Result<ExecutionResult, ExecutionError>,
        Vec<super::d09_support::Seen>,
    ) {
        let (http, seen) = RecordingHttp::new();
        let step = get_step(literal(path), Some(body));
        let mut executor = PlanExecutor::new(http, ExecutionConfig::default());
        let result = executor.execute(&plan(vec![step])).await;
        let seen = seen.lock().unwrap().clone();
        (result, seen)
    }

    #[tokio::test]
    async fn layer_two_substitutes_and_removes_the_consumed_key_from_the_body() {
        let (result, seen) = run("/users/{v}", serde_json::json!({"v": "7", "q": "keep"})).await;
        result.expect("a conforming placeholder must resolve");
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].path, "/users/7");
        assert_eq!(
            seen[0].body,
            Some(serde_json::json!({"q": "keep"})),
            "the path-consumed key must not also reach the body"
        );
    }

    #[tokio::test]
    async fn layer_two_conforming_call_produces_exactly_one_request() {
        let (result, seen) = run("/users/{v}", serde_json::json!({"v": "ada"})).await;
        result.expect("a conforming placeholder must resolve");
        assert_eq!(seen.len(), 1, "exactly one upstream request");
        assert_eq!(seen[0].method, "GET");
        assert_eq!(seen[0].path, "/users/ada");
    }

    #[tokio::test]
    async fn layer_two_refuses_a_query_separator_in_a_placeholder_value() {
        let payload = format!("2026AA?string={}", "z".repeat(60));
        let (result, seen) = run("/search/{v}", serde_json::json!({"v": payload})).await;
        let rendered = result
            .expect_err("a query separator in a {key} value must be refused")
            .to_string();
        assert!(
            seen.is_empty(),
            "no upstream request may be dispatched: {rendered}"
        );
        assert!(
            !rendered.contains("2026AA") && !rendered.contains('?'),
            "the refusal must carry no byte of the value: {rendered}"
        );
    }

    #[tokio::test]
    async fn layer_two_refuses_parent_traversal_in_a_placeholder_value() {
        let (result, seen) = run("/files/{v}", serde_json::json!({"v": "../../etc/passwd"})).await;
        let rendered = result
            .expect_err("parent traversal in a {key} value must be refused")
            .to_string();
        assert!(seen.is_empty(), "{rendered}");
        assert!(!rendered.contains("passwd"), "{rendered}");
    }

    #[tokio::test]
    async fn layer_two_refuses_an_object_valued_placeholder_naming_the_key_only() {
        // Preserves the WR-03 rule the toolkit's `scalar_str` used to apply, now
        // that resolution has moved up.
        let (result, seen) = run(
            "/users/{userId}",
            serde_json::json!({"userId": {"nested": [1, 2]}}),
        )
        .await;
        let rendered = result
            .expect_err("a non-scalar {key} value must be refused")
            .to_string();
        assert!(seen.is_empty(), "{rendered}");
        assert!(rendered.contains("userId"), "must name the key: {rendered}");
        for forbidden in ['{', '[', '"'] {
            assert!(
                !rendered.contains(forbidden),
                "must not echo JSON: {rendered}"
            );
        }
    }

    #[tokio::test]
    async fn layer_two_refuses_an_unsubstituted_placeholder_before_dispatch() {
        // No body key matches `{missing}`, so the placeholder survives. It must be
        // refused rather than sent upstream as literal braces.
        let (result, seen) = run("/users/{missing}", serde_json::json!({"other": "x"})).await;
        let rendered = result
            .expect_err("an unsubstituted placeholder must be refused")
            .to_string();
        assert!(
            seen.is_empty(),
            "literal braces must never reach the wire: {rendered}"
        );
    }

    #[tokio::test]
    async fn layer_two_aborts_before_any_substitution_reaches_a_request() {
        // Ordering: one conforming value and one refused value in the same path.
        // The refusal must abort the step, not dispatch a half-substituted path.
        let (result, seen) = run(
            "/a/{good}/b/{bad}",
            serde_json::json!({"good": "ok", "bad": "../escape"}),
        )
        .await;
        let rendered = result
            .expect_err("a refusal on any placeholder aborts the step")
            .to_string();
        assert!(seen.is_empty(), "{rendered}");
    }
}

/// The COMPOSED path check — the only check that can see an adjacency.
///
/// Per-value checking is insufficient BY CONSTRUCTION: 180 plus 200 code points
/// compose to an over-cap segment and `.` plus `.` composes to a traversal, from
/// contributions that each pass on their own (T-128-20b).
#[cfg(test)]
mod composition {
    use super::d09_support::{get_step, plan, RecordingHttp};
    use super::*;

    #[tokio::test]
    async fn composition_refuses_two_adjacent_values_over_the_cap_that_each_pass_alone() {
        let first = "a".repeat(180);
        let second = "b".repeat(200);

        // HALF ONE — each value passes `validate_path_placeholder` in isolation.
        // Without this assertion the test would still pass with the composed check
        // deleted, because some other rule could be doing the refusing.
        let rules = crate::PlaceholderRules::default();
        crate::validate_path_placeholder("a", &first, &rules)
            .expect("180 code points is under the cap and must pass alone");
        crate::validate_path_placeholder("b", &second, &rules)
            .expect("200 code points is under the cap and must pass alone");

        // HALF TWO — composed, the same two values are refused.
        let (http, seen) = RecordingHttp::new();
        let step = get_step(
            super::d09_support::literal("/search/{a}{b}"),
            Some(serde_json::json!({"a": first, "b": second})),
        );
        let mut executor = PlanExecutor::new(http, ExecutionConfig::default());
        let rendered = executor
            .execute(&plan(vec![step]))
            .await
            .expect_err("380 composed code points exceed the cap and must be refused")
            .to_string();
        assert!(seen.lock().unwrap().is_empty(), "{rendered}");
        assert!(
            !rendered.contains("aaaa") && !rendered.contains("bbbb"),
            "the composed refusal must carry no byte of either value: {rendered}"
        );
    }

    #[tokio::test]
    async fn composition_refuses_traversal_assembled_from_unchecked_literal_parts() {
        // The ISOLATING row. `PathPart::Literal` parts are deliberately NOT
        // floored, so NO per-value check runs here at all — the refusal can only
        // come from the composed check. This is the row that fails if
        // `validate_resolved_path` is removed.
        let (http, seen) = RecordingHttp::new();
        let step = get_step(
            vec![
                PathPart::Literal("/a/.".to_string()),
                PathPart::Literal(".".to_string()),
                PathPart::Literal("/b".to_string()),
            ],
            None,
        );
        let mut executor = PlanExecutor::new(http, ExecutionConfig::default());
        let rendered = executor
            .execute(&plan(vec![step]))
            .await
            .expect_err("a composed `..` segment must be refused")
            .to_string();
        assert!(seen.lock().unwrap().is_empty(), "{rendered}");
    }

    #[tokio::test]
    async fn composition_refuses_two_adjacent_single_dot_placeholders() {
        // The change request's row. After plan 02's single-dot floor this is
        // refused TWICE over — once per value, once composed — so it is the
        // `…_unchecked_literal_parts` sibling above that isolates the mechanism.
        let (http, seen) = RecordingHttp::new();
        let step = get_step(
            super::d09_support::literal("/a/{x}{y}/b"),
            Some(serde_json::json!({"x": ".", "y": "."})),
        );
        let mut executor = PlanExecutor::new(http, ExecutionConfig::default());
        let rendered = executor
            .execute(&plan(vec![step]))
            .await
            .expect_err("`.` + `.` composes to traversal and must be refused")
            .to_string();
        assert!(seen.lock().unwrap().is_empty(), "{rendered}");
    }

    #[tokio::test]
    async fn composition_refuses_a_mixed_layer_one_and_layer_two_over_cap_segment() {
        // One `${var}` part and one `{key}` placeholder adjacent in the SAME
        // segment. Neither layer alone can see this composition.
        let first = "a".repeat(180);
        let second = "b".repeat(200);
        let rules = crate::PlaceholderRules::default();
        crate::validate_path_placeholder("lookupKey", &first, &rules)
            .expect("the layer-1 contribution passes alone");
        crate::validate_path_placeholder("b", &second, &rules)
            .expect("the layer-2 contribution passes alone");

        let (http, seen) = RecordingHttp::new();
        let step = get_step(
            vec![
                PathPart::Literal("/search/".to_string()),
                PathPart::Variable("lookupKey".to_string()),
                PathPart::Literal("{b}".to_string()),
            ],
            Some(serde_json::json!({"b": second})),
        );
        let mut executor = PlanExecutor::new(http, ExecutionConfig::default());
        executor.set_variable("lookupKey", JsonValue::String(first));
        let rendered = executor
            .execute(&plan(vec![step]))
            .await
            .expect_err("a mixed-layer composed segment over the cap must be refused")
            .to_string();
        assert!(seen.lock().unwrap().is_empty(), "{rendered}");
    }
}

/// RESEARCH Pitfall 7 / SC-7 — no refusal reaching a client carries the path.
///
/// The module is named for the invariant so the plan's
/// `executor::error_does_not_echo_path` verify selection resolves.
#[cfg(test)]
mod error_does_not_echo_path {
    use super::d09_support::{get_step, literal, plan, FailingHttp};
    use super::*;

    #[tokio::test]
    async fn error_does_not_echo_path_when_the_executor_itself_fails() {
        let step = get_step(literal("/secret/inventory/endpoint"), None);
        let mut executor = PlanExecutor::new(FailingHttp, ExecutionConfig::default());
        let rendered = executor
            .execute(&plan(vec![step]))
            .await
            .expect_err("the failing executor must surface an error")
            .to_string();
        assert!(
            rendered.contains("GET"),
            "the wrap must still name the method: {rendered}"
        );
        assert!(
            rendered.contains("simulated transport failure"),
            "the wrap must still carry the underlying cause: {rendered}"
        );
        assert!(
            !rendered.contains("/secret/inventory/endpoint"),
            "the wrap must NOT re-attach the resolved path: {rendered}"
        );
    }

    #[tokio::test]
    async fn error_does_not_echo_path_in_the_parallel_arm() {
        let step = PlanStep::ParallelApiCalls {
            result_var: "out".to_string(),
            calls: vec![(
                "t0".to_string(),
                "GET".to_string(),
                PathTemplate {
                    parts: literal("/secret/inventory/endpoint"),
                },
                None,
            )],
        };
        let mut executor = PlanExecutor::new(FailingHttp, ExecutionConfig::default());
        let rendered = executor
            .execute(&plan(vec![step]))
            .await
            .expect_err("the failing executor must surface an error")
            .to_string();
        assert!(rendered.contains("GET"), "{rendered}");
        assert!(
            !rendered.contains("/secret/inventory/endpoint"),
            "the parallel arm's wrap must NOT re-attach the resolved path: {rendered}"
        );
    }

    #[tokio::test]
    async fn error_does_not_echo_path_on_a_refused_placeholder() {
        let (http, seen) = super::d09_support::RecordingHttp::new();
        let step = get_step(
            literal("/inventory/{sku}"),
            Some(serde_json::json!({"sku": "../../etc/shadow"})),
        );
        let mut executor = PlanExecutor::new(http, ExecutionConfig::default());
        let rendered = executor
            .execute(&plan(vec![step]))
            .await
            .expect_err("a refused placeholder must error")
            .to_string();
        assert!(seen.lock().unwrap().is_empty(), "{rendered}");
        for forbidden in ["shadow", "/inventory/", ".."] {
            assert!(
                !rendered.contains(forbidden),
                "a refusal carries neither the value nor the resolved path; \
                 found {forbidden:?} in {rendered:?}"
            );
        }
    }
}

/// The `?` narrowing, pinned in BOTH directions so it cannot become a hole.
///
/// `ResolvedPath::from_checked` splits at the first `?` and applies the full rule
/// set to each side, which exempts exactly one author-written query separator.
/// These rows assert what that buys AND everything it does not relax. A narrowing
/// with only accept-rows is indistinguishable from a deleted check.
#[cfg(test)]
mod query_separator {
    use super::d09_support::{get_step, literal, plan, RecordingHttp};
    use super::*;

    async fn run(
        parts: Vec<PathPart>,
        body: Option<JsonValue>,
    ) -> (
        Result<ExecutionResult, ExecutionError>,
        Vec<super::d09_support::Seen>,
    ) {
        let (http, seen) = RecordingHttp::new();
        let mut executor = PlanExecutor::new(http, ExecutionConfig::default());
        let result = executor.execute(&plan(vec![get_step(parts, body)])).await;
        let seen = seen.lock().unwrap().clone();
        (result, seen)
    }

    // ---- ACCEPTED: the separator an author wrote into a literal ----

    #[tokio::test]
    async fn query_separator_accepts_an_author_written_query_string() {
        // The row the operator's narrowing exists for: a `?` in the script's own
        // literal path text reaches the wire instead of being refused.
        let (result, seen) = run(literal("/Line/Mode/tube/Status?detail=true"), None).await;
        result.expect("an author-written query string must be accepted");
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].path, "/Line/Mode/tube/Status?detail=true");
    }

    #[tokio::test]
    async fn query_separator_accepts_a_literal_query_alongside_a_floored_placeholder() {
        // The separator is author-written; the `{v}` value still goes through the
        // per-value floor. Both mechanisms coexist on one path.
        let (result, seen) = run(
            literal("/content/{version}/CUI?string=headache"),
            Some(serde_json::json!({"version": "current"})),
        )
        .await;
        result.expect("an author query plus a conforming placeholder must be accepted");
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].path, "/content/current/CUI?string=headache");
    }

    #[tokio::test]
    async fn query_separator_accepts_a_graph_style_dollar_projection() {
        // The exact shape the in-tree Contoso M365 scripts author.
        let (result, seen) = run(
            literal("/drives/D/items/I/workbook/worksheets/Customers/range(address='A2:D7')?$select=values"),
            None,
        )
        .await;
        result.expect("a Graph $select projection in the path must be accepted");
        assert_eq!(seen.len(), 1);
        assert!(
            seen[0].path.ends_with("?$select=values"),
            "{:?}",
            seen[0].path
        );
    }

    // ---- STILL REFUSED: everything the split does not relax ----

    #[tokio::test]
    async fn query_separator_still_refuses_traversal_in_the_path_portion() {
        // `/a/../b?x=1` — the traversal rule applies to the whole string, so
        // appending a query does NOT launder a traversal past the composed check.
        let (result, seen) = run(literal("/a/../b?x=1"), None).await;
        let rendered = result
            .expect_err("traversal must still be refused when a query follows it")
            .to_string();
        assert!(seen.is_empty(), "{rendered}");
    }

    #[tokio::test]
    async fn query_separator_still_refuses_traversal_in_the_query_portion() {
        let (result, seen) = run(literal("/search?next=../../etc/passwd"), None).await;
        let rendered = result
            .expect_err("traversal inside the query portion must still be refused")
            .to_string();
        assert!(seen.is_empty(), "{rendered}");
        assert!(!rendered.contains("passwd"), "{rendered}");
    }

    #[tokio::test]
    async fn query_separator_still_refuses_a_control_byte_in_the_query_portion() {
        // Percent-encoded NUL, so the decode-once pass is what has to catch it.
        let (result, seen) = run(literal("/search?x=a%00b"), None).await;
        assert!(
            result.is_err(),
            "a control byte in the query portion must still be refused"
        );
        assert!(seen.is_empty());
    }

    #[tokio::test]
    async fn query_separator_still_refuses_an_over_cap_query_portion() {
        let long = "z".repeat(crate::PLACEHOLDER_MAX_LENGTH + 1);
        let (result, seen) = run(literal(&format!("/search?q={long}")), None).await;
        assert!(
            result.is_err(),
            "an over-cap query portion must still be refused"
        );
        assert!(seen.is_empty());
    }

    #[tokio::test]
    async fn query_separator_still_refuses_a_second_question_mark() {
        // Only the FIRST `?` is split off, so the query portion faces the
        // unmodified rule — which denies `?`. One exemption, not a general licence.
        let (result, seen) = run(literal("/search?a=1?b=2"), None).await;
        assert!(result.is_err(), "a second `?` must still be refused");
        assert!(seen.is_empty());
    }

    #[tokio::test]
    async fn query_separator_still_refuses_an_empty_query_portion() {
        // A dangling `/x?` is a trailing separator — the same class as a trailing
        // `/`, which plan 02 refuses deliberately.
        let (result, seen) = run(literal("/search?"), None).await;
        assert!(result.is_err(), "a dangling `?` must still be refused");
        assert!(seen.is_empty());
    }

    #[tokio::test]
    async fn query_separator_still_refuses_a_fragment_marker() {
        // The split is `?`-only. `#` is never sent to a server and stays denied.
        let (result, seen) = run(literal("/search#frag"), None).await;
        assert!(result.is_err(), "a fragment marker must still be refused");
        assert!(seen.is_empty());
    }

    #[tokio::test]
    async fn query_separator_still_refuses_an_injected_separator_from_a_value() {
        // THE row that proves the narrowing is not a hole. The template carries an
        // author-written `?` (now legal) AND a placeholder value carries an
        // injected one (still refused, by the per-value floor — which is the
        // mechanism the narrowing relies on for its safety argument).
        let payload = format!("2026AA?string={}", "z".repeat(60));
        let (result, seen) = run(
            literal("/search/{v}?detail=true"),
            Some(serde_json::json!({"v": payload})),
        )
        .await;
        let rendered = result
            .expect_err("an injected `?` in a VALUE must still be refused")
            .to_string();
        assert!(seen.is_empty(), "{rendered}");
        assert!(
            !rendered.contains("2026AA") && !rendered.contains('?'),
            "the refusal must still carry no byte of the value: {rendered}"
        );
    }

    #[tokio::test]
    async fn query_separator_still_refuses_an_injected_separator_from_a_layer_one_variable() {
        // Same boundary, layer-1 route: `${v}` rather than `{v}`.
        let (http, seen) = RecordingHttp::new();
        let mut executor = PlanExecutor::new(http, ExecutionConfig::default());
        executor.set_variable(
            "lookupKey",
            JsonValue::String("2026AA?string=x".to_string()),
        );
        let rendered = executor
            .execute(&plan(vec![get_step(
                vec![
                    PathPart::Literal("/search/".to_string()),
                    PathPart::Variable("lookupKey".to_string()),
                    PathPart::Literal("?detail=true".to_string()),
                ],
                None,
            )]))
            .await
            .expect_err("an injected `?` in a ${var} part must still be refused")
            .to_string();
        assert!(seen.lock().unwrap().is_empty(), "{rendered}");
        assert!(rendered.contains("lookupKey"), "{rendered}");
    }
}
