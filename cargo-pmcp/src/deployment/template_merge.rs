//! Post-render `CloudFormation` template merges shared by the AWS deploy
//! targets: `[server]` sizing onto the MCP function.
//!
//! Lifted out of `targets::pmcp_run::deploy` (debug session
//! `cargo-pmcp-deploy-targets`, #13) so the `aws-lambda` target's native
//! `CloudFormation` engine applies the same merge to the template
//! `pmcp-cfn-renderer` produces, instead of pinning the renderer's memory
//! constant. The `pmcp-run` target applies it after either synth engine
//! (debug session `deploy-server-memory-timeout`).
//!
//! # Which resource is targeted
//!
//! ONLY `AWS::Lambda::Function` resources whose `Properties.FunctionName`
//! equals the configured server name. An OAuth-enabled stack renders THREE
//! Lambdas at three different sizings (`<name>-oauth-proxy` 256/30, `<name>`
//! 512/30, `<name>-authorizer` 256/**10**), and resizing the authorizer would
//! reconfigure infrastructure the operator never mentioned. Both synth engines
//! set `FunctionName` the same way (the renderer via `d.server.name`, the TS
//! scaffold via `functionName`), so the match is exact on either. Logical IDs
//! are deliberately not used: they are CDK-generated and unknowable for
//! hand-authored or shared constructs.
//!
//! # Precedence
//!
//! A declared value OVERRIDES whatever the template carries. `memorySize` is a
//! scalar every construct sets, so "the template wins" would mean "the config
//! is inert", which is the defect this exists to fix. `None` leaves that
//! property exactly as synthesized.

use anyhow::{Context, Result};

use crate::deployment::config::ServerConfig;

/// The `[server]` sizing to merge into the MCP function. `None` leaves the
/// property as synthesized.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LambdaSizing {
    pub memory_mb: Option<u32>,
    pub timeout_seconds: Option<u32>,
    pub ephemeral_storage_mb: Option<u32>,
}

impl LambdaSizing {
    /// The sizing `server` declares.
    pub fn from_server(server: &ServerConfig) -> Self {
        Self {
            memory_mb: server.memory_mb,
            timeout_seconds: server.timeout_seconds,
            ephemeral_storage_mb: server.ephemeral_storage_mb,
        }
    }

    /// `true` when nothing is declared, so the merge is a no-op.
    pub fn is_empty(&self) -> bool {
        self.memory_mb.is_none()
            && self.timeout_seconds.is_none()
            && self.ephemeral_storage_mb.is_none()
    }
}

/// True when `resource` has `Type == "AWS::Lambda::Function"`.
pub fn is_lambda_function(resource: &serde_json::Value) -> bool {
    resource.get("Type").and_then(serde_json::Value::as_str) == Some("AWS::Lambda::Function")
}

/// True when `resource` is an `AWS::Lambda::Function` whose
/// `Properties.FunctionName` equals `function_name` — the MCP function, as
/// opposed to an OAuth proxy or authorizer sharing the same stack.
pub fn is_mcp_lambda_function(resource: &serde_json::Value, function_name: &str) -> bool {
    is_lambda_function(resource)
        && resource
            .get("Properties")
            .and_then(|p| p.get("FunctionName"))
            .and_then(serde_json::Value::as_str)
            == Some(function_name)
}

/// Get or create a JSON object at `key` within `parent`, returning a mutable
/// reference to it. Returns `None` only when an existing non-object value
/// occupies `key` (a non-object is never clobbered).
pub fn ensure_object<'a>(
    parent: &'a mut serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Option<&'a mut serde_json::Map<String, serde_json::Value>> {
    parent
        .entry(key.to_string())
        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()))
        .as_object_mut()
}

