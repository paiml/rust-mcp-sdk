//! The MCP Lambda function's settings from `.pmcp/deploy.toml`: `[server]`
//! sizing (`memory_mb`, `timeout_seconds`, `ephemeral_storage_mb`) and
//! `[environment]` (debug session `cargo-pmcp-deploy-targets`, #13 and #16).
//!
//! Both `aws-lambda` deploy paths read them from here, so they cannot drift
//! apart:
//!
//! - the native `CloudFormation` engine puts [`function_environment`] into the
//!   renderer's `RenderParams::environment` and merges the sizing into the
//!   rendered template (`deployment::template_merge`);
//! - the `npx cdk deploy` path renders [`ScaffoldFunction`] into the
//!   `deploy/lib/stack.ts` scaffold.
//!
//! # Environment
//!
//! `RUST_LOG=info` is the default; `[environment]` adds to it and overrides it.
//! A key that is also a secret is left out: no `[environment]` value may
//! shadow a secret, and no secret value is ever read here (only secret NAMES
//! are), so none can reach a template or `stack.ts`.

use anyhow::{bail, Result};
use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::deployment::config::DeployConfig;

/// The environment every scaffolded MCP function starts with. `[environment]`
/// overrides it.
pub const DEFAULT_ENVIRONMENT: [(&str, &str); 1] = [("RUST_LOG", "info")];

/// Smallest `[server] ephemeral_storage_mb` Lambda accepts (also its default).
pub const EPHEMERAL_STORAGE_MB_MIN: u32 = 512;

/// Largest `[server] ephemeral_storage_mb` Lambda accepts.
pub const EPHEMERAL_STORAGE_MB_MAX: u32 = 10_240;

/// Refuse an `ephemeral_storage_mb` outside Lambda's 512-10240 MB range.
pub fn validate_ephemeral_storage_mb(value: Option<u32>) -> Result<()> {
    match value {
        Some(size) if !(EPHEMERAL_STORAGE_MB_MIN..=EPHEMERAL_STORAGE_MB_MAX).contains(&size) => {
            bail!(
                "[server] ephemeral_storage_mb = {size} in .pmcp/deploy.toml is out of range: AWS \
                 Lambda accepts {EPHEMERAL_STORAGE_MB_MIN}-{EPHEMERAL_STORAGE_MB_MAX} (MB). Fix it \
                 (or remove it for the 512 MB default) and re-run; nothing was built or deployed."
            )
        },
        _ => Ok(()),
    }
}

/// Validate the Lambda settings in `config` (`[server] ephemeral_storage_mb`
/// and `[build]`) before anything is built or deployed. Run at the start of
/// the `aws-lambda` and `pmcp-run` builds and by `cargo pmcp validate deploy`,
/// so a bad value fails in seconds instead of after a multi-minute build.
pub fn validate_lambda_settings(config: &DeployConfig) -> Result<()> {
    validate_ephemeral_storage_mb(config.server.ephemeral_storage_mb)?;
    crate::deployment::builder::validate_build_config(&config.build)
}

/// The secret NAMES `config` knows about: the keys of `config.secrets` (on
/// `aws-lambda` the CLI replaces them with the resolved secrets; a config
/// loaded from disk carries the declared `[secrets]` names). Never the values.
pub fn secret_keys(config: &DeployConfig) -> BTreeSet<String> {
    config.secrets.keys().cloned().collect()
}

/// The MCP function's environment: [`DEFAULT_ENVIRONMENT`], overridden and
/// extended by `environment`, minus every key in `secret_keys`.
pub fn function_environment(
    environment: &HashMap<String, String>,
    secret_keys: &BTreeSet<String>,
) -> BTreeMap<String, String> {
    let mut out: BTreeMap<String, String> = DEFAULT_ENVIRONMENT
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect();
    out.extend(
        environment
            .iter()
            .filter(|(key, _)| !secret_keys.contains(*key))
            .map(|(key, value)| (key.clone(), value.clone())),
    );
    out
}

/// The declared `[environment]` keys [`function_environment`] leaves out
/// because they are also secrets, sorted.
pub fn environment_keys_shadowing_secrets(
    environment: &HashMap<String, String>,
    secret_keys: &BTreeSet<String>,
) -> Vec<String> {
    let mut keys: Vec<String> = environment
        .keys()
        .filter(|key| secret_keys.contains(*key))
        .cloned()
        .collect();
    keys.sort();
    keys
}

