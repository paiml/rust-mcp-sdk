# CFN Renderer Switch-Gate Runbook

**Scope.** `pmcp-cfn-renderer` (`crates/pmcp-cfn-renderer/`) is a pure
`DeployDescriptor -> CloudFormation` template renderer that replaces `npx cdk
synth` / `cdk deploy` for **unmodified** scaffold projects on two `cargo pmcp
deploy` targets: `pmcp-run` (synth only — pmcp.run's platform still applies
the template) and `aws-lambda` (synth **and** apply, via the zero-Node
CFN deploy engine at `cargo-pmcp/src/deployment/targets/aws_lambda/engine.rs`).
Both switches are already live in code (T1–T9 of
`docs/superpowers/plans/2026-07-21-cfn-renderer-extraction.md`; design spec:
`docs/superpowers/specs/2026-07-21-cfn-renderer-extraction-design.md`) — 147
renderer tests + 5 semantic goldens, 855 `cargo-pmcp` bin tests, all green
under `cargo fmt --all -- --check` / `pmat quality-gate
--fail-on-violation --checks complexity`.

None of that is a substitute for exercising the renderer against **real AWS**
and the **real pmcp.run platform**. This runbook is the pre-production gate:
it must be worked through — and the platform-validator check in particular
must come back green — before the renderer path is allowed to be the
*default* for `pmcp-run` or `aws-lambda` users. Until then, keep the legacy
`cdk`-based path as the safe fallback it already is (both targets already
fall back automatically whenever `deploy/lib/stack.ts` is hand-modified —
see the "custom_stack taint" note below).

Treat every row and count in this document as a snapshot to be re-verified
at execution time, not copied blind — the same convention used in
`docs/runbooks/package-capture-release.md`.

---

## 1. Real-deploy gate checklist

One dev-account `cargo pmcp deploy` + one `mcp-tester` E2E run, per family.
`mcp-tester quick <url>` (or `test`/`tools`/`health` for deeper checks — see
`docs/OAUTH_DEBUGGING_GUIDE.md` for the full verb set) is the standard
post-deploy smoke tool used elsewhere in this repo. Fill in each row as the
family is exercised; do not mark the overall gate passed until all five rows
have a `result` of PASS (or an explicitly accepted, documented FAIL with a
follow-up ticket).

| Family | Fixture | Stack name | Date | Result |
|---|---|---|---|---|
| (a) Plain Lambda | `plain-lambda` shape — `cargo pmcp deploy init --target aws-lambda` (no `--oauth`, no `[[widgets]]`); or the `pmcp-run` sibling shape via `--target pmcp-run` | | | |
| (b) HTTP API (aws-lambda) | `http-api` golden's shape — `cargo pmcp deploy init --target aws-lambda` (HttpApi/Integration/Route/Stage on top of plain Lambda) | | | |
| (c) OAuth/Cognito+DCR (aws-lambda) | `oauth-cognito-dcr` golden's shape — `cargo pmcp deploy init --target aws-lambda --oauth cognito` (UserPool + JWT authorizer + DCR ClientsTable) | | | |
| (d) Built-in server WITH Web Adapter | `[metadata].server_type = "sql-server"` (or `openapi-server`/`workbook-server`) project on `--target aws-lambda` — exercises `ServerShape::BuiltIn` fetch + the `LambdaAdapterLayerArm64:28` Lambda Web Adapter wiring. **Blocked until §2's release-asset gap closes** — the connector binaries (`pmcp-sql-server`/`pmcp-workbook-server`/`pmcp-openapi-server`) must be publishing via `release-binary.yml` first, or `acquire_artifact` 404s. | | | |
| (e) Widget-carrying | A project with a `[[widgets]]` block (`cargo-pmcp/src/deployment/widgets.rs`) on `--target aws-lambda` — exercises post-deploy widget upload running alongside the renderer + CFN engine, not just the render/apply path alone | | | |

Notes for whoever runs this:

- Families (a)–(c) map 1:1 to three of the five checked-in semantic goldens
  (`crates/pmcp-cfn-renderer/tests/goldens/{plain-lambda,http-api,oauth-cognito-dcr}.golden.json`)
  — the golden proves the *template shape* is correct; this gate proves the
  *engine* (STS resolve → bucket ensure → upload → create/update → poll →
  outputs) actually lands a working stack in a real account.
- Use a scratch/dev AWS account, not a shared one — `ensure_bucket` will
  create `pmcp-deploy-{account}-{region}` (see §3.3) and `apply_stack` will
  create/update a real CloudFormation stack.
- After each deploy, run at minimum `mcp-tester quick <ApiUrl-or-endpoint>`
  against the stack's `ApiUrl` output; for (c) additionally exercise the
  OAuth discovery/token flow `mcp-tester` supports.
