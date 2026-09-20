# Seed corpus — `oauth_authorization_response`

Hand-written seeds for the Phase 116-08 fuzz target over
`pmcp::shared::oauth_validation` (AUTH-01 / AUTH-03).

## Why these are committed when every other corpus is ignored

The target's `Ok`-side invariants — the ones that discharge `T-116-29`, "a rule
defect the crate and its test mirrors SHARE" — only run when
`validate_authorization_response` **accepts**. Acceptance requires the callback
query's `state` to reproduce a fixed 15-byte token byte for byte, and RFC 9207's
`iss` comparison permits no normalization at all. Random bytes essentially never
build that, so without seeds the target degenerates into a bare no-panic check.

This is not an argument, it is a measurement. With four deliberate breaks applied
to `src/shared/oauth_validation.rs` **at once** (an `iss` comparison that
case-folds, an absent `state` treated as a match, a duplicated security parameter
resolved first-wins, and the `OpenID` Connect appended discovery form dropped):

| Corpus | Runs | Defects found |
| --- | --- | --- |
| empty | 200 000 | **0** |
| these seeds | 14 (one per seed) | **4 of 4**, via 6 crashing seeds |

Same binary, same breaks. The seeds are the fence.

## The seeds

The `seed_accept_*` files are the surviving siblings: every one of them must keep
passing under every break, which is what makes the crashing seeds attributable to
a specific defect rather than to a target that fails wholesale.

| Seed | Role |
| --- | --- |
| `seed_accept_row4` | table row 4 — nothing advertised, no `iss` sent, accept |
| `seed_accept_iss_plain` | table row 3 — a matching `iss`, unencoded |
| `seed_accept_iss_encoded` | the same `iss` percent-encoded with uppercase hex |
| `seed_accept_iss_lowerhex` | the same `iss` percent-encoded with lowercase hex |
| `seed_accept_plus_and_bad_pct` | `+`-as-space, an invalid `%zz` escape, an empty pair and empty sequences |
| `seed_reject_iss_case` | an `iss` differing ONLY in host case — accepted only if the comparison folds case |
| `seed_reject_no_state` | a `code` with no `state` at all — the CSRF skip |
| `seed_reject_no_state_with_iss` | the same, with a valid `iss`, so `iss` cannot be what refuses it |
| `seed_reject_dup_state` | `state` twice — accepted only under a first-wins rule |
| `seed_reject_as_error` | an authorization-server `error` response |
| `seed_issuer_path` | a path-bearing issuer — three discovery candidates |
| `seed_issuer_pathless` | a path-less issuer — two candidates, and the appended form coincides with candidate 2 |
| `seed_issuer_loopback` | RFC 8252 §7.3's `http`-on-loopback issuer |
| `seed_issuer_wellknown_path` | an issuer whose own path is `/.well-known/openid-configuration`, where candidates 2 and 3 legitimately COINCIDE — the reason the target must not assert that candidates are distinct |

## Running

```bash
cargo fuzz run oauth_authorization_response
```

The seeds double as inputs the target can be replayed against one at a time:

```bash
cargo fuzz run oauth_authorization_response corpus/oauth_authorization_response/seed_reject_iss_case
```
