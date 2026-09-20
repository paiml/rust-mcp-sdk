//! Subprocess-spawn smoke test across ALL FOUR dev binaries (109-08, Task 4).
//!
//! This test parameterizes TEAM-01's "all four runnable binaries" claim: it
//! launches each of `team-fs`, `mem-mcp`, `approval-mcp`, and `team-mcp` as a
//! REAL child process (via `env!("CARGO_BIN_EXE_<bin>")`) and proves each
//! answers `tools/list` over stdio — removing the Manual-Only launch
//! verification.
//!
//! The handshake is driven by the SDK's stdio CLIENT ([`pmcp::Client`]) over a
//! [`ChildStdioTransport`] that reuses the SDK stdio transport's own framing
//! ([`StdioTransport::serialize_message`]/[`StdioTransport::parse_message`], the
//! single source of truth for the newline-delimited JSON-RPC wire encoding) —
//! NOT hand-written JSON-RPC framing. The SDK's own [`StdioTransport`] binds the
//! CURRENT process's stdin/stdout, so it cannot be pointed at a child's pipes;
//! this thin adapter binds the SAME framing to the child's `ChildStdin`/
//! `ChildStdout` instead.
//!
//! Determinism / isolation: every binary runs OFFLINE. `team-mcp` resolves its
//! member `AgentPackage` from a temp `--data-dir` and its mandatory llm slot
//! falls back to the package's tested value (no network at startup; `tools/list`
//! never invokes the LLM). Each child is bounded by a timeout and killed +
//! reaped BEFORE any assertion, so no process leaks.
//!
//! Gated on the four server features + `member-llm` (so every `CARGO_BIN_EXE_*`
//! resolves AND the `team-mcp` binary can construct its concrete member factory)
//! — i.e. it runs under `--all-features`:
//!
//! ```bash
//! cargo test -p pmcp-team-servers --test dev_binary_smoke --all-features
//! ```
#![cfg(all(
    feature = "team-fs",
    feature = "mem-mcp",
    feature = "approval-mcp",
    feature = "team-mcp",
    feature = "member-llm"
))]

use std::process::Stdio;
use std::time::Duration;

use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{ChildStdin, ChildStdout, Command};

use pmcp::error::TransportError;
use pmcp::shared::{StdioTransport, Transport, TransportMessage};
use pmcp::{Client, ClientCapabilities, Result};

// ---------------------------------------------------------------------------
// A child-process stdio transport that reuses the SDK stdio framing.
// ---------------------------------------------------------------------------

/// Drives a spawned MCP server child over its stdin/stdout pipes using the
/// SDK's own newline-delimited JSON-RPC framing
/// ([`StdioTransport::serialize_message`]/[`StdioTransport::parse_message`]).
#[derive(Debug)]
struct ChildStdioTransport {
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    line: Vec<u8>,
    connected: bool,
}

impl ChildStdioTransport {
    fn new(stdin: ChildStdin, stdout: ChildStdout) -> Self {
        Self {
            stdin,
            stdout: BufReader::new(stdout),
            line: Vec::new(),
            connected: true,
        }
    }
}

#[async_trait]
impl Transport for ChildStdioTransport {
    async fn send(&mut self, message: TransportMessage) -> Result<()> {
        // Reuse the SDK stdio transport's wire encoding (no hand-rolled JSON).
        let bytes = StdioTransport::serialize_message(&message)?;
        self.stdin
            .write_all(&bytes)
            .await
            .map_err(TransportError::from)?;
        self.stdin
            .write_all(b"\n")
            .await
            .map_err(TransportError::from)?;
        self.stdin.flush().await.map_err(TransportError::from)?;
        Ok(())
    }

    async fn receive(&mut self) -> Result<TransportMessage> {
        loop {
            self.line.clear();
            let n = self
                .stdout
                .read_until(b'\n', &mut self.line)
                .await
                .map_err(TransportError::from)?;
            if n == 0 {
                return Err(TransportError::ConnectionClosed.into());
            }
            let trimmed = trim_ascii(&self.line);
            if trimmed.is_empty() {
                continue;
            }
            // Skip any stray non-JSON line (e.g. a server log accidentally on
            // stdout) and resume until a real JSON-RPC frame arrives.
            match StdioTransport::parse_message(trimmed) {
                Ok(msg) => return Ok(msg),
                Err(_) => continue,
            }
        }
    }

