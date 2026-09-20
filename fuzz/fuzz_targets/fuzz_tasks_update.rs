//! Fuzz target for the RAW `tasks/update` params boundary (Phase 114, plan 14 —
//! TASK-02).
//!
//! CLAUDE.md ALWAYS / FUZZ Testing: `cargo fuzz run fuzz_tasks_update` (plain
//! form, no `+nightly` — matches the repo Makefile `test-fuzz` target).
//!
//! # Why THIS boundary
//!
//! `inputResponses` is the ENTIRE `tasks/update` request payload, it is the only
//! large client-supplied structure on the route, and its decode is the one place
//! in the tasks surface where guessing at overlapping shapes is actively wrong.
//! D-113-O is the measured precedent: `ElicitResult` and `CreateMessageResult`
//! structurally overlap, an untagged decoder silently reclassified an elicitation
//! answer as sampling, the handler's `Elicitation` arm never matched, and the
//! operation re-elicited **sixteen times** before dying on a misleading error.
//! The server knows the kinds because it minted them; this target exists to prove
//! no byte string can make it stop using them.
//!
//! The seam runs the route's pure prefix — parse, bound, kind-directed decode —
//! against a FIXED synthetic record, so a crash artifact replays
//! deterministically regardless of ambient process state. It performs no store
//! write.
//!
//! # Invariants
//!
//! 1. `judge_update_params` NEVER panics, whatever the input bytes. A remote
//!    party supplies these bytes, so an unwind here is a denial of service it can
//!    trigger at will (T-114-68).
//! 2. It NEVER returns [`VERDICT_ACCEPTED`] for a payload violating ANY of the
//!    four bounds — entry count, per-entry size, total size, nesting depth. The
//!    bounds are re-derived HERE from the payload this target parsed itself, so
//!    this is an independent check rather than the production code agreeing with
//!    itself. Deleting the seam's bounds pre-check makes this fire (recorded in
//!    `114-14-SUMMARY.md`).
//! 3. It NEVER accepts a key absent from the synthetic record's `inputRequests`.
//!    An accept there would be the "client chooses its own kind" break: the
//!    kinds must come from the server's record and from nowhere else (T-114-74).
//! 4. It NEVER accepts a value that does not decode as the RECORDED kind. Checked
//!    structurally and independently: a `roots/list` answer must carry a `roots`
//!    array, an `elicitation/create` answer must carry `action`, a
//!    `sampling/createMessage` answer must carry `content` and `model`.
//!
//! There is deliberately no fifth invariant for `MAX_REQUEST_STATE_LEN`: it
//! bounds the MRTR continuation token and `tasks/update` carries none.
//!
//! # Corpus cases worth seeding
//!
//!   - `{"taskId":"t","inputResponses":{}}` — the empty delivery
//!   - one entry past the count bound (65 entries)
//!   - one entry one byte past the per-entry size bound
//!   - many medium entries totalling past the total-size bound
//!   - a 33-deep nested value (one past the depth bound)
//!   - a valid `ElicitResult` under the elicitation key (the accept path)
//!   - a `CreateMessageResult` under the elicitation key (the D-113-O shape)
//!   - a duplicate key in the raw JSON text (last-wins in serde_json)
//!   - non-UTF8 bytes, and a truncated JSON object
//!   - a huge integer, and `null` for a whole entry
//!   - `inputResponses` absent, a string, an array, and `null`

#![no_main]

use libfuzzer_sys::fuzz_target;
use pmcp::server::task_dispatch::fuzz_support::{
    judge_update_params, MAX_DEPTH, MAX_ENTRIES, MAX_ENTRY_BYTES, MAX_TOTAL_BYTES,
    RECORDED_ELICITATION_KEY, RECORDED_ROOTS_KEY, RECORDED_SAMPLING_KEY, VERDICT_ACCEPTED,
};
use serde_json::{Map, Value};