/// Say so when `[environment]` keys were left out because they are secrets.
pub fn warn_environment_keys_shadowing_secrets(
    environment: &HashMap<String, String>,
    secret_keys: &BTreeSet<String>,
) {
    let keys = environment_keys_shadowing_secrets(environment, secret_keys);
    if keys.is_empty() {
        return;
    }
    eprintln!(
        "  {} [environment] {} also named as {}; not written into the function's \
         environment, so the [environment] value cannot shadow the secret.",
        console::style("warning:").yellow(),
        keys.join(", "),
        if keys.len() == 1 {
            "a secret"
        } else {
            "secrets"
        }
    );
}

/// The function settings the `aws-lambda` `stack.ts` scaffold renders.
///
/// [`ScaffoldFunction::default`] is what every cargo-pmcp up to and including
/// 0.28.0 baked into the scaffold regardless of `.pmcp/deploy.toml`, and it
/// still renders byte-for-byte as it did then (so pre-existing untouched
/// scaffolds keep being recognized as cargo-pmcp's own).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScaffoldFunction {
    pub memory_mb: u32,
    pub timeout_seconds: u32,
    pub ephemeral_storage_mb: Option<u32>,
    pub environment: BTreeMap<String, String>,
}

impl Default for ScaffoldFunction {
    fn default() -> Self {
        Self {
            memory_mb: crate::commands::deploy::init::AWS_LAMBDA_SCAFFOLD_MEMORY_MB,
            timeout_seconds: crate::commands::deploy::init::AWS_LAMBDA_SCAFFOLD_TIMEOUT_SECONDS,
            ephemeral_storage_mb: None,
            environment: function_environment(&HashMap::new(), &BTreeSet::new()),
        }
    }
}

impl ScaffoldFunction {
    /// The settings `config` declares. An omitted `memory_mb`/`timeout_seconds`
    /// keeps the scaffold's own value; an omitted `ephemeral_storage_mb` renders
    /// no `ephemeralStorageSize` (Lambda's 512 MB default).
    pub fn from_config(config: &DeployConfig, secret_keys: &BTreeSet<String>) -> Self {
        let defaults = Self::default();
        Self {
            memory_mb: config.server.memory_mb.unwrap_or(defaults.memory_mb),
            timeout_seconds: config
                .server
                .timeout_seconds
                .unwrap_or(defaults.timeout_seconds),
            ephemeral_storage_mb: config.server.ephemeral_storage_mb,
            environment: function_environment(&config.environment, secret_keys),
        }
    }

    /// The `lambda.Function` props this renders into the `aws-lambda`
    /// scaffold, from `memorySize` through the `environment` block's closing
    /// `},`, indented for the template and without a trailing newline.
    pub fn render_cdk_props(&self) -> String {
        let mut lines = vec![
            format!("      memorySize: {},", self.memory_mb),
            format!(
                "      timeout: cdk.Duration.seconds({}),",
                self.timeout_seconds
            ),
        ];
        if let Some(size) = self.ephemeral_storage_mb {
            lines.push(format!(
                "      ephemeralStorageSize: cdk.Size.mebibytes({size}),"
            ));
        }
        lines.push("      environment: {".to_string());
        lines.extend(self.environment.iter().map(|(key, value)| {
            format!(
                "        {}: {},",
                ts_property_name(key),
                ts_string_literal(value)
            )
        }));
        lines.push("      },".to_string());
        lines.join("\n")
    }
}

/// `value` as a TypeScript string literal: single-quoted when it is printable
/// ASCII without a quote or backslash (so `'info'` renders exactly as the
/// scaffold always has), otherwise a JSON string, which is valid TypeScript.
pub fn ts_string_literal(value: &str) -> String {
    let plain = value
        .chars()
        .all(|c| matches!(c, ' '..='~') && c != '\'' && c != '\\');
    if plain {
        format!("'{value}'")
    } else {
        serde_json::to_string(value).expect("a string always serializes as JSON")
    }
}