/// Outcome of [`merge_sizing_into_template`].
#[derive(Debug)]
pub struct SizingMergeOutcome {
    /// The re-serialized template JSON with the declared sizing applied.
    pub template: String,
    /// One human-readable line per property that moved, naming the
    /// before/after values (e.g. `"McpFunction: MemorySize 256 -> 1024"`).
    /// A property already carrying the declared value produces no entry, so an
    /// idempotent re-deploy stays quiet about it.
    pub changes: Vec<String>,
    /// `true` when at least one `AWS::Lambda::Function` whose `FunctionName`
    /// equals the configured server name was found. Distinct from `changes`
    /// being non-empty: a matched-but-already-correct template must NOT trip
    /// the fail-loud path.
    pub matched: bool,
}

/// Merge the declared `[server]` sizing into `template` and say what happened.
///
/// Thin deploy-time wrapper around the pure [`merge_sizing_into_template`]: it
/// prints a per-property before/after line, or the fail-loud "no matching
/// Lambda" warning, and returns the (possibly modified) template. When
/// nothing is declared the template is returned unchanged, byte for byte, and
/// nothing is printed.
pub fn apply_sizing_merge(
    template: String,
    function_name: &str,
    sizing: LambdaSizing,
) -> Result<String> {
    if sizing.is_empty() {
        return Ok(template);
    }

    let outcome = merge_sizing_into_template(&template, function_name, sizing)?;

    if outcome.matched {
        if outcome.changes.is_empty() {
            println!("   ✅ [server] sizing already matches the synthesized template");
        } else {
            for change in &outcome.changes {
                println!("   ✅ Applied [server] sizing — {change}");
            }
        }
    } else {
        // Fail-loud: sizing was declared but no Lambda in the template carries
        // this server's FunctionName, so there is nothing to apply it to.
        eprintln!("{}", sizing_no_lambda_warning(function_name, sizing));
    }

    Ok(outcome.template)
}

/// Rewrite `Properties.MemorySize`, `Properties.Timeout` and
/// `Properties.EphemeralStorage.Size` on the MCP function (see the module
/// docs for which resource that is). Pure: no synth, no I/O.
pub fn merge_sizing_into_template(
    template_json: &str,
    function_name: &str,
    sizing: LambdaSizing,
) -> Result<SizingMergeOutcome> {
    let mut template: serde_json::Value = serde_json::from_str(template_json)
        .context("Failed to parse synthesized CloudFormation template JSON")?;

    let mut changes: Vec<String> = Vec::new();
    let mut matched = false;

    if let Some(resources) = template
        .get_mut("Resources")
        .and_then(serde_json::Value::as_object_mut)
    {
        for (logical_id, resource) in resources.iter_mut() {
            if !is_mcp_lambda_function(resource, function_name) {
                continue;
            }
            matched = true;
            apply_sizing_to_lambda(resource, logical_id, sizing, &mut changes);
        }
    }

    changes.sort();

    let merged = serde_json::to_string_pretty(&template)
        .context("Failed to re-serialize merged CloudFormation template")?;

    Ok(SizingMergeOutcome {
        template: merged,
        changes,
        matched,
    })
}

/// Write the declared sizing onto one matched Lambda resource, appending a
/// `"<logical id>: <Property> <before> -> <after>"` line to `changes` for each
/// property whose value actually moved. Creates `Properties` if absent.
fn apply_sizing_to_lambda(
    resource: &mut serde_json::Value,
    logical_id: &str,
    sizing: LambdaSizing,
    changes: &mut Vec<String>,
) {
    let Some(properties) = resource
        .as_object_mut()
        .and_then(|r| ensure_object(r, "Properties"))
    else {
        return;
    };

    for (path, declared) in [
        ("MemorySize", sizing.memory_mb),
        ("Timeout", sizing.timeout_seconds),
        ("EphemeralStorage.Size", sizing.ephemeral_storage_mb),
    ] {
        let Some(declared) = declared else { continue };
        let before = read_property(properties, path);
        if before == Some(u64::from(declared)) {
            continue;
        }
        if write_property(properties, path, declared) {
            let before_label = before.map_or_else(|| "(unset)".to_string(), |v| v.to_string());
            changes.push(format!("{logical_id}: {path} {before_label} -> {declared}"));
        }
    }
}

