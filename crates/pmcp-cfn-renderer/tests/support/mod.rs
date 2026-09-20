//! The semantic-golden normalizer.
//!
//! [`normalize`] turns a CFN template (either a real `cdk synth` output or
//! this crate's own [`pmcp_cfn_renderer::CfnTemplate::to_canonical_json`]
//! output) into a comparable "resource graph" form, per
//! `docs/superpowers/plans/2026-07-21-cfn-renderer-extraction.md` Task 2's
//! normalization algorithm. Both a real synthesized template (carrying CDK
//! bootstrap noise: `CDKMetadata`, `BootstrapVersion`, `Rules`, `aws:cdk:*`
//! metadata, opaque hash-suffixed logical IDs) and this crate's pure output
//! (none of that noise, human logical IDs) normalize to the SAME shape when
//! the underlying infrastructure is semantically identical — that equality
//! is the whole point of the golden harness ([`crate::semantic_golden`],
//! actually `tests/semantic_golden.rs`).
//!
//! Shared (via `#[path]`) by `tests/semantic_golden.rs` and
//! `examples/normalize_json.rs` (the golden-generation script's normalizer
//! entry point) — see both files' doc comments.
//!
//! # The algorithm (brief-specified, Task 2)
//!
//! 1. **Drop** template-level noise: `Resources.CDKMetadata`,
//!    `Parameters.BootstrapVersion` (and any `AssetParameters*` entry —
//!    see "Beyond the brief" below), `Rules`, any `Conditions` entry whose
//!    key matches `CheckBootstrapVersion`, and the template-level
//!    `Metadata` block entirely (per-resource `aws:cdk:*`/`cdk_` metadata
//!    keys are covered by dropping each resource's whole `Metadata` in step
//!    2 — see "Beyond the brief").
//! 2. **Drop per-resource** `Metadata`; drop `UpdateReplacePolicy`/
//!    `DeletionPolicy` when equal to the CFN-wide default (`"Delete"` for
//!    both, when omitted — CFN does not vary this default by resource
//!    type).
//! 3. **Canonicalize logical IDs**: assign `"<TypeSuffix>-<n>"` (sorted by
//!    `(Type, fingerprint)`, `fingerprint` = the resource's `Properties`
//!    with every `Ref`/`Fn::GetAtt` TARGET erased — see
//!    [`fingerprint_properties`]), then rewrite every `Ref`/`Fn::GetAtt`/
//!    `DependsOn` reference to the canonical ID. Run this twice (a
//!    "two-pass fixpoint" per the brief) so old CDK ids and the renderer's
//!    own human ids both disappear identically.
//! 4. **Sort** `DependsOn` arrays; drop them when empty.
//! 5. **Rebuild** the whole value with every JSON object's keys sorted
//!    (`serde_json::Value`'s own `PartialEq` is already order-insensitive,
//!    so this step doesn't change WHETHER two normalized values compare
//!    equal — it exists so the checked-in golden `.json` files have a
//!    stable, diffable-in-git textual form).
//!
//! # Beyond the brief
//!
//! Two extensions the brief's algorithm block doesn't spell out, needed
//! because Task 1's renderer omits `Outputs`/`Metadata` when empty and
//! never emits `AWSTemplateFormatVersion`/`Parameters`/`Rules`/
//! `Conditions`/CDKMetadata at all, while a real `cdk synth` always does:
//!
//! - **`AWSTemplateFormatVersion`** is dropped unconditionally (the
//!   renderer never emits it; cdk always does).
//! - **Absent `Outputs`/`Parameters`/`Conditions` == empty**: after the
//!   drops above, an empty `Outputs`/`Parameters`/`Conditions` map is
//!   removed entirely rather than left as `{}`, so the renderer's
//!   "omit when empty" convention compares equal to cdk's "always present,
//!   possibly empty after drops" convention.
//! - **`AWS::Lambda::Function.Properties.Code` is replaced with a fixed
//!   sentinel** (`{"S3Bucket": "<artifact>", "S3Key": "<artifact>"}`)
//!   rather than compared literally. The deployable artifact's S3 location
//!   is inherently environment/build-specific: a real `cdk synth` resolves
//!   it to the CDK-bootstrap assets bucket plus a content-hash key (or, for
//!   an environment-agnostic stack, a `{"Ref": "AssetParametersXXX"}`
//!   pointing at a `Parameters.AssetParametersXXX` entry this module also
//!   strips), while the renderer takes an explicit, arbitrary
//!   `RenderParams::artifact` — neither is "the descriptor's shape," so
//!   literal comparison would fail for a reason that has nothing to do
//!   with whether the two templates describe the same infrastructure. This
//!   sentinel substitution is the mechanism that satisfies the brief's
//!   generation-script note ("make cdk's real asset-parameter references
//!   comparable... replace their Ref in Function.Code with the literal
//!   params bucket/key") without needing to thread `RenderParams` into
//!   this function's signature (which the brief fixes as
//!   `fn(&Value) -> Value`).
//! - **`AWS::IAM::Policy.Properties.PolicyName` is replaced with a fixed
//!   sentinel** (`"<policy-name>"`) rather than compared literally. CDK
//!   derives this name from a content hash of the resource's construct path
//!   (e.g. `McpFunctionServiceRoleDefaultPolicy29310C43`), which is
//!   CDK-synthesis-specific identity, not renderer truth — the design spec
//!   (§5) forbids CDK-style content hashes in renderer output. Different
//!   fixtures produce different hash suffixes (compare the `plain-lambda`
//!   golden's `McpFunctionServiceRoleDefaultPolicy29310C43` against
//!   `oauth-cognito-dcr`'s second Lambda policy,
//!   `OAuthProxyFunctionServiceRoleDefaultPolicy7EA1E8EC`), so literal
//!   comparison would fail for a reason that has nothing to do with whether
//!   the two templates describe the same infrastructure. The renderer emits
//!   a stable literal (`"pmcp-declared"`) instead of a hash; this
//!   sentinelization makes both sides comparable regardless of which
//!   literal either side used.
//! - **The two family-internal IAM ARNs** (`resources::iam::render_policy`'s
//!   `MCP_SERVERS_TABLE` discovery reads + cross-Lambda invoke wildcard) are
//!   normalized to a shared `<region>`/`<account>`-sentinel form (T7
//!   review). Post-fix, the renderer emits these as `{"Fn::Sub":
//!   "arn:aws:dynamodb:${AWS::Region}:${AWS::AccountId}:table/..."}`
//!   pseudo-parameter forms, while a real `cdk synth` — synthesized here
//!   with an explicit, fixed fake `env: {account, region}` (see
//!   `scripts/generate-cfn-goldens.sh`) — bakes the SAME two ARNs as
//!   literal `"arn:aws:dynamodb:us-east-1:123456789012:table/..."`
//!   strings. Neither form is "renderer truth" any more than the other (see
//!   `resources::iam::family_internal_pseudo_param_arn`'s doc comment for
//!   why the renderer chose pseudo-params); [`sentinelize_family_internal_arns`]
//!   collapses both to one comparable shape, scoped narrowly to just these
//!   two ARN shapes so region/account literals elsewhere (Outputs URLs, the
//!   golden-pinned `execute-api`/apigateway ARNs — see
//!   `resources::http_api::render_permission_for`'s doc comment) still
//!   compare literally.
//!
//! # Tied fingerprints and the two-pass fixpoint's tiebreak
//!
//! When two resources of the same `Type` have identical `Properties`
//! shapes up to their `Ref`/`Fn::GetAtt` targets (e.g. two
//! `AWS::ApiGatewayV2::Integration`s that differ only in which Lambda
//! function they point at), the naive `(Type, fingerprint)` sort key ties.
//! Pass 1 MUST still erase `Ref`/`Fn::GetAtt` targets when computing that
//! fingerprint (its resources carry each template's own arbitrary,
//! tool-specific original logical IDs, so embedding them would make the
//! sort key itself template-dependent) — but that means pass 1's tiebreak
//! for genuinely-tied resources (the original logical-ID string) is stable
//! WITHIN one template only, not guaranteed to agree ACROSS a cdk-synth
//! template and a renderer template.
//!
//! Pass 2 closes this gap: by the time it runs, every resource is already
//! keyed by pass 1's canonical id, and every `Ref`/`Fn::GetAtt` INSIDE
//! `Properties` has already been rewritten to point at canonical ids too
//! (see [`rewrite_and_rekey`]) — both template-independent, by construction
//! of pass 1. So pass 2's fingerprint ([`fingerprint_properties`] called
//! with `erase_ref_identity: false`) keeps those canonical reference
//! targets INSTEAD of erasing them, which deterministically separates two
//! same-Type, same-shape resources that point at two DIFFERENT (and
//! therefore already distinctly canonicalized) targets — e.g. one
//! `Integration` pointing at `Function-0` sorts before one pointing at
//! `Function-1`, regardless of what either template's ORIGINAL logical IDs
//! happened to spell or what order they were declared in.
//!
//! If two resources are STILL tied after pass 2 (identical `Type`,
//! identical `Properties` INCLUDING their canonical reference targets),
//! they are genuinely interchangeable: swapping which physical resource a
//! template labels `Foo-0` vs `Foo-1` cannot change the normalized value,
//! because both labels then map to equal content. The final id-string
//! tiebreak in that case is just "some deterministic order," not a
//! correctness requirement — see
//! `tied_routes_under_different_original_ids_and_order_normalize_identically`
//! below.

