#![no_main]

use libfuzzer_sys::fuzz_target;
use pmcp::server::roots::ListRootsResult;
use pmcp::types::elicitation::{ElicitRequestParams, ElicitResult};
use pmcp::types::sampling::{CreateMessageParams, CreateMessageResult};
use serde_json::{from_slice, from_value, Value};

// Fuzz the serde boundary that `DispatchPeerHandle::sample`,
// `DispatchPeerHandle::list_roots` and `DispatchPeerHandle::elicit` rely on when
// deserializing client responses via `serde_json::from_value`. The dispatcher
// returns an arbitrary `Value`; the peer impl must never panic on adversarial
// JSON — valid inputs round-trip, invalid inputs produce `Err`.
//
// Target surfaces:
// - `CreateMessageParams`  — outbound request (client may echo shape back)
// - `CreateMessageResult`  — sampling/createMessage response
// - `ListRootsResult`      — roots/list response
// - `ElicitRequestParams`  — elicitation/create request; its `Deserialize` is
//                            HAND-WRITTEN (optional `mode`, defaulting to
//                            "form"), so it is the one shape here whose decode
//                            is not purely serde-derived
// - `ElicitResult`         — elicitation/create response (Phase 118.1 plan 09).
//                            A user's answer is peer-supplied JSON that decides
//                            whether the server believes a human approved
//                            something, so it belongs on this boundary.
fuzz_target!(|data: &[u8]| {
    let Ok(json) = from_slice::<Value>(data) else {
        return;
    };
    let _ = from_value::<CreateMessageParams>(json.clone());
    let _ = from_value::<CreateMessageResult>(json.clone());
    let _ = from_value::<ElicitRequestParams>(json.clone());
    let _ = from_value::<ElicitResult>(json.clone());
    let _ = from_value::<ListRootsResult>(json);
});
