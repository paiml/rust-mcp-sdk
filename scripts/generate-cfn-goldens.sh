#!/usr/bin/env bash
# Regenerates crates/pmcp-cfn-renderer/tests/goldens/ from real `cdk synth`
# output. One-time, LOCAL, requires: node/npm/npx (with `aws-cdk` installed
# per-project via `npm install`, done automatically by `cargo pmcp deploy
# init`), jq, cargo, the `aws` CLI (only used as a config no-op — see
# FAKE_PROFILE below). NEVER runs in CI — goldens are checked in.
#
# Corpus reality (see tests/goldens/README.md for the full inventory):
# this repo has exactly ONE tracked `.pmcp/deploy.toml`
# (crates/pmcp-server/.pmcp/deploy.toml), not the 19 the original design
# doc assumed (those 19 live in the sibling `pmcp-run` repo). Since every
# golden file embeds its own `descriptor_toml` + `params`, CI stays
# self-contained regardless of where the TOML came from, so this script
# draws from two sources:
#   1. GENERATED: fresh scaffold projects via cargo-pmcp's OWN scaffolder
#      (`cargo pmcp deploy init`), one per resource-family variant.
#   2. WILD: a real fixture's `.pmcp/deploy.toml` + `deploy/{lib,bin,cdk.json,
#      package.json,tsconfig.json}` copied from the read-only sibling
#      `pmcp-run` checkout (never written to), re-synthesized here with a
#      fake account so no real AWS account number is ever committed.
#
# Account/region control: `cdk synth` resolves `CDK_DEFAULT_ACCOUNT`/
# `CDK_DEFAULT_REGION` from the ACTIVE AWS credential context and passes
# them to the app subprocess, overriding whatever this script exports —
# UNLESS credential resolution fails, in which case our exported values
# survive untouched. FAKE_PROFILE points AWS_PROFILE at a profile that does
# not exist, forcing that failure deterministically (no real AWS account
# number ever needs to end up in a checked-in golden). `bin/app.ts` (the
# scaffold's own template) additionally reads `AWS_REGION` BEFORE
# `CDK_DEFAULT_REGION`, so exporting it here pins the region even in the
# (untested) case where credential resolution unexpectedly succeeds.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
GOLDEN_DIR="$REPO_ROOT/crates/pmcp-cfn-renderer/tests/goldens"
PENDING_DIR="$GOLDEN_DIR/pending"

FAKE_ACCOUNT="123456789012"
FAKE_PROFILE="__nonexistent_profile_for_golden_gen__"
REGION="us-east-1"
PMCP_RUN_REPO="${PMCP_RUN_REPO:-$REPO_ROOT/../pmcp-run}"

WORKDIR="$(mktemp -d)"
trap 'rm -rf "$WORKDIR"' EXIT

for tool in node npm npx jq cargo; do
  command -v "$tool" >/dev/null 2>&1 || {
    echo "generate-cfn-goldens.sh: missing required tool '$tool'" >&2
    exit 1
  }
done

cargo_pmcp() {
  cargo run --manifest-path "$REPO_ROOT/Cargo.toml" -p cargo-pmcp --bin cargo-pmcp -- "$@"
}

normalize_json() {
  cargo run --manifest-path "$REPO_ROOT/Cargo.toml" -p pmcp-cfn-renderer \
    --example normalize_json --quiet
}

# A minimal (never built) Cargo "workspace root package" — just enough for
# cargo-pmcp's `deploy init` scaffolder to accept the directory as a project
# (it shells out to `cargo metadata`, which requires a target) and its
# Lambda-wrapper generator to find a `[workspace.dependencies].pmcp` entry.
write_stub_crate() {
  local project="$1" slug="$2"
  mkdir -p "$project/src"
  cat > "$project/Cargo.toml" <<EOF
[package]
name = "$slug-fixture"
version = "0.1.0"
edition = "2021"

[workspace]
members = []

[workspace.dependencies]
pmcp = { version = "2.17" }
EOF
  printf '//! scratch fixture stub for golden generation\n' > "$project/src/lib.rs"
}

# Scaffold a fresh project via cargo-pmcp's own `deploy init` — the PRIMARY
# corpus source. Extra args (e.g. `--oauth cognito`) are forwarded.
scaffold_generated() {
  local slug="$1" target_type="$2"
  shift 2
  local project="$WORKDIR/$slug"
  write_stub_crate "$project" "$slug"
  (cd "$project" && cargo_pmcp deploy --target-type "$target_type" init \
    --skip-credentials-check --region "$REGION" "$@" >/dev/null)
}

