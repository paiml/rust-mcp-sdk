# API Coverage — Phase 121 (Local Round-Trip E2E)

No external API integration: this phase is test-and-build-wiring only — its "backend" is an
in-process `wiremock::MockServer` mounted twice against canned responses, it adds no API client,
no new external service, and no network call on any path (SC1 requires it to run fully offline).

## Why the detector fires anyway

The deterministic scan matches the noun `api` inside the prose phrase "a real TfL **API** key",
which appears in plan 121-02's and 121-03's `must_haves.prohibitions` blocks — a prohibition
*forbidding* a credential from entering the repository, not a capability being integrated. The
detector is deterministic, not infallible; this declaration is the reasoned no-integration
outcome the checkpoint prescribes for exactly that case.

## Confirmation, re-read from the phase scope

- CONTEXT.md `<domain>`: *"This is a **test-only** phase. It adds one dev-dependency and test
  files. It changes no production API: no new toolkit resolution path, no new `pmcp-package`
  surface, no manifest schema change."*
- ROADMAP.md Phase 121 SC1 requires the round trip to run *"fully offline against a `wiremock`
  backend with no live network."*
- RESEARCH.md §Environment Availability: *"Network access — Required By: **Nothing**. The whole
  phase is offline by requirement."*
- RESEARCH.md §Package Legitimacy Audit: the only dependency added is an in-repo path dep
  (`pmcp-package`) plus a version-line reuse of `toml`; no registry surface is integrated.

## The TfL REST surface is not this phase's to decide

`pmcp-openapi-server` reaches the TfL Unified API through configuration
(`crates/pmcp-openapi-server/tests/fixtures/london-tube.toml`, `examples/london-tube.toml`),
and that surface was decided when the fixture and its OpenAPI spec were authored in earlier
phases. Phase 121 adds **zero** capability against it: it exercises the two already-curated
tools (`get-tube-status`, `disrupted-lines-with-detail`) plus the two the toolkit synthesizes,
entirely against mock responses, and integrates nothing new. Enumerating a TfL capability matrix
here would fabricate decisions this phase has no authority to make and no code to back.

No rows follow, by design.
