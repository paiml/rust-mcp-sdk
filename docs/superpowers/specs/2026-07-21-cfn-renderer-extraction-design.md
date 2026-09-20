# CFN Renderer Extraction — Design

**Date:** 2026-07-21
**Status:** Approved (brainstorm cycle; two user refinements incorporated)
**Context:** §7 of `docs/design/package-portability-and-audit.md` (PR #312) —
"the descriptor is the contract; the synthesized stack is a derived artifact;
synthesis runs at the deploying party via a shared open-source renderer."
This spec covers the SDK-side long pole: the renderer crate and the CLI
switch-over. No CDK-compat requirement exists (migration is by fleet
recreation).

---

## 1. Problem

cargo-pmcp deploys CFN-backed targets by generating a `stack.ts`, shelling
out to **`cdk synth` (Node)**, and — for pmcp.run — uploading the synthesized
template. Consequences: the CLI requires Node + CDK on PATH
(`is_available()` checks `npx cdk`); the platform validates client-shaped
CFN; the template is not deterministic or re-renderable by other parties;
and the renderer logic (`iam.rs` renders IAM as TypeScript *text*) cannot be
reused server-side.

**Goal:** a pure Rust library crate that renders a `DeployDescriptor`
directly to a deterministic CloudFormation template, wired into both CFN
targets of the CLI, dropping the Node/CDK dependency for the standard path.

**Audience framing that shapes the artifact model:** the intended majority
of MCP servers are **built-in type servers** (SQL / OpenAPI / GraphQL /
workbook / future cli-server) defined by schema + configuration files only,
deployed as **prebuilt published binaries** — their authors are not Rust
developers and have no local toolchain. Only the minority custom-Rust
servers are compiled locally (cargo-lambda). Today even a config-only
author needs Node + CDK to deploy; after this work they need **no dev
tooling at all**. The deployable artifact is therefore always a compiled
bootstrap-binary zip — prebuilt or locally built — never source code.

## 2. Scope decisions (fixed during brainstorm)

1. **Done =** crate exists **and both CFN paths switched**: the `pmcp_run`
   synth step and `aws_lambda` self-deploy use the renderer; `is_available()`
   no longer requires `npx cdk`.
2. **Input =** `pmcp_package::package::DeployDescriptor` — the canonical
   closed-set descriptor. No parallel input model.
3. **Parity bar =** semantic goldens (CI, forever) **plus** one real
   dev-account deploy + mcp-tester E2E per resource family before the switch.
4. **Resource surface = today's platform allowlist, not AWS.** The renderer
   models only the resource families an MCP server actually uses (§4). New
   families enter only via the `[[resources.*]]` closed-set process — the
   renderer must never grow toward CDK-completeness.
5. **CDK fallback retained** for hand-customized stacks (`custom_stack`
   taint). Enforcement is **platform policy, not CLI policy**: pmcp.run can
   simply block non-renderer templates server-side when ready. The CLI keeps
   the fallback for self-managed targets indefinitely (until a later major).

## 3. The crate

`crates/pmcp-cfn-renderer` — root workspace member.

- **Dependencies:** `pmcp-package` (descriptor types), `serde`,
  `serde_json`, `semver`. **No** AWS SDK, no tokio, no reqwest, no
  filesystem, no network. Pure functions only — same purity discipline as
  `pmcp-package`, and for the same reason: it is trust-kernel code that both
  the CLI and hosting platforms run.
- **Publish order:** after `pmcp-package`, before `cargo-pmcp` (which gains
  it as a dependency).
- **Core API:**

```rust
pub struct RenderParams {
    pub account_id: String,
    pub region: String,
    pub stack_name: String,
    /// Location of the deployable ARTIFACT: always a compiled bootstrap
    /// binary zip (Rust -> provided.al2023), NEVER source code. For most
    /// servers this is a PREBUILT built-in-server binary (+ its
    /// schema/config files); only custom-Rust servers carry a locally
    /// compiled binary. Provenance is the deploy engine's concern (§6b) —
    /// the renderer only needs where the zip lives. Optional expected
    /// digest aligns with pmcp-package's `BinaryRef` (verified by the
    /// engine before upload; CFN cannot verify it).
    pub artifact: ArtifactRef, // { s3_bucket, s3_key, digest: Option<…> }
    /// Resolved environment variables (identity-safe values only; secret
    /// VALUES never appear — secrets stay platform/deploy-time refs).
    pub environment: BTreeMap<String, String>,
    /// Synth-time metadata (the current `metadata.rs` synth-context:
    /// server_type, snapshot_baked, MCP tool metadata for the template).
    pub metadata: RenderMetadata,
}

pub fn render(descriptor: &DeployDescriptor, params: &RenderParams)
    -> Result<CfnTemplate, RenderError>;

impl CfnTemplate {
    /// Canonical JSON: sorted keys, stable ordering, byte-deterministic.
    pub fn to_canonical_json(&self) -> String;
}
```

- **Identity/environment split is enforced by the signature:** everything
  environmental arrives via `RenderParams`; the descriptor cannot smuggle
  environment into identity. Any field the renderer needs that today exists
  only in the CLI's `DeployConfig` is **promoted into the descriptor's
  closed set first** (a deliberate completeness forcing-function and the
  first live run of the descriptor-change process).

## 4. Resource surface (allowlist-scoped)

One module per family; each maps a descriptor section to typed CFN resource
structs. The v1 set — derived from the current scaffold + the platform's
allowlist — and nothing else:

| Module | CFN resources | Driven by |
|---|---|---|
| `lambda` | `AWS::Lambda::Function` (+ alias/permission as needed) | `[server]` (memory, timeout), `params.code_ref` |
| `iam` | execution role + inline policies | `[iam]` / `[[iam.statements]]`, table/bucket permission sugar (ports `iam.rs` validation + rendering, minus the TypeScript) |
| `logs` | `AWS::Logs::LogGroup` | `[observability]` |
| `http_api` | `AWS::ApiGatewayV2::Api/Integration/Route/Stage` + authorizer | `[server]`, `[auth]` |
| `cognito` | `UserPool`, `UserPoolResourceServer`, `UserPoolDomain` | `[auth]` (OAuth flavor only) |
| `dynamodb` | `AWS::DynamoDB::Table` | current scaffold usage; the future `[[resources.dynamodb]]` slot |
| `outputs` | `Outputs` section (endpoint URL, function name — matching today's `outputs.rs` consumers) | all |

An **action item for the joint review** (not a blocker): obtain the
platform's current allowlist as the authoritative enumeration and diff it
against this table — the table is derived from the scaffold
(`commands/deploy/init.rs`) and the 19 fixture descriptors, and must be
confirmed, not assumed.

`RenderError` is total and descriptive: a descriptor requesting anything
outside the surface fails loudly with the section/field named — never a
silent skip.

## 5. Determinism

- Canonical JSON (sorted keys via `BTreeMap` everywhere; no maps with
  nondeterministic iteration; no timestamps, no random suffixes, no
  absolute paths).
- **Logical-ID scheme:** derived from descriptor names by a documented,
  stable transform (`PmcpFn`, `PmcpHttpApi`, `PmcpTable<Name>`, …). No
  CDK-style content hashes. Renaming a descriptor entity renames its
  logical ID — acceptable, because migration is by fleet recreation and
  in-place updates of foreign stacks are out of scope by decision.
- Property test: `render(d, p)` is byte-identical across runs and across
  platforms. This is what makes `deploy --synth-preview` honest and the
  future `infra-auditor` re-render check bit-for-bit.

## 6. CLI switch-over

### 6a. `pmcp_run` target
`run_cdk_synth(...)` → `pmcp_cfn_renderer::render(...)`; the upload flow
(template + bootstrap binary via GraphQL) is unchanged this cycle. The
platform keeps validating uploaded templates; renderer output is plain CFN
(no CDK metadata resource, no bootstrap parameters) — **cross-team check:
confirm their validator accepts renderer-shaped templates before this path
ships** (expected trivial: plain CFN is strictly simpler than CDK output;
tracked on the joint-review ticket split). Their endpoint flip to
descriptor+binary (§7 of the design note) comes later and is unaffected.

### 6b. `aws_lambda` target — renderer + a small CFN deploy engine
Today `cdk deploy` both synthesizes *and* deploys. Dropping Node means
cargo-pmcp gains a deploy engine (CLI-side, NOT in the renderer crate).

**Artifact acquisition is shape-aware** — the deployable is always a
compiled bootstrap-binary zip, never source, and the two server shapes get
it differently:

- **Built-in servers (the intended majority):** defined by schema/config
  files only (`[server].binary` names the built-in;
  `[metadata].server_type` / `snapshot_baked` govern config baking). The
  engine **fetches the prebuilt, published binary** for the target
  platform (the release pipeline already ships per-arch `pmcp-server`
  binaries), bundles/bakes the config per `snapshot_baked`, and verifies
  the expected digest (`BinaryRef` alignment). **No Rust toolchain, no
  cargo-lambda, no Node** — a config-only author deploys with zero dev
  tooling installed.
- **Custom-Rust servers (developer workflow):** `cargo-lambda` builds the
  zip locally, as today.

Then, for both shapes:

- `aws-sdk-s3`: upload the artifact zip to the deploy bucket (explicit,
  descriptor-named — **no CDK bootstrap dependence**).
- `aws-sdk-cloudformation`: create/update stack with the rendered template,
  waiter loop to terminal status, surface stack events on failure, read
  `Outputs` (feeding the existing `outputs.rs` consumers).
- `is_available()` drops the `npx cdk` probe unconditionally, and requires
  `cargo-lambda` **only for custom-Rust servers** — built-in deploys must
  not probe for it.

### 6c. Custom-stack fallback
The existing fail-closed gate (`validate_and_regenerate_stack_ts`) already
detects stack.ts drift. Routing: unmodified scaffold → renderer path;
customized stack.ts → legacy cdk-synth path + warning + `custom_stack`
taint recorded in deploy metadata (capture then surfaces it;
`server.infra.non-declarative` in the auditor). **pmcp.run may block
taint-carrying/non-renderer deploys server-side at its discretion** — the
CLI does not enforce platform policy. Legacy path removal is a later major.

## 7. Verification

1. **Semantic goldens (CI, permanent):** render all 19 tracked fixture
   descriptors through the renderer; compare against **checked-in,
   normalized `cdk synth` goldens** (generated once; normalization strips
   CDK metadata/bootstrap artifacts and logical IDs, canonicalizes
   references into a resource-graph form). CI never needs Node. Any
   renderer change that alters semantics diffs a golden.
2. **Determinism property tests** (§5) in the crate.
3. **Real-deploy gate (pre-switch, once per resource family):** dev-account
   deploy + `mcp-tester` E2E for: plain Lambda server, OAuth/Cognito
   server, DynamoDB-carrying server, widget-carrying server. Recorded in
   the PR that flips each path.
4. Existing `iam.rs` validation tests port with the logic (same warnings,
   same fail-closed behavior).

## 8. Out of scope this cycle

- `[[resources.*]]` / descriptor-v2 (joint-review-gated; §4's per-family
  module structure is the landing zone).
- The platform's server-side adoption and endpoint flip (their 172/173
  window).
- Non-CFN targets (GCR/Azure/Cloudflare) — untouched.
- Legacy cdk-path removal; `--synth-preview` UX polish beyond printing the
  canonical template.

## 9. Success criteria

- `cargo pmcp deploy` to aws-lambda and the pmcp.run synth step run with
  **no Node/CDK installed**, standard scaffolds only.
- A **built-in (config-only) server deploys with zero dev tooling** — no
  Node, no CDK, no Rust toolchain, no cargo-lambda; the engine fetches the
  prebuilt binary and verifies its digest. `cargo-lambda` is probed only
  for custom-Rust servers.
- `pmcp-cfn-renderer` is pure (no AWS SDK/network/fs), consumes
  `DeployDescriptor` + `RenderParams`, and emits byte-deterministic
  canonical CFN.
- Semantic-golden CI suite green over the 19 fixtures; real-deploy gate
  passed once per resource family.
- Customized stacks still deploy via the legacy path, visibly tainted;
  platform-side blocking of that path requires no CLI change.