/// This target's OWN nesting-depth walk, deliberately not the crate's.
///
/// Invariant 2 is only evidence if the quantity it compares against the bound is
/// derived independently. Calling the production depth function would make the
/// check "production agrees with production".
fn depth_of(value: &Value) -> usize {
    let mut deepest = 0usize;
    let mut stack = vec![(value, 1usize)];
    while let Some((current, depth)) = stack.pop() {
        deepest = deepest.max(depth);
        // Bail out well past the bound so a pathological artifact cannot make
        // THIS function the slow part of the campaign.
        if depth > MAX_DEPTH + 1 {
            return depth;
        }
        match current {
            Value::Array(items) => stack.extend(items.iter().map(|item| (item, depth + 1))),
            Value::Object(entries) => {
                stack.extend(entries.iter().map(|(_, item)| (item, depth + 1)));
            },
            _ => {},
        }
    }
    deepest
}

/// Does `value` structurally satisfy the result shape `key`'s recorded kind
/// demands?
///
/// The three required-field sets, restated here from the wire types rather than
/// read from them, so invariant 4 is an independent statement about the shape.
fn matches_recorded_kind(key: &str, value: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    match key {
        // `ListRootsResult { roots: Vec<Root> }`
        RECORDED_ROOTS_KEY => object.get("roots").is_some_and(Value::is_array),
        // `ElicitResult { action: ElicitAction, content: Option<..> }`
        RECORDED_ELICITATION_KEY => object.get("action").is_some_and(Value::is_string),
        // `CreateMessageResult { content: Content, model: String, .. }`
        RECORDED_SAMPLING_KEY => {
            object.contains_key("content") && object.get("model").is_some_and(Value::is_string)
        },
        _ => false,
    }
}

/// The `inputResponses` map this target parsed for itself, if the bytes carried
/// one.
fn parsed_responses(data: &[u8]) -> Option<Map<String, Value>> {
    let value: Value = serde_json::from_slice(data).ok()?;
    value.get("inputResponses")?.as_object().cloned()
}

fuzz_target!(|data: &[u8]| {
    // Invariant 1: total over arbitrary bytes — a panic here fails the target.
    let outcome = judge_update_params(data);

    if outcome.verdict != VERDICT_ACCEPTED {
        assert!(
            outcome.accepted.is_empty(),
            "only an ACCEPTED verdict may name accepted keys"
        );
        return;
    }

    // An ACCEPTED verdict means the bytes parsed, so this re-parse cannot fail.
    let responses = parsed_responses(data)
        .expect("an ACCEPTED verdict implies a params object carrying an inputResponses map");

    // Invariant 2: the four bounds, re-derived from the payload independently.
    assert!(
        responses.len() <= MAX_ENTRIES,
        "accepted {} entries, over the {MAX_ENTRIES} bound — the bounds pre-check did not run",
        responses.len()
    );
    let mut total = 0usize;
    for (key, value) in &responses {
        let bytes = serde_json::to_string(value).map_or(usize::MAX, |s| s.len());
        assert!(
            bytes <= MAX_ENTRY_BYTES,
            "accepted a {bytes}-byte entry under `{key}`, over the {MAX_ENTRY_BYTES} bound"
        );
        assert!(
            depth_of(value) <= MAX_DEPTH,
            "accepted an over-deep entry under `{key}`, over the {MAX_DEPTH} bound"
        );
        total = total.saturating_add(bytes);
    }
    assert!(
        total <= MAX_TOTAL_BYTES,
        "accepted {total} total bytes, over the {MAX_TOTAL_BYTES} bound"
    );

    // Invariants 3 and 4: only recorded keys, and only kind-correct values.
    for key in &outcome.accepted {
        let value = responses
            .get(key)
            .expect("an accepted key was present in the delivered map");
        assert!(
            matches_recorded_kind(key, value),
            "accepted `{key}`: either the record never held it (the client chose its own \
             kind), or its value does not satisfy the recorded kind"
        );
    }
});
