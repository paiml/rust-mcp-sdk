# Count the entries in a cargo-deny config's `[bans].allow` array.
#
# Usage:
#   awk -f scripts/deny-allow-entry-count.awk crates/pmcp-package/deny.toml
#
# Prints exactly one non-negative integer on stdout:
#
#   N >= 1  the `[bans]` section has an `allow` array holding N entries
#   0       the file has no `[bans]` section, OR `[bans]` has no `allow` array,
#           OR that array is empty
#
# All three zero cases are the SAME verdict for the caller — the allowlist is
# not providing deny-by-default protection — so the caller's error message names
# all three possibilities rather than guessing which one occurred.
#
# ---------------------------------------------------------------------------
# Why this is a checked-in awk program and not a grep in the Makefile recipe
# ---------------------------------------------------------------------------
# A non-empty `[bans].allow` list is what makes cargo-deny deny-by-default. An
# EMPTIED list is therefore a complete, silent bypass: cargo-deny reports
# "bans ok" for `allow = []` exactly as vacuously as it does for a missing
# config file. The gate must be able to tell those apart from a healthy config.
#
# Two line-oriented spellings were proposed and BOTH are broken, which is why
# this is a parser (cross-AI review, Phase 122, MEDIUM):
#
#   grep 'allow = \['     ALSO MATCHES `allow = []`. This is the precise bypass
#                         the guard exists to prevent, so the naive guard passes
#                         exactly when it must fail.
#   grep '{ name ='       ALSO MATCHES entries in other sections. It is not a
#                         hypothetical: cargo-deny configs in this repo carry a
#                         `[licenses]` stanza with its OWN `allow` key, so a
#                         file-wide count reads the wrong section.
#
# Being a separate file makes it separately testable, exactly as
# `scripts/named-test-binary-count.awk` is. `make no-crypto-allowlist-guard-selftest`
# feeds it six fixtures and is a declared PREREQUISITE of the gate that reads
# it, so the gate and the proof of the gate cannot drift.
#
# ---------------------------------------------------------------------------
# The three rules that make the reading trustworthy
# ---------------------------------------------------------------------------
# 1. COMMENTS ARE STRIPPED BEFORE MATCHING. `crates/pmcp-package/deny.toml`'s
#    header prose explains this very guard, and in doing so mentions `allow` and
#    writes `{ name = ... }` inline. A comment-blind counter would count the
#    DOCUMENTATION that describes the check — a self-invalidating reading that
#    would report a healthy allowlist for a file whose array was empty. Stripping
#    is quote-aware so a `#` inside a crate-name string is not treated as a
#    comment start.
#
# 2. COUNTING IS SECTION-SCOPED TO `[bans]`. Only `[bans]` is in scope;
#    `[licenses]`, `[advisories]`, `[graph]` and `[sources]` are explicitly out.
#    The scoping works in BOTH directions — a `[licenses] allow = []` appearing
#    before or after the `[bans]` stanza changes nothing, because the count is
#    keyed on the section active at the time and is never reset by a later
#    section.
#
# 3. ARRAY STATE IS TRACKED BY BRACKET DEPTH, so a single-line
#    `allow = [ { name = "x" } ]` and a multi-line array are both handled. The
#    opening line is itself counted before the depth check, which is what makes
#    the single-line form work.
#
# ---------------------------------------------------------------------------
# Blind spot, stated (repo convention: a mechanism names what it cannot see)
# ---------------------------------------------------------------------------
# This counts SYNTACTIC entries. It does NOT validate that each named crate
# exists, that the names are unique, that they are spelled correctly, or that
# they correspond to anything in the resolved dependency graph. A list of five
# misspelled names reads as 5 here — and would then fail the actual cargo-deny
# run, which is the check that has the real answer. This program's only job is
# to prove the allowlist is non-empty so that cargo-deny's "ok" is meaningful.

# Remove a TOML comment from a line, respecting double-quoted strings.
# A line whose first non-whitespace character is `#` reduces to the empty string.
function strip_comment(s,   out, i, c, inq) {
    out = ""
    inq = 0
    for (i = 1; i <= length(s); i++) {
        c = substr(s, i, 1)
        if (c == "\"") {
            inq = !inq
            out = out c
            continue
        }
        if (c == "#" && !inq) {
            break
        }
        out = out c
    }
    return out
}

# Count `{ name = ` / `{ crate = ` entry openers on a line. cargo-deny 0.18.3
# accepts both key spellings, so both are counted.
function count_entries(s,   tmp) {
    tmp = s
    return gsub(/\{[[:space:]]*(name|crate)[[:space:]]*=/, "", tmp)
}

# Net bracket depth change contributed by a line.
function net_brackets(s,   tmp, opens, closes) {
    tmp = s
    opens = gsub(/\[/, "", tmp)
    tmp = s
    closes = gsub(/\]/, "", tmp)
    return opens - closes
}

{
    line = strip_comment($0)

    if (in_array) {
        count += count_entries(line)
        depth += net_brackets(line)
        if (depth <= 0) {
            in_array = 0
        }
        next
    }

    # Section header: `[bans]`, `[licenses]`, `[graph]`, ...
    if (line ~ /^[[:space:]]*\[[A-Za-z0-9_.-]+\]/) {
        section = line
        sub(/^[[:space:]]*\[/, "", section)
        sub(/\].*$/, "", section)
        next
    }

    # Only `[bans].allow` opens a counted array.
    if (section == "bans" && line ~ /^[[:space:]]*allow[[:space:]]*=[[:space:]]*\[/) {
        in_array = 1
        depth = 0
        count += count_entries(line)
        depth += net_brackets(line)
        if (depth <= 0) {
            in_array = 0
        }
    }
}

END { print count + 0 }
