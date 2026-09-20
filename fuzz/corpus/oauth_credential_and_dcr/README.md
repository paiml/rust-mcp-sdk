# Seed corpus — `oauth_credential_and_dcr`

Hand-written seeds for the Phase 116-08 fuzz target over the AUTH-02 and AUTH-03
pure surfaces: `parse_credential_snapshot`, `CredentialSnapshot::to_bytes`,
`derive_application_type` and `DcrResponse::application_type`.

## Why these are committed when every other corpus is ignored

All four entry points take STRUCTURED input. A schema-1 credential document needs
a `schema_version`, an `entries` object and a per-entry `access_token` and
`client_id` before it parses at all; a `DcrResponse` needs a `client_id`. Random
bytes degenerate into a JSON-tokenizer fuzz that never reaches the migration, the
round trip or the `application_type` accessor — the surfaces this target exists
to cover.

Measured, not argued. Five deliberate breaks were applied **at once** across
three files:

| # | Break | File |
| --- | --- | --- |
| A | an empty recorded issuer is no longer filtered, so an unkeyable entry migrates under an EMPTY issuer | `credential_store.rs` |
| B | an entry with no issuer is skipped WITHOUT being reported as dropped | `credential_store.rs` |
| C | the writer emits the LEGACY schema version | `credential_store.rs` |
| D | a mixed `redirect_uris` vector silently picks `Native` instead of refusing | `oauth_validation.rs` |
| E | `application_type()` stringifies a non-string JSON value instead of reporting absence | `provider.rs` |

| Corpus | Runs | Defects found |
| --- | --- | --- |
| empty | 200 000 | **0** |
| these seeds | 20 (one per seed) | **5 of 5** |

## The seeds and what each detects

| Seed | Role |
| --- | --- |
| `seed_schema2_two_servers` | current-schema document, two servers under one issuer — round trip (C) |
| `seed_schema2_empty` | current-schema document with no credentials — a SURVIVOR under every break, because an empty key set round-trips trivially |
| `seed_schema2_empty_issuer_key` | a hostile document whose issuer, server and account keys are all empty strings |
| `seed_schema1_migrates` | the migration happy path — round trip (C) |
| `seed_schema1_dropped_no_issuer` | an entry with no `issuer` at all — the drop-and-REPORT accounting (B) |
| `seed_schema1_dropped_empty_issuer` | an entry whose `issuer` is `""` — SEP-2352's non-empty issuer rule (A) |
| `seed_schema1_two_servers_one_issuer` | D-116-R1's collision class, through the migration path |
| `seed_schema1_mixed_migrate_and_drop` | one migrating and one dropping entry in one document (B) |
| `seed_schema1_no_entries_key` | a schema-1 document with no `entries` key — a SURVIVOR, since zero entries account for themselves |
| `seed_unsupported_version` | a future schema version — refused, a SURVIVOR |
| `seed_dcr_native` / `seed_dcr_web` | the two wire literals echoed by an authorization server — SURVIVORS under the stringify break |
| `seed_dcr_nonstring_app_type` | `"application_type": 42` — must be reported as ABSENT, not stringified (E) |
| `seed_dcr_escaped_app_type` | `"native"` — a JSON escape whose decoded value never appears literally in the input; a SURVIVOR that proves the backslash guard is a soundness condition and not a way of switching the check off |
| `seed_dcr_and_schema1` | one document that is simultaneously a valid `DcrResponse` and a valid schema-1 credential file |
| `seed_uris_native_only` | loopback IPv4, bracketed IPv6 and a private-use scheme — all `native` |
| `seed_uris_web_only` | two remote https redirects — `web` |
| `seed_uris_mixed` | one loopback and one remote — must be REFUSED, never picked (D) |
| `seed_uris_cleartext_remote` | `http` to a non-loopback host — a hard error |
| `seed_uris_localhost_https` | `https` on loopback, which is still a NATIVE application |

The `redirect_uris` seeds are newline-separated: the target splits the input on
`\n` to build the vector.

## Running

```bash
cargo fuzz run oauth_credential_and_dcr
```
