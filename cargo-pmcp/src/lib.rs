//! cargo-pmcp library — loadtest and pentest types plus a narrow subset of
//! the deployment layer (`deployment::config` + `deployment::iam`) for external
//! use by fuzz targets, integration tests, and examples, plus a narrow
//! integration-test seam onto `auth_cmd::cache`.
//!
//! The full `deployment` module tree lives in the bin target and transitively
//! depends on `commands::*`; exposing only the two deployment submodules that
//! Phase 76 Wave 5's fuzz target + example need keeps the lib surface minimal.

pub mod loadtest;
pub mod pentest;

// Phase 76 Wave 5: expose `deployment::config` + `deployment::iam` to the lib
// target. These two modules only cross-depend on each other and on
// `utils::config`, so they can be mounted via `#[path]` without pulling in the
// rest of the `deployment::*` tree (which references `crate::commands::*`,
// bin-only).
pub mod deployment {
    //! Narrow lib-visible view of the deployment subsystem: `config`, `iam`,
    //! `widgets`, and `post_deploy_tests`. The full module is in the bin
    //! target; this surface is sufficient for the Phase 76 fuzz target and
    //! the `deploy_with_iam` example, plus the Phase 79 Wave-1 schema types
    //! that `config.rs` references via `use crate::deployment::widgets::*`
    //! and `use crate::deployment::post_deploy_tests::*`.

    #[path = "../deployment/config.rs"]
    pub mod config;

    // Re-export the schema types the mounted Cloud Run Dockerfile generator
    // references via `crate::deployment::*` (`dockerfile.rs` imports
    // `crate::deployment::{DeployConfig, LayoutConfig}`).
    pub use config::{DeployConfig, LayoutConfig};

    #[path = "../deployment/iam.rs"]
    pub mod iam;

    /// Narrow lib-visible view of the Google Cloud Run Dockerfile generator.
    ///
    /// The full `targets::*` tree is bin-only (it cross-depends on
    /// `commands::*`), but the Dockerfile / cloudbuild rendering for Cloud Run
    /// only needs `deployment::config` + a sibling `env` helper. Mounting just
    /// these two leaf files via `#[path]` lets the env-gated
    /// `cloud_run_local_build` integration test render the multi-crate-isolated
    /// Dockerfile via the real generator (issue #258) without pulling in the
    /// command layer.
    #[path = "../deployment/targets/google_cloud_run"]
    pub mod google_cloud_run {
        pub mod env;

        pub mod dockerfile;
    }

    // Phase 79 Wave 1: schema types required by `config.rs` so the lib
    // target compiles. These modules are leaf — they cross-depend only on
    // serde and stdlib, so mounting them here does not drag in any further
    // bin-only `commands::*` references.
    #[path = "../deployment/widgets.rs"]
    pub mod widgets;

    #[path = "../deployment/post_deploy_tests.rs"]
    pub mod post_deploy_tests;

    // `widgets::enumerate_workspace_bin_crates` delegates here.
    #[path = "../deployment/naming.rs"]
    pub mod naming;
}

pub mod utils {
    //! Narrow lib-visible view of `utils::config` so `deployment::config` can
    //! resolve `crate::utils::config::WorkspaceConfig`.

    #[path = "../utils/config.rs"]
    pub mod config;
}

// Compiled via `#[path]` to bypass the bin-only `commands::auth_cmd` tree,
// which cross-depends on the CLI subsystem and cannot compile in the lib
// target without pulling in the entire command layer.
#[doc(hidden)]
#[path = "commands/auth_cmd/cache.rs"]
pub mod test_support_cache;

// Compiled via `#[path]` to bypass the bin-only `commands::configure` tree.
// Mirrors the test_support_cache pattern (see lib.rs above for the established convention).
// Only the leaf `config.rs` schema is bridged — the full configure command tree stays bin-only.
#[doc(hidden)]
#[path = "commands/configure/config.rs"]
pub mod test_support_configure;

#[doc(hidden)]
pub mod test_support {
    pub use crate::test_support_cache as cache;
    pub use crate::test_support_configure as configure_config;
}

// WBCL-05: expose ONLY the `workbook-server` scaffold emitter to the lib target
// (mirrors the `test_support_cache` `#[path]` convention). The full `templates`
// tree is BIN-ONLY (its siblings reference `crate::commands::*`); this single
// leaf is dependency-light (std::fs + include_dir + format! + colored), so it
// compiles in the lib target without dragging in the command layer. The
// `workbook_server_scaffold` example and the `workbook_scaffold` integration
// test reach `generate` through this seam, NOT the bin-only `templates::*`.
#[path = "templates/workbook_server.rs"]
pub mod templates_workbook_server;

