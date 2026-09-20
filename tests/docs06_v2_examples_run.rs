//! Phase 119 (DOCS-06): **proof that the shipped v2 (2026-07-28) examples work
//! TOGETHER over a real socket** — `s47_v2_stateless_mrtr` is spawned as a live
//! server and both documented client examples, `s48_v2_mrtr_client` and
//! `s53_v2_agent_client`, are driven against it as real peer processes.
//!
//! # Why this file exists at all
//!
//! DOCS-06's claim is "runnable v2 examples ship and pass". `make test-examples`
//! only BUILDS examples — its own banner says so — and a compiled server that
//! never answers a request is indistinguishable from a working one at build time.
//! `s47_v2_stateless_mrtr` in particular demonstrates NOTHING on its own: it is a
//! server, and every property the docs attribute to it (no `initialize`
//! handshake, no `Mcp-Session-Id`, a continuation resumed across an independent
//! HTTP request) is a property of an exchange, not of a binary. That is why the
//! phase pairs it with its two client examples here rather than asserting on it
//! alone.
//!
//! The sibling file `tests/docs04_examples_run.rs` covers the examples that bind
//! nothing and prove themselves by their stdout. This one covers the shape that
//! needs a port, and it is deliberately a SEPARATE file so that all the port
//! reasoning lives in one place.
//!
//! # Two harness idioms in one file, on purpose
//!
//! The server leg uses `spawn_example` + `wait_until_listening` +
//! `wait_until_released`: it discards the child's streams, hands back a reaping
//! guard, and the evidence that it started is the accepted TCP connection.
//!
//! The two client legs use `run_example_to_completion` instead, because they are
//! themselves example BINARIES rather than in-test HTTP calls — the documented
//! reader experience is `cargo run --example s48_v2_mrtr_client`, so that is what
//! gets run. `spawn_example` would be the wrong tool for them twice over: it
//! discards both streams (and their stdout IS the evidence) and it returns a
//! guard rather than an exit status (and their exit status is the first thing
//! asserted).
//!
//! Do not "unify" the two idioms. They answer different questions.
//!
//! # Port 8161, deliberately
//!
//! Ports 8147 (`s47_v2_stateless_mrtr`'s own default, and the default both client
//! examples fall back to), 8149, 8150, 8151, 8153, 8155, 8157 and 8159 are
//! already claimed across `tests/`, `scripts/` and `examples/`. 8161 is the next
//! free slot in that family and nothing else in the repo mentions it.
//!
//! Because s47's own default 8147 is claimed, this leg passes the address
//! EXPLICITLY as `argv[1]` — to the server and to both clients — rather than
//! relying on any of the three defaults agreeing.
//!
//! # Why both client legs run before either is asserted
//!
//! Asserting the `s48` outcome before `s53` has executed would leave half the
//! DOCS-06 claim with zero executed evidence exactly when something is wrong.
//! Both clients therefore run to completion and both outcomes are written to a
//! JSON artifact under `target/` BEFORE the first assertion fires, so a red on
//! either client is diagnosed against a recording of both. This is the same house
//! rule `tests/embedded_resource_example_run.rs` follows.
//!
//! ONE case is outside that rule, stated rather than implied: a client that
//! never EXITS. `run_example_to_completion` panics on its own deadline, so an
//! `s48` that hangs aborts this test before `s53` runs and before the artifact
//! is written. Nothing is lost by it — that panic already carries the hung
//! client's partial stdout and stderr, which is the same evidence the artifact
//! would have held for that leg — but do not read the artifact's absence as
//! "the legs did not run". A timeout red is diagnosed from the panic body.
//!
//! # Why the client legs need a budget at all
//!
//! These two are the highest-risk callers of `run_example_to_completion` in the
//! phase: each drives a live socket peer. `wait_until_listening` bounds only the
//! BIND — a server that accepts the connection and then never answers would leave
//! an unbounded client waiting forever, hanging the integration suite rather than
//! failing it. Nothing bounds the exchange that follows except `S48_TIMEOUT` and
//! `S53_TIMEOUT`. On expiry the helper kills AND reaps the child and panics with
//! both partial streams.
//!
//! # The `PMCP_REQUEST_STATE_KEY` startup warning
//!
//! On startup s47 emits a real `WARN` before its banner: with no key set it
//! generates a fresh PER-PROCESS key, so a multi-round-trip follow-up landing on
//! a different instance behind a load balancer cannot be resumed and is
//! re-elicited from scratch. This test deliberately does NOT set that variable, so
//! the leg exercises the default per-process-key path — which is also the path a
//! reader following the docs gets. No key value appears anywhere in this file.
//!
//! The warning itself is NOT asserted on here: `spawn_example` discards the
//! child's streams, so the test cannot see it. It is recorded in this header
//! because it is the source text plan 119-05 promotes into the migration
//! chapter's server track, and a later reader looking for where that text is
//! verified should find this note rather than assume a missing assertion.
#![cfg(all(
    feature = "streamable-http",
    feature = "http-client",
    not(target_arch = "wasm32")
))]

