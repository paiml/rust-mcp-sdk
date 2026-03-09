//! TOML-based load test scenario configuration.
//!
//! Defines typed structs for parsing load test scenario definitions
//! from TOML config files with weighted mixes of MCP operations.
//!
//! The configuration supports three MCP operation types as first-class
//! scenario steps: `tools/call`, `resources/read`, and `prompts/get`.
//! Each step carries a weight for proportional scheduling.
//!
//! # Example TOML
//!
//! ```toml
//! [settings]
//! virtual_users = 10
//! duration_secs = 60
//! timeout_ms = 5000
//! # request_interval_ms = 15000  # optional pacing delay per VU
//!
//! [[scenario]]
//! type = "tools/call"
//! weight = 60
//! tool = "calculate"
//! arguments = { expression = "2+2" }
//!
//! [[scenario]]
//! type = "resources/read"
//! weight = 30
//! uri = "file:///data/config.json"
//!
//! [[scenario]]
//! type = "prompts/get"
//! weight = 10
//! prompt = "summarize"
//! arguments = { text = "Hello world" }
//! ```
//!
//! Note: The target server URL is NOT part of the config file. It is provided
//! via the `--url` CLI flag per user decision.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use crate::loadtest::error::LoadTestError;

/// A load-shaping stage defining a target VU count and duration.
///
/// Stages are defined as `[[stage]]` blocks in the TOML config and enable
/// multi-phase load profiles (ramp-up, hold, ramp-down). The engine linearly
/// ramps VU count to `target_vus` over the stage's `duration_secs`.
///
/// # Example TOML
///
/// ```toml
/// [[stage]]
/// target_vus = 10
/// duration_secs = 30
///
/// [[stage]]
/// target_vus = 50
/// duration_secs = 60
///
/// [[stage]]
/// target_vus = 0
/// duration_secs = 30
/// ```
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Stage {
    /// Target number of virtual users at the end of this stage.
    pub target_vus: u32,
    /// Duration of this stage in seconds.
    pub duration_secs: u64,
}

/// Top-level load test configuration parsed from a TOML file.
///
/// Contains general settings (VU count, duration, timeout), a list of
/// weighted scenario steps defining the mix of MCP operations to execute,
/// and optional `[[stage]]` blocks for multi-phase load shaping.
///
/// When stages are present, the engine ramps VU count linearly through
/// each stage. When absent, flat load is applied (all VUs start immediately).
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LoadTestConfig {
    /// General load test settings.
    pub settings: Settings,
    /// Weighted scenario steps defining the MCP operation mix.
    pub scenario: Vec<ScenarioStep>,
    /// Optional load-shaping stages for multi-phase profiles.
    ///
    /// When present, the engine ramps VU count through each stage linearly.
    /// When absent (empty), flat load is applied with `settings.virtual_users`.
    /// The field name is `stage` (not `stages`) because TOML `[[stage]]`
    /// array-of-tables syntax creates a key called `stage`.
    #[serde(default)]
    pub stage: Vec<Stage>,
}

/// General load test settings controlling execution parameters.
///
/// The target server URL is intentionally absent -- it is provided via the
/// `--url` CLI flag, not the config file.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Settings {
    /// Number of concurrent virtual users to simulate.
    pub virtual_users: u32,
    /// Total test duration in seconds.
    pub duration_secs: u64,
    /// Per-request timeout in milliseconds.
    pub timeout_ms: u64,
    /// Expected interval between consecutive requests from a single VU (ms).
    ///
    /// Used for coordinated omission correction via HdrHistogram's
    /// `record_correct()`. Defaults to 100ms if not specified.
    #[serde(default = "default_expected_interval")]
    pub expected_interval_ms: u64,
    /// Optional delay between consecutive requests from a single VU (ms).
    ///
    /// When set, each VU sleeps this long after completing a request before
    /// starting the next one. This enables realistic steady-state load patterns
    /// (e.g., 15000ms = 4 requests/minute per VU). When `None` (the default),
    /// VUs fire requests as fast as possible (closed-loop).
    #[serde(default)]
    pub request_interval_ms: Option<u64>,
}

/// Default expected interval for coordinated omission correction: 100ms.
fn default_expected_interval() -> u64 {
    100
}