/// The integer at `path` under `Properties`: `"Key"`, or `"Outer.Inner"` for a
/// nested object such as `EphemeralStorage.Size`.
fn read_property(
    properties: &serde_json::Map<String, serde_json::Value>,
    path: &str,
) -> Option<u64> {
    match path.split_once('.') {
        Some((outer, inner)) => properties.get(outer)?.get(inner)?.as_u64(),
        None => properties.get(path)?.as_u64(),
    }
}

/// Set the integer at `path` (see [`read_property`]), creating a nested
/// object when absent. Returns `false`, writing nothing, when a non-object
/// occupies the outer key.
fn write_property(
    properties: &mut serde_json::Map<String, serde_json::Value>,
    path: &str,
    value: u32,
) -> bool {
    if let Some((outer, inner)) = path.split_once('.') {
        ensure_object(properties, outer).is_some_and(|object| {
            object.insert(inner.to_string(), serde_json::Value::from(value));
            true
        })
    } else {
        properties.insert(path.to_string(), serde_json::Value::from(value));
        true
    }
}

/// The keys of `expected` that the MCP function (see the module docs) does
/// NOT carry with exactly that value in `Environment.Variables`, sorted. Empty
/// when the template has no MCP function (the sizing merge already says so).
pub fn environment_not_carried(
    template_json: &str,
    function_name: &str,
    expected: &std::collections::BTreeMap<String, String>,
) -> Result<Vec<String>> {
    let template: serde_json::Value = serde_json::from_str(template_json)
        .context("Failed to parse rendered CloudFormation template JSON")?;
    let Some(function) = template
        .get("Resources")
        .and_then(serde_json::Value::as_object)
        .and_then(|resources| {
            resources
                .values()
                .find(|resource| is_mcp_lambda_function(resource, function_name))
        })
    else {
        return Ok(Vec::new());
    };
    let variables = function.pointer("/Properties/Environment/Variables");
    Ok(expected
        .iter()
        .filter(|(key, value)| {
            variables
                .and_then(|vars| vars.get(key.as_str()))
                .and_then(serde_json::Value::as_str)
                != Some(value.as_str())
        })
        .map(|(key, _)| key.clone())
        .collect())
}

/// The fail-loud warning shown when `[server]` sizing is declared but the
/// template contains no `AWS::Lambda::Function` whose `FunctionName` matches
/// the configured server name.
pub fn sizing_no_lambda_warning(function_name: &str, sizing: LambdaSizing) -> String {
    let declared: Vec<String> = [
        ("memory_mb", sizing.memory_mb),
        ("timeout_seconds", sizing.timeout_seconds),
        ("ephemeral_storage_mb", sizing.ephemeral_storage_mb),
    ]
    .into_iter()
    .filter_map(|(key, value)| value.map(|v| format!("{key} = {v}")))
    .collect();
    let declared = declared.join(", ");
    format!(
        "⚠️  [server] sizing declared but NOT applied — the synthesized CloudFormation \
         template contains no AWS::Lambda::Function whose FunctionName is \
         '{function_name}'.\n     \
         Declared: {declared}\n     \
         Check that [server] name matches the function your stack.ts creates; otherwise the \
         deployed function keeps whatever size the template hardcodes."
    )
}

#[cfg(test)]
mod tests {
    //! Moved from `targets::pmcp_run::deploy::sizing_merge_tests` with the
    //! merge (debug session `deploy-server-memory-timeout`), plus the
    //! ephemeral-storage cases (debug session `cargo-pmcp-deploy-targets`).
    use super::*;
    use serde_json::{json, Value};

    const SERVER: &str = "okf-demo";

    /// A single-Lambda pmcp-run template as the scaffold/`cdk synth` engine
    /// emits it: `memorySize: 256`, `timeout: cdk.Duration.seconds(30)`.
    fn scaffold_template() -> String {
        json!({
            "Resources": {
                "McpFunction": {
                    "Type": "AWS::Lambda::Function",
                    "Properties": {
                        "FunctionName": SERVER,
                        "MemorySize": 256,
                        "Timeout": 30,
                        "Runtime": "provided.al2023"
                    }
                }
            }
        })
        .to_string()
    }

