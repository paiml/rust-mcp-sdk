//! Property-based tests for loadtest config parsing and McpError classification.
//!
//! Uses proptest to verify invariants across randomized inputs:
//! - Config parsing roundtrips correctly for valid inputs
//! - Validation rejects semantically invalid configs
//! - McpError classification methods are correct for all code values
//! - Error categories are always non-empty strings

use cargo_pmcp::loadtest::config::{LoadTestConfig, ScenarioStep, Settings};
use cargo_pmcp::loadtest::error::{LoadTestError, McpError};
use proptest::prelude::*;
use std::collections::HashMap;

/// Generate a valid Settings struct with bounded field values.
fn arb_settings() -> impl Strategy<Value = Settings> {
    (1u32..=1000, 1u64..=3600, 100u64..=30000, 1u64..=500).prop_map(
        |(virtual_users, duration_secs, timeout_ms, expected_interval_ms)| Settings {
            virtual_users,
            duration_secs,
            timeout_ms,
            expected_interval_ms,
            request_interval_ms: None,
        },
    )
}

/// Generate a ScenarioStep with weight forced to 0.
fn arb_scenario_step_zero_weight() -> impl Strategy<Value = ScenarioStep> {
    prop_oneof![
        "[a-z]{1,20}".prop_map(|tool| ScenarioStep::ToolCall {
            weight: 0,
            tool,
            arguments: serde_json::Value::Null,
        }),
        "file:///[a-z]{1,20}".prop_map(|uri| ScenarioStep::ResourceRead { weight: 0, uri }),
        "[a-z]{1,20}".prop_map(|prompt| ScenarioStep::PromptGet {
            weight: 0,
            prompt,
            arguments: HashMap::new(),
        }),
    ]
}

proptest! {
    /// Valid config fields survive a serialize-then-parse roundtrip.
    ///
    /// Generates arbitrary valid Settings and a non-empty vec of weighted
    /// ScenarioStep::ToolCall variants, serializes to TOML, parses back,
    /// and asserts all numeric fields match.
    #[test]
    fn prop_valid_config_roundtrip(
        virtual_users in 1u32..=1000,
        duration_secs in 1u64..=3600,
        timeout_ms in 100u64..=30000,
        expected_interval_ms in 1u64..=500,
        tool_name in "[a-z]{1,10}",
        weight in 1u32..=100,
    ) {
        let toml_str = format!(
            r#"[settings]
virtual_users = {virtual_users}
duration_secs = {duration_secs}
timeout_ms = {timeout_ms}
expected_interval_ms = {expected_interval_ms}

[[scenario]]
type = "tools/call"
weight = {weight}
tool = "{tool_name}"
"#
        );

        let config = LoadTestConfig::from_toml(&toml_str).unwrap();
        prop_assert_eq!(config.settings.virtual_users, virtual_users);
        prop_assert_eq!(config.settings.duration_secs, duration_secs);
        prop_assert_eq!(config.settings.timeout_ms, timeout_ms);
        prop_assert_eq!(config.settings.expected_interval_ms, expected_interval_ms);
        prop_assert_eq!(config.scenario.len(), 1);
        prop_assert_eq!(config.scenario[0].weight(), weight);
    }

    /// An empty scenario vec always fails validation.
    ///
    /// No matter what the Settings values are, a config with zero scenario
    /// steps must be rejected with a ConfigValidation error.
    #[test]
    fn prop_empty_scenario_always_fails_validation(
        settings in arb_settings(),
    ) {
        let config = LoadTestConfig {
            settings,
            scenario: vec![],
            stage: vec![],
        };
        let result = config.validate();
        prop_assert!(result.is_err());
        match result.unwrap_err() {
            LoadTestError::ConfigValidation { .. } => {},
            other => prop_assert!(false, "Expected ConfigValidation, got: {:?}", other),
        }
    }

    /// A scenario where all steps have weight=0 always fails validation.
    ///
    /// Even with multiple steps, if total weight is zero the config is invalid.
    #[test]
    fn prop_all_zero_weights_fails_validation(
        settings in arb_settings(),
        steps in proptest::collection::vec(arb_scenario_step_zero_weight(), 1..=10),
    ) {
        let config = LoadTestConfig {
            settings,
            scenario: steps,
            stage: vec![],
        };
        let result = config.validate();
        prop_assert!(result.is_err());
        match result.unwrap_err() {
            LoadTestError::ConfigValidation { .. } => {},
            other => prop_assert!(false, "Expected ConfigValidation, got: {:?}", other),
        }
    }

    /// Settings::timeout_as_duration() always matches the raw timeout_ms value.
    ///
    /// For any timeout_ms value, converting to Duration and back to millis
    /// must produce the same value.
    #[test]
    fn prop_timeout_as_duration_matches_ms(
        timeout_ms in 1u64..=(u64::MAX / 2),
    ) {
        let settings = Settings {
            virtual_users: 1,
            duration_secs: 1,
            timeout_ms,
            expected_interval_ms: 100,
            request_interval_ms: None,
        };
        prop_assert_eq!(
            settings.timeout_as_duration().as_millis(),
            timeout_ms as u128
        );
    }

    /// is_method_not_found() returns true if and only if the code is -32601.
    #[test]
    fn prop_is_method_not_found_only_for_32601(code in proptest::num::i32::ANY) {
        let err = McpError::JsonRpc {
            code,
            message: "test".to_string(),
        };
        prop_assert_eq!(err.is_method_not_found(), code == -32601);
    }

    /// is_invalid_params() returns true if and only if the code is -32602.
    #[test]
    fn prop_is_invalid_params_only_for_32602(code in proptest::num::i32::ANY) {
        let err = McpError::JsonRpc {
            code,
            message: "test".to_string(),
        };
        prop_assert_eq!(err.is_invalid_params(), code == -32602);
    }

    /// error_category() never returns an empty string for any McpError variant.
    #[test]
    fn prop_error_category_never_empty(
        code in proptest::num::i32::ANY,
        status in 100u16..=599,
        msg in "[a-z]{0,50}",
        variant in 0u8..4,
    ) {
        let err = match variant {
            0 => McpError::JsonRpc { code, message: msg },
            1 => McpError::Http { status, body: msg },
            2 => McpError::Timeout,
            _ => McpError::Connection { message: msg },
        };
        prop_assert!(!err.error_category().is_empty());
    }
}