/// A single scenario step representing an MCP operation with a scheduling weight.
///
/// The `type` field in TOML determines the variant via serde's internally tagged
/// enum support. Supported types: `"tools/call"`, `"resources/read"`, `"prompts/get"`,
/// `"code_mode"`.
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(tag = "type")]
pub enum ScenarioStep {
    /// A `tools/call` MCP request.
    #[serde(rename = "tools/call")]
    ToolCall {
        /// Scheduling weight relative to other steps.
        weight: u32,
        /// Name of the tool to call.
        tool: String,
        /// JSON arguments to pass to the tool (defaults to null).
        #[serde(default)]
        arguments: serde_json::Value,
    },
    /// A `resources/read` MCP request.
    #[serde(rename = "resources/read")]
    ResourceRead {
        /// Scheduling weight relative to other steps.
        weight: u32,
        /// URI of the resource to read.
        uri: String,
    },
    /// A `prompts/get` MCP request.
    #[serde(rename = "prompts/get")]
    PromptGet {
        /// Scheduling weight relative to other steps.
        weight: u32,
        /// Name of the prompt to retrieve.
        prompt: String,
        /// String arguments to pass to the prompt (defaults to empty map).
        #[serde(default)]
        arguments: HashMap<String, String>,
    },
    /// A `code_mode` two-step flow: `validate_code` then `execute_code`.
    ///
    /// Used for servers that support Code Mode (e.g., GraphQL, SQL).
    /// The code is first validated, then executed if approved.
    #[serde(rename = "code_mode")]
    CodeMode {
        /// Scheduling weight relative to other steps.
        weight: u32,
        /// The code to validate and execute.
        code: String,
        /// Code format (e.g., "graphql", "sql", "javascript").
        #[serde(default = "default_code_format")]
        format: String,
    },
}

fn default_code_format() -> String {
    "graphql".to_string()
}

impl LoadTestConfig {
    /// Parse a TOML string into a validated [`LoadTestConfig`].
    ///
    /// Returns an error if the TOML is malformed or fails validation.
    pub fn from_toml(content: &str) -> Result<Self, LoadTestError> {
        let config: Self = toml::from_str(content)?;
        config.validate()?;
        Ok(config)
    }

    /// Load and validate a [`LoadTestConfig`] from a file path.
    ///
    /// Returns [`LoadTestError::ConfigIo`] if the file cannot be read,
    /// [`LoadTestError::ConfigParse`] if the TOML is malformed, or
    /// [`LoadTestError::ConfigValidation`] if validation fails.
    pub fn load(path: &Path) -> Result<Self, LoadTestError> {
        let content = std::fs::read_to_string(path).map_err(|source| LoadTestError::ConfigIo {
            source,
            path: path.display().to_string(),
        })?;
        Self::from_toml(&content)
    }

    /// Returns `true` if the config defines load-shaping stages.
    pub fn has_stages(&self) -> bool {
        !self.stage.is_empty()
    }

    /// Returns the sum of all stage durations in seconds (0 if no stages).
    pub fn total_stage_duration(&self) -> u64 {
        self.stage.iter().map(|s| s.duration_secs).sum()
    }

    /// Returns the effective test duration in seconds.
    ///
    /// When stages are present, returns the sum of stage durations.
    /// When absent, returns `settings.duration_secs`.
    pub fn effective_duration_secs(&self) -> u64 {
        if self.has_stages() {
            self.total_stage_duration()
        } else {
            self.settings.duration_secs
        }
    }

    /// Validate that the config is semantically correct.
    ///
    /// Checks:
    /// - At least one scenario step is defined
    /// - Total weight across all steps is greater than zero
    /// - If stages present: each stage must have `duration_secs > 0`
    /// - If stages absent: require valid `virtual_users` and `duration_secs`
    pub fn validate(&self) -> Result<(), LoadTestError> {
        if self.scenario.is_empty() {
            return Err(LoadTestError::ConfigValidation {
                message: "Config must contain at least one [[scenario]] step".to_string(),
            });
        }

        let total_weight: u32 = self.scenario.iter().map(|s| s.weight()).sum();
        if total_weight == 0 {
            return Err(LoadTestError::ConfigValidation {
                message: "Total scenario weights must be greater than 0".to_string(),
            });
        }

        if self.has_stages() {
            // Validate stage-specific rules
            for (i, stage) in self.stage.iter().enumerate() {
                if stage.duration_secs == 0 {
                    return Err(LoadTestError::ConfigValidation {
                        message: format!(
                            "Stage {} has duration_secs=0; each stage must have a positive duration",
                            i + 1
                        ),
                    });
                }
            }

            // Warn if virtual_users is also set (stages take precedence)
            if self.settings.virtual_users > 0 {
                eprintln!(
                    "Warning: settings.virtual_users={} is ignored when [[stage]] blocks are present",
                    self.settings.virtual_users
                );
            }
        }

        Ok(())
    }
}