use serde_json::{Map, Value};
use std::collections::BTreeMap;

/// Normalize a CFN template (cdk-synth OR renderer output) into a
/// comparable resource-graph form. See the module doc comment for the full
/// algorithm.
#[must_use]
pub fn normalize(template: &Value) -> Value {
    let mut root = template.as_object().cloned().unwrap_or_default();

    drop_template_level_noise(&mut root);
    let mut resources = take_object(&mut root, "Resources");
    resources.remove("CDKMetadata");
    for resource in resources.values_mut() {
        normalize_resource(resource);
    }

    // Two-pass fixpoint: canonicalize, rewrite, canonicalize again. Pass 1
    // (index 0) must erase Ref/GetAtt target identity (original ids are
    // template-specific); pass 2 (index 1) keeps it, now that every
    // reference points at a canonical, template-independent id — this is
    // what lets pass 2 break ties pass 1 couldn't. See the module doc
    // comment's "Tied fingerprints" section.
    for pass_index in 0..2 {
        let erase_ref_identity = pass_index == 0;
        let mapping = canonical_id_mapping(&resources, erase_ref_identity);
        resources = rewrite_and_rekey(resources, &mapping);
        if let Some(outputs) = root.get_mut("Outputs") {
            *outputs = rewrite_refs(outputs.clone(), &mapping);
        }
    }

    root.insert("Resources".to_string(), Value::Object(resources));
    drop_if_empty(&mut root, "Outputs");
    drop_if_empty(&mut root, "Parameters");
    drop_if_empty(&mut root, "Conditions");

    sort_keys_deep(&Value::Object(root))
}