    fn props(v: &Value, logical_id: &str) -> Value {
        v["Resources"][logical_id]["Properties"].clone()
    }

    fn sizing(memory_mb: Option<u32>, timeout_seconds: Option<u32>) -> LambdaSizing {
        LambdaSizing {
            memory_mb,
            timeout_seconds,
            ephemeral_storage_mb: None,
        }
    }

    fn merged(template: &str, sizing: LambdaSizing) -> (Value, Vec<String>, bool) {
        let out = merge_sizing_into_template(template, SERVER, sizing)
            .expect("merge must parse and re-serialize valid template JSON");
        let parsed: Value =
            serde_json::from_str(&out.template).expect("merged template must be valid JSON");
        (parsed, out.changes, out.matched)
    }

    /// (1) No-op when nothing is declared: `apply_sizing_merge` returns the
    /// template BYTE-IDENTICALLY rather than round-tripping it through serde.
    ///
    /// This is the case that protects the installed base. The sizing keys are
    /// `Option` precisely so a `deploy.toml` that never mentioned sizing leaves
    /// the engine's own default alone.
    #[test]
    fn no_op_when_nothing_declared() {
        let template = scaffold_template();
        let out = apply_sizing_merge(template.clone(), SERVER, LambdaSizing::default())
            .expect("no-op merge succeeds");
        assert_eq!(
            out, template,
            "an undeclared sizing must return the template untouched, byte for byte"
        );
    }

    /// (2) Declared sizing lands on the MCP function's `MemorySize`/`Timeout`.
    #[test]
    fn applies_declared_memory_and_timeout() {
        let (parsed, changes, matched) = merged(&scaffold_template(), sizing(Some(1024), Some(60)));
        assert!(matched, "the MCP function must be matched");
        assert_eq!(props(&parsed, "McpFunction")["MemorySize"], json!(1024));
        assert_eq!(props(&parsed, "McpFunction")["Timeout"], json!(60));
        assert_eq!(
            changes,
            vec![
                "McpFunction: MemorySize 256 -> 1024".to_string(),
                "McpFunction: Timeout 30 -> 60".to_string(),
            ],
            "both moves must be reported with before/after values"
        );
    }

    /// (3) A matched Lambda missing the properties gets them created.
    #[test]
    fn creates_properties_when_absent() {
        let template = json!({
            "Resources": {
                "Fn": {
                    "Type": "AWS::Lambda::Function",
                    "Properties": { "FunctionName": SERVER }
                }
            }
        })
        .to_string();

        let (parsed, changes, matched) = merged(&template, sizing(Some(1024), Some(60)));
        assert!(matched);
        assert_eq!(props(&parsed, "Fn")["MemorySize"], json!(1024));
        assert_eq!(props(&parsed, "Fn")["Timeout"], json!(60));
        assert_eq!(
            changes,
            vec![
                "Fn: MemorySize (unset) -> 1024".to_string(),
                "Fn: Timeout (unset) -> 60".to_string(),
            ],
            "an absent property must report `(unset)` as its before value"
        );
    }

    /// (4) Every other property on the matched Lambda is preserved.
    #[test]
    fn other_properties_preserved() {
        let template = json!({
            "Resources": {
                "McpFunction": {
                    "Type": "AWS::Lambda::Function",
                    "Properties": {
                        "FunctionName": SERVER,
                        "MemorySize": 256,
                        "Timeout": 30,
                        "Runtime": "provided.al2023",
                        "Handler": "bootstrap",
                        "Environment": { "Variables": { "RUST_LOG": "info" } }
                    }
                }
            }
        })
        .to_string();

        let (parsed, _, _) = merged(&template, sizing(Some(1024), None));
        let p = props(&parsed, "McpFunction");
        assert_eq!(p["Runtime"], json!("provided.al2023"));
        assert_eq!(p["Handler"], json!("bootstrap"));
        assert_eq!(
            p["Environment"],
            json!({ "Variables": { "RUST_LOG": "info" } })
        );
        assert_eq!(
            p["Timeout"],
            json!(30),
            "an undeclared property must be left exactly as synthesized"
        );
        assert!(
            p.get("EphemeralStorage").is_none(),
            "an undeclared ephemeral_storage_mb must not add EphemeralStorage"
        );
    }