mod common;

use common::example_process::{
    assert_ran_and_printed_banner, run_example_to_completion, spawn_example, target_dir,
    wait_until_listening, wait_until_released, ExampleLeg,
};
use serde_json::json;
use std::process::Output;
use std::time::Duration;

/// The stateless-MRTR server example's compiled path, relative to the target dir.
///
/// A root `examples/` binary requiring the two transport features, so
/// `cargo build --features full --example s47_v2_stateless_mrtr` produces it.
const S47_REL_PATH: &str = "debug/examples/s47_v2_stateless_mrtr";

/// The MRTR client example's compiled path, relative to the target dir.
///
/// This is the client the s47 header itself tells the reader to run in a second
/// terminal, which is precisely why it is the peer this leg drives.
const S48_REL_PATH: &str = "debug/examples/s48_v2_mrtr_client";

/// The `pmcp-agent` connector client example's compiled path.
///
/// The second, independent peer: it reaches the same server through the agent
/// connector rather than the MRTR client API, so a green here means the server's
/// v2 surface answers more than one client shape.
const S53_REL_PATH: &str = "debug/examples/s53_v2_agent_client";

/// See the module header for why this port and not another.
const BIND_ADDR: &str = "127.0.0.1:8161";

/// Where the recorded client outcomes land, for the SUMMARY to quote verbatim.
const ARTIFACT_REL_PATH: &str = "119-04-v2-example-run.json";

/// How long the server gets to bind its socket before the leg gives up.
const READY_TIMEOUT: Duration = Duration::from_secs(30);

/// How long the port gets to become free again after the server is killed.
///
/// A short budget on purpose: this one is not guarding against slowness, it is
/// asserting that the teardown TORE DOWN. A listener still answering ten seconds
/// after its process was killed and reaped is a leak, and the honest place to
/// discover it is here rather than in the next run's bind failure.
const RELEASE_TIMEOUT: Duration = Duration::from_secs(10);

/// The budget for the `s48_v2_mrtr_client` leg.
///
/// The exchange is three demonstrations over loopback against an in-process
/// server, measured well under a second. Sixty seconds is orders of magnitude
/// above that, deliberately: the budget exists to convert a HANG — a server that
/// accepted the connection and then stopped answering — into a red, not to police
/// performance.
///
/// UNIT NOTE: `from_mins`, not the equivalent `Duration::from_secs(60)`, because
/// `make lint` runs clippy's nursery group and
/// `clippy::duration_suboptimal_units` rejects a whole-minute duration written in
/// seconds. The same note is recorded on `S50_TIMEOUT` in
/// `tests/docs04_examples_run.rs`.
const S48_TIMEOUT: Duration = Duration::from_mins(1);

/// The budget for the `s53_v2_agent_client` leg.
///
/// A SEPARATE constant from [`S48_TIMEOUT`] even though the two currently share a
/// value. They are separate legs against separate binaries with different client
/// stacks, and a future retune of one must not silently retune the other. Per the
/// `example_process` module header's rule, both live in THIS file and are never
/// imported from a shared location.
///
/// Written in minutes for the lint reason recorded on [`S48_TIMEOUT`].
const S53_TIMEOUT: Duration = Duration::from_mins(1);