/// `key` as a TypeScript object property name: bare when it is an
/// identifier, otherwise quoted.
pub fn ts_property_name(key: &str) -> String {
    let mut chars = key.chars();
    let is_identifier = chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_' || c == '$')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$');
    if is_identifier {
        key.to_string()
    } else {
        serde_json::to_string(key).expect("a string always serializes as JSON")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect()
    }

    fn keys(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(|k| (*k).to_string()).collect()
    }

    fn aws_config(root: &std::path::Path) -> DeployConfig {
        let mut config = DeployConfig::default_for_server(
            "forecast-coach".to_string(),
            "us-east-1".to_string(),
            root.to_path_buf(),
        );
        config.target.target_type = "aws-lambda".to_string();
        config
    }

    /// The bounds Lambda documents: 512 and 10240 accepted, one past either
    /// refused, with a message naming the key and the range.
    #[test]
    fn ephemeral_storage_bounds() {
        assert!(validate_ephemeral_storage_mb(None).is_ok());
        assert!(validate_ephemeral_storage_mb(Some(512)).is_ok());
        assert!(validate_ephemeral_storage_mb(Some(10_240)).is_ok());
        for bad in [0, 511, 10_241, u32::MAX] {
            let err = validate_ephemeral_storage_mb(Some(bad))
                .expect_err("out-of-range ephemeral_storage_mb must be refused");
            let text = format!("{err:#}");
            assert!(text.contains("ephemeral_storage_mb"), "{text}");
            assert!(text.contains("512-10240"), "{text}");
        }
    }

    #[test]
    fn validate_lambda_settings_refuses_a_bad_ephemeral_storage_size() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let mut config = aws_config(tmp.path());
        config.server.ephemeral_storage_mb = Some(256);
        assert!(validate_lambda_settings(&config).is_err());
        config.server.ephemeral_storage_mb = Some(2048);
        assert!(validate_lambda_settings(&config).is_ok());
    }

    /// `[environment]` extends and overrides the `RUST_LOG` default.
    #[test]
    fn environment_overrides_and_extends_the_default() {
        let out = function_environment(
            &env(&[("RUST_LOG", "debug"), ("MODEL_URL", "s3://m")]),
            &keys(&[]),
        );
        assert_eq!(
            out,
            BTreeMap::from([
                ("MODEL_URL".to_string(), "s3://m".to_string()),
                ("RUST_LOG".to_string(), "debug".to_string()),
            ])
        );
    }

    /// A key that is also a secret is left out, and reported.
    #[test]
    fn secret_keys_are_left_out() {
        let environment = env(&[("API_TOKEN", "env"), ("PUBLIC", "x")]);
        let out = function_environment(&environment, &keys(&["API_TOKEN"]));
        assert!(!out.contains_key("API_TOKEN"));
        assert_eq!(out.get("PUBLIC").map(String::as_str), Some("x"));
        assert_eq!(
            environment_keys_shadowing_secrets(&environment, &keys(&["API_TOKEN"])),
            vec!["API_TOKEN".to_string()]
        );
    }

    /// The scaffold default renders exactly the literals every cargo-pmcp up
    /// to 0.28.0 hardcoded, so pre-existing untouched scaffolds still match.
    #[test]
    fn default_renders_the_historical_literals_byte_for_byte() {
        assert_eq!(
            ScaffoldFunction::default().render_cdk_props(),
            "      memorySize: 512,\n      timeout: cdk.Duration.seconds(30),\n      \
             environment: {\n        RUST_LOG: 'info',\n      },"
        );
    }

    /// #13/#16 on the `npx cdk deploy` path: the declared settings render into
    /// the scaffold's `lambda.Function` props.
    #[test]
    fn declared_settings_render_into_the_cdk_props() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let mut config = aws_config(tmp.path());
        config.server.memory_mb = Some(1769);
        config.server.timeout_seconds = Some(60);
        config.server.ephemeral_storage_mb = Some(4096);
        config.environment.insert(
            "MODEL_URL".to_string(),
            "s3://models/forecast.bin".to_string(),
        );
        config
            .environment
            .insert("RUST_LOG".to_string(), "warn".to_string());

        let props = ScaffoldFunction::from_config(&config, &BTreeSet::new()).render_cdk_props();

        assert!(props.contains("      memorySize: 1769,\n"), "{props}");
        assert!(
            props.contains("      timeout: cdk.Duration.seconds(60),\n"),
            "{props}"
        );
        assert!(
            props.contains("      ephemeralStorageSize: cdk.Size.mebibytes(4096),\n"),
            "{props}"
        );
        assert!(
            props.contains("        MODEL_URL: 's3://models/forecast.bin',\n"),
            "{props}"
        );
        assert!(props.contains("        RUST_LOG: 'warn',\n"), "{props}");
    }

    /// An omitted `memory_mb`/`timeout_seconds` keeps the scaffold's own
    /// value; an omitted `ephemeral_storage_mb` renders no
    /// `ephemeralStorageSize`.
    #[test]
    fn omitted_settings_keep_the_scaffold_values() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let mut config = aws_config(tmp.path());
        config.server.memory_mb = None;
        config.server.timeout_seconds = None;
        assert_eq!(
            ScaffoldFunction::from_config(&config, &BTreeSet::new()),
            ScaffoldFunction::default()
        );
    }

    /// Secret names are never rendered, and no secret value is read.
    #[test]
    fn secrets_never_reach_the_cdk_props() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let mut config = aws_config(tmp.path());
        config
            .environment
            .insert("API_TOKEN".to_string(), "env-value".to_string());
        config
            .secrets
            .insert("API_TOKEN".to_string(), "s3cr3t".to_string());

        let props =
            ScaffoldFunction::from_config(&config, &secret_keys(&config)).render_cdk_props();

        assert!(!props.contains("API_TOKEN"), "{props}");
        assert!(!props.contains("s3cr3t"), "{props}");
    }

    /// Values and keys that are not plain literals are encoded, not pasted.
    #[test]
    fn awkward_values_and_keys_are_encoded() {
        assert_eq!(ts_string_literal("info"), "'info'");
        assert_eq!(ts_string_literal("it's"), "\"it's\"");
        assert_eq!(ts_string_literal("a\\b"), "\"a\\\\b\"");
        assert_eq!(ts_string_literal("line\nbreak"), "\"line\\nbreak\"");
        assert_eq!(ts_property_name("RUST_LOG"), "RUST_LOG");
        assert_eq!(ts_property_name("my-key"), "\"my-key\"");
        assert_eq!(ts_property_name("1ST"), "\"1ST\"");
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    /// Decode a literal produced by [`ts_string_literal`] /
    /// [`ts_property_name`] back to the string it denotes.
    fn decode(literal: &str) -> String {
        if let Some(inner) = literal
            .strip_prefix('\'')
            .and_then(|s| s.strip_suffix('\''))
        {
            return inner.to_string();
        }
        if literal.starts_with('"') {
            return serde_json::from_str(literal).expect("a JSON string literal");
        }
        literal.to_string()
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        /// Every value round-trips through its TypeScript literal, so what
        /// `[environment]` declares is exactly what the function gets.
        #[test]
        fn string_literals_round_trip(value in any::<String>()) {
            prop_assert_eq!(decode(&ts_string_literal(&value)), value);
        }

        #[test]
        fn property_names_round_trip(key in any::<String>()) {
            prop_assert_eq!(decode(&ts_property_name(&key)), key);
        }

        /// Precedence: a non-secret declared key carries its declared value;
        /// `RUST_LOG` is always present (declared value, else `info`); no other
        /// secret key appears; nothing undeclared appears.
        #[test]
        fn environment_precedence(
            declared in prop::collection::hash_map("[A-Z_]{1,6}", "[ -~]{0,8}", 0..6),
            secret_names in prop::collection::btree_set("[A-Z_]{1,6}", 0..4),
        ) {
            let out = function_environment(&declared, &secret_names);
            for (key, value) in &declared {
                if !secret_names.contains(key) {
                    prop_assert_eq!(out.get(key), Some(value));
                }
            }
            let expected_log = declared
                .get("RUST_LOG")
                .filter(|_| !secret_names.contains("RUST_LOG"))
                .map_or("info", String::as_str);
            prop_assert_eq!(out.get("RUST_LOG").map(String::as_str), Some(expected_log));
            for key in out.keys() {
                prop_assert!(key == "RUST_LOG" || (declared.contains_key(key) && !secret_names.contains(key)));
            }
        }

        /// `ephemeral_storage_mb` is accepted exactly on Lambda's range.
        #[test]
        fn ephemeral_storage_accepted_exactly_in_range(value in any::<u32>()) {
            let in_range = (EPHEMERAL_STORAGE_MB_MIN..=EPHEMERAL_STORAGE_MB_MAX).contains(&value);
            prop_assert_eq!(validate_ephemeral_storage_mb(Some(value)).is_ok(), in_range);
        }
    }
}