/// Remove template-level CDK bootstrap noise that the renderer never emits:
/// `AWSTemplateFormatVersion`, the whole `Metadata` block, `Rules`, any
/// `Conditions` entry matching `CheckBootstrapVersion`, and
/// `Parameters.BootstrapVersion`/`Parameters.AssetParameters*`.
///
/// The template-level `Metadata` drop is unconditional and stays that way
/// even after the T7 review fix that made [`pmcp_cfn_renderer::render`]
/// emit `RenderParams::cloudformation_metadata` verbatim as
/// `CfnTemplate::metadata` (the `mcp:*` provenance block): a real
/// `cdk synth`'s `Metadata` carries its OWN CDK-specific keys (`aws:cdk:*`
/// path/analytics entries alongside whatever `mcp:*` keys `stack.ts` bakes
/// in) with a structurally different shape than what this renderer emits,
/// so literal comparison here would fail for reasons unrelated to whether
/// the two templates describe the same infrastructure — same rationale as
/// the per-resource `Metadata` drop below. That the renderer emits
/// `cloudformation_metadata` verbatim and byte-deterministically is a
/// UNIT-test-owned property (see `lib.rs`'s and `params.rs`'s own test
/// modules), not something this semantic-golden harness's normalizer is
/// positioned to check.
fn drop_template_level_noise(root: &mut Map<String, Value>) {
    root.remove("AWSTemplateFormatVersion");
    root.remove("Metadata");
    root.remove("Rules");

    if let Some(Value::Object(conditions)) = root.get_mut("Conditions") {
        conditions.retain(|name, _| !name.contains("CheckBootstrapVersion"));
    }

    if let Some(Value::Object(parameters)) = root.get_mut("Parameters") {
        parameters
            .retain(|name, _| name != "BootstrapVersion" && !name.starts_with("AssetParameters"));
    }
}

/// Drop `root[key]` entirely when it is present but an empty object/array,
/// so the renderer's "omit when empty" convention compares equal to cdk's
/// "always present, possibly empty after drops" convention.
fn drop_if_empty(root: &mut Map<String, Value>, key: &str) {
    let is_empty = match root.get(key) {
        Some(Value::Object(m)) => m.is_empty(),
        Some(Value::Array(a)) => a.is_empty(),
        _ => false,
    };
    if is_empty {
        root.remove(key);
    }
}

fn take_object(root: &mut Map<String, Value>, key: &str) -> Map<String, Value> {
    match root.remove(key) {
        Some(Value::Object(map)) => map,
        _ => Map::new(),
    }
}

/// Per-resource drops (step 2): `Metadata` unconditionally; `DeletionPolicy`/
/// `UpdateReplacePolicy` when equal to the CFN-wide default (`"Delete"`);
/// the `AWS::Lambda::Function` artifact-location sentinel substitution and
/// the `AWS::IAM::Policy` name sentinel substitution (see "Beyond the
/// brief" in the module doc comment); and DependsOn sorting (final re-sort
/// happens again after ID rewriting in [`rewrite_and_rekey`]).
fn normalize_resource(resource: &mut Value) {
    let Value::Object(fields) = resource else {
        return;
    };
    fields.remove("Metadata");
    remove_if_default_delete(fields, "DeletionPolicy");
    remove_if_default_delete(fields, "UpdateReplacePolicy");
    sort_depends_on(fields);

    let type_ = fields
        .get("Type")
        .and_then(Value::as_str)
        .map(str::to_string);
    if type_.as_deref() == Some("AWS::Lambda::Function") {
        sentinelize_function_code(fields);
    }
    if type_.as_deref() == Some("AWS::IAM::Policy") {
        sentinelize_policy_name(fields);
        sentinelize_family_internal_arns(fields);
    }
}

fn remove_if_default_delete(fields: &mut Map<String, Value>, key: &str) {
    if fields.get(key).and_then(Value::as_str) == Some("Delete") {
        fields.remove(key);
    }
}

fn sort_depends_on(fields: &mut Map<String, Value>) {
    let Some(Value::Array(depends)) = fields.get_mut("DependsOn") else {
        return;
    };
    depends.sort_by(|a, b| a.as_str().cmp(&b.as_str()));
    if depends.is_empty() {
        fields.remove("DependsOn");
    }
}

/// Replace `Properties.Code` on a Lambda function with a fixed sentinel —
/// see "Beyond the brief" in the module doc comment.
fn sentinelize_function_code(fields: &mut Map<String, Value>) {
    let Some(Value::Object(properties)) = fields.get_mut("Properties") else {
        return;
    };
    if properties.contains_key("Code") {
        properties.insert(
            "Code".to_string(),
            serde_json::json!({"S3Bucket": "<artifact>", "S3Key": "<artifact>"}),
        );
    }
}

/// Replace `Properties.PolicyName` on an `AWS::IAM::Policy` with a fixed
/// sentinel — see "Beyond the brief" in the module doc comment. CDK's
/// content-hash-derived name and the renderer's stable `"pmcp-declared"`
/// literal are both synthesis-specific labels, not renderer truth, so
/// neither should participate in golden comparison.
fn sentinelize_policy_name(fields: &mut Map<String, Value>) {
    let Some(Value::Object(properties)) = fields.get_mut("Properties") else {
        return;
    };
    if properties.contains_key("PolicyName") {
        properties.insert(
            "PolicyName".to_string(),
            Value::String("<policy-name>".to_string()),
        );
    }
}

/// Normalize `resources::iam::render_policy`'s two family-internal ARNs
/// (see the module doc comment's "Beyond the brief" section) wherever they
/// appear inside an `AWS::IAM::Policy` resource's `Properties` — collapsing
/// both the renderer's `{"Fn::Sub": "...${AWS::Region}...${AWS::AccountId}..."}`
/// pseudo-parameter form and a real `cdk synth`'s literal-account/region
/// string form to one comparable sentinel shape.
///
/// Recurses the WHOLE `Properties` value (not just
/// `PolicyDocument.Statement[].Resource`) for simplicity —
/// [`is_family_internal_arn`]'s shape gate (exact `table/McpServer(/*)?` /
/// `function:*` suffixes) is narrow enough that it never matches anything
/// else a policy resource could contain, including a USER-DECLARED
/// `[[iam.statements]]` ARN: no golden fixture's declared statements target
/// a table literally named `McpServer` or an unscoped Lambda `function:*`
/// (checked against the full corpus, `tests/goldens/*.golden.json`) — those
/// stay verbatim, compared literally, same as before this function existed.
fn sentinelize_family_internal_arns(fields: &mut Map<String, Value>) {
    if let Some(properties) = fields.get_mut("Properties") {
        sentinelize_arns_in(properties);
    }
}

