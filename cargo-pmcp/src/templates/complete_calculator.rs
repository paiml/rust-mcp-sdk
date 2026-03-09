//! Complete calculator template with tools, prompts, and resources

pub const COMPLETE_CALCULATOR_LIB: &str = r##"//! Complete calculator MCP server
//!
//! Demonstrates all three MCP capabilities:
//! - Tools: Basic arithmetic operations (add, subtract, multiply, divide, power, sqrt)
//! - Prompts: Two approaches to quadratic equation solving:
//!   - `solve_quadratic` - SequentialWorkflow that chains tools + fetches resources
//!   - `quadratic_simple` - SimplePrompt for comparison (self-contained calculation)
//! - Resources: Educational content about quadratic formulas
//!
//! ## Output Schema for Type-Safe Composition
//!
//! All arithmetic tools return `ArithmeticResult` with output schema annotations.
//! This enables:
//! 1. Type-safe workflow step chaining (the workflow uses `field("step", "result")`)
//! 2. Generated typed clients for external composition
//! 3. Self-documenting API for MCP clients
//!
//! The workflow prompt `solve_quadratic` demonstrates how output schemas enable
//! reliable data flow between tool calls - each step binds its result and the
//! next step can access it with compile-time safety.

use pmcp::{Error, Result, Server, SimplePrompt, ResourceCollection, StaticResource};
use pmcp::server::typed_tool::TypedToolWithOutput;
use pmcp::server::workflow::{SequentialWorkflow, WorkflowStep, ToolHandle, InternalPromptMessage};
use pmcp::server::workflow::dsl::{prompt_arg, field, constant};
use pmcp::types::{
    ServerCapabilities, GetPromptResult, PromptMessage, Role, MessageContent,
    ToolCapabilities, PromptCapabilities, ResourceCapabilities, PromptArgumentType,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use schemars::JsonSchema;
use validator::Validate;
use std::collections::HashMap;

// ============================================================================
// SHARED OUTPUT TYPE
// ============================================================================

/// Result from any arithmetic operation
///
/// All calculator tools return this consistent structure, enabling:
/// 1. **Workflow chaining**: The `solve_quadratic` workflow uses `field("step", "result")`
///    to pass values between steps - this works because all tools have a `result` field
/// 2. **Type-safe composition**: Code generators can produce typed client code:
///    ```rust,ignore
///    let result: ArithmeticResult = calculator.add(5.0, 3.0).await?;
///    println!("Answer: {}", result.result);  // Compiler knows the type!
///    ```
/// 3. **Human-readable output**: The `operation` field provides a formatted string
///    that can be displayed to users or included in LLM context
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ArithmeticResult {
    /// The numeric result of the calculation
    #[schemars(description = "The numeric result of the arithmetic operation")]
    pub result: f64,

    /// Human-readable description of the operation performed
    #[schemars(description = "A formatted string showing the operation (e.g., '5 + 3 = 8')")]
    pub operation: String,
}

// ============================================================================
// TOOL INPUT TYPES
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Validate)]
#[schemars(deny_unknown_fields)]
pub struct AddInput {
    /// First number to add
    #[validate(range(min = -1000000.0, max = 1000000.0))]
    #[schemars(description = "First number in the addition operation")]
    pub a: f64,

    /// Second number to add
    #[validate(range(min = -1000000.0, max = 1000000.0))]
    #[schemars(description = "Second number in the addition operation")]
    pub b: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Validate)]
#[schemars(deny_unknown_fields)]
pub struct SubtractInput {
    /// Number to subtract from
    #[validate(range(min = -1000000.0, max = 1000000.0))]
    #[schemars(description = "Minuend (number to subtract from)")]
    pub a: f64,

    /// Number to subtract
    #[validate(range(min = -1000000.0, max = 1000000.0))]
    #[schemars(description = "Subtrahend (number to subtract)")]
    pub b: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Validate)]
#[schemars(deny_unknown_fields)]
pub struct MultiplyInput {
    /// First factor
    #[validate(range(min = -1000000.0, max = 1000000.0))]
    #[schemars(description = "First number to multiply")]
    pub a: f64,

    /// Second factor
    #[validate(range(min = -1000000.0, max = 1000000.0))]
    #[schemars(description = "Second number to multiply")]
    pub b: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Validate)]
#[schemars(deny_unknown_fields)]
pub struct DivideInput {
    /// Dividend (number to be divided)
    #[validate(range(min = -1000000.0, max = 1000000.0))]
    #[schemars(description = "Dividend (number to be divided)")]
    pub a: f64,