// CLI-01 (Phase 110-02): expose ONLY the `agent new` scaffold emitter to the lib
// target (mirrors the `templates_workbook_server` `#[path]` convention). This
// leaf is dependency-light (std::fs + format! + colored + pmcp-package + semver;
// NO `clap`/`GlobalFlags`), so it compiles in the lib target without dragging in
// the bin-only command layer. It lets the emitter's drift-guard + manifest
// round-trip unit tests run under `cargo test --lib`, NOT only in the bin target.
// `#[doc(hidden)]` (Codex 110-06 MEDIUM): an internal support seam reached by the
// plan-110-06 example, not a stable public API.
#[doc(hidden)]
#[path = "templates/agent.rs"]
pub mod templates_agent;

// CLI-02 (Phase 110-03): expose ONLY the lib-safe fixed-source agent runner to the
// lib target (mirrors the `templates_agent` `#[path]` convention). `run.rs` is a
// LEAF that references only `pmcp-agent` + `pmcp` types + std (NO `clap` /
// `GlobalFlags` / the bin-only `commands::*` tree), so it compiles in the lib
// target on its own. The offline `agent_dev` integration test (and the plan-110-06
// example) reach `run_fixed_source` through this seam, NOT the bin-only
// `commands::agent::run` module.
// `#[doc(hidden)]` (Codex 110-06 MEDIUM): an internal support seam reached by the
// plan-110-06 example + the offline integration test, not a stable public API.
#[doc(hidden)]
#[path = "commands/agent/run.rs"]
pub mod agent_run;

// WBV2-06: expose ONLY the PURE `workbook explain` tool-surface projection + render
// to the lib target (mirrors the `templates_workbook_server` `#[path]` convention).
// This leaf is dependency-light (pmcp-workbook-compiler `ingest`/`synth` + serde +
// anyhow; NO `clap`/`GlobalFlags`), so it compiles in the lib target without dragging
// in the bin-only command layer. The `workbook_explain` example + the
// `workbook_explain` integration test reach `explain_workbook`/`format_tool_surface`
// through this seam, NOT the bin-only `commands::workbook::explain` arm.
#[path = "commands/workbook/explain_surface.rs"]
pub mod workbook_explain;

// CLI-04 (Phase 110-05): expose ONLY the PURE package-kind + manifest-parse leaf
// to the lib target (mirrors the `agent_run` `#[path]` convention). `kind.rs`
// references only `pmcp-package::oci::media_types` + `serde_json` + std (NO
// `clap`/`GlobalFlags`/`OciLayout`), so it compiles in the lib target on its own.
// This lets `detect_kind`'s proptest + `artifact_type_from_manifest_json`'s
// never-panic unit tests run under `cargo test --lib`, AND gives plan 110-06 a
// lib seam to mount + fuzz the untrusted manifest-parse boundary — NOT the
// bin-only `commands::package::kind` module.
// `#[doc(hidden)]` (Codex 110-06 MEDIUM): an internal support seam mounted + fuzzed
// by the plan-110-06 fuzz target, not a stable public API.
#[doc(hidden)]
#[path = "commands/package/kind.rs"]
pub mod package_kind;

// PKGX-02 (Phase 123-01): expose ONLY the `.tar` <-> OCI-layout codec to the lib
// target (mirrors the `package_kind` `#[path]` convention immediately above).
// `artifact.rs` references only `tar`, `anyhow`, `serde_json`, `oci_spec`,
// `tempfile`, `pmcp_package` and std (NO `clap`/`GlobalFlags`/the bin-only
// `commands::*` tree), so it compiles in the lib target on its own.
//
// This seam exists so the module's property tests run under `cargo test --lib`
// and so a fuzz target can point straight at `read_verified` — the
// untrusted-bytes boundary, where a `.tar` handed to `package load`/`pull`
// arrives with no known provenance. It is ALSO what forbids `super::` and
// `crate::commands::` inside that file: here its parent is the crate root,
// which declares no `commands` module. That constraint is why
// `install_layout` takes its semantic gate as a closure.
// `#[doc(hidden)]`: an internal support surface, not a stable public API.
#[doc(hidden)]
#[path = "commands/package/artifact.rs"]
pub mod package_artifact;

