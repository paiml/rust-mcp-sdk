# Package Portability & Audit — Design Note (SDK ⇄ Platform)

**Date:** 2026-07-21
**Status:** Proposal — for review by both the SDK team and the pmcp.run platform team
**Builds on:** the shipped `pmcp-package` format (content-addressed OCI), the
`capture` / `import` / `approve` verbs (cargo-pmcp 0.19.0), and the
capture contract seam (`contracts/pmcp-run/capture-v1.graphql`).

---

## 1. Motivation

A captured `pmcp-package` today is born in a platform's ECR and never leaves
it. That is enough for *deployment* (capture → approve → import across
environments) but not for *trust*: security review, policy compliance,
independent testing, and ultimately a marketplace ("agent store") all require
that a package be a **portable artifact a third party can obtain, verify, and
examine** — without platform credentials and without executing anything they
haven't inspected.

The format already has the right properties for this position:

- **Content-addressed, deterministic identity.** Same content → same payload
  digest (proven live: repeated captures of the same team yield byte-identical
  manifests). A review verdict bound to a digest is durable — it can never
  silently apply to different bytes.
- **No secrets inside.** Config slots declare *requirements* ("needs an AWS
  credential with scope X") but never carry values. A package is safe to hand
  to a reviewer, and the slot declarations are themselves reviewable policy
  objects.
- **Typed, statically scannable layers.** Vendor media types let a scanner
  mechanically extract agent instructions, LLM configuration, and server
  bootstrap from the layout without executing anything.
- **The composition is reviewable.** A team package captures the wiring —
  which agent talks to which MCP server. Cross-capability risks ("this agent
  has both a web-fetch server and a filesystem server") are only visible at
  the package level; no per-component scan can see them.
- **Real OCI.** Signatures (cosign), attestations (OCI referrers), SBOMs, and
  distribution are commodity infrastructure — none of it needs inventing.

What is missing is small and well-bounded: **artifact egress** (platform →
local), **audit tooling** over the already-existing local verification
primitives, and an **attestation convention**. This note designs those, plus
the dogfood proof: an agent team, built with this SDK, that audits packages.

## 2. Boundary principle

The division that has worked for capture/import/approve continues to govern:

| Concern | Owner |
|---|---|
| Package **format** (`pmcp-package` crate: pack/unpack/digest/slots/validation) | **Open-source SDK** — stays pure: no network, no AWS, no policy logic |
| CLI **verbs** (`pull`, `inspect`, future `audit`/`run --from-package`) | **Open-source SDK** (cargo-pmcp) — thin clients + local tooling |
| Audit **tooling & reference auditor team** | **Open-source SDK** — runs anywhere, extensible by anyone |
| Stack **renderer** (descriptor → CFN template; §7) | **Open-source SDK** — a library crate invoked by the CLI for self-managed targets *and* by platforms server-side for managed targets |
| Artifact **egress** (authenticated download of the OCI layout) | **Hosting platform** (pmcp.run first; the op is a documented contract any host can implement) |
| Deploy-time **synthesis policy** (allowlist over the descriptor, cost controls, account/region/VPC parameters) | **Hosting platform** — commercial surface (§7) |
| **Attestation storage / admission control** (which attestations must exist for import) | **Hosting platform** — commercial policy surface |
| Attestation **format** (digest-keyed report schema, referrer convention) | **Shared contract** — versioned, like `capture-v1.graphql` |

Two consequences worth stating explicitly:

1. `pull` is **not pmcp.run-specific**. It binds to a documented
   `getPackageArtifact` contract; pmcp.run is the first implementation, but any
   hosting service adopting the format implements the same op. This keeps the
   format's "open standard" position honest.
2. The format crate never learns about policies, registries, or reports. All
   audit logic lives *above* it. The small, auditable trust kernel is itself
   part of the security posture.

## 3. New capability: `pull` (platform egress + SDK verb)

### Platform side — `getPackageArtifact`

A GraphQL operation on the same API the existing verbs use:

```graphql
getPackageArtifact(reference: String!): GetPackageArtifactReturnType
# → { payloadDigest: String!, downloadUrl: String!, expiresAt: String! }
```

- Resolves a `name@version` reference (or accepts a raw digest), authorizes
  against the caller's org (same scoping as `show`), and returns a
  **short-lived presigned URL** for the packaged OCI layout (a tar of
  `index.json` + `blobs/` — one object, produced at capture time or on demand).
- **Audit-logged**: every egress records who pulled which digest when. This is
  the platform's commercial telemetry and the customer's compliance trail.
- Added to the contract seam as **`portability-v1.graphql`** (a sibling of
  `capture-v1.graphql`, same ownership model: platform exports the SDL, the
  SDK's offline blocking test pins the CLI to it).

### SDK side — `cargo pmcp package pull`

```bash
cargo pmcp package pull day-trip-planner-team@1.0.0 --output ./pkg/
```

- Calls `getPackageArtifact`, downloads, unpacks the tar into an on-disk
  `OciLayout`, then **re-verifies every blob digest and the payload digest**
  using the crate's existing `unpack_*` verification (never trust transport).
  Prints the verified payload digest.
- `cargo pmcp package inspect ./pkg/` then works **today, unchanged** — it
  already validates the layout, detects the kind, digest-verifies all layers,
  and renders components. Pull is the only missing link in the
  platform → local → verified-inspection chain.

### Explicitly out of scope: mutate-and-reimport

Editing a pulled package and importing the result into an approved slot is
**rejected by design**, not a gap: any byte change changes the payload digest,
which then matches no `ApprovedPackage` and fails import's digest assertion.
The supported paths remain: per-environment variation via **config slots**
(resolved at import), and content changes via **re-capture → re-approve** of
changed sources. Local `unpack → modify → pack` stays fully supported for
*local experimentation* — it just produces a new, unapproved identity.

## 4. The dogfood: the `package-auditor` reference team

The strongest proof of both the format and the SDK is to build the audit
tooling **as a pmcp agent team** — the same primitives customers use, published
as a reference implementation (in the spirit of `pmcp-team-servers`).

### Shape

**MCP servers (tools):**

- **`package-tools` server** — wraps the CLI/format capabilities as MCP tools:
  `pull(reference)`, `inspect(path)`, `unpack_component(path, name)`,
  `composition_graph(path)` (agents ⇄ servers wiring as structured data),
  `slot_declarations(path)`. This is dogfood at its purest: the SDK's own CLI
  surface, re-exposed through the SDK's own server toolkit.
- **`server-prober` server** — wraps `mcp-tester` (conformance) and the
  existing pentest payload corpus (prompt-injection probes) to exercise an
  unpacked MCP server **in a sandbox**: no real slot secrets exist in the
  package, and the probe run gets no network egress beyond the server under
  test.
- **`policy-pack` server(s)** — evaluate declarative policies (model
  allowlists, instruction lint rules, forbidden capability combinations)
  against extracted layers. Policies are data, versioned independently.

**Agents:**

| Agent | Reviews | Against |
|---|---|---|
| `composition-analyst` | the team graph | capability-combination rules (e.g. web-access + filesystem on one agent) |
| `instructions-auditor` | agent instructions layer | prompt-injection lint, exfiltration patterns, org policy packs |
| `config-auditor` | LLM configuration layer | model allowlists, cost/effort ceilings, provider policy |
| `server-auditor` | each MCP server component | conformance (mcp-tester) + injection probes (sandboxed, dynamic) |
| `slot-auditor` | config-slot declarations | secret-scope policy ("no package may demand credential scope X") |
| `reporter` | everything above | assembles the digest-keyed audit report |

### The report (the attestation candidate)

Output is a structured report keyed by the **payload digest** — the artifact a
marketplace later ingests as an attestation:

```json
{
  "schemaVersion": 1,
  "subject": {
    "payloadDigest": "sha256:af0ae208…",
    "reference": "day-trip-planner-team@1.0.0",
    "kind": "team"
  },
  "producedBy": {
    "auditor": "package-auditor-team",
    "auditorPackageDigest": "sha256:…",
    "policyPacks": [{ "id": "core-security", "version": "1.2.0" }]
  },
  "components": [
    { "name": "approval-mcp", "kind": "mcp-server", "digest": "sha256:…",
      "checks": [{ "id": "server.conformance.v1", "result": "pass" },
                  { "id": "server.injection-probe.v1", "result": "pass" }] }
  ],
  "composition": {
    "checks": [{ "id": "composition.capability-pairs.v1", "result": "warn",
                  "evidence": "planner-agent wires web-fetch + calendar-write" }]
  },
  "verdict": "pass-with-warnings"
}
```

Check IDs are versioned and open-ended — which is the extension mechanism.

### Customer extensibility — by composition, not plugin API

Because the auditor **is** a pmcp team, customers extend it with the same
moves they already know: add an auditor agent, add a policy MCP server, swap a
policy pack. An org's "our compliance checks" is just their fork of the team
definition plus their own servers. No bespoke plugin system to design,
document, or secure — the SDK's composition model *is* the plugin system.

### The recursive kicker

The auditor team is itself capturable as a `pmcp-package`. It can therefore
**audit itself**, and ship in the marketplace *with its own attestation
attached* — the first listing in the store is the tool that vets the store.
That is a self-hosting trust story none of the closed agent platforms can
tell, and it exercises every capability in this note end-to-end.

## 5. Format coverage today: authorization & infrastructure layers

Two server extensions were assessed against the shipped format (verified in
code, 2026-07-21):

### Cedar/AVP policies — covered end-to-end, first-class

- The format has a dedicated, digest-covered layer:
  `MT_SERVER_CEDAR_POLICY_SET` (`pmcp-package/src/oci/media_types.rs`),
  packed from `ServerPackage.policies` as canonical JSON and round-tripped by
  `unpack_server`.
- The platform capture worker reads the policies **from AVP itself** via a
  `PolicySource` seam (`package-capture-rust/model.rs` — "Cedar policies read
  from AVP"; `walk.rs` calls the policy manager) and packs them per component.

Consequence: a captured server carries its **actual enforcement policies**,
content-addressed — changing a Cedar policy changes the payload digest and
forces re-approval. The package ships its *permission contract*, not just its
code. This is the strongest single property the format holds for the
marketplace position.

**Scope caveat (platform-confirmed):** the capture read is
**single-store and server-filtered** — `walk.rs` reads only the store in the
server's own `codeModeConfig.policyStoreId`, filtered to that server id. Two
consequences: store-wide policies in the same store (default-deny/allow for
all principals) are excluded but still enforce at runtime, and **team-level
AVP authorization is not captured at all** — relevant precisely for team
packages. Until that widens, the claim above holds for *per-server code-mode
policies only*, and `authz-auditor` scenario suites must not overclaim.

### IAM statements — format yes; capture population is a CONFIRMED gap

`DeployDescriptor` models `[iam]` / `[[iam.statements]]` in its **closed set**
(an unrecognized deploy.toml table fails to parse — a deliberate, loud
tripwire). The platform team has confirmed (no experiment needed) that IAM
**deterministically does not survive capture**: `slot_extract.rs` synthesizes
the whole descriptor from `ServerRecord` rows, and deploy.toml's `[iam]`
never reaches the platform data model. Worse, the synthesized descriptor is
**systematically lossy, not just IAM-lossy** — memory defaulted, auth
hardcoded disabled, composition absent (`slot_extract.rs:290-340` documents
the gaps).

Two consequences adopted into this design:

- **Fidelity disclosure (transition mechanism):** captured descriptors carry
  an `authoritative | synthesized` provenance mark per section; the auditor
  surfaces it, and an `infra-auditor` verdict over synthesized sections is
  reported as *indicative, not authoritative* — an audit "pass" on an
  under-representative descriptor is worse than no audit.
- **The root fix is descriptor-verbatim persistence, not field harvest** (see
  §7): once the descriptor is the deploy input, the platform persists it
  verbatim and capture packs it — no synthesis, no per-field treadmill, and
  fidelity marks are needed only for the pre-refactor back-catalog.

### Custom CDK/CFN resources — the real gap

The closed set has no `[resources]` section, so resources added by hand-edited
stacks (DynamoDB tables, queues, …) are invisible to capture and to the
digest. Recommendation: **extend the closed set with declarative resource
tables** (e.g. `[[resources.dynamodb]]`) rather than embedding rendered CFN
templates — the source of truth stays declarative and digest-bound; the stack
remains a derived, re-renderable artifact. The closed-set tripwire then
guarantees new resource kinds are deliberate format decisions, never silent
passthrough.

### Policy testing — absent from the format *by design*, lands in the auditor

The format crate stays pure (no `cedar-policy` dependency). The audit tooling
(Phase B/D) gains two agents, both fully offline:

- **`authz-auditor`** — the `cedar-policy` crate parses, schema-validates, and
  *authorizes* offline: run scenario suites against the captured policy set
  ("can code-mode reach table X? API Y?") and diff against org baselines.
  Cedar being deny-by-default and formally analyzable is the best case for
  this. Check IDs: `server.authz.cedar-validate.v1`,
  `server.authz.scenario-suite.v1`, `server.authz.baseline-diff.v1`.
- **`infra-auditor`** — lint `[[iam.statements]]` for wildcards /
  least-privilege, and — because the **stack renderer lives in open-source
  cargo-pmcp** — deterministically re-render the stack from the captured
  descriptor and run `cfn-lint`/`cfn-guard` over the synthesized template.
  Declaration audited *and* derived infrastructure audited, no cloud calls.
  Check IDs: `server.infra.iam-lint.v1`, `server.infra.rendered-stack.v1`.

Both are new check IDs in the §4 report registry — no schema change.

## 6. A `cli-server` built-in: governed command surfaces

The toolkit's built-in family (SQL, OpenAPI, GraphQL, workbook) should gain a
**`cli-server`** kind: a declarative manifest (`cli.toml`) mapping an existing
CLI/SDK to MCP tools — Shape A binary + Shape B scaffold, like its siblings.

**Why it fits the business-customer positioning:** the audience objection
inverts. Business users don't write OpenAPI specs either — *integrators* do,
and consumers just get tools. A CLI built-in makes wrapping a vendor CLI a
configuration task instead of a Rust project, which grows the catalog.

**Why the most dangerous built-in should exist:** people wrap CLIs in MCP
servers *anyway* — as freeform code nobody can statically audit. A locked-down
built-in is strictly safer, and its declarative manifest is the feature:

- **argv-only execution** — no shell, ever; no interpolation.
- **Binary allowlist** with version pin and optional checksum; the package
  *declares* the binary requirement (like a slot), it does not carry
  platform-specific binaries.
- **Typed parameter schemas** — no raw flag passthrough.
- **Env allowlist, working-directory jail, timeouts, output caps.**
- **Cedar gating per tool invocation** — the *same* digest-bound policy layer
  (§5) that governs code-mode table/API access governs which subcommands and
  argument patterns may run. One policy language across data access and
  command execution, tested by the same `authz-auditor`.
- **No escape hatch.** The moment `cli.toml` admits a "run anything" tool,
  the audit story collapses. Declarative-only, closed like `DeployDescriptor`.

**Auditability:** the manifest is statically checkable — the auditor gains
`server.cli-surface.v1` (which binaries, which subcommands, what env — checks
a hand-rolled wrapper could never support).

**Dogfood is the first customer:** the §4 `package-tools` server
(pull/unpack/inspect) and `server-prober` (mcp-tester) *are* CLI wrappers —
with this built-in they become configurations, not code. Self-healing agents
need exactly a governed command surface, not a shell. And the recursion
extends: the auditor team is built from the built-in it audits.

## 7. Single source of truth: the descriptor — synthesis moves to the deploying party

The deepest simplification on the table, motivated by (but bigger than) the
§5 gaps. **Proposed end state: the synthesized CDK/CFN stack is demoted from
a *contract artifact* to a *derived artifact*.** The `DeployDescriptor`
becomes the complete declaration of an MCP server — tools, policies, slots,
IAM, **and owned resources** (`[[resources.*]]`) — and whoever deploys
renders the stack from it at deploy time. Nothing else authors infrastructure.

### Today's flow, and why it's the weak seam

Currently the CLI synthesizes the stack per target; for pmcp.run it uploads
the synthesized stack + binary, and the platform validates the stack against
an AWS-resource allowlist (security/cost), possibly modifies it, then calls
CFN. Three structural problems:

1. **Validation of client-synthesized CFN is validating attacker-controlled
   input in a hostile format** (conditions, intrinsics, `Fn::Sub`); the CLI
   is open-source and replaceable. Validating the closed-set *descriptor* is
   a small, semantic surface — and the stack the platform then generates is
   trusted by construction.
2. **Post-hoc stack modification means neither side owns the truth.** After
   the flip, platform adjustments become explicit *synthesis inputs*
   (account, region, VPC, naming/tagging policy): deployed infra =
   `render(descriptor, platform-params)` — reproducible and auditable.
3. **The implicit contract "the shape of a stack CLI version N synthesizes"
   is huge, unversioned, and drift-prone** (the Phase-110 class, for infra).
   After the flip, the contract is the descriptor schema — which is already
   the package contract, enforced at the **type level** because the platform
   already consumes the `pmcp-package` crate. One schema, three uses: deploy
   input, package layer, audit subject.

### The boundary resolution: mechanism open, policy commercial

Synthesis does not "move to the platform" — it runs **at the deploying
party**, using one shared renderer:

- The renderer (descriptor → CFN template) is **extracted from cargo-pmcp
  into an open-source library crate**, consumed by the CLI for self-managed
  targets (user's own Lambda/GCR/Azure/Cloudflare) and by the platform's
  Rust deploy path for managed targets. No duplication; the open SDK stays
  complete (anyone can self-host or build a competing host with the same
  mechanism).
- The platform's value-add is **policy**: descriptor allowlisting, cost
  controls, tenancy, environment parameters — never secret synthesis.
- `cargo pmcp deploy --synth-preview` falls out for free: the CLI runs the
  same renderer locally; the platform reports its renderer version;
  determinism makes the preview honest.

### Identity vs. environment — the split that keeps packages portable

The descriptor enters the payload digest, so it must carry **identity, not
environment**, or portability breaks (test→prod would change the digest of
identical intent):

- IAM statements reference package-declared resources **symbolically**
  (`resource = "@resources.orders-table"`), resolved at render time when the
  deploying party knows account/region. Concrete cross-account ARNs for
  *external* resources are **slots** (the mechanism already exists: declared
  requirement, environment-bound value).
- Existing environment-ish fields (`region`, arguably memory/cpu sizing)
  get the same audit: identity-bearing declarations in the digest;
  environment bindings resolved at import/deploy.

### Expressiveness ceiling — a priced escape hatch, not a silent hole

The closed set chases the ~90% (tables, queues, buckets, topics, schedules)
deliberately, table by table — never CDK-completeness. A server needing
genuinely custom infra must declare `custom_stack = true`, which **taints the
package visibly**: capture records it, the auditor flags
`server.infra.non-declarative`, managed platforms may refuse or gate it
(formalizing what the allowlist already enforces informally), and it remains
deployable to self-managed targets. Same philosophy as the cli-server's
"no run-anything tool."

### Flavors — pragmatic, not cloud-abstract

The descriptor already has target sections (`[aws]`, `[gcp]`, `[layout]`);
flavors formalize that: a **target-neutral core** (tools, policies, slots,
logical resource names) plus **target-flavored resource tables**
(`[[resources.dynamodb]]` — deliberately AWS-native, no leaky "kv-table"
abstraction layer). A package declares which flavors it provides; AWS/pmcp.run
first; a second flavor is added when a real second target demands it.

### Costs, named honestly

1. **CDK-codegen → direct CFN emission.** The renderer today generates CDK
   TypeScript; server-side synthesis wants hermetic **CFN-template emission
   from Rust** (no Node toolchain in a Lambda). This is the substantive
   rewrite — and what makes the auditor's re-render check bit-for-bit
   meaningful.
2. **Migration — by fleet recreation, not renderer compatibility.** The
   renderer carries **no CDK-compat requirement**: it never has to
   reproduce the logical IDs or update semantics of existing CDK-generated
   stacks (in-place CFN updates against foreign logical IDs are a classic
   migration tarpit and would force resource replacement anyway). Instead
   the platform **recreates the existing fleet through the new
   descriptor→render path, in waves**, each server's wave gated on
   `[[resources.*]]` being able to express that server's declarations
   (open question 11). Stateful resources get a per-wave carry-over
   treatment — a bounded, per-server operation rather than a renderer-wide
   constraint.

### Sequencing — must not gate `pull`/0.20

Adopt the end state now (this section); ship Phase A as scoped (egress
depends on none of this); extract the renderer + CFN emission as SDK work
(independently useful to self-deploy targets immediately); flip the pmcp.run
deploy endpoint to descriptor+binary when Phase 172/173 activation work
naturally touches that path — convergence, not a standalone migration. Once
flipped, deploy and import become **one activation path**
(descriptor + binary in → synthesize → materialize), and §5's
descriptor-verbatim persistence is free.

## 8. Phasing

Each phase is independently useful; nothing blocks the current release train.

| Phase | Deliverable | Owner(s) |
|---|---|---|
| **A** | `getPackageArtifact` op + `portability-v1.graphql` contract + `cargo pmcp package pull` | Platform (op, SDL export) + SDK (verb, offline contract test) |
| **B** | Static audit tooling: report schema v1, policy-pack format, checks over `unpack_*` layers | SDK |
| **C** | Dynamic bridge: materialize a pulled package into the local runtime (`team dev --from-package`), mcp-tester/pentest integration, sandbox profile | SDK |
| **D** | `package-auditor` reference team (dogfood), published as a reference implementation; self-audit demo | SDK |
| **E** | Attestation convention (OCI referrers + signing) + platform admission policy ("import requires attestation X") + marketplace surface | Shared contract + Platform |

Phase A is the gate for everything and is deliberately tiny — it repeats the
capture-seam playbook (platform op + exported SDL + blocking CLI test) that
both teams have now executed successfully once.

Three items run **parallel** to this track, independent of Phase A:

- **Near-term hardening (§5):** fidelity marks on synthesized descriptors
  (platform) and the `[[resources.*]]` closed-set extension (SDK format,
  platform capture) — superseded long-term by §7's descriptor-verbatim
  persistence, but they harden what capture ships in the interim.
- **Renderer extraction + CFN emission (§7):** SDK-side, independently
  useful to self-deploy targets immediately; the pmcp.run endpoint flip
  lands with Phase 172/173 activation work on the platform's schedule.
- **`cli-server` built-in (§6):** SDK-side; naturally lands with Phase D,
  whose reference team is its first consumer.

**Release-bundling note:** the phases are technically independent — the
contract seam exists precisely so the two teams release on their own
cadences. Whether the next *published* CLI bundles the package verbs with
`pull` (waiting on Phase A) or ships them now with `pull` following in the
next minor is a release-management choice, not an engineering constraint;
it is an explicit joint-review agenda item.

## 9. Security considerations

- **Egress is authorized and logged** — packages are org-scoped IP; the
  download trail is part of the compliance story, not an afterthought.
  Precision matters here: a presigned URL is a **bearer token**, and the
  platform's audit row records *issuance*, not *download*. Short expiry
  (~5 min) plus S3 access logs where the compliance trail needs actual-GET
  evidence — the platform specifies this, since it is their compliance
  surface.
- **Reviewers never receive secrets** — guaranteed by the format (slots carry
  no values), not by egress-time filtering.
- **Dynamic probing is sandboxed — and its reach must be stated honestly.**
  Unpacked servers run with no real credentials (none exist in the package)
  and constrained egress. But "no secrets in the package" also means
  **unresolved slots**: many servers won't boot meaningfully without the
  env/config the platform injects at runtime, so naive probing exercises only
  the unauthenticated surface. Phase C therefore includes **local slot
  resolution with reviewer-supplied test bindings** as an explicit design
  item, not an assumption; probe findings must not require trusting the
  probed code.
- **Reports bind to digests and name their policy versions** — a `pass` is
  meaningless without *which checks, which versions, against which bytes*.
- **Revocation exists** — `revokeApprovedPackage` already gives the lifecycle
  endpoint attestation-based admission needs when a previously-passed package
  is found bad.

## 10. Open questions (for the two-team review)

Items with a **[proposed]** answer carry the platform team's recommendation
from their design-note review (2026-07-21); the joint review ratifies them.
Resolved items are kept for the record.

1. Artifact packaging for egress — **[proposed: tar at capture time]**,
   written digest-keyed to S3, presigned from there. On-demand assembly would
   force an async submit/poll op (a blob-by-blob ECR pull + tar cannot fit
   AppSync's resolver cap); tar-at-capture keeps the synchronous
   `getPackageArtifact` shape viable. Decide **now** even though the op ships
   later: the back-catalog is a handful of dev packages today, and the
   backfill window closes as packages accumulate.
2. Digest-addressed fetch — **[proposed: yes in v1]**. Admission is already
   digest-asserted and attestations are digest-keyed; adding it later is
   contract churn. Platform cost: a GSI on payload digest.
3. Report signing in Phase B (SDK-side keypair? sigstore keyless?) or defer
   all signing to Phase E?
4. Where does the reference auditor team live — this repo (like
   `pmcp-team-servers`) or its own repo with its own release cadence?
5. Marketplace namespace/identity model (org-scoped names vs. global) —
   Phase E, but the reference format for `subject.reference` should not
   foreclose it.
6. ~~IAM population~~ — **RESOLVED (platform-confirmed):** deterministically
   not captured; the synthesized descriptor is systematically lossy (§5).
   Successor question: fidelity-mark shape, and the §7 descriptor-verbatim
   flip as the root fix (superseding field-by-field harvest).
7. **`[[resources.*]]` shape:** which resource kinds enter the closed set
   first (DynamoDB tables?), symbolic-reference syntax for IAM statements
   (§7), and `deploy-descriptor.v2` media-type versioning.
8. ~~AVP read scope~~ — **RESOLVED (platform-confirmed):** single store,
   server-filtered (`codeModeConfig.policyStoreId` + `list_policies(server)`).
   Successor question: capturing store-wide policies and **team-level authz
   stores** (they enforce at runtime but are absent from team packages — §5
   scope caveat).
9. **Release bundling:** publish the CLI package verbs now (0.19.x) with
   `pull` following in the next minor, or hold one bundled "portable
   artifact" release gated on Phase A? Decision holder: SDK side; the
   platform has argued for shipping now (it aids their 172/173 dogfood) and
   Phase A's realistic slot is after their Phase 172.
10. **§7 ratification:** adopt "descriptor is the contract, stack is
    derived, renderer is a shared open-source crate" as the end state? If
    yes: renderer-crate naming/extraction plan (SDK) and the deploy-endpoint
    flip's placement in the 172/173 window (platform).
11. **Migration expressiveness inventory (§7):** with migration by **fleet
    recreation** (platform commitment; the renderer carries no CDK-compat
    requirement), this is rescoped from "what stack behaviors must the
    renderer reproduce" to **"what must `[[resources.*]]` express before
    each server's recreation wave."** Output: a per-wave expressiveness
    checklist that orders both the closed-set tables and the wave schedule.
    (Platform post-edits of uploaded stacks are covered the same way — as
    declarations the descriptor must express, or synthesis params, not as
    behaviors to reproduce.)
