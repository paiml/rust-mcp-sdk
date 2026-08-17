---
schema_version: 1
open_count: 1
waived_count: 0
fixed_count: 0
total_count: 1
last_updated: 2026-08-17T08:07:00.553Z
---

# Broken Windows Ledger

> Cross-phase defect register. With `workflow.windows_enforce` enabled, `/gsd-ship` blocks while `open_count > 0`.
> Waive with `gsd-tools windows waive <id> "<reason>"` (reason required).
> Mark fixed with `gsd-tools windows fixed <id>`.

| id | phase | kind | file | line | description | status | reason | recorded_at | resolved_at |
|----|-------|------|------|------|-------------|--------|--------|-------------|-------------|
| 1 | 118.2 | deviation | src/shared/streamable_http.rs |  | 118.2-03 rewrote two collected_body_cap unit tests that measured a whole-body cap the POST SSE path no longer has (parser bound + receive() refusal is the new contract) | open |  | 2026-08-17T08:07:00.553Z |  |

````json
[
  {
    "id": 1,
    "kind": "deviation",
    "phase": "118.2",
    "file": "src/shared/streamable_http.rs",
    "line": null,
    "description": "118.2-03 rewrote two collected_body_cap unit tests that measured a whole-body cap the POST SSE path no longer has (parser bound + receive() refusal is the new contract)",
    "status": "open",
    "reason": "",
    "recorded_at": "2026-08-17T08:07:00.553Z",
    "resolved_at": null
  }
]
````
