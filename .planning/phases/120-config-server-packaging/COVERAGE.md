# Phase 120 — API Coverage Decision Checkpoint

**Detector:** `api-coverage.cjs` returned `detected: true` on a single signal —
`verb: "wire"`, `noun: "api"`, from the snippet
`"wire tolerance while \`BinaryMode\` is the non-optional API type)"` in `120-01-PLAN.md`.

**Confirmed against the phase scope: false positive.** The match is the noun "API" used to
mean "Rust API type", next to the verb "wire" used to mean "serialized wire format" — neither
in the sense of integrating an external service.

No external API integration: Phase 120 is local OCI-layout packaging plus TOML config parsing
— every operation is on-disk and offline (`pmcp-package` has no network variant in
`PackageError` and milestone Decision 2 forbids an OCI registry client), and the OpenAPI spec
this phase carries is treated as opaque bytes that are never parsed, so no third-party
capability surface exists to enumerate or opt out of.

Supporting evidence from the phase scope:

- `crates/pmcp-package/src/oci/layout.rs` module doc: "Pure local-disk I/O — no network calls."
- CONTEXT.md D-07: unpacking a referenced package never resolves the blob — resolution is the
  target environment's job, explicitly out of scope for this crate.
- CONTEXT.md D-16: the spec layer carries "whatever the author supplied, byte-for-byte";
  `pmcp-package` never parses it.
- The one HTTP-adjacent change in the phase (plan 120-04's `backend.base_url` `${VAR}`
  expansion) resolves an environment variable into an existing, already-integrated toolkit
  code path; it adds no new external capability and no new endpoint.

The nearest thing to an external surface in this phase is the `pmcp-openapi-server` London
Tube fixture's TfL backend, and it is exercised only through `wiremock` offline
(`parity_replay.rs`) plus one env-gated live test that skips by default — a pre-existing test
harness, not an integration this phase introduces.
