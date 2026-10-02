//! Static class policy for OpenAPI Code Mode.
//!
//! Every OpenAPI policy key in [`CodeModeConfig`] is enforced here, without a
//! [`PolicyEvaluator`](crate::PolicyEvaluator). A configured evaluator (Cedar,
//! AVP) runs AFTER this gate and can only narrow its verdict, never widen it.
//!
//! Before this module the static gate looked at HTTP methods only and left
//! every other key — the write allowlist, the read switch, deletes, blocked
//! paths, catalog classes, `admin` — to the evaluator. Under
//! [`NoopPolicyEvaluator`](crate::NoopPolicyEvaluator) those keys were inert,
//! and a non-empty write allowlist widened to allow-all.
//!
//! # Classes
//!
//! Each API call gets one [`UnifiedAction`] class:
//!
//! 1. The declared `category` of the matching `[[code_mode.operations]]`
//!    entry (see [`OperationRegistry::lookup_entry`]). A category that is not
//!    `read`, `write`, `delete` or `admin` is refused, never guessed.
//! 2. Otherwise the HTTP method: GET/HEAD/OPTIONS are `read`, POST/PUT/PATCH
//!    are `write`, DELETE is `delete`.
//!
//! A call whose path is only known at run time keeps the stricter of the two,
//! so a catalog entry cannot relax a path the validator cannot see. The
//! run-time half of the check is [`OpenApiClassPolicy::check_request`], which
//! sees the resolved path.
//!
//! # Modes, derived from the existing keys
//!
//! | Class  | Mode |
//! |--------|------|
//! | read   | `openapi_reads_enabled` ? `allow_all` : `deny_all` |
//! | write  | `!openapi_allow_writes` → `deny_all`; non-empty `openapi_allowed_writes` → `allowlist`; else `allow_all` |
//! | delete | `!openapi_allow_deletes` → `deny_all`; non-empty `openapi_allowed_deletes` → `allowlist`; else `allow_all` |
//! | admin  | always `deny_all` (no key enables it) |
//!
//! `openapi_blocked_writes` blocks every call it names, in any class. An entry
//! that is an HTTP method name (`"POST"`) blocks that method; any other entry
//! is an operation (`"POST /users"`, `"POST:/users/{id}"` or a catalog id).
//! `openapi_blocked_paths` blocks every call whose path falls under one of its
//! patterns (`*` matches any run of characters; a pattern with no `*` blocks
//! the path itself and everything below it). Both comparisons ignore case.
//!
//! The write and delete rules mirror
//! [`CodeModeConfig::to_openapi_server_entity`]'s `write_mode`, so the static
//! gate and the Cedar entity describe the same policy.
//!
//! # Scope
//!
//! SDK-backed Code Mode (`sdk_operations`) issues no HTTP calls and is not
//! classified here.

use crate::config::{CodeModeConfig, OperationRegistry};
use crate::javascript::{HttpMethod, JavaScriptCodeInfo};
use crate::policy::types::normalize_operation_format;
use crate::types::{PolicyViolation, UnifiedAction};
use std::collections::HashSet;

const POLICY_NAME: &str = "code_mode";

const HTTP_METHODS: [&str; 7] = ["GET", "HEAD", "OPTIONS", "POST", "PUT", "PATCH", "DELETE"];

/// How one class of operations is governed.
///
/// `#[non_exhaustive]`: a later mode is a new variant, so a `match` needs a
/// wildcard arm.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ClassMode {
    /// No operation of the class is allowed.
    DenyAll,
    /// Every operation of the class is allowed (subject to the blocklists).
    AllowAll,
    /// Only the listed operations are allowed. An entry is a catalog id or an
    /// operation (`"GET /items/{id}"`, `"GET:/items/{id}"`); the policy stores
    /// it normalized (`METHOD:/path`, `{param}` segments as `*`).
    Allowlist(HashSet<String>),
}

impl ClassMode {
    /// The mode's config name: `deny_all`, `allow_all` or `allowlist`.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::DenyAll => "deny_all",
            Self::AllowAll => "allow_all",
            Self::Allowlist(_) => "allowlist",
        }
    }

    fn allowlist(entries: &HashSet<String>) -> Self {
        Self::Allowlist(
            entries
                .iter()
                .map(|e| normalize_operation_format(e))
                .collect(),
        )
    }

    fn normalized(self) -> Self {
        match self {
            Self::Allowlist(entries) => Self::allowlist(&entries),
            other => other,
        }
    }
}

/// The static OpenAPI policy of one server, derived from its [`CodeModeConfig`].
#[derive(Debug, Clone)]
pub struct OpenApiClassPolicy {
    read: ClassMode,
    write: ClassMode,
    delete: ClassMode,
    admin: ClassMode,
    blocked_methods: HashSet<String>,
    blocked_operations: HashSet<String>,
    blocked_paths: Vec<String>,
}