/// The line both client examples print once all three of their demonstrations
/// have behaved as their documentation says.
///
/// It is the last thing either binary prints before returning `Ok(())`, so it is
/// the clients' own statement that no demonstration was skipped — a stronger
/// claim than the exit status alone, which a client that returned early would
/// also satisfy.
const CLIENT_BANNER: &str = "All three demonstrations behaved as documented.";

/// The argv both client legs are invoked with.
///
/// Shared by the run and by [`record`] so the artifact cannot describe an
/// invocation that did not happen — the artifact's whole value is being a
/// trustworthy record of what actually ran.
const CLIENT_ARGS: &[&str] = &[BIND_ADDR];

/// Turn one client's captured `Output` into the artifact's record of that leg.
///
/// Both streams go in as TEXT, not just the one asserted on: a red whose stdout
/// says nothing usually has the reason on stderr.
fn record(rel_path: &str, args: &[&str], output: &Output) -> serde_json::Value {
    json!({
        "binary": format!("target/{rel_path}"),
        "args": args,
        "exit_status": output.status.to_string(),
        "exit_success": output.status.success(),
        "stdout": String::from_utf8_lossy(&output.stdout),
        "stderr": String::from_utf8_lossy(&output.stderr),
    })
}

/// Assert one client leg exited 0 and printed its completion banner.
///
/// Status FIRST, then the banner: a client that died mid-exchange produces a
/// banner mismatch whose message says nothing about the death, so the crash has
/// to be reported AS a crash. Every message names the leg, states what was
/// expected, and carries the captured body — a red that says only
/// `assertion failed` is a red that says nothing.
fn assert_client_leg(rel_path: &str, output: &Output) {
    assert_ran_and_printed_banner(
        &ExampleLeg {
            rel_path,
            banner: CLIENT_BANNER,
            rebuild: "`cargo build --features full --example s47_v2_stateless_mrtr --example \
                      s48_v2_mrtr_client --example s53_v2_agent_client`",
            claim: "DOCS-06 claims these v2 examples ship AND pass, so a non-zero exit here is \
                    a broken documented command.",
            banner_means: "That line is printed only after all three of the client's \
                           demonstrations have run against the live server, so its absence means \
                           the round trip did not complete — which is the whole of what DOCS-06 \
                           claims about this pair.",
        },
        output,
    );
}

/// The stateless v2 server example serves BOTH documented v2 client examples,
/// each run as a real peer process over a real socket.
#[tokio::test]
async fn s47_serves_both_v2_client_examples_end_to_end() {
    let (addr, mut guard) = spawn_example(S47_REL_PATH, BIND_ADDR);
    wait_until_listening(addr, &mut guard, READY_TIMEOUT).await;

    // BOTH client legs run before EITHER is asserted — see the module header.
    let s48 = run_example_to_completion(S48_REL_PATH, CLIENT_ARGS, S48_TIMEOUT);
    let s53 = run_example_to_completion(S53_REL_PATH, CLIENT_ARGS, S53_TIMEOUT);

    let artifact = json!({
        "note": format!(
            "Two v2 (2026-07-28) client examples driven as real peer processes against \
             target/{S47_REL_PATH} bound to {BIND_ADDR}. No `initialize` handshake and no \
             `Mcp-Session-Id` are involved on the v2 path. Phase 119-04, DOCS-06."
        ),
        "server": format!("target/{S47_REL_PATH}"),
        "bind_addr": BIND_ADDR,
        "s48_v2_mrtr_client": record(S48_REL_PATH, CLIENT_ARGS, &s48),
        "s53_v2_agent_client": record(S53_REL_PATH, CLIENT_ARGS, &s53),
    });
    let artifact_path = target_dir().join(ARTIFACT_REL_PATH);
    std::fs::write(
        &artifact_path,
        serde_json::to_string_pretty(&artifact).expect("the artifact always serializes"),
    )
    .unwrap_or_else(|error| panic!("could not write {}: {error}", artifact_path.display()));

    assert_client_leg(S48_REL_PATH, &s48);
    assert_client_leg(S53_REL_PATH, &s53);

    drop(guard);
    wait_until_released(addr, RELEASE_TIMEOUT).await;
}