impl Settings {
    /// Convert the `timeout_ms` field to a [`Duration`].
    pub fn timeout_as_duration(&self) -> Duration {
        Duration::from_millis(self.timeout_ms)
    }
}

impl ScenarioStep {
    /// Returns the scheduling weight of this step, regardless of variant.
    pub fn weight(&self) -> u32 {
        match self {
            Self::ToolCall { weight, .. } => *weight,
            Self::ResourceRead { weight, .. } => *weight,
            Self::PromptGet { weight, .. } => *weight,
            Self::CodeMode { weight, .. } => *weight,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loadtest::error::LoadTestError;
    use std::io::Write;
    use std::time::Duration;

    #[test]
    fn test_parse_minimal_config() {
        let toml_str = r#"
[settings]
virtual_users = 10
duration_secs = 60
timeout_ms = 5000

[[scenario]]
type = "tools/call"
weight = 100
tool = "echo"
arguments = { text = "hello" }
"#;
        let config = LoadTestConfig::from_toml(toml_str).unwrap();
        assert_eq!(config.settings.virtual_users, 10);
        assert_eq!(config.settings.duration_secs, 60);
        assert_eq!(config.settings.timeout_ms, 5000);
        assert_eq!(config.scenario.len(), 1);

        match &config.scenario[0] {
            ScenarioStep::ToolCall {
                weight,
                tool,
                arguments,
            } => {
                assert_eq!(*weight, 100);
                assert_eq!(tool, "echo");
                assert_eq!(arguments["text"], "hello");
            },
            other => panic!("Expected ToolCall, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_weighted_mix() {
        let toml_str = r#"
[settings]
virtual_users = 20
duration_secs = 120
timeout_ms = 3000

[[scenario]]
type = "tools/call"
weight = 60
tool = "calculate"
arguments = { expression = "2+2" }

[[scenario]]
type = "resources/read"
weight = 30
uri = "file:///data/config.json"

[[scenario]]
type = "prompts/get"
weight = 10
prompt = "summarize"
arguments = { text = "Hello world" }
"#;
        let config = LoadTestConfig::from_toml(toml_str).unwrap();
        assert_eq!(config.scenario.len(), 3);

        assert!(matches!(
            &config.scenario[0],
            ScenarioStep::ToolCall { weight: 60, .. }
        ));
        assert!(matches!(
            &config.scenario[1],
            ScenarioStep::ResourceRead { weight: 30, .. }
        ));
        assert!(matches!(
            &config.scenario[2],
            ScenarioStep::PromptGet { weight: 10, .. }
        ));
    }

    #[test]
    fn test_parse_code_mode_scenario() {
        let toml_str = r#"
[settings]
virtual_users = 5
duration_secs = 30
timeout_ms = 5000

[[scenario]]
type = "tools/call"
weight = 50
tool = "list-agents"

[[scenario]]
type = "code_mode"
weight = 10
code = """
query ListAgents {
  listAgentsFromRegistry { id name }
}
"""
format = "graphql"
"#;
        let config = LoadTestConfig::from_toml(toml_str).unwrap();
        assert_eq!(config.scenario.len(), 2);
        assert!(matches!(
            &config.scenario[1],
            ScenarioStep::CodeMode {
                weight: 10,
                format,
                ..
            } if format == "graphql"
        ));
    }

    #[test]
    fn test_parse_code_mode_default_format() {
        let toml_str = r#"
[settings]
virtual_users = 5
duration_secs = 30
timeout_ms = 5000

[[scenario]]
type = "code_mode"
weight = 5
code = "SELECT 1"
"#;
        let config = LoadTestConfig::from_toml(toml_str).unwrap();
        assert!(matches!(
            &config.scenario[0],
            ScenarioStep::CodeMode {
                format,
                ..
            } if format == "graphql"
        ));
    }

    #[test]
    fn test_parse_default_expected_interval() {
        let toml_str = r#"
[settings]
virtual_users = 5
duration_secs = 30
timeout_ms = 2000

[[scenario]]
type = "tools/call"
weight = 1
tool = "ping"
"#;
        let config = LoadTestConfig::from_toml(toml_str).unwrap();
        assert_eq!(config.settings.expected_interval_ms, 100);
    }

    #[test]
    fn test_parse_custom_expected_interval() {
        let toml_str = r#"
[settings]
virtual_users = 5
duration_secs = 30
timeout_ms = 2000
expected_interval_ms = 50

[[scenario]]
type = "tools/call"
weight = 1
tool = "ping"
"#;
        let config = LoadTestConfig::from_toml(toml_str).unwrap();
        assert_eq!(config.settings.expected_interval_ms, 50);
    }

    #[test]
    fn test_validate_empty_scenario_fails() {
        let config = LoadTestConfig {
            settings: Settings {
                virtual_users: 10,
                duration_secs: 60,
                timeout_ms: 5000,
                expected_interval_ms: 100,
                request_interval_ms: None,
            },
            scenario: vec![],
            stage: vec![],
        };
        let result = config.validate();
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            LoadTestError::ConfigValidation { .. }
        ));
    }

    #[test]
    fn test_validate_zero_total_weight_fails() {
        let config = LoadTestConfig {
            settings: Settings {
                virtual_users: 10,
                duration_secs: 60,
                timeout_ms: 5000,
                expected_interval_ms: 100,
                request_interval_ms: None,
            },
            scenario: vec![
                ScenarioStep::ToolCall {
                    weight: 0,
                    tool: "echo".to_string(),
                    arguments: serde_json::Value::Null,
                },
                ScenarioStep::ResourceRead {
                    weight: 0,
                    uri: "file:///data".to_string(),
                },
            ],
            stage: vec![],
        };
        let result = config.validate();
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            LoadTestError::ConfigValidation { .. }
        ));
    }