/// One call, as the policy sees it.
struct Call<'a> {
    method: &'a str,
    path: &'a str,
    /// The path holds a segment only known at run time.
    dynamic: bool,
    /// How violations name the call: by method and source line, never by
    /// path. A path can carry caller-supplied values, and a refusal must not
    /// echo them (the same rule `ValidationResult` explanations follow). A
    /// matched catalog id is added, since it comes from the operator's config.
    label: String,
}

impl OpenApiClassPolicy {
    /// Derive the policy from the OpenAPI keys of `config` (table in the
    /// module docs).
    #[must_use]
    pub fn from_config(config: &CodeModeConfig) -> Self {
        let read = if config.openapi_reads_enabled {
            ClassMode::AllowAll
        } else {
            ClassMode::DenyAll
        };
        let write = gated_mode(config.openapi_allow_writes, &config.openapi_allowed_writes);
        let delete = gated_mode(
            config.openapi_allow_deletes,
            &config.openapi_allowed_deletes,
        );

        Self {
            read,
            write,
            delete,
            admin: ClassMode::DenyAll,
            blocked_methods: HashSet::new(),
            blocked_operations: HashSet::new(),
            blocked_paths: Vec::new(),
        }
        .with_blocked_operations(&config.openapi_blocked_writes)
        .with_blocked_paths(&config.openapi_blocked_paths)
    }

    /// Replace the mode of one class. Allowlist entries are normalized.
    ///
    /// For a caller whose own config can say more than the `openapi_*` keys
    /// of [`CodeModeConfig`] — a read allowlist, an `admin` mode. Install the
    /// result with
    /// [`ValidationPipeline::with_openapi_class_policy`](crate::ValidationPipeline::with_openapi_class_policy).
    #[must_use]
    pub fn with_mode(mut self, class: UnifiedAction, mode: ClassMode) -> Self {
        let mode = mode.normalized();
        match class {
            UnifiedAction::Read => self.read = mode,
            UnifiedAction::Write => self.write = mode,
            UnifiedAction::Delete => self.delete = mode,
            UnifiedAction::Admin => self.admin = mode,
        }
        self
    }

    /// Replace the blocked operations. Same entry forms as
    /// `openapi_blocked_writes`: an HTTP method name blocks the method, any
    /// other entry names an operation. A block applies in every class.
    #[must_use]
    pub fn with_blocked_operations<I, S>(mut self, entries: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.blocked_methods.clear();
        self.blocked_operations.clear();
        for entry in entries {
            let entry = entry.as_ref();
            let upper = entry.trim().to_ascii_uppercase();
            if HTTP_METHODS.contains(&upper.as_str()) {
                self.blocked_methods.insert(upper);
            } else {
                self.blocked_operations
                    .insert(normalize_operation_format(entry));
            }
        }
        self
    }

    /// Replace the blocked path patterns (same rules as
    /// `openapi_blocked_paths`).
    #[must_use]
    pub fn with_blocked_paths<I, S>(mut self, patterns: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.blocked_paths = patterns
            .into_iter()
            .map(|p| normalize_blocked_path(p.as_ref()))
            .collect();
        self
    }

    /// The mode governing `class`.
    #[must_use]
    pub fn mode(&self, class: UnifiedAction) -> &ClassMode {
        match class {
            UnifiedAction::Read => &self.read,
            UnifiedAction::Write => &self.write,
            UnifiedAction::Delete => &self.delete,
            UnifiedAction::Admin => &self.admin,
        }
    }

    /// Check every API call of a parsed script. Returns one violation per
    /// refused call; an empty list means the script passes the static policy.
    #[must_use]
    pub fn check_script(
        &self,
        info: &JavaScriptCodeInfo,
        registry: &OperationRegistry,
    ) -> Vec<PolicyViolation> {
        info.api_calls
            .iter()
            .filter_map(|api_call| {
                let method = method_name(api_call.method);
                let call = Call {
                    method,
                    path: &api_call.path,
                    dynamic: api_call.is_dynamic_path || api_call.path.contains('{'),
                    label: format!("the {method} call on line {}", api_call.line),
                };
                self.check_call(&call, registry).err()
            })
            .collect()
    }

    /// Check one request at execution time, after its path is resolved.
    ///
    /// The validator classifies a dynamic path conservatively; this is the
    /// check that sees where the request actually goes. Any query string is
    /// ignored for matching. Violation messages name the method and class but
    /// never the path, which carries caller-supplied values.
    ///
    /// # Errors
    ///
    /// Returns the violation when the request is refused.
    pub fn check_request(
        &self,
        method: &str,
        path: &str,
        registry: &OperationRegistry,
    ) -> Result<(), PolicyViolation> {
        let upper = method.to_ascii_uppercase();
        if HttpMethod::from_str(&upper).is_none() {
            return Err(violation(
                "unknown_method",
                format!("HTTP method '{method}' is not supported"),
            ));
        }
        let call = Call {
            method: &upper,
            path: strip_query(path),
            dynamic: false,
            label: format!("this {upper} request"),
        };
        self.check_call(&call, registry)
    }

