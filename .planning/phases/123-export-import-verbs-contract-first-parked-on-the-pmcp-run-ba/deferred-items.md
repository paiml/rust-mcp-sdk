# Phase 123 — deferred items

Out-of-scope discoveries made while executing this phase. Each was MEASURED, not
suspected. None is fixed here, and none was caused by this phase's changes.

---

## D1. `cargo-pmcp/fuzz/` is compiled by NO gate, and `make test-fuzz` cannot fail

**Found during:** plan 123-07 Task 1, while landing `fuzz_package_artifact`.

**Measured.** `make test-fuzz` (Makefile `:730-740`) does two things that together
make it a non-gate:

1. It is scoped to the ROOT fuzz tree only — `if [ -d "fuzz" ]; then cd fuzz &&
   ...`. `cargo-pmcp/fuzz/` is never visited, so none of its **ten** targets
   (nine pre-existing plus this phase's `fuzz_package_artifact`) is built or run
   by `make quality-gate`.
2. Every invocation is `timeout 30s $(CARGO) fuzz run $$target || echo "Fuzz
   target $$target completed"`. The `|| echo` swallows every non-zero exit, so a
   target that fails to COMPILE, a crash, and a clean run are indistinguishable
   at the recipe's exit status. It also calls `$(CARGO) fuzz` without
   `+nightly`, which cannot work on a stable default toolchain — and that
   failure is swallowed by the same `||`.

**Why it is not fixed here.** Making it blocking requires deciding that a
nightly toolchain is a hard prerequisite of `make quality-gate` on every
developer machine and in CI. That is a Phase-level decision with CI
implications, not a plan-07 cleanup. Plan 07's fuzz evidence is therefore a
HAND-RUN campaign with its command lines and counts recorded in
`123-07-SUMMARY.md`, and the SUMMARY says plainly that no gate re-runs it.

**Owner:** whoever next touches the ALWAYS-requirements gating. The narrow
version — a build-only `cargo +nightly fuzz build --fuzz-dir cargo-pmcp/fuzz`
leg — is cheap and was measured green (exit 0, 2m47s cold) during plan 07, but
it hard-requires nightly.

---

## D2. `cargo-pmcp/fuzz/fuzz_targets/fuzz_widgets_config.rs` is not rustfmt-clean

**Found during:** plan 123-07 Task 1, running `cargo fmt --manifest-path
cargo-pmcp/fuzz/Cargo.toml -- --check`.

One pre-existing diff at `fuzz_widgets_config.rs:44` (a `let` binding rustfmt
wants on one line). It has gone unnoticed because the fuzz crate carries its own
`[workspace]` table, so root `cargo fmt --all` does not reach it and neither
does `make fmt-check`. Not introduced by this phase; the new
`fuzz_package_artifact.rs` is clean under the same check.

---

## D3. `cargo-pmcp/examples/deploy_stack_metadata.rs` carries an unused import

**Found during:** plan 123-07 Task 2, adding the `build-cargo-pmcp-examples`
gate leg.

`warning: unused import: MetadataConfig` at `deploy_stack_metadata.rs:27:52`.
Pre-existing, and the reason the new leg pins `RUSTFLAGS=` — CI exports
`RUSTFLAGS: -D warnings`, which would turn this warning into a red gate over rot
the leg was never meant to lint. Fixing it is a one-line deletion someone should
do deliberately, with the leg then free to stop pinning.

---

## D4. The example gate still covers only four of the workspace's example trees

**Found during:** plan 123-07 Task 2.

`scripts/run-example-builds.sh` covers three trees (root `pmcp`, `pmcp-agent`,
`pmcp-team-servers`) and its own header records `cargo-pmcp`, the toolkit crates
and `mcp-tester` as "ALSO NOT COVERED", noting that widening to `--workspace
--examples` "is cheap to attempt but was not measured, so it is not claimed
here". Plan 123-07 measured and claimed the `cargo-pmcp` subset only
(`cargo build -p cargo-pmcp --examples`, exit 0) and added a leg for exactly
that. The remaining members are still unmeasured and still ungated.