/// Recursive worker for [`sentinelize_family_internal_arns`]. Checks (via a
/// read-only borrow that ends before any write) whether `value` is either a
/// bare family-internal ARN string or a `{"Fn::Sub": <family-internal ARN
/// template>}` wrapper; the latter is collapsed to a bare sentinelized
/// string so both source forms converge on identical JSON. Anything else is
/// recursed into unchanged.
fn sentinelize_arns_in(value: &mut Value) {
    if let Value::String(s) = value {
        if is_family_internal_arn(s) {
            *s = sentinelize_arn_string(s);
        }
        return;
    }

    let sub_replacement = match value.as_object() {
        Some(map) if map.len() == 1 => match map.get("Fn::Sub").and_then(Value::as_str) {
            Some(template) if is_family_internal_arn(template) => {
                Some(sentinelize_arn_string(template))
            },
            _ => None,
        },
        _ => None,
    };
    if let Some(sentinelized) = sub_replacement {
        *value = Value::String(sentinelized);
        return;
    }

    match value {
        Value::Object(map) => {
            for v in map.values_mut() {
                sentinelize_arns_in(v);
            }
        },
        Value::Array(items) => {
            for item in items.iter_mut() {
                sentinelize_arns_in(item);
            }
        },
        _ => {},
    }
}

/// `true` for exactly the two ARN shapes
/// [`crate::support::sentinelize_family_internal_arns`] normalizes — an
/// `arn:aws:dynamodb:...` ending in the fixed `MCP_SERVERS_TABLE` name
/// (`"McpServer"`, either the base table or its `/*` index wildcard), or an
/// `arn:aws:lambda:...` ending in the unscoped `function:*` invoke
/// wildcard. Matches both the literal-account/region form and the
/// `${AWS::Region}`/`${AWS::AccountId}` pseudo-parameter form (the suffix
/// this checks is identical either way).
fn is_family_internal_arn(s: &str) -> bool {
    (s.starts_with("arn:aws:dynamodb:")
        && (s.ends_with("table/McpServer") || s.ends_with("table/McpServer/*")))
        || (s.starts_with("arn:aws:lambda:") && s.ends_with("function:*"))
}

/// Collapse an ARN string's region (segment 3) and account (segment 4) to
/// `<region>`/`<account>` sentinels, regardless of which concrete form they
/// arrived in.
///
/// Pseudo-param templates (`${AWS::Region}`/`${AWS::AccountId}`) are
/// substituted via a plain textual replace FIRST — a naive `':'`-split
/// would incorrectly treat the `::` inside `${AWS::Region}` itself as ARN
/// field separators. Only when that substitution is a no-op (the literal
/// form, with no `${AWS::...}` tokens) does this fall back to splitting on
/// `':'` and overwriting the region/account POSITIONS directly — safe
/// there because a literal ARN segment never itself contains `':'`.
fn sentinelize_arn_string(s: &str) -> String {
    let substituted = s
        .replace("${AWS::Region}", "<region>")
        .replace("${AWS::AccountId}", "<account>");
    if substituted != s {
        return substituted;
    }

    let mut parts: Vec<&str> = s.splitn(6, ':').collect();
    if parts.len() < 6 || parts[0] != "arn" {
        return s.to_string();
    }
    parts[3] = "<region>";
    parts[4] = "<account>";
    parts.join(":")
}

// ---------------------------------------------------------------------
// Logical-ID canonicalization
// ---------------------------------------------------------------------

/// Build the `old-id -> "<TypeSuffix>-<n>"` mapping per the brief's sort
/// rule: `(Type, fingerprint)` ascending, tie-broken by the original
/// logical ID for determinism. `erase_ref_identity` selects which
/// [`fingerprint_properties`] behavior pass 1 vs pass 2 needs — see the
/// module doc comment's "Tied fingerprints" section.
fn canonical_id_mapping(
    resources: &Map<String, Value>,
    erase_ref_identity: bool,
) -> BTreeMap<String, String> {
    let mut ids: Vec<&String> = resources.keys().collect();
    ids.sort_by(|a, b| {
        let key_a = sort_key(&resources[*a], erase_ref_identity);
        let key_b = sort_key(&resources[*b], erase_ref_identity);
        key_a.cmp(&key_b).then_with(|| a.cmp(b))
    });

    let mut counters: BTreeMap<String, usize> = BTreeMap::new();
    let mut mapping = BTreeMap::new();
    for old_id in ids {
        let type_ = resources[old_id]
            .get("Type")
            .and_then(Value::as_str)
            .unwrap_or("Unknown");
        let suffix = type_suffix(type_);
        let n = counters.entry(suffix.clone()).or_insert(0);
        mapping.insert(old_id.clone(), format!("{suffix}-{n}"));
        *n += 1;
    }
    mapping
}

/// `(Type, fingerprint-of-Properties)` — see [`fingerprint_properties`] for
/// what `erase_ref_identity` does to `Ref`/`Fn::GetAtt` targets within.
fn sort_key(resource: &Value, erase_ref_identity: bool) -> (String, String) {
    let type_ = resource
        .get("Type")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let properties = resource.get("Properties").cloned().unwrap_or(Value::Null);
    let fingerprint = fingerprint_properties(&properties, erase_ref_identity);
    let json = serde_json::to_string(&sort_keys_deep(&fingerprint))
        .expect("a Value built from another Value always serializes");
    (type_, json)
}