    fn check_call(
        &self,
        call: &Call<'_>,
        registry: &OperationRegistry,
    ) -> Result<(), PolicyViolation> {
        let entry = registry.lookup_entry(Some(call.method), call.path);
        let label = match entry {
            Some(e) => format!("{} (operation '{}')", call.label, e.id),
            None => call.label.clone(),
        };
        if self.blocked_methods.contains(call.method) {
            return Err(violation(
                "blocked_method",
                format!("HTTP method '{}' is blocked for this server", call.method),
            ));
        }
        if self.path_blocked(call) {
            return Err(violation(
                "blocked_path",
                format!("{label} is under a blocked path"),
            ));
        }

        let class = classify(call, entry.map(|e| e.category.as_str()))
            .ok_or_else(|| {
                violation(
                    "unknown_category",
                    format!(
                        "{label} is declared with category '{}', which is not read, write, delete or admin",
                        entry.map_or("", |e| e.category.as_str())
                    ),
                )
            })?;

        let candidates = operation_candidates(call, entry.map(|e| e.id.as_str()));
        if candidates.iter().any(|c| {
            self.blocked_operations
                .iter()
                .any(|b| operation_matches(b, c))
        }) {
            return Err(violation(
                "blocked_operation",
                format!("{label} is a blocked operation"),
            ));
        }

        let class_name = class_label(class);
        let article = indefinite_article(class_name);
        match self.mode(class) {
            ClassMode::AllowAll => Ok(()),
            ClassMode::DenyAll => Err(violation(
                "class_denied",
                format!(
                    "{label} is {article} {class_name} operation, and {class_name} operations are deny_all on this server"
                ),
            )
            .with_suggestion(format!(
                "Only operations whose class is allowed can be called. This server does not allow {class_name} operations."
            ))),
            ClassMode::Allowlist(allowed) => {
                if candidates
                    .iter()
                    .any(|c| allowed.iter().any(|a| operation_matches(a, c)))
                {
                    Ok(())
                } else {
                    Err(violation(
                        "not_in_allowlist",
                        format!(
                            "{label} is {article} {class_name} operation that is not in this server's {class_name} allowlist"
                        ),
                    ))
                }
            },
        }
    }

    fn path_blocked(&self, call: &Call<'_>) -> bool {
        if self.blocked_paths.is_empty() {
            return false;
        }
        let path = call.path.to_ascii_lowercase();
        if call.dynamic {
            // The run-time value could land anywhere below the static prefix,
            // so refuse when the prefix could still reach a blocked pattern.
            let prefix = if path.starts_with('/') {
                path.split('{').next().unwrap_or("")
            } else {
                ""
            };
            return self.blocked_paths.iter().any(|pattern| {
                let literal = pattern.split('*').next().unwrap_or("");
                prefix.starts_with(literal) || literal.starts_with(prefix)
            });
        }
        self.blocked_paths
            .iter()
            .any(|pattern| blocked_path_matches(pattern, &path))
    }
}

/// One line naming each class's mode and the block counts, for a startup
/// log: `read=allow_all write=deny_all delete=deny_all admin=deny_all
/// blocked_operations=0 blocked_paths=0`. Lists are counted, not printed.
impl std::fmt::Display for OpenApiClassPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "read={} write={} delete={} admin={} blocked_operations={} blocked_paths={}",
            self.read.name(),
            self.write.name(),
            self.delete.name(),
            self.admin.name(),
            self.blocked_methods.len() + self.blocked_operations.len(),
            self.blocked_paths.len()
        )
    }
}

fn gated_mode(allowed: bool, allowlist: &HashSet<String>) -> ClassMode {
    if !allowed {
        ClassMode::DenyAll
    } else if allowlist.is_empty() {
        ClassMode::AllowAll
    } else {
        ClassMode::allowlist(allowlist)
    }
}

fn violation(rule: &str, message: String) -> PolicyViolation {
    PolicyViolation::new(POLICY_NAME, rule, message)
}

fn method_name(method: HttpMethod) -> &'static str {
    match method {
        HttpMethod::Get => "GET",
        HttpMethod::Post => "POST",
        HttpMethod::Put => "PUT",
        HttpMethod::Delete => "DELETE",
        HttpMethod::Patch => "PATCH",
        HttpMethod::Head => "HEAD",
        HttpMethod::Options => "OPTIONS",
    }
}

/// "an admin", "a read": the four class names are the only words placed after
/// it, and only "admin" starts with a vowel sound.
fn indefinite_article(word: &str) -> &'static str {
    if word.starts_with(['a', 'e', 'i', 'o', 'u']) {
        "an"
    } else {
        "a"
    }
}

fn class_label(class: UnifiedAction) -> &'static str {
    match class {
        UnifiedAction::Read => "read",
        UnifiedAction::Write => "write",
        UnifiedAction::Delete => "delete",
        UnifiedAction::Admin => "admin",
    }
}

/// Parse a declared category. Only the four class names are accepted
/// (ASCII case is ignored); anything else is `None`.
pub(crate) fn parse_category(category: &str) -> Option<UnifiedAction> {
    match category.trim().to_ascii_lowercase().as_str() {
        "read" => Some(UnifiedAction::Read),
        "write" => Some(UnifiedAction::Write),
        "delete" => Some(UnifiedAction::Delete),
        "admin" => Some(UnifiedAction::Admin),
        _ => None,
    }
}