- Tear down (`aws cloudformation delete-stack`) after each row unless you're
  intentionally reusing the stack for the next family in sequence.

---

## 2. Platform-validator acceptance check (REQUIRED before the pmcp_run renderer path ships to users)

This is the hard gate on the `pmcp-run` side specifically (the `aws-lambda`
side has no external validator — its own `apply_stack` call against real
CloudFormation **is** the validation).

pmcp.run's platform runs its own template allowlist check before accepting
an uploaded CloudFormation template for a `pmcp-run` deployment. The
renderer's output is deliberately **shaped differently** from what `cdk
synth` has always produced, in ways the platform validator has never seen
before:

- **No `CDKMetadata` resource and no CDK bootstrap parameters**
  (`BootstrapVersion`, `CheckBootstrapVersion` rule, etc.) — the renderer
  emits a plain template with no CDK synthesis fingerprint at all.
- **Full `mcp:*` `Metadata` via `to_cloudformation_metadata`** — as of the
  T7 fix (`crates/pmcp-cfn-renderer` `RenderParams::cloudformation_metadata`,
  commit `f17c85c8`), the renderer emits the same provenance-superset
  `mcp:version`/`mcp:serverType`/`mcp:serverId`/`mcp:resources`/
  `mcp:capabilities`[/`mcp:templateId`/`mcp:snapshotBaked`] map cdk's
  `stack.ts` bakes via context args — but as a **template-level `Metadata`
  block**, not synthesized through `cdk`'s own machinery. The platform's
  parser needs to accept this shape.
- **`Fn::Sub` pseudo-parameter composition ARNs** — the two family-internal
  IAM ARNs (`MCP_SERVERS_TABLE` discovery read, cross-Lambda invoke
  wildcard) are `{"Fn::Sub": "arn:aws:...:${AWS::Region}:${AWS::AccountId}:..."}`
  rather than literal-account strings (fixed post-T7-review specifically
  because `pmcp-run`'s CLI path has no resolved account to bake in — see
  §3.6). Confirm the platform's allowlist/validator doesn't require a
  literal account segment here.

**Action**: before flipping the `pmcp-run` target's default routing (today
it already routes to the renderer automatically whenever `stack.ts` is
byte-identical to the regenerated scaffold — see §3.7's taint note for the
fallback condition), get explicit confirmation from the pmcp.run platform
team that a renderer-produced template for each of the checklist's (a)/(b)/(c)
shapes passes their upload-time validator unmodified. Until that
confirmation lands, treat every real `pmcp-run` deploy that silently took
the renderer path (i.e., every unmodified-scaffold project) as **at risk**
of a platform-side rejection at upload time — the CLI-side render succeeding
is necessary but not sufficient.

---

## 3. Operational notes

Each of these is a known, disclosed behavior or gap surfaced during T1–T9
review — not a surprise to be discovered in production. Read all of them
before running the gate in §1.

### 3.1 Unbounded stack-poll loop

`poll_to_terminal` (`cargo-pmcp/src/deployment/targets/aws_lambda/engine.rs`)
polls `describe_stacks` every 5 seconds in a plain `loop {}` with no timeout
and no maximum iteration count — it only exits on a terminal status
(`CREATE_COMPLETE`/`UPDATE_COMPLETE`/failure) or if the stack disappears
mid-poll. This matches `cdk deploy`'s own progress-wait behavior, which is
also unbounded: the escape hatch is the operator hitting Ctrl-C. A stuck
CloudFormation operation (e.g. a resource stuck `CREATE_IN_PROGRESS`
indefinitely) will hang the CLI exactly as it would under `cdk deploy` —
this is a deliberate parity choice, not an oversight, but it means the CLI
gives no on-screen warning that it's been polling for an unusually long
time. Ctrl-C is safe: it only kills the local CLI process, not the
in-flight CloudFormation operation.

### 3.2 `ROLLBACK_COMPLETE` redeploy dead-end

If a stack lands in `ROLLBACK_COMPLETE` (a failed `CREATE_STACK` that
finished rolling back) or `UPDATE_ROLLBACK_COMPLETE`, `apply_stack`'s
`classify_action` still sees an existing stack and issues an `UpdateStack`
call — which CloudFormation **rejects** for a stack in `ROLLBACK_COMPLETE`
state (a stack in that state can only be deleted, never updated). The
engine does not special-case this: the operator gets a hard error from the
`UpdateStack` API call, not an actionable message. **Manual recovery**: `aws
cloudformation delete-stack --stack-name <name>` (then wait for
`DELETE_COMPLETE` via `aws cloudformation wait stack-delete-complete` or a
console check), and re-run `cargo pmcp deploy`. For comparison, `cdk
deploy` **does** special-case this — it detects `ROLLBACK_COMPLETE` and
auto-deletes the stack before recreating it. This is a known, disclosed UX
gap versus the legacy path, not a defect nobody knew about; closing it
would mean teaching `apply_stack` the same auto-delete-and-recreate
behavior, deliberately left out of T9's scope.

### 3.3 Bucket convention: `pmcp-deploy-{account_id}-{region}`

`engine::bucket_name` defines (there is no config field for it — the T9
brief explicitly left this undefined for the engine to establish) the
deploy-artifact S3 bucket as `pmcp-deploy-{account_id}-{region}`, created
private (no explicit ACL/public-access-block call — relies on the
account-level Block Public Access default, which has been on since April
2023) and region-aware (`us-east-1` omits `LocationConstraint`, since S3
rejects an explicit constraint for its own default region; every other
region requires one). `ensure_bucket` is idempotent: `head_bucket` first,
and a `BucketAlreadyOwnedByYou` race on `create_bucket` is tolerated as
success, not an error. There is no cross-account bucket sharing — each AWS
account gets its own bucket per region it deploys into.

### 3.4 Web Adapter layer pin: `LambdaAdapterLayerArm64:28` (external pin, drifts)

Built-in-server (`ServerShape::BuiltIn`) deploys attach the AWS Lambda Web
Adapter as a **pinned layer version**:
`arn:aws:lambda:${AWS::Region}:753240598075:layer:LambdaAdapterLayerArm64:28`
(`crates/pmcp-cfn-renderer/src/resources/lambda.rs`, from the T8 review
fix, commit `4dfc692b`). The account id (`753240598075`) is the adapter
project's own publishing account — a fixed literal by design, not something
`Fn::Sub` needs to touch (only `${AWS::Region}` is templated). The version
number (`:28`) is **not** — it was the latest published layer version as of
the live `awslabs/aws-lambda-web-adapter` README fetch on 2026-07-21 (tag
`v1.0.1`). This is an external pin with no drift detection: if the adapter
project ships a new layer version, this repo's templates keep referencing
`:28` until someone manually re-verifies and bumps it. Check
`github.com/awslabs/aws-lambda-web-adapter`'s README before relying on this
in a new region or if a built-in deploy starts failing to invoke.

### 3.5 `[target].version` doubles as the built-in binary version (promotion candidate)

`aws_lambda::artifact::release_tag()` reuses the existing
`DeployConfig`/`.pmcp/deploy.toml` field `[target].version` (normalized to a
`v`-prefixed tag) to decide **which** GitHub Release to fetch
`pmcp-sql-server`/`pmcp-workbook-server`/`pmcp-openapi-server` from for
`ServerShape::BuiltIn` deploys. That field's scaffold default (`"1.0.0"`,
set for *every* target by `default_for_server`) predates this use and
doesn't correspond to any real SDK release tag — an operator who never
customizes it gets a clean 404 rather than a silently wrong binary, but it
is overloading a previously near-vestigial field for a new, semantically
different purpose (SDK release tag, not "my server's version"). Flagged in
the T8 report as a promotion candidate: a dedicated
`[metadata].server_version` (or similar), scoped specifically to built-in
binary acquisition, should be added in a follow-up phase rather than
continuing to overload `[target].version`.

### 3.6 `[metadata].server_type` is shape-determining on `aws-lambda` only (dual-target footgun)

On the `pmcp-run` target, `[metadata].server_type` is purely descriptive
platform metadata — it does not affect which build path runs. On
`aws-lambda`, as of T8, its presence is **shape-determining**:
`detect_shape` routes to `ServerShape::BuiltIn` (fetch a prebuilt Shape A
binary) whenever `[metadata].server_type` is set, **unconditionally**, even
if the project also has a `Cargo.toml` + `src/` at the project root (a
Shape B `cargo pmcp new --kind sql-server` scaffold with real compiled
Rust). A project deployed to both targets — `pmcp-run` for the platform
listing and `aws-lambda` for a self-hosted deploy — needs to be aware that
setting `server_type` for the `pmcp-run` side has a real, possibly
unwanted, side effect on the `aws-lambda` side: it will fetch the generic
published binary instead of building the project's own compiled server.
Covered by
`detect_shape_builtin_wins_even_with_cargo_toml_and_src` but not otherwise
guarded — no warning is printed today.

### 3.7 Legacy-path `custom_stack` taint not yet in template `Metadata` (init.rs codegen follow-up)

`McpMetadata::custom_stack` (T7) is set whenever a project's
`deploy/lib/stack.ts` has been hand-modified from the regenerated scaffold
— this is also the condition that routes `synth_template` to the legacy
`cdk synth`/`cdk deploy` fallback instead of the renderer (see
`stack_routing::custom_stack_ts_reason`, shared between both targets in
`cargo-pmcp/src/deployment/stack_routing.rs`). The taint correctly reaches
`cdk synth`'s `-c mcp:customStack=true` **context argument** on the
`pmcp-run` target's legacy path. It does **not** yet reach the actual
uploaded CFN template's `Metadata` JSON block, because the generated
`stack.ts` (`commands/deploy/init.rs`) only bakes a fixed, enumerated set of
`this.node.tryGetContext(...)` keys into `Metadata` — `mcpVersion`,
`mcpServerType`, `mcpServerId`, `mcpTemplateId`, `mcpTemplateVersion`,
`mcpResources` — and `customStack` isn't one of them. On `aws-lambda`'s
legacy path the taint has **no consumer at all** today:
`DeployExecutor::run_cdk_deploy` passes zero `-c` context flags to `cdk
deploy`, so there is currently nowhere for the computed taint to flow on
that target (T9 computes it anyway, for structural parity, but leaves it
unwired — see the T9 report §3). Closing this gap means editing
`init.rs`'s TypeScript-string-templating code to add a `metadata['mcp:
customStack']` line, which risks the `tests/backward_compat_stack_ts.rs`
byte-identity goldens — deliberately deferred as a follow-up, not attempted
in T7–T9.

**Taint semantics, stated plainly**: the CLI **warns and records** when it
falls back due to a modified `stack.ts` (an `eprintln!` naming the file,
plus the metadata flag where it does reach a consumer). The CLI does **not**
enforce any policy based on the taint — whether a tainted/custom-stack
deployment is acceptable is entirely a platform-side decision. The platform
**may** choose to block or flag custom-stack deployments server-side once
(and if) the taint reaches its `Metadata` channel; today, for `pmcp-run`, it
doesn't reach that channel at all, so the platform currently has no visible
signal to act on.

### 3.8 `pmcp-run` OAuth servers fall back to legacy `cdk` (renderer family unimplemented)

Any `pmcp-run`-target project with `[auth].enabled = true` routes to
`LegacyCdk` today, unconditionally — `pmcp_cfn_renderer::render` doesn't
implement `pmcp-run`'s own OAuth stack shape (as distinct from the
`aws-lambda` target's Cognito+DCR shape, which **is** implemented and
golden-covered by `oauth-cognito-dcr.golden.json`). This is a real, expected
coverage gap: most production `pmcp-run` servers behind auth will not use
the renderer path at all until this family is implemented, meaning the
renderer cutover's real-world reach on `pmcp-run` is narrower than "every
unmodified scaffold" — it's closer to "every unmodified, unauthenticated
scaffold." Not a blocker for shipping the renderer for what it does cover,
but should be communicated as a known limitation, not discovered later as a
silent gap.