# Copy a real fixture's descriptor + CDK scaffold (never its `cdk.out/` or
# `node_modules/`) from the read-only sibling repo — the SECONDARY corpus
# source. Re-synthesizing here (rather than reusing any checked-in
# `cdk.out/`) guarantees the fake account, never a real one, ends up in the
# committed golden.
scaffold_wild() {
  local slug="$1" source_dir="$2"
  local project="$WORKDIR/$slug"
  mkdir -p "$project/.pmcp" "$project/deploy/lib" "$project/deploy/bin"
  cp "$source_dir/.pmcp/deploy.toml" "$project/.pmcp/deploy.toml"
  cp "$source_dir/deploy/lib/stack.ts" "$project/deploy/lib/stack.ts"
  cp "$source_dir/deploy/bin/app.ts" "$project/deploy/bin/app.ts"
  cp "$source_dir/deploy/cdk.json" "$project/deploy/cdk.json"
  cp "$source_dir/deploy/package.json" "$project/deploy/package.json"
  cp "$source_dir/deploy/tsconfig.json" "$project/deploy/tsconfig.json"
  (cd "$project/deploy" && npm install --silent >/dev/null)
}

# `lambda.Code.fromAsset('.build'[-oauth-proxy|-authorizer])` needs a
# non-empty directory to exist at synth time; content is never inspected.
write_placeholder_assets() {
  local project="$1" dir
  for dir in .build .build-oauth-proxy .build-authorizer; do
    mkdir -p "$project/deploy/$dir"
    printf '#!/bin/sh\necho stub\n' > "$project/deploy/$dir/bootstrap"
    chmod +x "$project/deploy/$dir/bootstrap"
  done
}

synth() {
  local slug="$1"
  local project="$WORKDIR/$slug"
  write_placeholder_assets "$project"
  (
    cd "$project/deploy"
    [ -d node_modules ] || npm install --silent >/dev/null
    rm -rf cdk.out
    AWS_PROFILE="$FAKE_PROFILE" AWS_REGION="$REGION" \
      CDK_DEFAULT_ACCOUNT="$FAKE_ACCOUNT" CDK_DEFAULT_REGION="$REGION" \
      npx cdk synth --quiet
  )
}

# Assemble `{descriptor_toml, params, normalized}` and write it to $dest.
# `params` mirrors tests/determinism.rs's convention: fixed fake account,
# stack_name "<server_name>-stack", artifact bucket
# "pmcp-deploy-<account>-<region>", key "<server_name>/bootstrap.zip".
assemble_golden() {
  local slug="$1" server_name="$2" dest="$3"
  local project="$WORKDIR/$slug"
  local template
  template="$(find "$project/deploy/cdk.out" -maxdepth 1 -name '*.template.json' | head -1)"
  [ -n "$template" ] || {
    echo "generate-cfn-goldens.sh: no template.json produced for $slug" >&2
    exit 1
  }

  local descriptor_toml normalized params
  descriptor_toml="$(cat "$project/.pmcp/deploy.toml")"
  normalized="$(normalize_json < "$template")"
  params="$(jq -n \
    --arg account "$FAKE_ACCOUNT" \
    --arg region "$REGION" \
    --arg stack "${server_name}-stack" \
    --arg bucket "pmcp-deploy-${FAKE_ACCOUNT}-${REGION}" \
    --arg key "${server_name}/bootstrap.zip" \
    '{account_id: $account, region: $region, stack_name: $stack,
      artifact: {s3_bucket: $bucket, s3_key: $key},
      environment: {RUST_LOG: "info"},
      metadata: {version: "1.0.0", snapshot_baked: false}}')"

  mkdir -p "$(dirname "$dest")"
  jq -n --arg descriptor "$descriptor_toml" --argjson params "$params" \
    --argjson normalized "$normalized" \
    '{descriptor_toml: $descriptor, params: $params, normalized: $normalized}' \
    > "$dest"
  echo "wrote $dest"
}

generate() {
  local slug="$1" server_name="$2" dest="$3"
  synth "$slug"
  assemble_golden "$slug" "$server_name" "$dest"
}

# --- PRIMARY corpus: fresh scaffolds, one per resource-family variant ---
scaffold_generated plain-lambda pmcp-run
generate plain-lambda plain-lambda-fixture "$GOLDEN_DIR/plain-lambda.golden.json"

scaffold_generated http-api aws-lambda
generate http-api http-api-fixture "$PENDING_DIR/http-api.golden.json"

scaffold_generated oauth-cognito-dcr aws-lambda --oauth cognito
generate oauth-cognito-dcr oauth-cognito-dcr-fixture "$PENDING_DIR/oauth-cognito-dcr.golden.json"