/// Order classes by how much they can change: read < write < delete < admin.
pub(crate) fn strictness(class: UnifiedAction) -> u8 {
    match class {
        UnifiedAction::Read => 0,
        UnifiedAction::Write => 1,
        UnifiedAction::Delete => 2,
        UnifiedAction::Admin => 3,
    }
}

/// The class of a call. `None` when its catalog entry declares a category
/// that is not one of the four classes.
fn classify(call: &Call<'_>, declared: Option<&str>) -> Option<UnifiedAction> {
    let by_method = UnifiedAction::from_http_method(call.method);
    let Some(category) = declared.filter(|c| !c.trim().is_empty()) else {
        return Some(by_method);
    };
    let declared = parse_category(category)?;
    if call.dynamic && strictness(by_method) > strictness(declared) {
        Some(by_method)
    } else {
        Some(declared)
    }
}

/// The identifiers a call can be listed under: its catalog id, and its
/// normalized `METHOD:/path`.
fn operation_candidates(call: &Call<'_>, catalog_id: Option<&str>) -> Vec<String> {
    let path: Vec<&str> = call
        .path
        .split('/')
        .map(|segment| if segment.contains('{') { "{}" } else { segment })
        .collect();
    let mut candidates = vec![normalize_operation_format(&format!(
        "{}:{}",
        call.method,
        path.join("/")
    ))];
    if let Some(id) = catalog_id {
        candidates.push(id.to_string());
    }
    candidates
}

/// Whether a normalized list entry names a call candidate. A `*` segment in
/// the entry matches any one segment; a `*` in the candidate (a segment the
/// validator cannot see) matches only a `*` in the entry.
///
/// Both sides come from `normalize_operation_format`, which also turns the
/// empty segment before a leading `/` into `*` (an empty string is "all
/// digits"). The Cedar side depends on that form, so it is matched as is: the
/// leading `*` lines up on both sides.
fn operation_matches(entry: &str, candidate: &str) -> bool {
    if entry == candidate {
        return true;
    }
    let (Some((entry_method, entry_path)), Some((cand_method, cand_path))) =
        (entry.split_once(':'), candidate.split_once(':'))
    else {
        return false;
    };
    if entry_method != cand_method {
        return false;
    }
    let entry_segments: Vec<&str> = entry_path.split('/').collect();
    let cand_segments: Vec<&str> = cand_path.split('/').collect();
    entry_segments.len() == cand_segments.len()
        && entry_segments
            .iter()
            .zip(&cand_segments)
            .all(|(e, c)| *e == "*" || e == c)
}

fn strip_query(path: &str) -> &str {
    path.split(['?', '#']).next().unwrap_or(path)
}

fn normalize_blocked_path(pattern: &str) -> String {
    let lower = pattern.trim().to_ascii_lowercase();
    if lower.len() > 1 {
        lower.trim_end_matches('/').to_string()
    } else {
        lower
    }
}

fn blocked_path_matches(pattern: &str, path: &str) -> bool {
    if pattern.contains('*') {
        return glob_matches(pattern.as_bytes(), path.as_bytes());
    }
    if pattern == "/" {
        return true;
    }
    path == pattern
        || path
            .strip_prefix(pattern)
            .is_some_and(|rest| rest.starts_with('/'))
}

/// `*` matches any run of bytes, including `/`.
fn glob_matches(pattern: &[u8], text: &[u8]) -> bool {
    let (mut p, mut t) = (0, 0);
    let mut star: Option<(usize, usize)> = None;
    while t < text.len() {
        if p < pattern.len() && pattern[p] == b'*' {
            star = Some((p, t));
            p += 1;
        } else if p < pattern.len() && pattern[p] == text[t] {
            p += 1;
            t += 1;
        } else if let Some((star_p, star_t)) = star {
            p = star_p + 1;
            t = star_t + 1;
            star = Some((star_p, star_t + 1));
        } else {
            return false;
        }
    }
    pattern[p..].iter().all(|&b| b == b'*')
}

/// An [`HttpExecutor`](crate::HttpExecutor) that re-checks every request
/// against the static class policy once its path is resolved, then delegates.
///
/// Validation sees a dynamic path (`/items/${id}`) only as a template; this
/// wrapper sees the path the request actually goes to, so a run-time value
/// cannot carry a call into a class, blocked path or blocked operation the
/// policy refuses. A refusal is
/// [`ExecutionError::RequestRefused`](crate::ExecutionError::RequestRefused),
/// and its message never contains the resolved path.
#[cfg(feature = "js-runtime")]
pub struct ClassPolicyHttpExecutor<H> {
    inner: H,
    policy: std::sync::Arc<OpenApiClassPolicy>,
    registry: std::sync::Arc<OperationRegistry>,
}