    async fn close(&mut self) -> Result<()> {
        self.connected = false;
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.connected
    }

    fn transport_type(&self) -> &'static str {
        "child-stdio"
    }
}

/// Trim leading/trailing ASCII whitespace (incl. `\r`) from a byte line.
fn trim_ascii(bytes: &[u8]) -> &[u8] {
    let start = bytes.iter().position(|b| !b.is_ascii_whitespace());
    let Some(start) = start else { return &[] };
    let end = bytes
        .iter()
        .rposition(|b| !b.is_ascii_whitespace())
        .unwrap();
    &bytes[start..=end]
}

// ---------------------------------------------------------------------------
// Deterministic, network-free package fixtures.
// ---------------------------------------------------------------------------

mod fixtures {
    use pmcp_package::package::team::{HumanRole, TeamLimits, TeamMember, TeamRole};
    use pmcp_package::reference::ComponentType;
    use pmcp_package::slot::SlotType;
    use pmcp_package::{AgentPackage, ComponentRef, ConfigSlot, TeamPackage};
    use std::path::Path;

    pub const MEMBER_NAME: &str = "soloagent";

    fn agent_ref(name: &str) -> ComponentRef {
        ComponentRef::Range {
            name: name.to_string(),
            range: semver::VersionReq::parse("^1").unwrap(),
            component_type: ComponentType::Agent,
        }
    }

    pub fn member_pkg(name: &str) -> AgentPackage {
        AgentPackage {
            name: name.to_string(),
            version: semver::Version::parse("1.0.0").unwrap(),
            instructions: "A minimal offline team member.".to_string(),
            // Mandatory llm slot — the EnvVarResolver falls back to this tested
            // value (no env var, no network); the concrete factory is built but
            // never invoked by tools/list.
            llm: ConfigSlot::new(SlotType::LlmProvider {
                name: "primary-llm".to_string(),
                tested_value: "smoke-model".to_string(),
            }),
            max_tokens: 1024,
            max_iterations: 2,
            connectors: vec![],
            tool_selection: None,
            input_schema: None,
            output_schema: None,
            importance: None,
            finalizer_role: None,
            budget_defaults: vec![],
        }
    }

    /// A minimal, valid `TeamPackage`: one member + one human role. team-fs and
    /// mem-mcp use it for roster context; approval-mcp derives its ask family
    /// from the human role; team-mcp resolves the member from the data dir.
    pub fn team_package() -> TeamPackage {
        TeamPackage {
            name: "smoke-team".to_string(),
            version: semver::Version::parse("1.0.0").unwrap(),
            entry_point: agent_ref(MEMBER_NAME),
            members: vec![TeamMember {
                agent: agent_ref(MEMBER_NAME),
                role: TeamRole::EntryPoint,
            }],
            human_roles: vec![HumanRole {
                role: "reviewer".to_string(),
                description: "A human reviewer.".to_string(),
                responsibilities: vec![],
                channel_hints: vec![],
            }],
            limits: TeamLimits {
                max_team_depth: 2,
                max_team_total_tokens: 1,
                max_team_wall_clock_seconds: 1,
                poll_interval_ms: 1,
            },
            built_in_servers: vec![],
            finalizer_agents: vec![],
            budget_defaults: vec![],
            config_slots: vec![],
        }
    }

    /// Serialize the `TeamPackage` to `<dir>/team.json`; returns its path.
    pub fn write_team_package(dir: &Path) -> std::path::PathBuf {
        let path = dir.join("team.json");
        std::fs::write(&path, serde_json::to_vec(&team_package()).unwrap()).unwrap();
        path
    }

    /// Write the member `AgentPackage` to `<dir>/<name>.json` so the team-mcp
    /// binary's `LocalDirPackageResolver` can resolve it.
    pub fn write_member(dir: &Path) {
        std::fs::write(
            dir.join(format!("{MEMBER_NAME}.json")),
            serde_json::to_vec(&member_pkg(MEMBER_NAME)).unwrap(),
        )
        .unwrap();
    }
}

// ---------------------------------------------------------------------------
// Probe: spawn one dev binary, drive initialize + list_tools, reap it.
// ---------------------------------------------------------------------------