// PKGX-02 (Phase 123-03): expose the ONE human-text report renderer that the
// local read verbs share, mirroring the `package_kind`/`package_artifact`
// `#[path]` convention immediately above.
//
// This seam is what makes the renderer testable AND shareable. Two things
// depend on it and neither works without it: the module's own unit tests run
// under `cargo test -p cargo-pmcp --lib` (declared only in
// `commands/package/mod.rs`, they would compile into the bin tree, where no
// `--lib` run and no `tests/` binary can see them — they would simply be
// ABSENT rather than failing), and plan 05's lib-mounted `pull` pipeline calls
// the same renderer `load` calls, which is what keeps the two verbs on one
// output shape by construction rather than by discipline.
//
// `render.rs` references only `pmcp_package` types and std — NO `clap`, NO
// `GlobalFlags`, NO `crate::commands::*` — which is exactly what lets it
// compile in the lib target on its own, where the crate root declares no
// `commands` module. It is also why the file may never name `super::`.
//
// `#[doc(hidden)]`: an internal support surface, not a stable public API.
#[doc(hidden)]
#[path = "commands/package/render.rs"]
pub mod package_render;

// PKGX-02 (Phase 123-05): expose the `package pull` PIPELINE — stages 2 through
// 6 plus the ONE transport seam — to the lib target, mirroring the
// `package_artifact` / `package_render` `#[path]` convention immediately above.
//
// This mount is what makes the seam REACHABLE, and that is not a style point.
// `lib.rs` declares no `mod commands`, so a pipeline declared only under
// `commands/package/` is compiled into the BIN target and nowhere else; a file
// under `cargo-pmcp/tests/` is an external crate linking the LIB and can neither
// see a bin-private module nor implement a trait declared in one. Without this
// line `tests/package_portability_contract.rs` cannot substitute a fake
// transport, and `pull`'s verification half — the security-relevant half —
// would be claimed rather than exercised.
//
// It is mounted into the LIB ONLY, and deliberately NOT declared in
// `commands/package/mod.rs`. A second declaration would compile the file twice
// and split `ArtifactTransport` into two incompatible types, so the bin's live
// implementation and the test's fake one would stop being interchangeable —
// which is the whole point of having one seam.
//
// `pull_pipeline.rs` references only `anyhow`, `serde_json`, `pmcp_package`,
// `async-trait`, std and the sibling lib mounts (`package_artifact`,
// `package_kind`, `package_render`, `pmcp_run_graphql`) — NO `clap`, NO
// `GlobalFlags`, NO `reqwest`, NO `crate::commands::*` — which is exactly what
// lets it compile in the lib target, where the crate root declares no `commands`
// module. It is also why the file may never name `super::`.
// `#[doc(hidden)]`: an internal support surface, not a stable public API.
#[doc(hidden)]
#[path = "commands/package/pull_pipeline.rs"]
pub mod package_pull_pipeline;

// Package-capture contract test seam (170-08 Task 3; extended in the
// final-review fix wave with pure SDL-extraction helpers): expose the two
// dependency-light capture GraphQL query consts, plus the pure,
// IO-free SDL-extraction helpers (`extract_capture_sdl`,
// `assert_capture_ops_present`, `strip_provenance_header`), to the lib target
// (mirrors the `package_kind` / `templates_agent` `#[path]` convention).
// `graphql_contract.rs` references only `&str` literals plus `anyhow`/
// `serde_json` (NO `reqwest`/oauth2/the bin-only `pmcp_run` auth+deploy tree
// that the rest of `deployment/targets/pmcp_run/graphql.rs` depends on), so
// it compiles in the lib target on its own. This lets the offline blocking
// contract test (`tests/package_capture_contract.rs`) validate the real
// runtime `submitPackageCapture`/`getPackageCaptureStatus` queries against
// the vendored SDL (`contracts/pmcp-run/capture-v1.graphql`) without pulling
// in the bin-only command layer, AND lets the feature-gated `capture_contract`
// dev binary (`src/bin/capture_contract.rs`) reuse these pure helpers (and
// their unit tests run under the default `cargo test -p cargo-pmcp`) without
// linking the bin-only `deployment` module tree.
// `#[doc(hidden)]`: an internal test-facing seam, not a stable public API.
#[doc(hidden)]
#[path = "deployment/targets/pmcp_run/graphql_contract.rs"]
pub mod pmcp_run_graphql;
