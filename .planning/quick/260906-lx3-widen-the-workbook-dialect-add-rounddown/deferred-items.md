# Deferred items — quick task 260906-lx3

Out-of-scope discoveries made while executing the dialect-widening plan. None are
caused by this task's changes; all are logged rather than fixed, per the
executor scope boundary (only fix what the current task's changes caused).

## D1 — `cargo clippy -p pmcp-workbook-runtime --all-targets -- -D warnings` is not clean at base

Three findings, all in files this task never touched (`git status --short`
reports both files unmodified against the base commit
`6bd7e0e492b084075b9b872ae8d26badfaffe1f6`, and each lint is local to the item it
flags, so it fires identically at the base commit):

| Site | Lint | Note |
|------|------|------|
| `crates/pmcp-workbook-runtime/src/manifest_model.rs:941` | `clippy::map_or` ("this `map_or` can be simplified") | inside a `#[cfg(test)]` proptest; `map_or(false, ..)` -> `is_some_and(..)` |
| `crates/pmcp-workbook-runtime/src/render/mod.rs:420` | `clippy::too_many_arguments` (8/7) | private `write_computed_value` |
| `crates/pmcp-workbook-runtime/src/render/mod.rs:511` | `clippy::too_many_arguments` (8/7) | private `write_formula_or_value` |

The plan's Task-1 and Task-3 `<done>` blocks assert this command "is clean". That
premise was false against the working tree BEFORE this task began. The plan
itself records why it is not a CI blocker: `make lint` (`Makefile:216-221`)
resolves to the root `pmcp` package only, so no CI job runs clippy over
`pmcp-workbook-runtime`.

Both `too_many_arguments` sites are private functions, so a fix is not an API
break — but refactoring the xlsx render writer inside a dialect-widening commit
is scope creep. Deferred.

**Scoped assertion actually run in its place:** clippy over the three workbook
crates reports ZERO findings in any file this task modified.