### 3.9 `TemplateBody` inline size: 28.9 KB largest golden vs. CloudFormation's 51,200-byte hard limit

`apply_stack` (`engine.rs`) always passes the rendered template **inline**
via `TemplateBody` — never via an S3-staged `TemplateURL`. CloudFormation
caps `TemplateBody` at 51,200 bytes. The largest checked-in semantic golden
today, `oauth-cognito-dcr.golden.json` (Cognito + DCR + a 3-Lambda HTTP API
— the most resource-dense shape currently implemented), is **28,879 bytes**
(≈28.9 KB) — comfortable headroom today, but a future resource-heavy stack
(more Lambdas, more IAM statements, more routes) could realistically exceed
the limit, especially since the renderer's canonical JSON serialization is
pretty-printed with the CDK-parity key ordering, not compact. **Mitigation,
not yet implemented**: switch to compact-JSON serialization (`serde_json`
without pretty-printing) before falling back to an S3 `TemplateURL` upload
path — compact JSON alone likely buys enough headroom for realistic stack
sizes given the pretty-printed golden's actual byte count includes a
meaningful fraction of whitespace. If a real deploy in §1 hits the 51,200
limit, that's the first fix to reach for; a full `TemplateURL` upload path
is the fallback if compact serialization alone isn't enough.

---

## 4. Sign-off

The renderer path is safe to make the *default* (i.e., no longer something
only reached by coincidence when a scaffold happens to be unmodified, but a
path the team is comfortable with every affected user silently taking) only
once:

- [ ] All five rows in §1 are PASS.
- [ ] §2's platform-validator acceptance is explicitly confirmed by the
      pmcp.run platform team, in writing, for shapes (a)/(b)/(c).
- [ ] §1 row (d)'s blocker — connector binaries publishing via
      `release-binary.yml` — is closed (see the `pmcp-cfn-renderer` release
      wiring changes to `.github/workflows/release-binary.yml` /
      `release.yml` landed alongside this runbook).
- [ ] Every operational note in §3 has been read by whoever is operating
      the first production rollout.