    /// (5) THE REGRESSION GUARD. An OAuth-enabled stack renders THREE Lambdas
    /// at three DIFFERENT sizings — measured from
    /// `crates/pmcp-cfn-renderer/tests/goldens/oauth-cognito-dcr.golden.json`:
    /// `<name>-oauth-proxy` 256/30, `<name>` 512/30, `<name>-authorizer`
    /// 256/**10**. Matching on `Type` alone would resize the 10-second
    /// authorizer — a real regression, not a fix.
    #[test]
    fn discriminates_the_mcp_function_in_a_three_lambda_oauth_stack() {
        let template = json!({
            "Resources": {
                "OAuthProxy": {
                    "Type": "AWS::Lambda::Function",
                    "Properties": {
                        "FunctionName": format!("{SERVER}-oauth-proxy"),
                        "MemorySize": 256, "Timeout": 30
                    }
                },
                "McpFunction": {
                    "Type": "AWS::Lambda::Function",
                    "Properties": {
                        "FunctionName": SERVER,
                        "MemorySize": 512, "Timeout": 30
                    }
                },
                "Authorizer": {
                    "Type": "AWS::Lambda::Function",
                    "Properties": {
                        "FunctionName": format!("{SERVER}-authorizer"),
                        "MemorySize": 256, "Timeout": 10
                    }
                }
            }
        })
        .to_string();

        let all = LambdaSizing {
            memory_mb: Some(3008),
            timeout_seconds: Some(900),
            ephemeral_storage_mb: Some(2048),
        };
        let (parsed, changes, matched) = merged(&template, all);
        assert!(matched);
        assert_eq!(
            changes,
            vec![
                "McpFunction: EphemeralStorage.Size (unset) -> 2048".to_string(),
                "McpFunction: MemorySize 512 -> 3008".to_string(),
                "McpFunction: Timeout 30 -> 900".to_string(),
            ],
            "ONLY the MCP function may be reported as changed"
        );

        assert_eq!(props(&parsed, "McpFunction")["MemorySize"], json!(3008));
        assert_eq!(props(&parsed, "McpFunction")["Timeout"], json!(900));
        assert_eq!(
            props(&parsed, "McpFunction")["EphemeralStorage"],
            json!({ "Size": 2048 })
        );

        assert_eq!(
            props(&parsed, "OAuthProxy"),
            json!({ "FunctionName": format!("{SERVER}-oauth-proxy"), "MemorySize": 256, "Timeout": 30 }),
            "the OAuth proxy must be byte-preserved"
        );
        assert_eq!(
            props(&parsed, "Authorizer"),
            json!({ "FunctionName": format!("{SERVER}-authorizer"), "MemorySize": 256, "Timeout": 10 }),
            "the 10-second authorizer must be byte-preserved"
        );
    }

    /// (6) Non-Lambda resources are never touched.
    #[test]
    fn non_lambda_resources_untouched() {
        let template = json!({
            "Resources": {
                "McpFunction": {
                    "Type": "AWS::Lambda::Function",
                    "Properties": { "FunctionName": SERVER, "MemorySize": 256, "Timeout": 30 }
                },
                "ClientsTable": {
                    "Type": "AWS::DynamoDB::Table",
                    "Properties": { "TableName": SERVER, "MemorySize": 1 }
                }
            }
        })
        .to_string();

        let (parsed, _, _) = merged(&template, sizing(Some(1024), Some(60)));
        assert_eq!(
            props(&parsed, "ClientsTable"),
            json!({ "TableName": SERVER, "MemorySize": 1 }),
            "a non-Lambda resource must be byte-preserved even when its own \
             properties collide by name"
        );
    }