    /// Divisor (number to divide by)
    #[validate(range(min = -1000000.0, max = 1000000.0))]
    #[schemars(description = "Divisor (number to divide by, must be non-zero)")]
    pub b: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Validate)]
#[schemars(deny_unknown_fields)]
pub struct PowerInput {
    /// Base number
    #[validate(range(min = -1000.0, max = 1000.0))]
    #[schemars(description = "Base number")]
    pub base: f64,

    /// Exponent
    #[validate(range(min = -100.0, max = 100.0))]
    #[schemars(description = "Exponent (power to raise to)")]
    pub exponent: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Validate)]
#[schemars(deny_unknown_fields)]
pub struct SqrtInput {
    /// Number to calculate square root of
    #[validate(range(min = 0.0, max = 1000000.0))]
    #[schemars(description = "Number to calculate square root of (must be non-negative)")]
    pub n: f64,
}

/// Build the calculator server with all capabilities
pub fn build_calculator_server() -> Result<Server> {
    // ========================================================================
    // WORKFLOW PROMPT: solve_quadratic
    // Demonstrates SequentialWorkflow with ACTUAL TOOL EXECUTION
    //
    // This workflow executes calculator tools server-side during prompts/get,
    // demonstrating the power of computational workflows in MCP. The server
    // performs all calculations deterministically and returns the complete
    // execution trace to the client.
    //
    // Formula: x = (-b ± √(b² - 4ac)) / 2a
    // ========================================================================
    let quadratic_workflow = SequentialWorkflow::new(
        "solve_quadratic",
        "Solve quadratic equations (ax² + bx + c = 0) using tool-chained computation"
    )
    // Define the prompt arguments with type hints for proper type conversion
    .typed_argument("a", "Coefficient of x² term (must be non-zero)", true, PromptArgumentType::Number)
    .typed_argument("b", "Coefficient of x term", true, PromptArgumentType::Number)
    .typed_argument("c", "Constant term", true, PromptArgumentType::Number)
    // System instruction explaining the workflow
    .instruction(InternalPromptMessage::system(
        "This workflow solves quadratic equations ax² + bx + c = 0 using the quadratic formula.\n\
         The server executes all calculation steps automatically using the calculator tools.\n\
         Watch as each step is computed and the results flow into subsequent calculations."
    ))
    // Step 1: Fetch the educational resource (context for the LLM)
    .step(
        WorkflowStep::fetch_resources("fetch_guide")
            .with_resource("calculator://help/quadratic-formula")
            .expect("Valid resource URI")
            .with_guidance("First, let me show you the quadratic formula reference:")
    )
    // Step 2: Calculate b² using power tool
    .step(
        WorkflowStep::new("calc_b_squared", ToolHandle::new("power"))
            .with_guidance("Step 1: Calculate b² (b squared)")
            .arg("base", prompt_arg("b"))
            .arg("exponent", constant(json!(2.0)))
            .bind("b_squared")
    )
    // Step 3: Calculate 4 * a
    .step(
        WorkflowStep::new("calc_4a", ToolHandle::new("multiply"))
            .with_guidance("Step 2: Calculate 4 × a")
            .arg("a", constant(json!(4.0)))
            .arg("b", prompt_arg("a"))
            .bind("four_a")
    )
    // Step 4: Calculate 4ac (using result from step 3)
    .step(
        WorkflowStep::new("calc_4ac", ToolHandle::new("multiply"))
            .with_guidance("Step 3: Calculate 4ac (4a × c)")
            .arg("a", field("four_a", "result"))
            .arg("b", prompt_arg("c"))
            .bind("four_ac")
    )
    // Step 5: Calculate discriminant: b² - 4ac
    .step(
        WorkflowStep::new("calc_discriminant", ToolHandle::new("subtract"))
            .with_guidance("Step 4: Calculate discriminant Δ = b² - 4ac")
            .arg("a", field("b_squared", "result"))
            .arg("b", field("four_ac", "result"))
            .bind("discriminant")
    )
    // Step 6: Calculate √discriminant (only works if discriminant >= 0)
    .step(
        WorkflowStep::new("calc_sqrt_discriminant", ToolHandle::new("sqrt"))
            .with_guidance("Step 5: Calculate √Δ (square root of discriminant)")
            .arg("n", field("discriminant", "result"))
            .bind("sqrt_discriminant")
    )
    // Step 7: Calculate 2a (denominator)
    .step(
        WorkflowStep::new("calc_2a", ToolHandle::new("multiply"))
            .with_guidance("Step 6: Calculate 2a (denominator for the formula)")
            .arg("a", constant(json!(2.0)))
            .arg("b", prompt_arg("a"))
            .bind("two_a")
    )
    // Step 8: Calculate -b (negate b)
    .step(
        WorkflowStep::new("calc_neg_b", ToolHandle::new("multiply"))
            .with_guidance("Step 7: Calculate -b")
            .arg("a", constant(json!(-1.0)))
            .arg("b", prompt_arg("b"))
            .bind("neg_b")
    )
    // Step 9: Calculate -b + √Δ (numerator for x₁)
    .step(
        WorkflowStep::new("calc_x1_numerator", ToolHandle::new("add"))
            .with_guidance("Step 8: Calculate -b + √Δ (numerator for first root)")
            .arg("a", field("neg_b", "result"))
            .arg("b", field("sqrt_discriminant", "result"))
            .bind("x1_numerator")
    )
    // Step 10: Calculate x₁ = (-b + √Δ) / 2a
    .step(
        WorkflowStep::new("calc_x1", ToolHandle::new("divide"))
            .with_guidance("Step 9: Calculate x₁ = (-b + √Δ) / 2a")
            .arg("a", field("x1_numerator", "result"))
            .arg("b", field("two_a", "result"))
            .bind("x1")
    )
    // Step 11: Calculate -b - √Δ (numerator for x₂)
    .step(
        WorkflowStep::new("calc_x2_numerator", ToolHandle::new("subtract"))
            .with_guidance("Step 10: Calculate -b - √Δ (numerator for second root)")
            .arg("a", field("neg_b", "result"))
            .arg("b", field("sqrt_discriminant", "result"))
            .bind("x2_numerator")
    )
    // Step 12: Calculate x₂ = (-b - √Δ) / 2a
    .step(
        WorkflowStep::new("calc_x2", ToolHandle::new("divide"))
            .with_guidance("Step 11: Calculate x₂ = (-b - √Δ) / 2a")
            .arg("a", field("x2_numerator", "result"))
            .arg("b", field("two_a", "result"))
            .bind("x2")
    );

    // ========================================================================
    // SIMPLE PROMPT: quadratic_simple
    // Demonstrates SimplePrompt for comparison (self-contained calculation)
    // ========================================================================
    let quadratic_simple = SimplePrompt::new(
        "quadratic_simple",
        Box::new(|args: HashMap<String, String>, _extra| {
            Box::pin(async move {
                // Parse arguments
                let a: f64 = args.get("a")
                    .ok_or_else(|| Error::validation("Missing required argument 'a'"))?
                    .parse()
                    .map_err(|_| Error::validation("Argument 'a' must be a number"))?;
                let b: f64 = args.get("b")
                    .ok_or_else(|| Error::validation("Missing required argument 'b'"))?
                    .parse()
                    .map_err(|_| Error::validation("Argument 'b' must be a number"))?;
                let c: f64 = args.get("c")
                    .ok_or_else(|| Error::validation("Missing required argument 'c'"))?
                    .parse()
                    .map_err(|_| Error::validation("Argument 'c' must be a number"))?;

                if a == 0.0 {
                    return Err(Error::validation("Coefficient 'a' cannot be zero (not a quadratic equation)"));
                }

                // Calculate discriminant: b² - 4ac
                let discriminant = b * b - 4.0 * a * c;

                let mut messages = vec![
                    PromptMessage {
                        role: Role::User,
                        content: MessageContent::Text {
                            text: format!(
                                "Solve the quadratic equation: {}x² + {}x + {} = 0",
                                a, b, c
                            ),
                        },
                    }
                ];

                if discriminant < 0.0 {
                    messages.push(PromptMessage {
                        role: Role::Assistant,
                        content: MessageContent::Text {
                            text: format!(
                                "This equation has no real roots.\n\
                                 Discriminant Δ = b² - 4ac = {} < 0\n\
                                 The roots are complex numbers.",
                                discriminant
                            ),
                        },
                    });
                } else if discriminant == 0.0 {
                    let x = -b / (2.0 * a);
                    messages.push(PromptMessage {
                        role: Role::Assistant,
                        content: MessageContent::Text {
                            text: format!(
                                "This equation has one repeated real root.\n\
                                 Discriminant Δ = b² - 4ac = 0\n\
                                 Solution: x = {}\n\n\
                                 Using the formula: x = -b / 2a = -{} / (2 × {}) = {}",
                                x, b, a, x
                            ),
                        },
                    });
                } else {
                    let sqrt_discriminant = discriminant.sqrt();
                    let x1 = (-b + sqrt_discriminant) / (2.0 * a);
                    let x2 = (-b - sqrt_discriminant) / (2.0 * a);

                    messages.push(PromptMessage {
                        role: Role::Assistant,
                        content: MessageContent::Text {
                            text: format!(
                                "This equation has two distinct real roots.\n\
                                 Discriminant Δ = b² - 4ac = {} > 0\n\
                                 √Δ = {}\n\n\
                                 Using the quadratic formula: x = (-b ± √Δ) / 2a\n\n\
                                 Solution 1: x₁ = ({} + {}) / {} = {}\n\
                                 Solution 2: x₂ = ({} - {}) / {} = {}",
                                discriminant,
                                sqrt_discriminant,
                                -b, sqrt_discriminant, 2.0 * a, x1,
                                -b, sqrt_discriminant, 2.0 * a, x2
                            ),
                        },
                    });
                }

                Ok(GetPromptResult::new(messages, Some(format!(
                        "Solution for {}x² + {}x + {} = 0",
                        a, b, c
                    ))))
            }) as std::pin::Pin<Box<dyn std::future::Future<Output = Result<GetPromptResult>> + Send>>
        })
    )
    .with_description("Solve quadratic equations (SimplePrompt version - self-contained calculation)")
    .with_argument("a", "Coefficient of x² term (a in ax² + bx + c = 0)", true)
    .with_argument("b", "Coefficient of x term (b in ax² + bx + c = 0)", true)
    .with_argument("c", "Constant term (c in ax² + bx + c = 0)", true);

    // ========================================================================
    // RESOURCES
    // ========================================================================
    let resources = ResourceCollection::new()
        .add_resource(
            StaticResource::new_text(
                "calculator://help/quadratic-formula",
                r#"# Quadratic Formula Guide

## The Formula

For equations in the form: **ax² + bx + c = 0** (where a ≠ 0)

The solutions are given by:

**x = (-b ± √(b² - 4ac)) / 2a**

## The Discriminant

The discriminant **Δ = b² - 4ac** determines the nature of the roots:

- **Δ > 0**: Two distinct real roots
- **Δ = 0**: One repeated real root (the parabola touches the x-axis at one point)
- **Δ < 0**: No real roots (the parabola doesn't cross the x-axis)

## Step-by-Step Process

1. **Identify coefficients**: Determine values of a, b, and c
2. **Calculate discriminant**: Δ = b² - 4ac
3. **Check discriminant**: Determine how many real roots exist
4. **Apply formula**: x = (-b ± √Δ) / 2a
5. **Simplify**: Calculate both roots (if they exist)

## Example 1: Two Real Roots

Solve: **x² - 5x + 6 = 0**

- a = 1, b = -5, c = 6
- Δ = (-5)² - 4(1)(6) = 25 - 24 = 1
- √Δ = 1
- x = (5 ± 1) / 2

**Solutions:**
- x₁ = (5 + 1) / 2 = 3
- x₂ = (5 - 1) / 2 = 2

## Example 2: One Repeated Root

Solve: **x² - 6x + 9 = 0**

- a = 1, b = -6, c = 9
- Δ = (-6)² - 4(1)(9) = 36 - 36 = 0
- x = 6 / 2 = 3

**Solution:** x = 3 (repeated)

## Example 3: No Real Roots

Solve: **x² + 2x + 5 = 0**

- a = 1, b = 2, c = 5
- Δ = 2² - 4(1)(5) = 4 - 20 = -16

**Result:** No real solutions (Δ < 0)
"#,
            )
            .with_name("Quadratic Formula Guide")
            .with_description("Learn how to solve quadratic equations using the quadratic formula")
            .with_mime_type("text/markdown"),
        );

    // ========================================================================
    // BUILD SERVER
    // ========================================================================
    Server::builder()
        .name("calculator")
        .version("1.0.0")
        .capabilities(ServerCapabilities {
            tools: Some(ToolCapabilities { list_changed: Some(true) }),
            prompts: Some(PromptCapabilities { list_changed: Some(true) }),
            resources: Some(ResourceCapabilities {
                subscribe: Some(true),
                list_changed: Some(true),
            }),
            ..Default::default()
        })
        // Add tools with TypedToolWithOutput for output schema annotations
        // Each tool returns ArithmeticResult, enabling:
        // 1. Workflow step chaining via field("step_name", "result")
        // 2. Type-safe client generation for composition
        .tool(
            "add",
            TypedToolWithOutput::new("add", |input: AddInput, _extra| {
                Box::pin(async move {
                    input.validate()
                        .map_err(|e| Error::validation(format!("Validation failed: {}", e)))?;
                    let result = input.a + input.b;
                    Ok(ArithmeticResult {
                        result,
                        operation: format!("{} + {} = {}", input.a, input.b, result),
                    })
                })
            })
            .with_description("Add two numbers together"),
        )
        .tool(
            "subtract",
            TypedToolWithOutput::new("subtract", |input: SubtractInput, _extra| {
                Box::pin(async move {
                    input.validate()
                        .map_err(|e| Error::validation(format!("Validation failed: {}", e)))?;
                    let result = input.a - input.b;
                    Ok(ArithmeticResult {
                        result,
                        operation: format!("{} - {} = {}", input.a, input.b, result),
                    })
                })
            })
            .with_description("Subtract one number from another"),
        )
        .tool(
            "multiply",
            TypedToolWithOutput::new("multiply", |input: MultiplyInput, _extra| {
                Box::pin(async move {
                    input.validate()
                        .map_err(|e| Error::validation(format!("Validation failed: {}", e)))?;
                    let result = input.a * input.b;
                    Ok(ArithmeticResult {
                        result,
                        operation: format!("{} × {} = {}", input.a, input.b, result),
                    })
                })
            })
            .with_description("Multiply two numbers together"),
        )
        .tool(
            "divide",
            TypedToolWithOutput::new("divide", |input: DivideInput, _extra| {
                Box::pin(async move {
                    input.validate()
                        .map_err(|e| Error::validation(format!("Validation failed: {}", e)))?;

                    if input.b == 0.0 {
                        return Err(Error::validation("Cannot divide by zero"));
                    }

                    let result = input.a / input.b;
                    Ok(ArithmeticResult {
                        result,
                        operation: format!("{} ÷ {} = {}", input.a, input.b, result),
                    })
                })
            })
            .with_description("Divide one number by another (with zero-division check)"),
        )
        .tool(
            "power",
            TypedToolWithOutput::new("power", |input: PowerInput, _extra| {
                Box::pin(async move {
                    input.validate()
                        .map_err(|e| Error::validation(format!("Validation failed: {}", e)))?;
                    let result = input.base.powf(input.exponent);
                    Ok(ArithmeticResult {
                        result,
                        operation: format!("{}^{} = {}", input.base, input.exponent, result),
                    })
                })
            })
            .with_description("Raise a number to a power (exponentiation)"),
        )
        .tool(
            "sqrt",
            TypedToolWithOutput::new("sqrt", |input: SqrtInput, _extra| {
                Box::pin(async move {
                    input.validate()
                        .map_err(|e| Error::validation(format!("Validation failed: {}", e)))?;
                    let result = input.n.sqrt();
                    Ok(ArithmeticResult {
                        result,
                        operation: format!("√{} = {}", input.n, result),
                    })
                })
            })
            .with_description("Calculate square root of a non-negative number"),
        )
        // IMPORTANT: Register resources BEFORE workflow prompt (workflow needs access to resources)
        .resources(resources)
        // Add workflow prompt (chains tools + fetches resources)
        .prompt_workflow(quadratic_workflow)?
        // Add simple prompt (self-contained calculation)
        .prompt("quadratic_simple", quadratic_simple)
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_server_creation() {
        let server = build_calculator_server();
        assert!(server.is_ok());
    }

    #[tokio::test]
    async fn test_add_validation() {
        let input = AddInput { a: 5.0, b: 3.0 };
        assert!(input.validate().is_ok());

        // Test out of range
        let input = AddInput { a: 2000000.0, b: 3.0 };
        assert!(input.validate().is_err());
    }

    #[tokio::test]
    async fn test_sqrt_validation() {
        let input = SqrtInput { n: 16.0 };
        assert!(input.validate().is_ok());

        // Test negative (invalid)
        let input = SqrtInput { n: -1.0 };
        assert!(input.validate().is_err());
    }

    #[tokio::test]
    async fn test_quadratic_discriminant() {
        // Two roots: x² - 5x + 6 = 0
        let (a, b, c) = (1.0_f64, -5.0_f64, 6.0_f64);
        let discriminant = b * b - 4.0 * a * c;
        assert_eq!(discriminant, 1.0);

        // One root: x² - 6x + 9 = 0
        let (a, b, c) = (1.0_f64, -6.0_f64, 9.0_f64);
        let discriminant = b * b - 4.0 * a * c;
        assert_eq!(discriminant, 0.0);

        // No real roots: x² + 2x + 5 = 0
        let (a, b, c) = (1.0_f64, 2.0_f64, 5.0_f64);
        let discriminant = b * b - 4.0 * a * c;
        assert!(discriminant < 0.0);
    }
}
"##;
