# Phase 74: Add cargo pmcp auth subcommand with multi-server OAuth token management - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-04-21
**Phase:** 74-add-cargo-pmcp-auth-subcommand-with-multi-server-oauth-token
**Areas discussed:** Cache schema & migration, Command surface, Precedence, Token output + DX, Scope/release/refresh, --client DCR

---

## Cache schema & migration

### Q1: What should identify a server in the per-server token cache?
| Option | Description | Selected |
|--------|-------------|----------|
| Normalized mcp_server_url (Recommended) | Key by scheme+host+port | ✓ |
| Composite (mcp_server_url, client_id) | Handle multiple OAuth apps per server | |
| Composite (issuer, client_id) | Durable across URL changes | |

**User's choice:** Normalized mcp_server_url — matches user mental model, handles common case.

### Q2: What to do with existing single-blob cache on disk?
| Option | Description | Selected |
|--------|-------------|----------|
| Versioned file, leave old one (Recommended) | New file `~/.pmcp/oauth-cache.json` with schema_version | ✓ |
| Auto-migrate in place | Detect legacy, migrate | |
| Discard silently + banner | Overwrite with new format | |

**User's choice:** Versioned file. No migration risk; legacy file untouched; users re-login once.

---

## Command surface

### Q3: Which subcommands should ship?
| Option | Description | Selected |
|--------|-------------|----------|
| Full sketch: login, logout, status, token, refresh (Recommended) | 5 subcommands | ✓ |
| Minimum viable: login, logout, status | Drop token + refresh | |
| Full sketch + servers alias | 5 + `servers` alias for status | |

**User's choice:** All 5 subcommands.

### Q4: What should `auth logout` do with no URL?
| Option | Description | Selected |
|--------|-------------|----------|
| Error: require explicit --all or <url> (Recommended) | Safest, no accidental wipes | ✓ |
| Interactive confirm before wiping all | Friendlier, doesn't work in CI | |
| Always log out of all (git-style) | Matches `git clean` | |

**User's choice:** Error out. No silent mass-wipe.

---

## Precedence: flag vs env vs cache

### Q5: When a server-connecting command runs, what precedence?
| Option | Description | Selected |
|--------|-------------|----------|
| Explicit flag > env > cache (Recommended) | Flags/envs win; cache is fallback | ✓ |
| Cache > flag > env | Once logged in, cache authoritative | |
| Flag > cache > env | Flag wins, then cache, then env | |

**User's choice:** Flag > env > cache. Additive, no CI breakage.

### Q6: Warn user when both cached token + explicit flag exist?
| Option | Description | Selected |
|--------|-------------|----------|
| No warning, silent fallback (Recommended) | UNIX composition | ✓ |
| Warn once per run | Helpful debug, noisy in CI | |
| Warn only if --verbose | Silent by default | |

**User's choice:** Silent fallback.

---

## Token command output + DX

### Q7: What should `auth token <url>` write to stdout?
| Option | Description | Selected |
|--------|-------------|----------|
| Raw access token, nothing else (Recommended) | Matches `gh auth token` | ✓ |
| Authorization header line | `Authorization: Bearer <token>` | |
| Raw token by default, --header flag | Both via opt-in flag | |

**User's choice:** Raw access token. Enables clean `$(…)` substitution.

### Q8: What should `auth login` print on success?
| Option | Description | Selected |
|--------|-------------|----------|
| Success message with expiry + issuer (Recommended) | No token in output | ✓ |
| Success + full token | Convenient but leaks to shell history | |
| Success + token only with --print-token | Opt-in | |

**User's choice:** Message only, no token. Safer for shared terminals.

---

## Scope / Release / Refresh

### Q9: Semver bump for cargo-pmcp?
| Option | Description | Selected |
|--------|-------------|----------|
| Minor: 0.8.1 → 0.9.0 (Recommended) | New top-level group + cache format | ✓ |
| Patch: 0.8.1 → 0.8.2 | Additive only | |

**User's choice:** Minor bump.

### Q10: Include pentest.rs migration in this phase?
| Option | Description | Selected |
|--------|-------------|----------|
| Defer — keep pentest.rs as-is (Recommended) | Avoid scope creep | |
| Include — migrate pentest.rs in this phase | Thoroughness win | ✓ |

**User's choice:** Include in Phase 74 scope.

### Q11: When should cached tokens be refreshed?
| Option | Description | Selected |
|--------|-------------|----------|
| On-demand only (Recommended) | Refresh at use if expired/near-expiry | ✓ |
| Proactive background refresh | Not applicable to CLI | |

