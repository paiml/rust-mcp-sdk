# Security Policy

## Reporting a vulnerability

**Please do not report security vulnerabilities through public GitHub issues,
discussions, or pull requests.**

Report privately through GitHub's private vulnerability reporting:

**[Report a vulnerability](https://github.com/paiml/rust-mcp-sdk/security/advisories/new)**

This creates a draft advisory visible only to you and the maintainers. It is the
preferred channel because it keeps the report, the discussion, the fix and the
eventual CVE in one place.

If you cannot use GitHub advisories, email **contact@paiml.com** with `SECURITY`
in the subject line.

### What to include

A report is most useful when it carries enough for us to reproduce it:

- the affected crate and version (`pmcp`, `cargo-pmcp`, `mcp-tester`, a toolkit
  or connector crate, …)
- what an attacker can achieve, and what access they need to start
- reproduction steps, ideally a minimal example or failing test
- affected configuration — transport (stdio / HTTP / streamable-HTTP / SSE),
  feature flags, and auth mode, since much of the surface is feature-gated

Redact credentials, tokens, and deployment identifiers. We do not need them to
reproduce a protocol- or library-level defect, and a public advisory must not
carry them.

### What to expect

We will acknowledge your report, tell you whether we consider it a vulnerability
and why, and keep you informed while a fix is prepared. If we disagree that a
report is a security issue we will say so plainly rather than let it go quiet.
We will credit you in the advisory unless you ask us not to.

Please give us a reasonable opportunity to release a fix before disclosing
publicly.

## Scope

This repository holds the `pmcp` Rust MCP SDK and the crates published from this
workspace — the `cargo-pmcp` CLI, `mcp-tester`, `mcp-preview`, the server toolkit
and its connectors, and the agent/package/workbook crates.

**In scope:** defects in this code — protocol parsing and validation,
authentication and OAuth handling, credential storage, transport handling,
sandbox or capability boundaries, and code generation or scaffolding that emits
insecure output.

**Out of scope for this repository:** vulnerabilities in a *service* built with
this SDK, or in a hosting platform that deploys servers written with it. Those
belong to whoever operates the service. If a platform's behaviour is only
exploitable because this SDK mishandles it, that part is in scope here — report
it and we will route what is not ours.

Reports that consist solely of automated scanner output, dependency advisories
with no demonstrated path to exploitation in this code, or missing hardening
headers on a site we do not operate are unlikely to be actionable.

## Supported versions

Security fixes land on the current minor release line of each crate. The `pmcp`
crate is at `2.20.x`; older lines are not patched. Pre-`1.0` crates in this
workspace (the `0.x` agent, package, toolkit, workbook and CLI crates) are
supported at their latest published version only.

## Dependency advisories

`cargo audit` runs as part of `make quality-gate` and in CI. An advisory in a
transitive dependency is tracked as ordinary maintenance rather than through this
policy, unless it is reachable and exploitable through this SDK's own API — in
which case please report it privately as above.