    /// (7) Fail-loud: sizing declared, but no Lambda carries this server's
    /// `FunctionName`. `matched` stays false (the caller's warning trigger) and
    /// the warning names the server and the declared values.
    #[test]
    fn fail_loud_when_no_matching_lambda() {
        let template = json!({
            "Resources": {
                "SomeoneElse": {
                    "Type": "AWS::Lambda::Function",
                    "Properties": { "FunctionName": "a-different-server", "MemorySize": 256 }
                }
            }
        })
        .to_string();

        let declared = LambdaSizing {
            memory_mb: Some(1024),
            timeout_seconds: Some(60),
            ephemeral_storage_mb: Some(2048),
        };
        let (parsed, changes, matched) = merged(&template, declared);
        assert!(
            !matched,
            "no FunctionName match must yield matched = false (fail-loud trigger)"
        );
        assert_eq!(changes, Vec::<String>::new());
        assert_eq!(
            props(&parsed, "SomeoneElse")["MemorySize"],
            json!(256),
            "the non-matching Lambda must be left alone"
        );

        let warning = sizing_no_lambda_warning(SERVER, declared);
        assert!(warning.contains("NOT applied"), "warning is prominent");
        assert!(
            warning.contains(SERVER),
            "warning names the expected function"
        );
        assert!(warning.contains("memory_mb = 1024"));
        assert!(warning.contains("timeout_seconds = 60"));
        assert!(warning.contains("ephemeral_storage_mb = 2048"));
    }

    /// (8) A partial declaration touches only the declared property.
    #[test]
    fn partial_declaration_leaves_the_other_property_alone() {
        let (parsed, changes, _) = merged(&scaffold_template(), sizing(None, Some(120)));
        assert_eq!(
            changes,
            vec!["McpFunction: Timeout 30 -> 120".to_string()],
            "only the declared property may be reported"
        );
        assert_eq!(
            props(&parsed, "McpFunction")["MemorySize"],
            json!(256),
            "an undeclared memory_mb must leave the engine's own value in place"
        );
        assert_eq!(props(&parsed, "McpFunction")["Timeout"], json!(120));
    }

    /// (9) Idempotent: a template already carrying the declared values is
    /// matched with ZERO changes.
    #[test]
    fn idempotent_when_already_correct() {
        let already = json!({
            "Resources": {
                "McpFunction": {
                    "Type": "AWS::Lambda::Function",
                    "Properties": {
                        "FunctionName": SERVER,
                        "MemorySize": 256,
                        "Timeout": 30,
                        "EphemeralStorage": { "Size": 1024 }
                    }
                }
            }
        })
        .to_string();
        let declared = LambdaSizing {
            memory_mb: Some(256),
            timeout_seconds: Some(30),
            ephemeral_storage_mb: Some(1024),
        };
        let (parsed, changes, matched) = merged(&already, declared);
        assert!(matched, "an already-correct template is still a MATCH");
        assert!(
            changes.is_empty(),
            "no property moved, so nothing may be reported as changed: {changes:?}"
        );
        assert_eq!(props(&parsed, "McpFunction")["MemorySize"], json!(256));
        assert_eq!(props(&parsed, "McpFunction")["Timeout"], json!(30));
    }