    #[test]
    fn test_validate_valid_config_passes() {
        let config = LoadTestConfig {
            settings: Settings {
                virtual_users: 10,
                duration_secs: 60,
                timeout_ms: 5000,
                expected_interval_ms: 100,
                request_interval_ms: None,
            },
            scenario: vec![ScenarioStep::ToolCall {
                weight: 100,
                tool: "echo".to_string(),
                arguments: serde_json::json!({"text": "hello"}),
            }],
            stage: vec![],
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_load_from_file() {
        let toml_content = r#"
[settings]
virtual_users = 5
duration_secs = 30
timeout_ms = 2000

[[scenario]]
type = "tools/call"
weight = 100
tool = "echo"
"#;
        let mut tmpfile = tempfile::NamedTempFile::new().unwrap();
        tmpfile.write_all(toml_content.as_bytes()).unwrap();
        tmpfile.flush().unwrap();

        let config = LoadTestConfig::load(tmpfile.path()).unwrap();
        assert_eq!(config.settings.virtual_users, 5);
        assert_eq!(config.scenario.len(), 1);
    }

    #[test]
    fn test_load_missing_file_fails() {
        let result = LoadTestConfig::load(std::path::Path::new("/nonexistent/path.toml"));
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            LoadTestError::ConfigIo { .. }
        ));
    }

    #[test]
    fn test_timeout_as_duration() {
        let settings = Settings {
            virtual_users: 10,
            duration_secs: 60,
            timeout_ms: 5000,
            expected_interval_ms: 100,
            request_interval_ms: None,
        };
        assert_eq!(settings.timeout_as_duration(), Duration::from_millis(5000));
    }

    #[test]
    fn test_parse_config_with_stages() {
        let toml_str = r#"
[settings]
virtual_users = 10
duration_secs = 60
timeout_ms = 5000

[[scenario]]
type = "tools/call"
weight = 100
tool = "echo"

[[stage]]
target_vus = 10
duration_secs = 30

[[stage]]
target_vus = 50
duration_secs = 60

[[stage]]
target_vus = 0
duration_secs = 30
"#;
        let config = LoadTestConfig::from_toml(toml_str).unwrap();
        assert_eq!(config.stage.len(), 3);
        assert_eq!(config.stage[0].target_vus, 10);
        assert_eq!(config.stage[0].duration_secs, 30);
        assert_eq!(config.stage[1].target_vus, 50);
        assert_eq!(config.stage[1].duration_secs, 60);
        assert_eq!(config.stage[2].target_vus, 0);
        assert_eq!(config.stage[2].duration_secs, 30);
    }

    #[test]
    fn test_parse_config_without_stages_backwards_compatible() {
        let toml_str = r#"
[settings]
virtual_users = 10
duration_secs = 60
timeout_ms = 5000

[[scenario]]
type = "tools/call"
weight = 100
tool = "echo"
"#;
        let config = LoadTestConfig::from_toml(toml_str).unwrap();
        assert!(config.stage.is_empty());
        assert!(!config.has_stages());
    }