# --- Task 4 (iam module): a minimal, purpose-built [[iam.statements]]
# fixture pinning the single-vs-array collapse rule (a single-action
# wildcard statement collapses Action/Resource to a bare scalar; a
# two-action/two-resource statement stays arrays). `deploy init`'s scaffold
# takes a STATIC snapshot of stack.ts at scaffold time — it is not
# regenerated from `.pmcp/deploy.toml` by `npx cdk synth` alone (only a full
# `cargo pmcp deploy` build+deploy cycle re-renders it, which needs a real
# Lambda-buildable crate, impractical for a throwaway fixture) — so this
# helper appends the `[[iam.statements]]` block to deploy.toml AND splices
# the matching `mcpFunction.addToRolePolicy(...)` calls into stack.ts by
# hand, mirroring exactly what `cargo-pmcp/src/deployment/iam.rs::render_statement`
# would emit for the same statements.
scaffold_iam_statements() {
  local slug="iam-statements"
  local project="$WORKDIR/$slug"
  write_stub_crate "$project" "$slug"
  (cd "$project" && cargo_pmcp deploy --target-type pmcp-run init \
    --skip-credentials-check --region "$REGION" >/dev/null)

  cat >>"$project/.pmcp/deploy.toml" <<'IAMEOF'

[[iam.statements]]
effect = "Allow"
actions = ["dynamodb:GetItem", "dynamodb:Query"]
resources = [
    "arn:aws:dynamodb:us-east-1:123456789012:table/orders",
    "arn:aws:dynamodb:us-east-1:123456789012:table/customers",
]

[[iam.statements]]
effect = "Allow"
actions = ["*"]
resources = ["arn:aws:s3:::iam-statements-fixture-bucket/*"]
IAMEOF

  local stack_ts="$project/deploy/lib/stack.ts"
  local splice_file="$project/.iam-splice.ts"
  cat >"$splice_file" <<'TSEOF'

    // ========================================================================
    // Operator-declared IAM (from .pmcp/deploy.toml [iam])
    // ========================================================================
    mcpFunction.addToRolePolicy(new iam.PolicyStatement({
      effect: iam.Effect.ALLOW,
      actions: ['dynamodb:GetItem', 'dynamodb:Query'],
      resources: [
        `arn:aws:dynamodb:us-east-1:123456789012:table/orders`,
        `arn:aws:dynamodb:us-east-1:123456789012:table/customers`,
      ],
    }));
    mcpFunction.addToRolePolicy(new iam.PolicyStatement({
      effect: iam.Effect.ALLOW,
      actions: ['*'],
      resources: [
        `arn:aws:s3:::iam-statements-fixture-bucket/*`,
      ],
    }));

    // Outputs
TSEOF
  # Portable (macOS/BSD + GNU) in-place edit: rewrite the file inserting the
  # splice text (read from a file via `getline`, not a `-v` string — BSD
  # awk's `-v` mangles embedded literal newlines) just before the
  # `// Outputs` line, rather than `sed -i` (whose in-place-backup-suffix
  # argument differs between BSD and GNU sed).
  awk -v splice_file="$splice_file" '
    /^    \/\/ Outputs$/ {
      while ((getline line < splice_file) > 0) print line
      next
    }
    { print }
  ' "$stack_ts" > "$stack_ts.new"
  mv "$stack_ts.new" "$stack_ts"
  rm -f "$splice_file"
}
scaffold_iam_statements
generate iam-statements iam-statements-fixture "$GOLDEN_DIR/iam-statements.golden.json"

# --- SECONDARY corpus: a real, custom-[[iam.statements]]-carrying fixture
# from the sibling pmcp-run repo (read-only; copied, never modified). Its
# stack.ts came from a real prior `cargo pmcp deploy` cycle in that repo, so
# (unlike the generated fixtures above) it already reflects its declared
# `[[iam.statements]]` without needing the splice-by-hand workaround. ---
if [ -d "$PMCP_RUN_REPO/built-in/sql-api/servers/msr-vtt/deploy" ]; then
  scaffold_wild wild-msr-vtt "$PMCP_RUN_REPO/built-in/sql-api/servers/msr-vtt"
  generate wild-msr-vtt msr-vtt "$GOLDEN_DIR/wild-msr-vtt.golden.json"
else
  echo "skip wild-msr-vtt: $PMCP_RUN_REPO not found locally (set PMCP_RUN_REPO)" >&2
fi

# --- Known skips (see tests/goldens/README.md for the full reasoning) ---
echo "skip crates/pmcp-server (this repo's only tracked fixture): same" \
  "target=pmcp-run + non-oauth shape as the generated plain-lambda fixture" \
  "and has no checked-in deploy/ dir — redundant, no coverage added" >&2
echo "skip pmcp-run:built-in/test-harness/oauth-external-google (target=" \
  "google-cloud-run): non-CFN target, out of this renderer's scope" >&2
echo "deferred: the remaining pmcp-run wild fixtures (graphrag/openapi/" \
  "agents-api families) were not processed this task — candidates for" \
  "Task 4/6 corpus expansion when their resource families need broader" \
  "coverage" >&2