    /// (10) THE ENGINE-DIVERGENCE FIXTURE: the two synth engines used to emit
    /// different timeouts for the same deploy.toml (the renderer threaded the
    /// descriptor's value, the TS scaffold hardcoded 30); after the merge they
    /// must land on identical sizing.
    #[test]
    fn both_synth_engines_converge_on_the_declared_timeout() {
        let legacy_cdk = scaffold_template();
        let renderer = json!({
            "Resources": {
                "McpFunction": {
                    "Type": "AWS::Lambda::Function",
                    "Properties": {
                        "FunctionName": SERVER,
                        "MemorySize": 256,
                        "Timeout": 60,
                        "Runtime": "provided.al2023"
                    }
                }
            }
        })
        .to_string();

        let (from_legacy, legacy_changes, _) = merged(&legacy_cdk, sizing(Some(1024), Some(60)));
        let (from_renderer, renderer_changes, _) = merged(&renderer, sizing(Some(1024), Some(60)));

        assert_eq!(
            from_legacy, from_renderer,
            "the two synth engines must land on IDENTICAL sizing for the same deploy.toml"
        );
        assert!(
            legacy_changes.contains(&"McpFunction: Timeout 30 -> 60".to_string()),
            "the cdk-synth engine's hardcoded 30 must be corrected, got {legacy_changes:?}"
        );
        assert!(
            !renderer_changes
                .iter()
                .any(|c| c.starts_with("McpFunction: Timeout")),
            "the renderer already honored the descriptor timeout, got {renderer_changes:?}"
        );
    }

    /// (11) `is_mcp_lambda_function` matches on BOTH the CFN type and the
    /// `FunctionName` — neither alone is sufficient.
    #[test]
    fn is_mcp_lambda_function_requires_type_and_name() {
        assert!(is_mcp_lambda_function(
            &json!({ "Type": "AWS::Lambda::Function", "Properties": { "FunctionName": SERVER } }),
            SERVER
        ));
        assert!(!is_mcp_lambda_function(
            &json!({ "Type": "AWS::Lambda::Url", "Properties": { "FunctionName": SERVER } }),
            SERVER
        ));
        assert!(!is_mcp_lambda_function(
            &json!({ "Type": "AWS::Lambda::Function", "Properties": { "FunctionName": "other" } }),
            SERVER
        ));
        assert!(!is_mcp_lambda_function(
            &json!({ "Type": "AWS::Lambda::Function", "Properties": {} }),
            SERVER
        ));
    }

    /// (12) Invalid template JSON surfaces a parse error rather than silently
    /// dropping the merge.
    #[test]
    fn invalid_template_json_errors() {
        let err = merge_sizing_into_template("{ not valid json", SERVER, sizing(Some(1024), None))
            .expect_err("invalid JSON must error");
        assert!(
            err.to_string().contains("parse synthesized CloudFormation"),
            "error must name the parse failure"
        );
    }

    /// #16: a declared `ephemeral_storage_mb` lands on the MCP function's
    /// `EphemeralStorage.Size`, and a different existing size is replaced.
    #[test]
    fn applies_declared_ephemeral_storage() {
        let declared = LambdaSizing {
            ephemeral_storage_mb: Some(10_240),
            ..LambdaSizing::default()
        };
        let (parsed, changes, matched) = merged(&scaffold_template(), declared);
        assert!(matched);
        assert_eq!(
            props(&parsed, "McpFunction")["EphemeralStorage"],
            json!({ "Size": 10_240 })
        );
        assert_eq!(
            changes,
            vec!["McpFunction: EphemeralStorage.Size (unset) -> 10240".to_string()]
        );

        let resized = LambdaSizing {
            ephemeral_storage_mb: Some(2048),
            ..LambdaSizing::default()
        };
        let (again, changes, _) =
            merged(&serde_json::to_string(&parsed).expect("serialize"), resized);
        assert_eq!(
            props(&again, "McpFunction")["EphemeralStorage"],
            json!({ "Size": 2048 })
        );
        assert_eq!(
            changes,
            vec!["McpFunction: EphemeralStorage.Size 10240 -> 2048".to_string()]
        );
    }

    /// A non-object `EphemeralStorage` (never emitted by either engine) is left
    /// alone, and no change is reported for it.
    #[test]
    fn a_non_object_ephemeral_storage_is_not_clobbered() {
        let template = json!({
            "Resources": {
                "McpFunction": {
                    "Type": "AWS::Lambda::Function",
                    "Properties": { "FunctionName": SERVER, "EphemeralStorage": "odd" }
                }
            }
        })
        .to_string();
        let declared = LambdaSizing {
            ephemeral_storage_mb: Some(2048),
            ..LambdaSizing::default()
        };
        let (parsed, changes, matched) = merged(&template, declared);
        assert!(matched);
        assert!(changes.is_empty(), "{changes:?}");
        assert_eq!(
            props(&parsed, "McpFunction")["EphemeralStorage"],
            json!("odd")
        );
    }

