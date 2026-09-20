# Phase 62: MCP Pen Test - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-03-28
**Phase:** 62-mcp-pen-test
**Areas discussed:** Attack categories, CLI integration, Reporting & output, Scope & safety

---

## Attack Categories

### Primary Focus
| Option | Description | Selected |
|--------|-------------|----------|
| MCP-specific attacks first | Prioritize MCP protocol attacks: prompt injection, tool poisoning, session hijacking, capability escalation, resource URI traversal | ✓ |
| Full-spectrum from day one | Ship MCP-specific, transport, auth, AND general web attacks all in v1 | |
| Transport & auth only | Focus on HTTP-layer security only | |

**User's choice:** MCP-specific attacks first
**Notes:** General web and transport attacks deferred to future phases.

### Prompt Injection Testing
| Option | Description | Selected |
|--------|-------------|----------|
| Payload library + fuzzing | Curated injection payloads plus fuzzing tool arguments. Report if server echoes injected content or changes behavior. | ✓ |
| Behavioral analysis | Paired requests (benign vs injected) with response diffing | |
| Payload library only | Static payloads, simple pass/fail | |

**User's choice:** Payload library + fuzzing

### Tool Poisoning Detection
| Option | Description | Selected |
|--------|-------------|----------|
| Validate tool response _meta | Inspect for unexpected _meta keys, script injection in resourceUri, oversized payloads, schema mismatches | ✓ |
| Plus active poisoning simulation | Beyond validation, attempt to register tools with malicious _meta | |
| Not in v1 | Skip tool poisoning | |

**User's choice:** Validate tool response _meta

### Session Security
| Option | Description | Selected |
|--------|-------------|----------|
| Full session tests | Session ID entropy, replay, fixation, concurrent sessions, timeout enforcement | ✓ |
| Basic session checks only | Verify session IDs present and change between sessions | |
| Skip sessions | Focus on MCP protocol attacks only | |

**User's choice:** Full session tests

---

## CLI Integration

### Subcommand Design
| Option | Description | Selected |
|--------|-------------|----------|
| cargo pmcp pentest | New top-level subcommand alongside test/loadtest | ✓ |
| cargo pmcp test --security | Extend existing test command with flag | |
| cargo pmcp test security | New subcommand under test | |

**User's choice:** cargo pmcp pentest

### Target Specification
| Option | Description | Selected |
|--------|-------------|----------|
| URL arg (same as test/loadtest) | cargo pmcp pentest http://localhost:3000 | ✓ |
| Config file with multiple targets | pentest.toml listing targets | |
| Both URL arg and config | URL for one-off, config for CI | |

**User's choice:** URL arg

### Auth Integration
| Option | Description | Selected |
|--------|-------------|----------|
| Reuse AuthFlags | Same --api-key, --oauth-client-id, --bearer flags from Phase 29 | ✓ |
| Dual mode: auth + no-auth | Run with and without auth side-by-side | |
| Pentest-specific auth flags | --stolen-token, --expired-token flags | |

**User's choice:** Reuse AuthFlags

---

## Reporting & Output

### Severity Classification
| Option | Description | Selected |
|--------|-------------|----------|
| Critical/High/Medium/Low/Info | Standard 5-level severity (OWASP-style) | ✓ |
| CVSS scoring | Full CVSS v3.1 with attack vector/complexity/impact | |
| Pass/Fail only | Binary vulnerable/not-vulnerable | |

**User's choice:** Critical/High/Medium/Low/Info

### Output Formats
| Option | Description | Selected |
|--------|-------------|----------|
| Terminal + JSON | Rich terminal + JSON for CI | |
| Terminal + JSON + SARIF | Add SARIF for GitHub Security tab integration | ✓ |
| Terminal only | Developer experience focus | |

**User's choice:** Terminal + JSON + SARIF

### CI Gate
| Option | Description | Selected |
|--------|-------------|----------|
| Fail on Critical or High | --fail-on flag, default high | ✓ |
| Fail on any finding | Strictest mode | |
| Advisory only | Always exit 0 | |

**User's choice:** Fail on Critical or High

---

## Scope & Safety

### Guardrails
| Option | Description | Selected |
|--------|-------------|----------|
| Explicit target + rate limiting | Only test provided URL, 10 req/s default, no redirect following, clear banner | ✓ |
| Strict mode with confirmation | Require --yes-i-want-to-pentest flag | |
| Minimal guardrails | Trust the developer | |

**User's choice:** Explicit target + rate limiting

### Destructive Tests
| Option | Description | Selected |
|--------|-------------|----------|
| Non-destructive by default, opt-in | Default read-only, --destructive flag for mutations | ✓ |
| All non-destructive | Never modify server state | |
| Destructive by default | Full attack suite including mutations | |

**User's choice:** Non-destructive by default, opt-in destructive

### Discovery
| Option | Description | Selected |
|--------|-------------|----------|
| Auto-discover from MCP | Call tools/list and resources/list to map attack surface | ✓ |
| Explicit tool/resource list | User specifies scope via flags | |
| Both modes | Auto-discover default, --scope to restrict | |

**User's choice:** Auto-discover from MCP

---

## Claude's Discretion

- Internal pentest engine architecture
- Payload library format and content
- SARIF schema mapping
- Rate limiter implementation

## Deferred Ideas

- Transport-level attacks (CORS bypass, TLS, header injection) — future phase
- General web attacks (SSRF, path traversal) — future phase
- Config file for multi-target CI scans — enhancement
- Pentest-specific auth flags (--stolen-token, --expired-token) — advanced
- Dual auth/no-auth testing mode — advanced
