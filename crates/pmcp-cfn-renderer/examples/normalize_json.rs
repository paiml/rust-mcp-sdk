//! `cargo run -p pmcp-cfn-renderer --example normalize_json < template.json`
//!
//! Reads a CFN template as JSON on stdin, runs it through the exact same
//! [`support::normalize`] the semantic-golden harness (`tests/semantic_golden.rs`)
//! uses, and writes the normalized, pretty-printed JSON to stdout.
//!
//! This is `scripts/generate-cfn-goldens.sh`'s normalizer entry point: the
//! script never reimplements the normalization algorithm, it shells out to
//! this binary so a real `cdk synth` template is normalized with the SAME
//! Rust code the harness compares the renderer's own output against.
//!
//! Shares `tests/support/mod.rs` via `#[path]` rather than duplicating it —
//! see that module's doc comment for the algorithm.

#[path = "../tests/support/mod.rs"]
mod support;

use std::io::{self, Read, Write};

fn main() {
    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .expect("failed to read stdin");

    let template: serde_json::Value =
        serde_json::from_str(&input).expect("stdin is not valid JSON");
    let normalized = support::normalize(&template);

    let output = serde_json::to_string_pretty(&normalized)
        .expect("a normalized serde_json::Value always serializes");
    io::stdout()
        .write_all(output.as_bytes())
        .expect("failed to write stdout");
    println!();
}