    /// `environment_not_carried` names exactly the expected keys the MCP
    /// function does not carry with the expected value.
    #[test]
    fn environment_not_carried_names_overridden_and_missing_keys() {
        let template = json!({
            "Resources": {
                "McpFunction": {
                    "Type": "AWS::Lambda::Function",
                    "Properties": {
                        "FunctionName": SERVER,
                        "Environment": { "Variables": { "PORT": "8080", "A": "1" } }
                    }
                },
                "Other": {
                    "Type": "AWS::Lambda::Function",
                    "Properties": { "FunctionName": "other",
                                    "Environment": { "Variables": { "B": "2" } } }
                }
            }
        })
        .to_string();
        let expected = std::collections::BTreeMap::from([
            ("A".to_string(), "1".to_string()),
            ("B".to_string(), "2".to_string()),
            ("PORT".to_string(), "3000".to_string()),
        ]);
        assert_eq!(
            environment_not_carried(&template, SERVER, &expected).expect("parse"),
            vec!["B".to_string(), "PORT".to_string()]
        );
        assert_eq!(
            environment_not_carried(&template, "absent", &expected).expect("parse"),
            Vec::<String>::new()
        );
    }

    /// `ensure_object` never clobbers a non-object value.
    #[test]
    fn ensure_object_refuses_to_clobber_a_non_object() {
        let mut parent = serde_json::Map::new();
        parent.insert("Properties".to_string(), json!("not-an-object"));
        assert!(ensure_object(&mut parent, "Properties").is_none());
        assert_eq!(parent["Properties"], json!("not-an-object"));
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;
    use serde_json::json;

    fn opt_size() -> impl Strategy<Value = Option<u32>> {
        prop::option::of(128u32..=10_240)
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(128))]

        /// Whatever is declared is what the MCP function carries afterwards;
        /// whatever is not declared is left exactly as it was; the other
        /// Lambda in the stack is never touched.
        #[test]
        fn declared_sizing_lands_and_nothing_else_moves(
            memory in opt_size(),
            timeout in prop::option::of(1u32..=900),
            ephemeral in opt_size(),
        ) {
            let template = json!({
                "Resources": {
                    "Mcp": { "Type": "AWS::Lambda::Function",
                             "Properties": { "FunctionName": "srv", "MemorySize": 512, "Timeout": 30 } },
                    "Other": { "Type": "AWS::Lambda::Function",
                               "Properties": { "FunctionName": "srv-authorizer", "MemorySize": 256, "Timeout": 10 } }
                }
            }).to_string();
            let sizing = LambdaSizing { memory_mb: memory, timeout_seconds: timeout, ephemeral_storage_mb: ephemeral };
            let out = merge_sizing_into_template(&template, "srv", sizing).expect("merge");
            let v: serde_json::Value = serde_json::from_str(&out.template).expect("json");
            let mcp = &v["Resources"]["Mcp"]["Properties"];
            prop_assert_eq!(&mcp["MemorySize"], &json!(memory.unwrap_or(512)));
            prop_assert_eq!(&mcp["Timeout"], &json!(timeout.unwrap_or(30)));
            match ephemeral {
                Some(size) => prop_assert_eq!(&mcp["EphemeralStorage"], &json!({ "Size": size })),
                None => prop_assert!(mcp.get("EphemeralStorage").is_none()),
            }
            prop_assert_eq!(
                &v["Resources"]["Other"]["Properties"],
                &json!({ "FunctionName": "srv-authorizer", "MemorySize": 256, "Timeout": 10 })
            );
            prop_assert!(out.matched);
        }
    }
}