#[cfg(feature = "js-runtime")]
impl<H> ClassPolicyHttpExecutor<H> {
    /// Wrap `inner` with the policy and catalog of `config`.
    pub fn new(inner: H, config: &CodeModeConfig) -> Self {
        Self {
            inner,
            policy: std::sync::Arc::new(OpenApiClassPolicy::from_config(config)),
            registry: std::sync::Arc::new(OperationRegistry::from_entries(&config.operations)),
        }
    }

    /// Wrap `inner` with an explicit policy and operation catalog — the
    /// executor counterpart of
    /// [`ValidationPipeline::with_openapi_class_policy`](crate::ValidationPipeline::with_openapi_class_policy).
    pub fn with_policy(
        inner: H,
        policy: OpenApiClassPolicy,
        operations: &[crate::config::OperationEntry],
    ) -> Self {
        Self {
            inner,
            policy: std::sync::Arc::new(policy),
            registry: std::sync::Arc::new(OperationRegistry::from_entries(operations)),
        }
    }

    /// The wrapped executor.
    pub fn inner(&self) -> &H {
        &self.inner
    }
}

#[cfg(feature = "js-runtime")]
impl<H: Clone> Clone for ClassPolicyHttpExecutor<H> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            policy: std::sync::Arc::clone(&self.policy),
            registry: std::sync::Arc::clone(&self.registry),
        }
    }
}