/// Spawn `exe` with `args`, drive the SDK stdio-client handshake
/// (`initialize` + `tools/list`) over the child's pipes with a bounded timeout,
/// then KILL + REAP the child before returning (no leak). Returns the advertised
/// tool names. Panics (with the binary name) on spawn/handshake/timeout failure.
async fn probe(label: &str, exe: &str, args: &[&str]) -> Vec<String> {
    let mut child = Command::new(exe)
        .args(args)
        // Silence the binary's tracing (its default writer is stdout, which
        // shares the JSON-RPC channel) so the wire stays clean.
        .env("RUST_LOG", "off")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap_or_else(|e| panic!("[{label}] failed to spawn {exe}: {e}"));

    let stdin = child.stdin.take().expect("child stdin piped");
    let stdout = child.stdout.take().expect("child stdout piped");
    let transport = ChildStdioTransport::new(stdin, stdout);
    let mut client = Client::new(transport);

    let result = tokio::time::timeout(Duration::from_secs(30), async {
        client.initialize(ClientCapabilities::default()).await?;
        let list = client.list_tools(None).await?;
        Ok::<_, pmcp::Error>(list.tools.into_iter().map(|t| t.name).collect::<Vec<_>>())
    })
    .await;

    // Teardown FIRST — before any assertion can panic — so no child leaks:
    // dropping the client closes the child's stdin (server sees EOF), then a
    // kill + implicit reap guarantees the process is gone.
    drop(client);
    let _ = child.kill().await;

    match result {
        Ok(Ok(names)) => names,
        Ok(Err(e)) => panic!("[{label}] MCP handshake failed: {e}"),
        Err(_) => panic!("[{label}] timed out waiting for tools/list over stdio"),
    }
}

#[tokio::test]
async fn all_four_dev_binaries_answer_tools_list_over_stdio() {
    // team-fs — advertises the 11 fs__* tools.
    {
        let data = tempfile::tempdir().unwrap();
        let pkg = fixtures::write_team_package(data.path());
        let tools = probe(
            "team-fs",
            env!("CARGO_BIN_EXE_team-fs"),
            &[
                "--stdio",
                "--package",
                pkg.to_str().unwrap(),
                "--data-dir",
                data.path().to_str().unwrap(),
            ],
        )
        .await;
        assert!(
            tools.iter().any(|t| t == "fs__list"),
            "team-fs must advertise fs__list; got {tools:?}"
        );
    }

    // mem-mcp — advertises the 6 mem__* tools.
    {
        let data = tempfile::tempdir().unwrap();
        let pkg = fixtures::write_team_package(data.path());
        let tools = probe(
            "mem-mcp",
            env!("CARGO_BIN_EXE_mem-mcp"),
            &[
                "--stdio",
                "--package",
                pkg.to_str().unwrap(),
                "--data-dir",
                data.path().to_str().unwrap(),
            ],
        )
        .await;
        assert!(
            tools.iter().any(|t| t == "mem__add"),
            "mem-mcp must advertise mem__add; got {tools:?}"
        );
    }

    // approval-mcp — advertises the unnamespaced resolve_approval + get_approval
    // plus one team_approval__ask_<role> per human role.
    {
        let data = tempfile::tempdir().unwrap();
        let pkg = fixtures::write_team_package(data.path());
        let tools = probe(
            "approval-mcp",
            env!("CARGO_BIN_EXE_approval-mcp"),
            &[
                "--stdio",
                "--package",
                pkg.to_str().unwrap(),
                "--data-dir",
                data.path().to_str().unwrap(),
            ],
        )
        .await;
        assert!(
            tools.iter().any(|t| t == "resolve_approval"),
            "approval-mcp must advertise resolve_approval; got {tools:?}"
        );
    }

    // team-mcp — advertises one team_mcp__<member> tool per roster member. Needs
    // the member AgentPackage resolvable in the data dir.
    {
        let data = tempfile::tempdir().unwrap();
        let pkg_dir = tempfile::tempdir().unwrap();
        let pkg = fixtures::write_team_package(pkg_dir.path());
        fixtures::write_member(data.path());
        let tools = probe(
            "team-mcp",
            env!("CARGO_BIN_EXE_team-mcp"),
            &[
                "--stdio",
                "--package",
                pkg.to_str().unwrap(),
                "--data-dir",
                data.path().to_str().unwrap(),
            ],
        )
        .await;
        assert!(
            tools.iter().any(|t| t.starts_with("team_mcp__")),
            "team-mcp must advertise a team_mcp__<member> tool; got {tools:?}"
        );
    }
}
