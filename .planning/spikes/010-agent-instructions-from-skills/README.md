---
spike: 010
idea: skills-positioning
name: agent-instructions-from-skills
type: standard
validates: "Given a pmcp-agent config referencing a skill URI on one of its servers, when resolved, then instructions compose base + fetched skill body (digest-verified, origin-tagged) and the engine's system prompt provably contains it."
verdict: VALIDATED
related: [008-sep-2640-drift-check, 009-workflow-skill-projection]
tags: [skills, pmcp-agent, pmcp-package, content-bound-approval, trust-boundary]
---

# Spike 010: Agent Instructions from MCP-Served Skills

## What This Validates

Given an AgentPackage-style skill reference `{connector, uri, digest}`, when
resolution fetches the SKILL.md over the connector's resource surface,
verifies the digest, and composes an origin-tagged block into
`ResolvedAgentConfig.instructions`, then the `AgentEngine` provably delivers
the skill body to the model as the system prompt — and tampered content is
refused before the loop starts.

## Research

- `ResolvedAgentConfig.instructions: String` → engine.rs:221 wires it
  verbatim into `CreateMessageParams.system_prompt`. The engine's own test
  suite's `ScriptSource` pattern showed how to mock `CompletionSource`; the
  spike's `CaptureSource` records the system prompt via a shared
  `Arc<Mutex<_>>`.
- SEP-2640 §Security Implications (from spike 008's capture of the current
  draft): skill content is untrusted model input; origin MUST be visible to
  the model; persisted approvals MUST be content-bound to the entry's
  digest set; resource reads driven by skill content MUST be origin-scoped.
- The WG explicitly scoped OUT "installable bundles (skills + servers +
  subagents + configuration as a single artifact)" — which is what
  pmcp-package already is. The packaging side of skills is ours to define.

## How to Run

```bash
cargo run --manifest-path .planning/spikes/010-agent-instructions-from-skills/Cargo.toml
```

## What to Expect

Five steps: the pack-time pin (proposed `[[skills]]` TOML), resolve-time
fetch + digest verification (including the tamper-refusal case), the
origin-tagged composition, the engine run proving
`system_prompt == composed` byte-equal, and the trust-rules analysis.
All assertions pass; exit 0.

## Investigation Trail

1. Modeled the AgentPackage addition as `SkillPin {connector, uri, digest}` —
   digest required, computed at pack time.
2. `fetch_and_verify` (~25 lines) reads the pinned URI through the shipped
   `Skills` ResourceHandler in-process (conventions pattern) and compares
   sha256 digests. The tamper case serves a modified body at the same URI
   ("ONLY if eligible" → "for every request" — a semantically hostile edit)
   and is refused with an "approval REVOKED" error.
3. `compose_instructions` wraps the verified body in
   `--- BEGIN SKILL (origin: MCP connector "billing", uri …, digest
   verified) ---` delimiters after the base instructions — the SEP's
   origin-tagging requirement realized in prompt text. Pure function,
   byte-equal on recomposition (replay-deterministic).
4. `CaptureSource` + `NoToolsInvoker` + `InMemoryStore` drive a real
   `AgentEngine::run`; the run completes and the captured
   `CreateMessageParams.system_prompt` equals the composed text byte-for-byte.
5. One harness iteration: the initial capture design tried to read the
   source back out of the engine (which owns its seams privately); switched
   to the `Arc<Mutex<Option<String>>>` shared-cell pattern.

## Results

**✓ VALIDATED — zero changes to pmcp-agent needed.** The whole resolve-time
layer is ~60 lines of SDK-ownable code: fetch → verify → compose →
`ResolvedAgentConfig::new` → engine delivers, byte-equal and origin-tagged.

**The load-bearing conceptual finding: the AgentPackage digest pin and
SEP-2640 content-bound approval are the same mechanism viewed from two
sides.** The package author is the approving user; packing is the approval
ceremony; a digest mismatch at resolve time is revocation (and for a
headless agent the only honest "re-prompt" is failing the run and requiring
a re-packed package — never fetch-and-continue). This hands the
"skills as distribution format for agent instructions" story a security
model for free, in exactly the bundling territory the WG scoped out of
SEP-2640 where pmcp-package is already ahead.

**Trust rules the SDK must encode:**
1. **System-prompt placement is a privilege decision.** Pinned skill → may
   compose into instructions/system prompt (author approved the bytes).
   Unpinned or `"resources": "dynamic"` → must NOT reach the system prompt;
   inject as a user-role turn or refuse to resolve.
2. **Digest mismatch = fatal ResolveError, pre-loop.**
3. **Origin-scoped reads**: a skill from connector "billing" may only cause
   reads against "billing"; `ToolCall.connector` is the enforcement point
   for supporting-file fetches (follow-up).
4. **Grow the pin to the full entry manifest** (`{uri, digest, size}` per
   file — spike 008's shape) once `skills/get` lands, so lazily-fetched
   supporting files verify exactly like SKILL.md.

**Recommended SDK shape:** optional `[[skills]]` slots on AgentPackage
(digest required; 0.x break acceptable per the audience philosophy);
resolver-side fetch/verify/compose in `pmcp-agent::config::resolver`.