#[cfg(feature = "js-runtime")]
#[async_trait::async_trait]
impl<H: crate::HttpExecutor> crate::HttpExecutor for ClassPolicyHttpExecutor<H> {
    async fn execute_request(
        &self,
        method: &str,
        path: crate::ResolvedPath<'_>,
        body: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, crate::ExecutionError> {
        self.policy
            .check_request(method, path.as_str(), &self.registry)
            .map_err(|v| crate::ExecutionError::RequestRefused { message: v.message })?;
        self.inner.execute_request(method, path, body).await
    }

    fn placeholder_rules(
        &self,
        method: &str,
        path_template: &str,
        param: &str,
    ) -> crate::PlaceholderRules<'_> {
        self.inner.placeholder_rules(method, path_template, param)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::OperationEntry;
    use crate::javascript::JavaScriptValidator;

    fn script(code: &str) -> JavaScriptCodeInfo {
        JavaScriptValidator::default()
            .validate(code)
            .expect("script parses")
    }

    fn entry(id: &str, category: &str, path: &str) -> OperationEntry {
        OperationEntry {
            id: id.into(),
            category: category.into(),
            description: String::new(),
            path: Some(path.into()),
        }
    }

    fn rules(policy: &OpenApiClassPolicy, code: &str, registry: &OperationRegistry) -> Vec<String> {
        policy
            .check_script(&script(code), registry)
            .into_iter()
            .map(|v| v.rule)
            .collect()
    }

    fn set(items: &[&str]) -> HashSet<String> {
        items.iter().map(|s| (*s).to_string()).collect()
    }

    const GET: &str = "const r = await api.get('/items'); return r;";
    const POST: &str = "const r = await api.post('/items', {}); return r;";
    const DELETE: &str = "const r = await api.delete('/items/1'); return r;";

    #[test]
    fn defaults_allow_reads_and_deny_writes_and_deletes() {
        let policy = OpenApiClassPolicy::from_config(&CodeModeConfig::enabled());
        let reg = OperationRegistry::default();
        assert!(rules(&policy, GET, &reg).is_empty());
        assert_eq!(rules(&policy, POST, &reg), ["class_denied"]);
        assert_eq!(rules(&policy, DELETE, &reg), ["class_denied"]);
    }

    /// D2: the read switch was only used for auto-approval.
    #[test]
    fn reads_disabled_refuses_a_get_only_script() {
        let mut config = CodeModeConfig::enabled();
        config.openapi_reads_enabled = false;
        let policy = OpenApiClassPolicy::from_config(&config);
        let violations = policy.check_script(&script(GET), &OperationRegistry::default());
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].rule, "class_denied");
        assert!(violations[0]
            .message
            .contains("read operations are deny_all"));
    }

    /// D1: a non-empty allowlist used to skip the static check entirely.
    #[test]
    fn write_allowlist_admits_only_its_entries() {
        let mut config = CodeModeConfig::enabled();
        config.openapi_allow_writes = true;
        config.openapi_allowed_writes = set(&["POST /items"]);
        let policy = OpenApiClassPolicy::from_config(&config);
        let reg = OperationRegistry::default();
        assert!(rules(&policy, POST, &reg).is_empty());
        assert_eq!(
            rules(&policy, "await api.put('/items/7', {}); return 1;", &reg),
            ["not_in_allowlist"]
        );
        assert_eq!(
            rules(
                &policy,
                "await api.post('/admin/reset', {}); return 1;",
                &reg
            ),
            ["not_in_allowlist"]
        );
    }

    #[test]
    fn write_allowlist_without_allow_writes_is_deny_all() {
        let mut config = CodeModeConfig::enabled();
        config.openapi_allowed_writes = set(&["POST /items"]);
        let policy = OpenApiClassPolicy::from_config(&config);
        assert_eq!(policy.mode(UnifiedAction::Write), &ClassMode::DenyAll);
        assert_eq!(
            rules(&policy, POST, &OperationRegistry::default()),
            ["class_denied"]
        );
    }

    #[test]
    fn allowlist_matches_templates_and_catalog_ids() {
        let mut config = CodeModeConfig::enabled();
        config.openapi_allow_writes = true;
        config.openapi_allowed_writes = set(&["PUT:/items/{id}", "renameItem"]);
        let policy = OpenApiClassPolicy::from_config(&config);
        let reg = OperationRegistry::from_entries(&[entry(
            "renameItem",
            "write",
            "POST /items/{id}/rename",
        )]);
        assert!(rules(&policy, "await api.put('/items/7', {}); return 1;", &reg).is_empty());
        assert!(rules(
            &policy,
            "const id = 3; await api.put(`/items/${id}`, {}); return 1;",
            &reg
        )
        .is_empty());
        assert!(rules(
            &policy,
            "await api.post('/items/9/rename', {}); return 1;",
            &reg
        )
        .is_empty());
        // A template entry admits a literal, non-numeric segment.
        assert!(rules(&policy, "await api.put('/items/abc', {}); return 1;", &reg).is_empty());
        assert_eq!(
            rules(
                &policy,
                "await api.put('/items/7/owner', {}); return 1;",
                &reg
            ),
            ["not_in_allowlist"]
        );
    }

    /// Deletes used to be governed by `openapi_allow_writes` under Noop.
    #[test]
    fn deletes_are_their_own_class() {
        let mut config = CodeModeConfig::enabled();
        config.openapi_allow_writes = true;
        let policy = OpenApiClassPolicy::from_config(&config);
        let reg = OperationRegistry::default();
        assert!(rules(&policy, POST, &reg).is_empty());
        assert_eq!(rules(&policy, DELETE, &reg), ["class_denied"]);

        let mut config = CodeModeConfig::enabled();
        config.openapi_allow_deletes = true;
        let policy = OpenApiClassPolicy::from_config(&config);
        assert!(rules(&policy, DELETE, &reg).is_empty());
        assert_eq!(rules(&policy, POST, &reg), ["class_denied"]);
    }

    #[test]
    fn delete_allowlist_admits_only_its_entries() {
        let mut config = CodeModeConfig::enabled();
        config.openapi_allow_deletes = true;
        config.openapi_allowed_deletes = set(&["DELETE /items/{id}"]);
        let policy = OpenApiClassPolicy::from_config(&config);
        let reg = OperationRegistry::default();
        assert!(rules(&policy, DELETE, &reg).is_empty());
        assert_eq!(
            rules(&policy, "await api.delete('/users/1'); return 1;", &reg),
            ["not_in_allowlist"]
        );
    }

    #[test]
    fn catalog_reclassification_changes_the_verdict() {
        let policy = OpenApiClassPolicy::from_config(&CodeModeConfig::enabled());
        let search = "await api.post('/search', {q: 'x'}); return 1;";
        let reg = OperationRegistry::default();
        assert_eq!(rules(&policy, search, &reg), ["class_denied"]);

        let reg = OperationRegistry::from_entries(&[entry("search", "read", "/search")]);
        assert!(rules(&policy, search, &reg).is_empty());

        let reg = OperationRegistry::from_entries(&[entry("listItems", "admin", "GET /items")]);
        let violations = policy.check_script(&script(GET), &reg);
        assert_eq!(violations.len(), 1);
        assert!(violations[0]
            .message
            .contains("admin operations are deny_all"));
    }

    #[test]
    fn admin_is_deny_all_even_when_writes_and_deletes_are_allowed() {
        let mut config = CodeModeConfig::enabled();
        config.openapi_allow_writes = true;
        config.openapi_allow_deletes = true;
        let policy = OpenApiClassPolicy::from_config(&config);
        let reg = OperationRegistry::from_entries(&[entry("reset", "admin", "/reset")]);
        let violations =
            policy.check_script(&script("await api.post('/reset', {}); return 1;"), &reg);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].rule, "class_denied");
        assert!(
            violations[0].message.contains("is an admin operation"),
            "{}",
            violations[0].message
        );
    }

    #[test]
    fn allowlist_refusal_uses_the_right_article() {
        let policy = OpenApiClassPolicy::from_config(&CodeModeConfig::enabled())
            .with_mode(UnifiedAction::Admin, ClassMode::Allowlist(set(&["other"])));
        let reg = OperationRegistry::from_entries(&[entry("reset", "admin", "/reset")]);
        let violations =
            policy.check_script(&script("await api.post('/reset', {}); return 1;"), &reg);
        assert_eq!(violations[0].rule, "not_in_allowlist");
        assert!(
            violations[0].message.contains("is an admin operation"),
            "{}",
            violations[0].message
        );
    }

    #[test]
    fn unknown_category_fails_closed() {
        let policy = OpenApiClassPolicy::from_config(&CodeModeConfig::enabled());
        let reg = OperationRegistry::from_entries(&[entry("listItems", "reed", "/items")]);
        assert_eq!(rules(&policy, GET, &reg), ["unknown_category"]);
    }

    #[test]
    fn dynamic_path_cannot_be_relaxed_by_the_catalog() {
        let policy = OpenApiClassPolicy::from_config(&CodeModeConfig::enabled());
        let reg =
            OperationRegistry::from_entries(&[entry("lookup", "read", "POST /lookup/{kind}")]);
        assert!(rules(&policy, "await api.post('/lookup/a', {}); return 1;", &reg).is_empty());
        assert_eq!(
            rules(
                &policy,
                "const k = 'a'; await api.post(`/lookup/${k}`, {}); return 1;",
                &reg
            ),
            ["class_denied"]
        );
    }

    #[test]
    fn catalog_entry_with_a_method_matches_only_that_method() {
        let mut config = CodeModeConfig::enabled();
        config.openapi_allow_writes = true;
        let policy = OpenApiClassPolicy::from_config(&config);
        let reg = OperationRegistry::from_entries(&[entry("wipe", "admin", "POST /items")]);
        assert!(rules(&policy, GET, &reg).is_empty());
        assert_eq!(rules(&policy, POST, &reg), ["class_denied"]);
    }

    #[test]
    fn blocked_writes_block_methods_and_operations_in_any_class() {
        let mut config = CodeModeConfig::enabled();
        config.openapi_allow_writes = true;
        config.openapi_blocked_writes = set(&["PATCH", "POST /items", "GET:/secrets"]);
        let policy = OpenApiClassPolicy::from_config(&config);
        let reg = OperationRegistry::default();
        assert_eq!(
            rules(&policy, "await api.patch('/x', {}); return 1;", &reg),
            ["blocked_method"]
        );
        assert_eq!(rules(&policy, POST, &reg), ["blocked_operation"]);
        assert_eq!(
            rules(&policy, "await api.get('/secrets'); return 1;", &reg),
            ["blocked_operation"]
        );
        assert!(rules(&policy, "await api.put('/items', {}); return 1;", &reg).is_empty());
    }

    #[test]
    fn blocked_paths_block_reads_and_cover_subtrees() {
        let mut config = CodeModeConfig::enabled();
        config.openapi_blocked_paths = set(&["/admin", "/users/*/secrets"]);
        let policy = OpenApiClassPolicy::from_config(&config);
        let reg = OperationRegistry::default();
        for path in ["/admin", "/ADMIN/users", "/users/4/secrets"] {
            let code = format!("await api.get('{path}'); return 1;");
            assert_eq!(rules(&policy, &code, &reg), ["blocked_path"], "{path}");
        }
        for path in ["/administrators", "/users/4", "/items"] {
            let code = format!("await api.get('{path}'); return 1;");
            assert!(rules(&policy, &code, &reg).is_empty(), "{path}");
        }
    }

    #[test]
    fn blocked_paths_refuse_dynamic_paths_that_could_reach_them() {
        let mut config = CodeModeConfig::enabled();
        config.openapi_blocked_paths = set(&["/users/*/secrets"]);
        let policy = OpenApiClassPolicy::from_config(&config);
        let reg = OperationRegistry::default();
        assert_eq!(
            rules(
                &policy,
                "const i = 1; await api.get(`/users/${i}`); return 1;",
                &reg
            ),
            ["blocked_path"]
        );
        assert_eq!(
            rules(&policy, "const p = '/x'; await api.get(p); return 1;", &reg),
            ["blocked_path"]
        );
        assert!(rules(
            &policy,
            "const i = 1; await api.get(`/items/${i}`); return 1;",
            &reg
        )
        .is_empty());
    }

    #[test]
    fn every_refused_call_is_reported() {
        let policy = OpenApiClassPolicy::from_config(&CodeModeConfig::enabled());
        let code =
            "await api.post('/a', {}); await api.get('/b'); await api.delete('/c'); return 1;";
        assert_eq!(
            rules(&policy, code, &OperationRegistry::default()),
            ["class_denied", "class_denied"]
        );
    }

    #[test]
    fn check_request_classifies_the_resolved_path() {
        let policy = OpenApiClassPolicy::from_config(&CodeModeConfig::enabled());
        let reg = OperationRegistry::from_entries(&[
            entry("lookup", "read", "POST /lookup/{kind}"),
            entry("purge", "admin", "POST /lookup/purge"),
        ]);
        assert!(policy.check_request("post", "/lookup/a?x=1", &reg).is_ok());
        let refused = policy
            .check_request("POST", "/lookup/purge", &reg)
            .unwrap_err();
        assert!(refused.message.contains("admin"));
        assert_eq!(
            policy.check_request("TRACE", "/x", &reg).unwrap_err().rule,
            "unknown_method"
        );
    }

    #[cfg(feature = "js-runtime")]
    #[derive(Clone)]
    struct Echo;

    #[cfg(feature = "js-runtime")]
    #[async_trait::async_trait]
    impl crate::HttpExecutor for Echo {
        async fn execute_request(
            &self,
            method: &str,
            path: crate::ResolvedPath<'_>,
            _body: Option<serde_json::Value>,
        ) -> Result<serde_json::Value, crate::ExecutionError> {
            Ok(serde_json::json!(format!("{method} {}", path.as_str())))
        }
    }

    /// The validator passes `PUT /items/${id}` against an allowlisted
    /// template; the run-time check refuses the value that leaves it.
    #[cfg(feature = "js-runtime")]
    #[tokio::test]
    async fn executor_refuses_a_resolved_path_outside_the_allowlist() {
        use crate::HttpExecutor;
        let mut config = CodeModeConfig::enabled();
        config.openapi_allow_writes = true;
        config.openapi_allowed_writes = set(&["PUT /items/{id}"]);
        let executor = ClassPolicyHttpExecutor::new(Echo, &config);

        let ok = crate::ResolvedPath::from_checked("/items/7").unwrap();
        assert_eq!(
            executor.execute_request("PUT", ok, None).await.unwrap(),
            serde_json::json!("PUT /items/7")
        );

        let escaped = crate::ResolvedPath::from_checked("/items/7/owner").unwrap();
        match executor.execute_request("PUT", escaped, None).await {
            Err(crate::ExecutionError::RequestRefused { message }) => {
                assert!(message.contains("write"), "{message}");
                assert!(
                    !message.contains("owner"),
                    "refusal must not echo the path: {message}"
                );
            },
            other => panic!("expected RequestRefused, got {other:?}"),
        }

        let read = crate::ResolvedPath::from_checked("/items/7").unwrap();
        assert!(executor.execute_request("GET", read, None).await.is_ok());
    }

    /// Every refusal names the call by method and line, never by path.
    #[test]
    fn violations_never_echo_the_path() {
        let mut config = CodeModeConfig::enabled();
        config.openapi_reads_enabled = false;
        config.openapi_blocked_paths = set(&["/blocked"]);
        config.openapi_blocked_writes = set(&["PUT /x/SECRET"]);
        config.openapi_allow_writes = true;
        config.openapi_allowed_writes = set(&["POST /only"]);
        let policy = OpenApiClassPolicy::from_config(&config);
        let reg = OperationRegistry::from_entries(&[entry("odd", "reed", "/odd/{id}")]);
        let code = "await api.get('/x/SECRET');\n\
                    await api.get('/blocked/SECRET');\n\
                    await api.put('/x/SECRET', {});\n\
                    await api.post('/x/SECRET', {});\n\
                    await api.post('/odd/SECRET', {});\n\
                    return 1;";
        let violations = policy.check_script(&script(code), &reg);
        let rules: Vec<&str> = violations.iter().map(|v| v.rule.as_str()).collect();
        assert_eq!(
            rules,
            [
                "class_denied",
                "blocked_path",
                "blocked_operation",
                "not_in_allowlist",
                "unknown_category"
            ]
        );
        for v in &violations {
            assert!(!v.message.contains("SECRET"), "{}", v.message);
            assert!(
                !v.suggestion.as_deref().unwrap_or("").contains("SECRET"),
                "{:?}",
                v.suggestion
            );
        }
        assert!(violations[0].message.contains("GET call on line 1"));
        assert!(violations[4].message.contains("operation 'odd'"));
    }

    #[test]
    fn builder_sets_modes_the_legacy_keys_cannot_express() {
        let policy = OpenApiClassPolicy::from_config(&CodeModeConfig::enabled())
            .with_mode(
                UnifiedAction::Read,
                ClassMode::Allowlist(set(&["GET /items", "searchConcepts"])),
            )
            .with_mode(UnifiedAction::Admin, ClassMode::AllowAll)
            .with_blocked_operations(["PATCH"])
            .with_blocked_paths(["/internal/"]);
        let reg = OperationRegistry::from_entries(&[
            entry("searchConcepts", "read", "POST /search"),
            entry("reindex", "admin", "POST /reindex"),
        ]);
        assert!(rules(&policy, GET, &reg).is_empty());
        assert!(rules(&policy, "await api.post('/search', {}); return 1;", &reg).is_empty());
        assert_eq!(
            rules(&policy, "await api.get('/other'); return 1;", &reg),
            ["not_in_allowlist"]
        );
        assert!(rules(&policy, "await api.post('/reindex', {}); return 1;", &reg).is_empty());
        assert_eq!(
            rules(&policy, "await api.get('/internal/x'); return 1;", &reg),
            ["blocked_path"]
        );
        assert_eq!(
            policy.to_string(),
            "read=allowlist write=deny_all delete=deny_all admin=allow_all \
             blocked_operations=1 blocked_paths=1"
        );
    }

    #[test]
    fn glob_matching() {
        assert!(glob_matches(b"/a/*", b"/a/b/c"));
        assert!(glob_matches(b"*secret*", b"/x/secrets"));
        assert!(!glob_matches(b"/a/*/c", b"/a/b/d"));
        assert!(glob_matches(b"/a/*/c", b"/a/b/c"));
    }
}
