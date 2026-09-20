//! The semantic-golden harness — the parity authority every resource module
//! (Tasks 3–6) develops against.
//!
//! Each file in `tests/goldens/*.json` (non-recursive — `tests/goldens/pending/`
//! is deliberately NOT scanned, see that directory's README) embeds its own
//! inputs (`descriptor_toml`, `params`) plus the expected
//! [`support::normalize`]-d output (`normalized`). For every golden: parse
//! the descriptor + params, run [`pmcp_cfn_renderer::render`], normalize the
//! result the same way, and assert it equals the golden's `normalized`
//! field. A mismatch means either the renderer drifted from what a real
//! `cdk synth` produces, or the normalizer needs a fix — per the plan's
//! Global Constraints, "the golden wins" (fix the module, never the golden;
//! goldens only change by regenerating from cdk via
//! `scripts/generate-cfn-goldens.sh`).
//!
//! `all_goldens_match_renderer_output` runs unconditionally as of Task 3:
//! the `lambda`/`logs`/`outputs` modules (plus the base `iam` execution
//! role/policy) make [`pmcp_cfn_renderer::render`] match
//! `tests/goldens/plain-lambda.golden.json`. As of Task 4, `iam`'s declared
//! `[[iam.statements]]` expansion is also active, pinned by
//! `tests/goldens/iam-statements.golden.json` (a minimal, purpose-built
//! fixture) and `tests/goldens/wild-msr-vtt.golden.json` (a real `pmcp-run`
//! fixture with 4 declared statements, promoted from `pending/`). As of
//! Task 5, `http_api` is active too, pinned by
//! `tests/goldens/http-api.golden.json`. As of Task 6 — the last two
//! resource-family modules, `cognito` and `dynamodb` — the full 5-golden
//! corpus is active: `tests/goldens/oauth-cognito-dcr.golden.json` pins the
//! `aws-lambda` target's Cognito+DCR OAuth stack shape, and
//! `tests/goldens/pending/` is now empty.

mod support;

use std::path::PathBuf;

#[test]
fn all_goldens_match_renderer_output() {
    let golden_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/goldens");
    let mut checked = 0usize;
    for entry in std::fs::read_dir(&golden_dir).expect("tests/goldens exists") {
        let path = entry.expect("readable dir entry").path();
        if path.extension().map(|e| e == "json") != Some(true) {
            continue;
        }
        check_golden(&path);
        checked += 1;
    }
    assert!(
        checked >= 1,
        "no goldens found — run scripts/generate-cfn-goldens.sh"
    );
}

/// Render one golden's embedded descriptor/params, normalize the result,
/// and assert it matches the golden's precomputed `normalized` field.
fn check_golden(path: &std::path::Path) {
    let golden: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect("golden file readable"))
            .expect("golden file is valid JSON");

    let descriptor: pmcp_package::package::DeployDescriptor = toml::from_str(
        golden["descriptor_toml"]
            .as_str()
            .expect("descriptor_toml is a string"),
    )
    .expect("descriptor_toml parses as a DeployDescriptor");
    let params: pmcp_cfn_renderer::RenderParams =
        serde_json::from_value(golden["params"].clone()).expect("params deserializes");

    let rendered = pmcp_cfn_renderer::render(&descriptor, &params).expect("render succeeds");
    let rendered_v: serde_json::Value =
        serde_json::from_str(&rendered.to_canonical_json()).expect("canonical JSON parses");

    assert_eq!(
        support::normalize(&rendered_v),
        golden["normalized"],
        "golden mismatch: {}",
        path.display()
    );
}