**User's choice:** On-demand only; `auth refresh` is the explicit escape hatch.

---

## --client flag & DCR

### Q12: How should --client be passed through?
User initially asked to check pmcp.run's actual wire behavior. Investigation revealed:
- pmcp.run's `oauth-proxy` parses `client_name` from **Dynamic Client Registration (DCR, RFC 7591) POST /register body**
- `classify_client_type` (main.rs:2853) lowercases and substring-matches against `ClientTypeMatcher::name_patterns`
- Matches route to shared Cognito client_id + branded Managed Login UI

**Resolution:** `--client` must trigger DCR, not a URL query param.

### Q13: Which subcommands accept --client?
| Option | Description | Selected |
|--------|-------------|----------|
| login only (Recommended) | Only login hits branded UI | ✓ |
| login + refresh | IdP might re-render on refresh | |
| All auth subcommands | Uniform surface | |

**User's choice:** login only.

### Q14: Persist --client in cache entry?
| Option | Description | Selected |
|--------|-------------|----------|
| Yes, persist it (Recommended) | Show in status, reuse in refresh | |
| No, transient flag only | Simpler cache schema | ✓ |

**User's choice:** Transient only.

### Q15: DCR scope for Phase 74?
| Option | Description | Selected |
|--------|-------------|----------|
| Full DCR + --client in Phase 74 (Recommended) | End-to-end branded login test | ✓ |
| Scaffold only, DCR in follow-up | Half a feature | |
| Defer --client entirely | Phase 74 stays tight | |

**User's choice:** Full DCR + --client in Phase 74.

---

## DCR architecture (revision after initial CONTEXT.md written)

User feedback: "I'm not sure why the --client is the reason that we need to do DCR, as I thought that DCR is needed anyway. However, it seems that we will want to support DCR logic as part of all clients (at least as an option) and it should be implemented in the SDK and not only in the cargo pmcp CLI."

**Correct framing:** DCR is a general-purpose SDK feature (any MCP client built on pmcp should be able to auto-register), not a CLI-specific feature tied to `--client`. `--client` is just a UX hook that sets `client_name` for the DCR request.

### Q16: When should DCR fire automatically?
| Option | Description | Selected |
|--------|-------------|----------|
| When server advertises registration_endpoint AND no client_id provided (Recommended) | Spec-compliant zero-config | ✓ |
| Opt-in via explicit --dcr flag | Safer, less ergonomic | |
| Always, with fallback to --oauth-client-id | Wasteful discovery calls | |

**User's choice:** Auto-fire when server supports DCR and caller didn't provide client_id.

### Q17: Default client_name when --client is absent?
| Option | Description | Selected |
|--------|-------------|----------|
| Library-caller default, fall back to "pmcp-sdk" (Recommended) | OAuthConfig::client_name is Option<String>; cargo-pmcp sets "cargo-pmcp" when --client absent | ✓ |
| Hard-coded "pmcp-cli" in SDK | Library users get confusing server logs | |

**User's choice:** Caller-configurable default.

### Q18: Keep --oauth-client-id as escape hatch?
| Option | Description | Selected |
|--------|-------------|----------|
| Yes — --oauth-client-id wins, skips DCR (Recommended) | Additive, no breakage for enterprise IdPs with DCR disabled | ✓ |
| No — remove --oauth-client-id, DCR only | Cleaner surface but breaking | |

**User's choice:** Keep escape hatch.

**Scope bump:** Phase 74 now spans **both `pmcp` (SDK) and `cargo-pmcp` (CLI)**. Semver: `pmcp` minor bump (additive DCR public API) + `cargo-pmcp 0.8.1 → 0.9.0`. CONTEXT.md renumbered to D-01..D-23 to reflect the SDK-first decision order.

---

## Claude's Discretion

- Concrete struct/enum shapes for `TokenCacheV1`
- File-locking strategy for concurrent logins
- `status` tabular output library (use existing `colored`, avoid new dep)
- DCR integration test harness (mock HTTP)
- Error message copy for failure modes

## Deferred Ideas

- Multiple OAuth apps per server (composite key)
- `auth servers` alias for `auth status` no-args
- `--verbose` mode on server-connecting commands
- `--client` on `auth refresh`
- Clipboard copy for `auth token`
- Interactive TUI for `auth status`
- Encrypted cache at rest (keyring integration)