/// Recursively fingerprint `value` for the canonical-id sort key.
///
/// When `erase_ref_identity` is `true` (pass 1), `Ref`/`Fn::GetAtt`
/// reference TARGETS are erased (replaced with a placeholder that keeps
/// only the `Fn::GetAtt` attribute name for mild disambiguation), so the
/// fingerprint depends only on a resource's own shape, never on which
/// (necessarily non-canonical, at pass 1) logical ID it happens to
/// reference.
///
/// When `false` (pass 2), reference targets are kept AS-IS — by pass 2,
/// every target is already a pass-1 canonical, template-independent id
/// (see [`rewrite_and_rekey`]), so keeping it lets two same-Type,
/// same-shape-when-erased resources that reference two DIFFERENT targets
/// sort deterministically apart instead of tying. See the module doc
/// comment's "Tied fingerprints" section.
fn fingerprint_properties(value: &Value, erase_ref_identity: bool) -> Value {
    match value {
        Value::Object(map) => {
            if erase_ref_identity {
                if map.len() == 1 && map.contains_key("Ref") {
                    return Value::String("<ref>".to_string());
                }
                if let Some(getatt) = map.get("Fn::GetAtt") {
                    if map.len() == 1 {
                        return Value::String(format!("<getatt:{}>", getatt_attribute(getatt)));
                    }
                }
            }
            let mut out = Map::new();
            for (k, v) in map {
                out.insert(k.clone(), fingerprint_properties(v, erase_ref_identity));
            }
            Value::Object(out)
        },
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|item| fingerprint_properties(item, erase_ref_identity))
                .collect(),
        ),
        other => other.clone(),
    }
}

/// The last `::`-separated segment of a CFN resource `Type`, e.g.
/// `"AWS::Lambda::Function"` -> `"Function"`, `"AWS::IAM::Role"` -> `"Role"`.
fn type_suffix(type_: &str) -> String {
    type_.rsplit("::").next().unwrap_or(type_).to_string()
}

fn getatt_attribute(getatt: &Value) -> String {
    match getatt {
        Value::Array(parts) if parts.len() == 2 => parts[1].as_str().unwrap_or("").to_string(),
        Value::String(dotted) => dotted
            .split_once('.')
            .map_or("", |(_, attr)| attr)
            .to_string(),
        _ => String::new(),
    }
}

/// Rewrite every `Ref`/`Fn::GetAtt`/`DependsOn` reference to its canonical
/// id (dropping any that don't appear in `mapping`, e.g. AWS pseudo
/// parameters like `AWS::Region`), then re-key `resources` by canonical id
/// and re-sort/drop each `DependsOn` array (values changed, so the step-2
/// sort must run again).
fn rewrite_and_rekey(
    resources: Map<String, Value>,
    mapping: &BTreeMap<String, String>,
) -> Map<String, Value> {
    let mut out = Map::new();
    for (old_id, resource) in resources {
        let canonical_id = mapping.get(&old_id).cloned().unwrap_or(old_id);
        let mut rewritten = rewrite_refs(resource, mapping);
        if let Value::Object(fields) = &mut rewritten {
            sort_depends_on(fields);
        }
        out.insert(canonical_id, rewritten);
    }
    out
}

/// Recursively rewrite `{"Ref": old}` / `{"Fn::GetAtt": [old, attr]}` /
/// `{"Fn::GetAtt": "old.attr"}` / `DependsOn` string entries to their
/// canonical id, per `mapping`. IDs not present in `mapping` (AWS pseudo
/// parameters, or an id this pass didn't touch) pass through unchanged.
fn rewrite_refs(value: Value, mapping: &BTreeMap<String, String>) -> Value {
    match value {
        Value::Object(map) => rewrite_refs_object(map, mapping),
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .map(|item| rewrite_refs(item, mapping))
                .collect(),
        ),
        other => other,
    }
}

fn rewrite_refs_object(map: Map<String, Value>, mapping: &BTreeMap<String, String>) -> Value {
    if map.len() == 1 {
        if let Some(Value::String(id)) = map.get("Ref") {
            let new_id = mapping.get(id).cloned().unwrap_or_else(|| id.clone());
            return serde_json::json!({"Ref": new_id});
        }
        if let Some(getatt) = map.get("Fn::GetAtt") {
            return rewrite_getatt(getatt, mapping);
        }
    }
    let mut out = Map::new();
    for (k, v) in map {
        if k == "DependsOn" {
            out.insert(k, rewrite_depends_on(v, mapping));
        } else {
            out.insert(k, rewrite_refs(v, mapping));
        }
    }
    Value::Object(out)
}

fn rewrite_getatt(getatt: &Value, mapping: &BTreeMap<String, String>) -> Value {
    match getatt {
        Value::Array(parts) if parts.len() == 2 => {
            let id = parts[0].as_str().unwrap_or_default();
            let new_id = mapping.get(id).cloned().unwrap_or_else(|| id.to_string());
            serde_json::json!({"Fn::GetAtt": [new_id, parts[1].clone()]})
        },
        Value::String(dotted) => {
            let (id, attr) = dotted.split_once('.').unwrap_or((dotted.as_str(), ""));
            let new_id = mapping.get(id).cloned().unwrap_or_else(|| id.to_string());
            serde_json::json!({"Fn::GetAtt": format!("{new_id}.{attr}")})
        },
        other => serde_json::json!({"Fn::GetAtt": other.clone()}),
    }
}

fn rewrite_depends_on(value: Value, mapping: &BTreeMap<String, String>) -> Value {
    match value {
        Value::String(id) => Value::String(mapping.get(&id).cloned().unwrap_or(id)),
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .map(|item| rewrite_depends_on(item, mapping))
                .collect(),
        ),
        other => other,
    }
}

// ---------------------------------------------------------------------
// Deterministic, sorted-key rebuild (step 5)
// ---------------------------------------------------------------------

