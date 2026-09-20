#![no_main]

use libfuzzer_sys::fuzz_target;
use pmcp::{
    CallToolRequest, CallToolResult, ClientCapabilities, CompleteRequest, CompleteResult, Content,
    GetPromptResult, ListResourcesResult, PromptMessage, ReadResourceResult, ResourceInfo, Role,
    ServerCapabilities,
};
use serde_json::{from_slice, from_value, Value};

fuzz_target!(|data: &[u8]| {
    // Try to parse as various protocol messages

    // 1. Try parsing as generic JSON
    if let Ok(json) = from_slice::<Value>(data) {
        // Try parsing as specific request/result types (using only exported types)
        let _ = from_value::<CallToolRequest>(json.clone());
        let _ = from_value::<CallToolResult>(json.clone());
        let _ = from_value::<ListResourcesResult>(json.clone());
        let _ = from_value::<ReadResourceResult>(json.clone());
        let _ = from_value::<GetPromptResult>(json.clone());

        // `completion/complete` (Phase 118.1-04, CONF-05 / G-4). Added because
        // the claim "the existing serde surface already covers the request
        // parse" was MEASURED FALSE: no fuzz target deserialized either of
        // these before this line. `CompleteRequest` carries a peer-supplied
        // internally-tagged `ref` (`ref/prompt` / `ref/resource`) whose params
        // reach a registered completion provider, so it is exactly the shape a
        // parse fence belongs on.
        let _ = from_value::<CompleteRequest>(json.clone());
        let _ = from_value::<CompleteResult>(json.clone());

        // Try parsing as capability types
        let _ = from_value::<ClientCapabilities>(json.clone());
        let _ = from_value::<ServerCapabilities>(json.clone());

        // Try parsing as content types
        let _ = from_value::<ResourceInfo>(json.clone());
        let _ = from_value::<PromptMessage>(json.clone());
        let _ = from_value::<Role>(json.clone());
        let _ = from_value::<Content>(json.clone());
    }

    // 2. Try parsing as raw protocol message bytes
    if data.len() >= 4 {
        // Simulate message framing
        let len = u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as usize;
        if len < 1_000_000 && data.len() >= 4 + len {
            let message_data = &data[4..4 + len];
            let _ = from_slice::<Value>(message_data);
        }
    }

    // 3. Try parsing as newline-delimited JSON
    for line in data.split(|&b| b == b'\n') {
        if !line.is_empty() {
            let _ = from_slice::<Value>(line);
        }
    }
});