    #[test]
    fn test_validate_stage_with_zero_duration_fails() {
        let config = LoadTestConfig {
            settings: Settings {
                virtual_users: 10,
                duration_secs: 60,
                timeout_ms: 5000,
                expected_interval_ms: 100,
                request_interval_ms: None,
            },
            scenario: vec![ScenarioStep::ToolCall {
                weight: 100,
                tool: "echo".to_string(),
                arguments: serde_json::Value::Null,
            }],
            stage: vec![
                Stage {
                    target_vus: 10,
                    duration_secs: 30,
                },
                Stage {
                    target_vus: 20,
                    duration_secs: 0,
                },
            ],
        };
        let result = config.validate();
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(
            err_msg.contains("duration_secs=0"),
            "Error should mention zero duration: {err_msg}"
        );
    }

    #[test]
    fn test_validate_stages_present_is_valid() {
        let config = LoadTestConfig {
            settings: Settings {
                virtual_users: 1,
                duration_secs: 10,
                timeout_ms: 5000,
                expected_interval_ms: 100,
                request_interval_ms: None,
            },
            scenario: vec![ScenarioStep::ToolCall {
                weight: 100,
                tool: "echo".to_string(),
                arguments: serde_json::Value::Null,
            }],
            stage: vec![Stage {
                target_vus: 50,
                duration_secs: 60,
            }],
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_has_stages() {
        let config_no_stages = LoadTestConfig {
            settings: Settings {
                virtual_users: 10,
                duration_secs: 60,
                timeout_ms: 5000,
                expected_interval_ms: 100,
                request_interval_ms: None,
            },
            scenario: vec![ScenarioStep::ToolCall {
                weight: 100,
                tool: "echo".to_string(),
                arguments: serde_json::Value::Null,
            }],
            stage: vec![],
        };
        assert!(!config_no_stages.has_stages());

        let config_with_stages = LoadTestConfig {
            settings: Settings {
                virtual_users: 10,
                duration_secs: 60,
                timeout_ms: 5000,
                expected_interval_ms: 100,
                request_interval_ms: None,
            },
            scenario: vec![ScenarioStep::ToolCall {
                weight: 100,
                tool: "echo".to_string(),
                arguments: serde_json::Value::Null,
            }],
            stage: vec![Stage {
                target_vus: 10,
                duration_secs: 30,
            }],
        };
        assert!(config_with_stages.has_stages());
    }

    #[test]
    fn test_total_stage_duration() {
        let config = LoadTestConfig {
            settings: Settings {
                virtual_users: 10,
                duration_secs: 60,
                timeout_ms: 5000,
                expected_interval_ms: 100,
                request_interval_ms: None,
            },
            scenario: vec![ScenarioStep::ToolCall {
                weight: 100,
                tool: "echo".to_string(),
                arguments: serde_json::Value::Null,
            }],
            stage: vec![
                Stage {
                    target_vus: 10,
                    duration_secs: 30,
                },
                Stage {
                    target_vus: 50,
                    duration_secs: 60,
                },
                Stage {
                    target_vus: 0,
                    duration_secs: 20,
                },
            ],
        };
        assert_eq!(config.total_stage_duration(), 110);

        let config_no_stages = LoadTestConfig {
            settings: Settings {
                virtual_users: 10,
                duration_secs: 60,
                timeout_ms: 5000,
                expected_interval_ms: 100,
                request_interval_ms: None,
            },
            scenario: vec![ScenarioStep::ToolCall {
                weight: 100,
                tool: "echo".to_string(),
                arguments: serde_json::Value::Null,
            }],
            stage: vec![],
        };
        assert_eq!(config_no_stages.total_stage_duration(), 0);
    }

    #[test]
    fn test_effective_duration_secs() {
        // With stages: sum of stage durations
        let config_with_stages = LoadTestConfig {
            settings: Settings {
                virtual_users: 10,
                duration_secs: 60,
                timeout_ms: 5000,
                expected_interval_ms: 100,
                request_interval_ms: None,
            },
            scenario: vec![ScenarioStep::ToolCall {
                weight: 100,
                tool: "echo".to_string(),
                arguments: serde_json::Value::Null,
            }],
            stage: vec![
                Stage {
                    target_vus: 10,
                    duration_secs: 30,
                },
                Stage {
                    target_vus: 50,
                    duration_secs: 60,
                },
            ],
        };
        assert_eq!(config_with_stages.effective_duration_secs(), 90);

        // Without stages: settings.duration_secs
        let config_no_stages = LoadTestConfig {
            settings: Settings {
                virtual_users: 10,
                duration_secs: 120,
                timeout_ms: 5000,
                expected_interval_ms: 100,
                request_interval_ms: None,
            },
            scenario: vec![ScenarioStep::ToolCall {
                weight: 100,
                tool: "echo".to_string(),
                arguments: serde_json::Value::Null,
            }],
            stage: vec![],
        };
        assert_eq!(config_no_stages.effective_duration_secs(), 120);
    }
}