/// Rebuild `value` with every JSON object's keys sorted. `serde_json::Value`
/// equality is already order-insensitive (so this doesn't change whether two
/// normalized templates compare equal), but it gives the checked-in golden
/// `.json` files a stable, diffable-in-git textual form.
fn sort_keys_deep(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut entries: BTreeMap<String, Value> = BTreeMap::new();
            for (k, v) in map {
                entries.insert(k.clone(), sort_keys_deep(v));
            }
            let mut out = Map::new();
            for (k, v) in entries {
                out.insert(k, v);
            }
            Value::Object(out)
        },
        Value::Array(items) => Value::Array(items.iter().map(sort_keys_deep).collect()),
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// A hand-written mini template exercising every drop rule + canonical
    /// IDs + Ref rewrite, per the brief's Step 3 instruction.
    fn mini_template() -> Value {
        json!({
            "AWSTemplateFormatVersion": "2010-09-09",
            "Metadata": {"aws:cdk:version": "2.100.0"},
            "Parameters": {
                "BootstrapVersion": {"Type": "AWS::SSM::Parameter::Value<String>"}
            },
            "Rules": {"CheckBootstrapVersion": {"Assertions": []}},
            "Resources": {
                "ZzzRoleAbc123": {
                    "Type": "AWS::IAM::Role",
                    "Properties": {"AssumeRolePolicyDocument": {"Version": "2012-10-17"}},
                    "Metadata": {"aws:cdk:path": "Stack/Role/Resource"}
                },
                "AaaFunctionXyz789": {
                    "Type": "AWS::Lambda::Function",
                    "Properties": {
                        "FunctionName": "demo",
                        "Role": {"Fn::GetAtt": ["ZzzRoleAbc123", "Arn"]},
                        "Code": {"S3Bucket": "cdk-hnb659fds-assets-1-us-east-1", "S3Key": "abc123.zip"}
                    },
                    "DependsOn": ["ZzzRoleAbc123"],
                    "UpdateReplacePolicy": "Delete",
                    "DeletionPolicy": "Delete",
                    "Metadata": {"aws:cdk:path": "Stack/Function/Resource"}
                },
                "CDKMetadata": {
                    "Type": "AWS::CDK::Metadata",
                    "Properties": {"Analytics": "v2:deflate64:xyz"}
                }
            },
            "Outputs": {
                "RoleArn": {"Value": {"Fn::GetAtt": ["ZzzRoleAbc123", "Arn"]}}
            }
        })
    }

    #[test]
    fn drops_cdk_bootstrap_noise() {
        let normalized = normalize(&mini_template());
        assert!(normalized.get("AWSTemplateFormatVersion").is_none());
        assert!(normalized.get("Metadata").is_none());
        assert!(normalized.get("Parameters").is_none());
        assert!(normalized.get("Rules").is_none());
        assert!(normalized["Resources"].get("CDKMetadata").is_none());
    }

    #[test]
    fn drops_per_resource_metadata_and_default_policies() {
        let normalized = normalize(&mini_template());
        let function = &normalized["Resources"]["Function-0"];
        assert!(function.get("Metadata").is_none());
        assert!(function.get("UpdateReplacePolicy").is_none());
        assert!(function.get("DeletionPolicy").is_none());
    }

    #[test]
    fn canonicalizes_logical_ids_and_rewrites_references() {
        let normalized = normalize(&mini_template());
        let resources = normalized["Resources"].as_object().unwrap();
        assert!(resources.contains_key("Role-0"), "{resources:?}");
        assert!(resources.contains_key("Function-0"), "{resources:?}");
        assert!(!resources.contains_key("ZzzRoleAbc123"));
        assert!(!resources.contains_key("AaaFunctionXyz789"));

        // Fn::GetAtt inside Properties.Role rewritten to the canonical id.
        assert_eq!(
            normalized["Resources"]["Function-0"]["Properties"]["Role"],
            json!({"Fn::GetAtt": ["Role-0", "Arn"]})
        );
        // DependsOn rewritten too.
        assert_eq!(
            normalized["Resources"]["Function-0"]["DependsOn"],
            json!(["Role-0"])
        );
        // Outputs rewritten as well.
        assert_eq!(
            normalized["Outputs"]["RoleArn"]["Value"],
            json!({"Fn::GetAtt": ["Role-0", "Arn"]})
        );
    }

    #[test]
    fn sentinelizes_function_code() {
        let normalized = normalize(&mini_template());
        assert_eq!(
            normalized["Resources"]["Function-0"]["Properties"]["Code"],
            json!({"S3Bucket": "<artifact>", "S3Key": "<artifact>"})
        );
    }

    /// Two templates whose ONLY difference is the IAM policy's
    /// `PolicyName` (a CDK content-hash literal on one side, the
    /// renderer's stable `"pmcp-declared"` literal on the other) must
    /// normalize identically — this is the mechanism that lets the golden
    /// harness compare cdk-synth output against renderer output without
    /// hardcoding either side's synthesis-specific literal. See the
    /// module doc comment's "Beyond the brief" section.
    #[test]
    fn sentinelizes_iam_policy_name() {
        let cdk_hash_named = json!({
            "Resources": {
                "Policy": {
                    "Type": "AWS::IAM::Policy",
                    "Properties": {
                        "PolicyName": "McpFunctionServiceRoleDefaultPolicy29310C43",
                        "Roles": [{"Ref": "Role"}]
                    }
                },
                "Role": {
                    "Type": "AWS::IAM::Role",
                    "Properties": {}
                }
            }
        });
        let renderer_named = json!({
            "Resources": {
                "Policy": {
                    "Type": "AWS::IAM::Policy",
                    "Properties": {
                        "PolicyName": "pmcp-declared",
                        "Roles": [{"Ref": "Role"}]
                    }
                },
                "Role": {
                    "Type": "AWS::IAM::Role",
                    "Properties": {}
                }
            }
        });

        let normalized = normalize(&cdk_hash_named);
        assert_eq!(
            normalized["Resources"]["Policy-0"]["Properties"]["PolicyName"],
            json!("<policy-name>")
        );
        assert_eq!(normalized, normalize(&renderer_named));
    }

    #[test]
    fn is_stable_across_renumbered_but_equivalent_ids() {
        // Same template, different (still-non-canonical) source IDs and
        // insertion order — must normalize identically.
        let alt = json!({
            "Resources": {
                "LambdaOne": {
                    "Type": "AWS::Lambda::Function",
                    "Properties": {
                        "FunctionName": "demo",
                        "Role": {"Fn::GetAtt": ["RoleOne", "Arn"]},
                        "Code": {"S3Bucket": "other-bucket", "S3Key": "other-key.zip"}
                    },
                    "DependsOn": ["RoleOne"]
                },
                "RoleOne": {
                    "Type": "AWS::IAM::Role",
                    "Properties": {"AssumeRolePolicyDocument": {"Version": "2012-10-17"}}
                }
            },
            "Outputs": {
                "RoleArn": {"Value": {"Fn::GetAtt": ["RoleOne", "Arn"]}}
            }
        });
        assert_eq!(normalize(&mini_template()), normalize(&alt));
    }

    #[test]
    fn empty_outputs_normalizes_to_absent() {
        let template = json!({
            "Resources": {
                "Role": {
                    "Type": "AWS::IAM::Role",
                    "Properties": {}
                }
            },
            "Outputs": {}
        });
        assert!(normalize(&template).get("Outputs").is_none());
    }

    /// Controller-mandated regression: two templates carrying the same TWO
    /// `AWS::ApiGatewayV2::Route`s (genuinely interchangeable — identical
    /// `Properties`, including which `Integration` they target), declared
    /// under different original logical IDs and in different declaration
    /// order, must normalize identically. Per the module doc comment's
    /// "Tied fingerprints" section, resources this symmetric stay tied even
    /// after pass 2's ref-identity-aware fingerprint — and that's fine,
    /// because whichever physical Route a template labels `Route-0` vs
    /// `Route-1`, both labels end up mapped to equal content.
    #[test]
    fn tied_routes_under_different_original_ids_and_order_normalize_identically() {
        fn route(api_ref: &str, integration_ref: &str) -> Value {
            json!({
                "Type": "AWS::ApiGatewayV2::Route",
                "Properties": {
                    "ApiId": {"Ref": api_ref},
                    "RouteKey": "POST /{proxy+}",
                    "Target": {
                        "Fn::Join": ["", ["integrations/", {"Ref": integration_ref}]]
                    }
                }
            })
        }

        let template_a = json!({
            "Resources": {
                "RouteFoo": route("MyApi", "MyIntegration"),
                "RouteBar": route("MyApi", "MyIntegration"),
                "MyApi": {"Type": "AWS::ApiGatewayV2::Api", "Properties": {}},
                "MyIntegration": {"Type": "AWS::ApiGatewayV2::Integration", "Properties": {}}
            }
        });
        // Same graph: same two routes, different original ids, declared in
        // the opposite order.
        let template_b = json!({
            "Resources": {
                "MyIntegration": {"Type": "AWS::ApiGatewayV2::Integration", "Properties": {}},
                "ZRoute": route("MyApi", "MyIntegration"),
                "AnotherRoute": route("MyApi", "MyIntegration"),
                "MyApi": {"Type": "AWS::ApiGatewayV2::Api", "Properties": {}}
            }
        });

        assert_eq!(normalize(&template_a), normalize(&template_b));
    }

    /// Proves pass 2's fix actually has teeth: two `Integration`s that tie
    /// under pass 1's erased fingerprint (same `IntegrationType`, same
    /// erased `IntegrationUri`) but point at two DIFFERENT (non-tied,
    /// deterministically-orderable-by-own-Properties) Lambda functions.
    /// The two templates below name their resources so that a NAIVE
    /// original-logical-ID tiebreak would disagree between them (in
    /// `template_a`, `"AIntegration"` — alphabetically first — points at
    /// the function that ends up `Function-0`; in `template_b`,
    /// `"AIntegration"` points at the OTHER function instead). Without pass
    /// 2's ref-identity-aware fingerprint, this pair would normalize
    /// DIFFERENTLY; with it, both converge on "the Integration targeting
    /// `Function-0` is always `Integration-0`," regardless of either
    /// template's original spelling.
    #[test]
    fn tied_integrations_disambiguated_by_canonical_target_identity() {
        fn integration(function_ref: &str) -> Value {
            json!({
                "Type": "AWS::ApiGatewayV2::Integration",
                "Properties": {
                    "IntegrationType": "AWS_PROXY",
                    "IntegrationUri": {"Fn::GetAtt": [function_ref, "Arn"]}
                }
            })
        }
        fn function(name: &str) -> Value {
            json!({
                "Type": "AWS::Lambda::Function",
                "Properties": {"FunctionName": name}
            })
        }

        // "alpha" < "beta" as a fingerprint string, so AlphaFn always
        // becomes Function-0 regardless of original id — independent of
        // this test's fix.
        let template_a = json!({
            "Resources": {
                "AIntegration": integration("AlphaFn"),
                "ZIntegration": integration("BetaFn"),
                "AlphaFn": function("alpha"),
                "BetaFn": function("beta"),
            }
        });
        // Same graph, but original ids point the OTHER way: here
        // "AIntegration" (still alphabetically first) targets BetaFn
        // instead of AlphaFn.
        let template_b = json!({
            "Resources": {
                "AIntegration": integration("BetaFn"),
                "ZIntegration": integration("AlphaFn"),
                "AlphaFn": function("alpha"),
                "BetaFn": function("beta"),
            }
        });

        let normalized_a = normalize(&template_a);
        let normalized_b = normalize(&template_b);
        assert_eq!(normalized_a, normalized_b);

        // Pin the actual disambiguation, not just cross-template equality:
        // the Integration targeting Function-0 (AlphaFn) is Integration-0
        // in both templates.
        assert_eq!(
            normalized_a["Resources"]["Integration-0"]["Properties"]["IntegrationUri"],
            json!({"Fn::GetAtt": ["Function-0", "Arn"]})
        );
        assert_eq!(
            normalized_a["Resources"]["Integration-1"]["Properties"]["IntegrationUri"],
            json!({"Fn::GetAtt": ["Function-1", "Arn"]})
        );
    }

    /// T7 review fix: a template carrying the renderer's post-fix
    /// `{"Fn::Sub": "...${AWS::Region}...${AWS::AccountId}..."}`
    /// pseudo-parameter form for the two family-internal ARNs, and one
    /// carrying a real `cdk synth`'s literal-account/region form for the
    /// SAME two ARNs, must normalize identically — this is the mechanism
    /// that lets the golden harness compare renderer output (pseudo-params)
    /// against `cdk synth` output (literal, fixed fake account/region)
    /// without hardcoding either side's form. See the module doc comment's
    /// "Beyond the brief" section.
    #[test]
    fn sentinelizes_family_internal_arns_both_forms_identically() {
        fn policy(dynamodb_arns: [Value; 2], lambda_arn: Value) -> Value {
            json!({
                "Resources": {
                    "Policy": {
                        "Type": "AWS::IAM::Policy",
                        "Properties": {
                            "PolicyName": "pmcp-declared",
                            "PolicyDocument": {
                                "Version": "2012-10-17",
                                "Statement": [
                                    {
                                        "Effect": "Allow",
                                        "Action": ["dynamodb:GetItem", "dynamodb:Query"],
                                        "Resource": dynamodb_arns,
                                    },
                                    {
                                        "Effect": "Allow",
                                        "Action": "lambda:InvokeFunction",
                                        "Resource": lambda_arn,
                                    },
                                ],
                            },
                            "Roles": [{"Ref": "Role"}]
                        }
                    },
                    "Role": {"Type": "AWS::IAM::Role", "Properties": {}}
                }
            })
        }

        let pseudo_param_form = policy(
            [
                json!({"Fn::Sub": "arn:aws:dynamodb:${AWS::Region}:${AWS::AccountId}:table/McpServer"}),
                json!({"Fn::Sub": "arn:aws:dynamodb:${AWS::Region}:${AWS::AccountId}:table/McpServer/*"}),
            ],
            json!({"Fn::Sub": "arn:aws:lambda:${AWS::Region}:${AWS::AccountId}:function:*"}),
        );
        let literal_form = policy(
            [
                json!("arn:aws:dynamodb:us-east-1:123456789012:table/McpServer"),
                json!("arn:aws:dynamodb:us-east-1:123456789012:table/McpServer/*"),
            ],
            json!("arn:aws:lambda:us-east-1:123456789012:function:*"),
        );

        let normalized_pseudo = normalize(&pseudo_param_form);
        let normalized_literal = normalize(&literal_form);
        assert_eq!(normalized_pseudo, normalized_literal);

        // Pin the actual sentinelized shape, not just cross-form equality.
        let statements = normalized_pseudo["Resources"]["Policy-0"]["Properties"]["PolicyDocument"]
            ["Statement"]
            .as_array()
            .unwrap();
        assert_eq!(
            statements[0]["Resource"],
            json!([
                "arn:aws:dynamodb:<region>:<account>:table/McpServer",
                "arn:aws:dynamodb:<region>:<account>:table/McpServer/*",
            ])
        );
        assert_eq!(
            statements[1]["Resource"],
            json!("arn:aws:lambda:<region>:<account>:function:*")
        );
    }

    /// A user-declared `[[iam.statements]]` ARN that merely LOOKS like a
    /// DynamoDB/Lambda ARN (but targets a different table / isn't the
    /// unscoped `function:*` wildcard) must NOT be touched by
    /// [`sentinelize_family_internal_arns`] — it stays comparable literally,
    /// same as before this normalizer extension existed.
    #[test]
    fn does_not_sentinelize_a_user_declared_lookalike_arn() {
        let template = json!({
            "Resources": {
                "Policy": {
                    "Type": "AWS::IAM::Policy",
                    "Properties": {
                        "PolicyName": "pmcp-declared",
                        "PolicyDocument": {
                            "Version": "2012-10-17",
                            "Statement": [{
                                "Effect": "Allow",
                                "Action": "dynamodb:GetItem",
                                "Resource": "arn:aws:dynamodb:us-east-1:123456789012:table/orders",
                            }],
                        },
                        "Roles": [{"Ref": "Role"}]
                    }
                },
                "Role": {"Type": "AWS::IAM::Role", "Properties": {}}
            }
        });
        let normalized = normalize(&template);
        let statement =
            &normalized["Resources"]["Policy-0"]["Properties"]["PolicyDocument"]["Statement"][0];
        assert_eq!(
            statement["Resource"],
            json!("arn:aws:dynamodb:us-east-1:123456789012:table/orders"),
            "a non-family-internal ARN must be compared literally, unmodified"
        );
    }

    #[test]
    fn non_default_deletion_policy_is_preserved() {
        let template = json!({
            "Resources": {
                "Table": {
                    "Type": "AWS::DynamoDB::Table",
                    "Properties": {},
                    "DeletionPolicy": "Retain"
                }
            }
        });
        let normalized = normalize(&template);
        assert_eq!(
            normalized["Resources"]["Table-0"]["DeletionPolicy"],
            json!("Retain")
        );
    }
}
