//! Pack-time validation of a config server's `[[config_slots]]` declaration
//! block against the package's own slot list, and of the shipped config
//! against BOTH (D-01, D-04, D-17).
//!
//! Three gates, in two directions. Two run SLOT -> CONFIG
//! ([`validate_config_slot_agreement`], [`validate_config_slot_placeholders`]):
//! is every declared slot well-formed, and does it point at a placeholder? One
//! runs CONFIG -> SLOT ([`validate_no_undeclared_env_refs`]): does every
//! slot-addressable key that defers a value have a slot? The summary line above
//! described only the first direction for three releases, which is the mental
//! model that let the second go ungated.
//!
//! # Why this module exists
//!
//! A Shape A config server's whole identity is its config file. That file
//! declares, in its own `[[config_slots]]` table, which values the TARGET
//! environment must supply. The `ServerPackage` being packed carries a
//! parallel `config_slots: Vec<ConfigSlot>` list. Before this module existed
//! the two were parallel representations that nothing compared: a caller could
//! hand [`pack_server`](crate::oci::pack_server) a slot list contradicting the
//! config it ships, or edit the config's declaration block while the package
//! slot list stayed put, and no code path would notice.
//!
//! [`parse_declared_config_slots`] reads the declaration table out of the SAME
//! bytes that become the config layer, and [`validate_config_slot_agreement`]
//! requires the two to agree exactly. The TOML block is the source of truth —
//! that is what D-01's "`pack` reads them" means, and it is now exercised by
//! the real API path rather than asserted in prose.
//!
//! # Untrusted input
//!
//! These config bytes are untrusted input to THIS crate. They need not have
//! come through `pmcp-server-toolkit`'s `ServerConfig`, so the `kind`
//! vocabulary is re-validated here rather than assumed. Every function in this
//! module returns `Result` on malformed input and never panics.
//!
//! # Error hygiene
//!
//! No error raised here ever echoes a config VALUE (T-120-21). A config slot
//! may name a credential, and the whole point of the placeholder rule is to
//! keep a resolved secret out of a packed layer — an error message is the
//! wrong place to put one. Errors name the config KEY and the FIELD or RULE
//! that was violated.

use crate::error::{PackageError, Result};
use crate::slot::{ConfigSlot, SlotType};
use std::collections::{BTreeMap, BTreeSet};

/// The closed `kind` vocabulary a `[[config_slots]]` entry may declare. These
/// are byte-identical to `pmcp-server-toolkit`'s `ConfigSlotKind` snake_case
/// discriminators AND to the corresponding [`SlotType::key`] kind strings —
/// that three-way string correspondence is what lets the two crates agree
/// without either depending on the other.
const ACCEPTED_KINDS: [&str; 3] = ["endpoint", "secret", "auth_mode"];

/// The complete field vocabulary of a `[[config_slots]]` entry, byte-identical
/// to `pmcp-server-toolkit`'s `ConfigSlotDecl` fields.
///
/// That struct carries `#[serde(deny_unknown_fields)]`, so the SERVER refuses
/// an entry with a stray field. A pack-side parser that merely `get()`s the
/// four known keys would be strictly more permissive than the runtime parser
/// reading the same bytes: a typo'd `tested_vaule`, or a `description` an
/// author added for documentation, would PACK cleanly and then fail at boot
/// with `unknown field`. That is the "packs cleanly and then fails to resolve
/// at boot" class this module exists to close, so the vocabulary is enforced
/// here too — see [`parse_declaration_entry`].
const ACCEPTED_FIELDS: [&str; 4] = ["key", "kind", "name", "tested_value"];

/// Error label used when a failure is a property of the DOCUMENT rather than
/// of any single declared key.
const DOCUMENT_LABEL: &str = "<config document>";

/// Error label for the declaration table itself.
const TABLE_LABEL: &str = "config_slots";

/// Build a [`PackageError::ConfigSlotViolation`]. Centralized so every message
/// in this module goes through one place that takes a key and a reason and
/// nothing else — there is no parameter here a config VALUE could ride in on.
fn violation(key: &str, reason: impl Into<String>) -> PackageError {
    PackageError::ConfigSlotViolation {
        key: key.to_string(),
        reason: reason.into(),
    }
}

/// Parse `config_bytes` as a TOML document.
///
/// The parser's own error message is deliberately NOT propagated: `toml`'s
/// `Display` renders a snippet of the offending source line, which for a
/// credential-bearing config is exactly the value this crate exists to keep out
/// of error text. The byte span is reported instead — enough to locate the
/// problem, incapable of quoting it.
pub(crate) fn parse_document(config_bytes: &[u8]) -> Result<toml::Value> {
    let text = std::str::from_utf8(config_bytes)
        .map_err(|_| violation(DOCUMENT_LABEL, "config bytes are not valid UTF-8"))?;
    toml::from_str::<toml::Value>(text).map_err(|e| {
        let where_ = e
            .span()
            .map_or_else(String::new, |s| format!(" at byte offset {}", s.start));
        violation(
            DOCUMENT_LABEL,
            format!(
                "config bytes are not valid TOML{where_} (the parser's message is withheld \
                 because it would quote config content)"
            ),
        )
    })
}

/// One `[[config_slots]]` entry as it appears in the config document.
///
/// A plain mirror of `pmcp-server-toolkit`'s `ConfigSlotDecl` wire shape,
/// deliberately RE-DECLARED here rather than imported.
///
/// # Why re-declared instead of shared
///
/// `pmcp-package` is the workspace-excluded leaf crate and must not depend on
/// `pmcp-server-toolkit`; the toolkit must not depend on `pmcp-package` either,
/// because that inverts the layering (plan 120-04 machine-checks it does not).
/// So there is no place a shared type could live. The two shapes are kept in
/// step by the TOML FIELD NAMES, which are the actual contract, and by a test
/// that parses the real `london-tube.toml` the reference server boots from — if
/// a field is renamed on either side, that test stops finding three slots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredConfigSlot {
    /// The dotted TOML path this slot fills, e.g. `backend.base_url`.
    pub key: String,
    /// The declared kind — one of `endpoint`, `secret`, `auth_mode`.
    pub kind: String,
    /// The slot's declared name (for a `secret`, the environment-variable name).
    pub name: String,
    /// The value exercised when the server was tested. `None` for an
    /// identity-bearing slot, which structurally carries no value.
    pub tested_value: Option<String>,
}

/// Read the `[[config_slots]]` declaration table out of a server config
/// document.
///
/// A document with no `config_slots` table returns an empty vec — declaring
/// nothing is legal; such a package simply cannot then claim any config slots
/// (see [`validate_config_slot_agreement`]).
///
/// # Errors
///
/// Returns [`PackageError::ConfigSlotViolation`] if the bytes are not valid
/// UTF-8/TOML, if `config_slots` is not an array of tables, or if any entry
/// is malformed — a missing or non-string `key`/`name`, a missing `kind`, or a
/// `kind` outside the closed vocabulary. The error names the entry's `key`
/// when one is readable and its index (`config_slots[N]`) otherwise.
///
/// # Examples
///
/// ```
/// use pmcp_package::parse_declared_config_slots;
///
/// let config = br#"
/// [[config_slots]]
/// key = "backend.base_url"
/// kind = "endpoint"
/// name = "TFL_BASE_URL"
/// tested_value = "https://api.tfl.gov.uk"
/// "#;
///
/// let declared = parse_declared_config_slots(config).unwrap();
/// assert_eq!(declared.len(), 1);
/// assert_eq!(declared[0].key, "backend.base_url");
/// assert_eq!(declared[0].kind, "endpoint");
/// assert_eq!(declared[0].tested_value.as_deref(), Some("https://api.tfl.gov.uk"));
///
/// // A config that declares nothing is legal, not an error.
/// assert!(parse_declared_config_slots(b"name = \"x\"\n").unwrap().is_empty());
/// ```
pub fn parse_declared_config_slots(config_bytes: &[u8]) -> Result<Vec<DeclaredConfigSlot>> {
    parse_declared_config_slots_in(&parse_document(config_bytes)?)
}

/// Document-taking half of [`parse_declared_config_slots`], so a caller that
/// runs several gates over the same config (`pack_server`) parses the TOML once.
pub(crate) fn parse_declared_config_slots_in(
    document: &toml::Value,
) -> Result<Vec<DeclaredConfigSlot>> {
    let Some(raw) = document.get(TABLE_LABEL) else {
        return Ok(Vec::new());
    };
    let entries = raw.as_array().ok_or_else(|| {
        violation(
            TABLE_LABEL,
            "`config_slots` must be an array of tables (`[[config_slots]]`)",
        )
    })?;
    entries
        .iter()
        .enumerate()
        .map(|(index, entry)| parse_declaration_entry(index, entry))
        .collect()
}

/// Parse one `[[config_slots]]` entry, re-validating `kind` against the closed
/// vocabulary because these bytes are untrusted input to this crate.
fn parse_declaration_entry(index: usize, entry: &toml::Value) -> Result<DeclaredConfigSlot> {
    let positional = format!("{TABLE_LABEL}[{index}]");
    let table = entry
        .as_table()
        .ok_or_else(|| violation(&positional, "declaration entry is not a table"))?;

    let key = required_string(table, "key", &positional)?;
    // Prefer the entry's own key as the error label once it is readable — a
    // positional index is only useful when the key itself is unreadable.
    let label = if key.is_empty() {
        positional.clone()
    } else {
        key.clone()
    };

    let kind = required_string(table, "kind", &label)?;
    if !ACCEPTED_KINDS.contains(&kind.as_str()) {
        // The rejected discriminator is deliberately NOT echoed: it is
        // attacker-controlled text from the document, and the uniform rule is
        // that errors name the key and the rule, never document content.
        return Err(violation(
            &label,
            format!(
                "unknown config-slot kind; the accepted kinds are {}",
                ACCEPTED_KINDS.join(", ")
            ),
        ));
    }

    let name = required_string(table, "name", &label)?;

    let tested_value = match table.get("tested_value") {
        None => None,
        Some(toml::Value::String(value)) => Some(value.clone()),
        Some(_) => return Err(violation(&label, "`tested_value` must be a string")),
    };

    // Match the runtime parser's `deny_unknown_fields` (see [`ACCEPTED_FIELDS`]).
    // The stray field name IS echoed: it is a KEY, not a value, and naming it is
    // the whole point — the same information `serde`'s own `unknown field` error
    // gives at boot. The uniform "never echo document CONTENT" rule is about
    // values.
    for field in table.keys() {
        if !ACCEPTED_FIELDS.contains(&field.as_str()) {
            return Err(violation(
                &label,
                format!(
                    "unknown field `{field}` on a `[[config_slots]]` entry; the accepted fields \
                     are {}. The server that boots from these same bytes parses this table with \
                     `deny_unknown_fields`, so packing it would ship a package that cannot boot",
                    ACCEPTED_FIELDS.join(", ")
                ),
            ));
        }
    }

    Ok(DeclaredConfigSlot {
        key,
        kind,
        name,
        tested_value,
    })
}

/// Read a required string field off a declaration entry.
fn required_string(table: &toml::Table, field: &str, label: &str) -> Result<String> {
    match table.get(field) {
        Some(toml::Value::String(value)) => Ok(value.clone()),
        Some(_) => Err(violation(label, format!("`{field}` must be a string"))),
        None => Err(violation(
            label,
            format!("`{field}` is required on a `[[config_slots]]` entry"),
        )),
    }
}

/// The comparable projection of a slot: `(kind, name, tested_value)`.
type SlotFacts<'a> = (&'a str, &'a str, Option<&'a str>);

/// Require the config's `[[config_slots]]` declarations and the package's
/// `config_slots` list to describe the SAME slots.
///
/// Compared as SETS keyed on the config key — declaration order in the TOML is
/// not load-bearing. A [`ConfigSlot`] whose `config_key` is `None` does not
/// participate: it fills no config path, so there is nothing for a declaration
/// to correspond to. Whether such a slot is legal at all is
/// [`crate::validate_config_slot_placeholders`]'s
/// rule, not this one.
///
/// The first disagreement is reported in a deterministic (sorted-key) order, so
/// a config with several problems always fails the same way.
///
/// # Errors
///
/// Returns [`PackageError::ConfigSlotViolation`] naming the offending key when
/// a declaration has no matching package slot, a package slot has no matching
/// declaration, either side declares the same key twice, or a matched pair
/// disagrees on `kind`, `name` or `tested_value`. The message names the key and
/// the FIELD that disagreed — never the two values, so a future slot kind
/// cannot leak by inheriting an exception.
///
/// # Examples
///
/// ```
/// use pmcp_package::{
///     parse_declared_config_slots, validate_config_slot_agreement, ConfigSlot, SlotType,
/// };
///
/// let config = br#"
/// [[config_slots]]
/// key = "backend.base_url"
/// kind = "endpoint"
/// name = "TFL_BASE_URL"
/// tested_value = "https://api.tfl.gov.uk"
/// "#;
/// let declared = parse_declared_config_slots(config).unwrap();
///
/// let matching = vec![ConfigSlot::new(SlotType::Endpoint {
///     name: "TFL_BASE_URL".to_string(),
///     tested_value: "https://api.tfl.gov.uk".to_string(),
/// })
/// .with_config_key("backend.base_url")];
/// assert!(validate_config_slot_agreement(&declared, &matching).is_ok());
///
/// // A package that claims a slot its shipped config never declares is refused.
/// let invented = vec![ConfigSlot::new(SlotType::Secret {
///     name: "SOME_KEY".to_string(),
/// })
/// .with_config_key("backend.auth.api_key")];
/// assert!(validate_config_slot_agreement(&declared, &invented).is_err());
/// ```
pub fn validate_config_slot_agreement(
    declared: &[DeclaredConfigSlot],
    package_slots: &[ConfigSlot],
) -> Result<()> {
    let declared_facts = declared_fact_map(declared)?;
    let package_facts = package_fact_map(package_slots)?;

    // Sorted union of both key sets — BTreeMap iteration is already ordered,
    // and chaining two ordered maps into a BTreeSet keeps the union ordered,
    // so "the first disagreement" is reproducible.
    let keys: std::collections::BTreeSet<&str> = declared_facts
        .keys()
        .chain(package_facts.keys())
        .copied()
        .collect();

    for key in keys {
        match (declared_facts.get(key), package_facts.get(key)) {
            (Some(declaration), Some(package)) => compare_facts(key, *declaration, *package)?,
            (Some(_), None) => {
                return Err(violation(
                    key,
                    "declared in the config's `[[config_slots]]` table but absent from the \
                     package's config_slots list — the shipped config is the source of truth, \
                     so add the slot to the package rather than dropping the declaration",
                ))
            },
            (None, Some(_)) => {
                return Err(violation(
                    key,
                    "present in the package's config_slots list but absent from the config's \
                     `[[config_slots]]` table — a package may not claim a slot the config it \
                     ships does not declare",
                ))
            },
            // Unreachable: `key` came from the union of the two maps.
            (None, None) => {},
        }
    }
    Ok(())
}

/// Compare a matched declaration/package pair field by field.
fn compare_facts(key: &str, declaration: SlotFacts<'_>, package: SlotFacts<'_>) -> Result<()> {
    if declaration.0 != package.0 {
        // Kinds are a closed vocabulary, not config content, so naming both is
        // safe and is what makes the error actionable.
        return Err(violation(
            key,
            format!(
                "the config declares kind '{}' but the package slot is '{}'",
                declaration.0, package.0
            ),
        ));
    }
    if declaration.1 != package.1 {
        return Err(violation(
            key,
            "the declared slot `name` disagrees with the package slot's name",
        ));
    }
    if declaration.2 != package.2 {
        return Err(violation(
            key,
            "the declared `tested_value` disagrees with the package slot's tested value",
        ));
    }
    Ok(())
}

/// Index the declarations by config key, rejecting a duplicated key.
fn declared_fact_map(declared: &[DeclaredConfigSlot]) -> Result<BTreeMap<&str, SlotFacts<'_>>> {
    let mut map = BTreeMap::new();
    for declaration in declared {
        let facts = (
            declaration.kind.as_str(),
            declaration.name.as_str(),
            declaration.tested_value.as_deref(),
        );
        if map.insert(declaration.key.as_str(), facts).is_some() {
            return Err(violation(
                &declaration.key,
                "declared more than once in the config's `[[config_slots]]` table",
            ));
        }
    }
    Ok(map)
}

/// Index the package slots by their `config_key`, rejecting a duplicated key.
/// Slots with no `config_key` fill no config path and are not indexed.
fn package_fact_map(package_slots: &[ConfigSlot]) -> Result<BTreeMap<&str, SlotFacts<'_>>> {
    let mut map = BTreeMap::new();
    for slot in package_slots {
        let Some(config_key) = slot.config_key.as_deref() else {
            continue;
        };
        let (kind, name) = slot.slot.key();
        let facts = (kind, name, slot.slot.tested_value());
        if map.insert(config_key, facts).is_some() {
            return Err(violation(
                config_key,
                "claimed by more than one slot in the package's config_slots list",
            ));
        }
    }
    Ok(map)
}

// =======================================================================
// D-04 (as amended by D-17): a slot-declared VALUE key must hold an
// environment reference, never a resolved literal.
// =======================================================================

/// Require every slot-declared VALUE key in `config_bytes` to hold an
/// environment reference (`${VAR}` or `env:VAR`) rather than a resolved
/// literal, so no resolved secret or environment-specific endpoint can travel
/// inside a packed layer (D-04, T-120-20).
///
/// # The slot split is THREE-way and exhaustive
///
/// The rule is written as a `match` over [`SlotType`] with NO catch-all arm, so
/// a future variant is a compile error until someone decides which arm it
/// belongs in:
///
/// - **Value slots — [`SlotType::Endpoint`] and [`SlotType::Secret`].** Subject
///   to the placeholder rule. When a config file is present, a `config_key` of
///   `None` on one of these is itself a violation: a packable config server
///   whose endpoint or credential slot does not say WHERE it lives cannot be
///   validated by pack and cannot tell a target environment where to write.
/// - **Structural — [`SlotType::AuthMode`].** Exempt from the PLACEHOLDER rule
///   (D-17). The toolkit's `AuthConfig` is internally tagged
///   (`#[serde(tag = "type")]`), so a reference-shaped value at that key fails
///   serde's variant dispatch before any resolution could happen — there is no
///   placeholder form of that key that both parses and defers, which makes the
///   baked literal the only legal content. Deviation on it surfaces through
///   slot classification instead. Exempt is not unchecked, though: when the
///   slot names a `config_key`, that key must resolve to a real STRING key
///   (a dangling declaration is a defect, not a pass), and the baked literal
///   must equal the declared `tested_value` — a package that ships one mode
///   while claiming it was tested with another records a false baseline.
/// - **Not config-value slots — [`SlotType::OauthClient`],
///   [`SlotType::ChannelBinding`], [`SlotType::HumanRole`],
///   [`SlotType::LlmProvider`], [`SlotType::BudgetOverride`].** With
///   `config_key: None`, skipped. With a `config_key`, a violation: declaring a
///   TOML path for a slot kind that fills none is a defect, and silently
///   ignoring it would let a package claim a coverage it does not have.
///   ([`SlotType::HumanRole`] has no simple value field at all.)
///
/// Callers reach this through [`pack_server`](crate::oci::pack_server), which
/// runs it before writing any blob. It is public so a CLI can pre-check a
/// config before building a package at all.
///
/// # Errors
///
/// Returns [`PackageError::ConfigSlotViolation`] naming the offending config
/// key. The offending VALUE is never echoed — it may be the exact resolved
/// secret the rule exists to keep out of a layer.
///
/// # Examples
///
/// ```
/// use pmcp_package::{validate_config_slot_placeholders, ConfigSlot, SlotType};
///
/// let slots = vec![ConfigSlot::new(SlotType::Secret {
///     name: "TFL_APP_KEY".to_string(),
/// })
/// .with_config_key("backend.auth.app_key")];
///
/// // Accepted: the credential key defers to the environment.
/// let deferred = b"[backend.auth]\napp_key = \"${TFL_APP_KEY}\"\n";
/// assert!(validate_config_slot_placeholders(deferred, &slots).is_ok());
///
/// // Refused: the credential was resolved into the file that is about to be
/// // packed. The error names the key and never the value.
/// let baked = b"[backend.auth]\napp_key = \"a-real-credential\"\n";
/// let err = validate_config_slot_placeholders(baked, &slots).unwrap_err();
/// assert!(err.to_string().contains("backend.auth.app_key"));
/// assert!(!err.to_string().contains("a-real-credential"));
/// ```
pub fn validate_config_slot_placeholders(config_bytes: &[u8], slots: &[ConfigSlot]) -> Result<()> {
    validate_config_slot_placeholders_in(&parse_document(config_bytes)?, slots)
}

/// Document-taking half of [`validate_config_slot_placeholders`], so a caller
/// that runs several gates over the same config (`pack_server`) parses once.
pub(crate) fn validate_config_slot_placeholders_in(
    document: &toml::Value,
    slots: &[ConfigSlot],
) -> Result<()> {
    for slot in slots {
        check_slot_placeholder(document, slot)?;
    }
    Ok(())
}

/// The per-slot half of [`validate_config_slot_placeholders`] — the exhaustive
/// three-way match itself, split out so the public function stays a loop.
fn check_slot_placeholder(document: &toml::Value, slot: &ConfigSlot) -> Result<()> {
    match &slot.slot {
        // --- Value slots: subject to the placeholder rule -----------------
        SlotType::Endpoint { name, .. } | SlotType::Secret { name } => {
            let Some(config_key) = slot.config_key.as_deref() else {
                return Err(violation(
                    name,
                    "a value slot (endpoint or secret) on a package that ships a config file \
                     must name the config key it fills — without one, pack cannot validate it \
                     and a target environment cannot be told where to write",
                ));
            };
            let value = resolve_dotted_key(document, config_key)?;
            let toml::Value::String(raw) = value else {
                return Err(violation(
                    config_key,
                    "a slot-declared value key must hold a string; this key holds a non-string \
                     TOML value, which no environment reference can be expressed as",
                ));
            };
            if is_env_reference(raw) {
                Ok(())
            } else if is_malformed_env_reference(raw) {
                Err(violation(
                    config_key,
                    "holds a malformed environment reference; a reference is exactly one \
                     `${VAR}` or `env:VAR` naming a single variable ([A-Za-z0-9_]+) — a \
                     multi-placeholder composition like `${SCHEME}://${HOST}` cannot be \
                     resolved by any target environment, so compose the full value in ONE \
                     variable instead",
                ))
            } else {
                Err(violation(
                    config_key,
                    "holds a resolved literal; a slot-declared value key must hold an \
                     environment reference (`${VAR}` or `env:VAR`) so the resolved value never \
                     travels inside a packed layer",
                ))
            }
        },
        // --- Structural: exempt from the PLACEHOLDER rule (D-17) ----------
        //
        // Exempt does not mean unchecked. The baked literal is the only legal
        // content for the auth-mode key, so what CAN be validated is anchoring
        // and honesty: a declared `config_key` must resolve to a real string
        // key ("a slot declaration pointing at no key is a defect, not a
        // pass"), and the baked value must BE the declared `tested_value` —
        // the tested_value is the package's claim about what it ships, and a
        // config that ships `bearer` while claiming it was tested with
        // `api_key` gives downstream deviation classification a false
        // baseline. A slot with no `config_key` anchors nothing and is left
        // alone (agreement with a declaring config already forces the key on).
        SlotType::AuthMode { tested_value, .. } => {
            let Some(config_key) = slot.config_key.as_deref() else {
                return Ok(());
            };
            let value = resolve_dotted_key(document, config_key)?;
            let toml::Value::String(baked) = value else {
                return Err(violation(
                    config_key,
                    "an auth-mode slot must address a string key (the serde tag); this key \
                     holds a non-string TOML value",
                ));
            };
            if baked == tested_value {
                Ok(())
            } else {
                // Names the key and the FIELD only — the baked discriminator is
                // document content and is not echoed, per the uniform rule.
                Err(violation(
                    config_key,
                    "the baked auth-mode literal disagrees with the slot's declared \
                     `tested_value`; the config is the source of truth, so update the \
                     declaration (and the package slot) to the mode the config actually ships",
                ))
            }
        },
        // --- Not config-value slots ---------------------------------------
        SlotType::OauthClient { .. }
        | SlotType::ChannelBinding { .. }
        | SlotType::HumanRole { .. }
        | SlotType::LlmProvider { .. }
        | SlotType::BudgetOverride { .. } => match slot.config_key.as_deref() {
            None => Ok(()),
            Some(config_key) => Err(violation(
                config_key,
                "this slot kind has no config-value semantics, so it cannot fill a TOML config \
                 key; only endpoint and secret slots address a config value, and auth_mode is \
                 structural",
            )),
        },
    }
}

/// Resolve a dotted `config_key` against a parsed config document.
///
/// # Grammar
///
/// A `config_key` is one or more `.`-separated components. Each component must
/// be a non-empty TOML **bare key** — ASCII letters, digits, `_` and `-` — and
/// every component except the last must address a TOML **table**.
///
/// Deliberately OUT OF SCOPE, and REJECTED rather than mis-resolved:
/// array indexing (`tools[0].path`), and TOML quoted keys — which means a TOML
/// key whose literal name contains a dot is unaddressable by this grammar.
/// Saying so in an error is honest where silently splitting on the dot is not.
///
/// # Errors
///
/// Returns [`PackageError::ConfigSlotViolation`] naming the key and the rule it
/// broke: an empty key, an empty component (a leading dot, a trailing dot or a
/// doubled dot), a component that is not a bare key, a path traversing a
/// non-table, or a path that resolves to nothing. None of these silently
/// resolves to "absent, therefore fine".
fn resolve_dotted_key<'a>(document: &'a toml::Value, config_key: &str) -> Result<&'a toml::Value> {
    if config_key.is_empty() {
        return Err(violation(
            config_key,
            "config key is empty; the grammar is one or more dot-separated non-empty TOML bare keys",
        ));
    }
    let mut current = document;
    for component in config_key.split('.') {
        if component.is_empty() {
            return Err(violation(
                config_key,
                "config key has an empty path component (a leading dot, a trailing dot or a \
                 doubled dot); the grammar is dot-separated non-empty TOML bare keys",
            ));
        }
        if !is_bare_key(component) {
            return Err(violation(
                config_key,
                "config key has a component that is not a TOML bare key (A-Z a-z 0-9 _ -); \
                 quoted keys and array indexing are out of scope, so a TOML key whose literal \
                 name contains a dot is unaddressable by this grammar",
            ));
        }
        let table = current.as_table().ok_or_else(|| {
            violation(
                config_key,
                "config key traverses a value that is not a table; the grammar addresses TOML \
                 tables only",
            )
        })?;
        current = table.get(component).ok_or_else(|| {
            violation(
                config_key,
                "config key resolves to nothing in the packed config — a slot declaration \
                 pointing at no key is a defect, not a pass",
            )
        })?;
    }
    Ok(current)
}

/// Whether `component` is a TOML bare key (non-emptiness is checked by the
/// caller, which reports it as its own distinct rule).
fn is_bare_key(component: &str) -> bool {
    component
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Whether `raw` is an environment REFERENCE rather than a resolved literal.
///
/// Recognises exactly two forms: an `env:` prefix with a non-empty remainder,
/// and a `${` … `}` wrapper whose inner name is ONE valid variable
/// (`[A-Za-z0-9_]+`). Everything else — a bare literal, an unterminated
/// brace, text after the closing brace, the malformed empty-name forms `${}`
/// and `env:`, and multi-placeholder brace compositions like
/// `${SCHEME}://${HOST}` (whose "name" would be the unsettable
/// `SCHEME}://${HOST`) — answers `false`, because none of it names a variable
/// a target environment could supply. The malformed shapes get their own pack
/// error (see [`is_malformed_env_reference`]) so the refusal names the real
/// defect.
///
/// # A deliberate duplication, kept honest by a table
///
/// The crate that OWNS this grammar is `pmcp-server-toolkit`
/// (`src/env_ref.rs::parse_env_ref`). It is duplicated here rather than shared
/// because neither crate may depend on the other: `pmcp-package` is the
/// workspace-excluded leaf, and a toolkit dependency on it inverts the
/// layering. A silent divergence would be a real bug — a config that packs
/// cleanly and then fails to resolve at boot, or one the runtime resolves being
/// refused at pack — so the two implementations are held to a shared
/// accept/reject table, `tests/golden_fixtures/env_ref_grammar_v1.tsv`,
/// asserted from BOTH crates. A row one side disagrees with fails a test in
/// whichever crate is wrong.
///
/// Note the two implementations differ in SHAPE, not in verdict:
/// `parse_env_ref` returns `Some("")` for every malformed brace form — `${}`
/// and multi-placeholder compositions alike (its caller resolves an empty
/// name to omission or an error) — while this predicate answers `false`,
/// because a malformed reference is not something a package can ask an
/// environment to fill. The table encodes that correspondence explicitly
/// rather than papering over it, and its coherence rule (a reject row must
/// resolve to nothing at runtime) is why the `env:` arm is identical on both
/// sides.
fn is_env_reference(raw: &str) -> bool {
    env_ref_name(raw).is_some()
}

/// The name-returning half of [`is_env_reference`], so the reverse gate
/// ([`validate_no_undeclared_env_refs`]) can NAME the variable a config defers
/// to without restating the grammar. `is_env_reference` is defined in terms of
/// this function precisely so the two can never disagree: the parity table
/// `tests/golden_fixtures/env_ref_grammar_v1.tsv` asserts the predicate, and a
/// second independent implementation here would be a second thing to keep in
/// step with the toolkit.
///
/// Returns the variable name for the two accepted forms and `None` for
/// everything else — plain literals, the empty forms (`${}`, `env:`) and
/// multi-placeholder compositions alike. Verdict-identical to the predicate it
/// replaced, row for row.
fn env_ref_name(raw: &str) -> Option<&str> {
    if let Some(rest) = raw.strip_prefix("env:") {
        return (!rest.is_empty()).then_some(rest);
    }
    raw.strip_prefix("${")
        .and_then(|inner| inner.strip_suffix('}'))
        .filter(|name| is_valid_env_var_name(name))
}

/// Whether `name` is a variable name a TARGET environment can actually be
/// told to set: non-empty, ASCII alphanumerics and `_` only. Applied to the
/// `${NAME}` form only — that is the form a config author composes by
/// accident (`${SCHEME}://${HOST}`). The explicit `env:NAME` form keeps its
/// any-non-empty-remainder rule on BOTH sides of the grammar: the parity
/// contract forbids a value one side resolves and the other refuses.
fn is_valid_env_var_name(name: &str) -> bool {
    !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// What [`undeclared_reason`] may print in place of a deferred variable's name
/// when the name is not one a target environment could be told to set.
const UNREPORTABLE_NAME: &str = "<name withheld: not a settable variable name>";

/// The deferred variable's name, rendered so it is safe to put in an error.
///
/// # Why this is not just `name.to_string()`
///
/// [`validate_no_undeclared_env_refs`]'s rationale for naming the variable at
/// all is that a value already proven to be a reference is a variable NAME and
/// therefore not a secret. That argument holds for the `${NAME}` form, whose
/// interior [`env_ref_name`] has already constrained to
/// [`is_valid_env_var_name`]'s `[A-Za-z0-9_]+`. It does NOT hold for the
/// `env:` form: its grammar accepts ANY non-empty remainder — the pinned
/// parity table (`tests/golden_fixtures/env_ref_grammar_v1.tsv`) has
/// `env:FOO}BAR` as an ACCEPT row precisely to record that — so the
/// "name" there is arbitrary document text.
///
/// Two things ride on that difference, and the module's error-hygiene rule
/// ("No error raised here ever echoes a config VALUE", T-120-21) forbids
/// both. An author who writes `api_key = "env:sk-live-…"` believing `env:` is
/// an encoding prefix has the credential printed by `cargo pmcp package save`
/// and captured in CI logs. And because [`undeclared_reason`] renders the
/// violations as one comma-separated list, a remainder containing `, ` or a
/// newline forges extra entries in it — a value that reads as a second,
/// non-existent offending key.
///
/// Withholding the name costs nothing the author needs: the KEY is what says
/// where the `[[config_slots]]` entry goes, and the config line it points at
/// is what says which variable to declare.
fn reportable_name(name: &str) -> &str {
    if is_valid_env_var_name(name) {
        name
    } else {
        UNREPORTABLE_NAME
    }
}

/// Reference-SHAPED but not a valid single reference: the empty forms (`${}`,
/// `env:`) and multi-placeholder brace compositions (`${A}://${B}`, whose
/// interior is the unsettable `A}://${B`). Distinguished from a plain literal
/// so the pack error can name the real defect — "this composition can never
/// resolve" — instead of claiming the value is a resolved literal.
fn is_malformed_env_reference(raw: &str) -> bool {
    if let Some(rest) = raw.strip_prefix("env:") {
        return rest.is_empty();
    }
    raw.strip_prefix("${")
        .and_then(|inner| inner.strip_suffix('}'))
        .is_some_and(|name| !is_valid_env_var_name(name))
}

/// Gate C — the CONFIG -> SLOT direction: refuse a config document that defers
/// a value to the environment without a `[[config_slots]]` entry naming that
/// key.
///
/// # Why this gate exists — the other two only run one way
///
/// [`validate_config_slot_agreement`] and [`validate_config_slot_placeholders`]
/// both START from the declared slot list. Both answer *"is every declared slot
/// well-formed, and does it point at a placeholder?"* — a good question,
/// correctly gated. Neither answers the converse, *"does every placeholder have
/// a slot?"*, and a config declaring NO slots at all satisfies both trivially:
/// iterating an empty list finds no violations.
///
/// Since the slot list is the whole mechanism for telling a target environment
/// what it must supply, the un-gated direction produced a package that installs
/// cleanly into a new environment and then cannot authenticate — reported
/// against `pmcp-package` 0.3.0 / `cargo-pmcp` 0.23.0, where a real OpenAPI
/// server carrying four `${...}` references packed at exit 0 and unpacked
/// saying "This package declares no config slots — nothing to fill."
///
/// The gap survived because the fixture the feature was built against,
/// `london-tube.toml`, declares all three of its slots and is fully
/// self-consistent, so the corpus never contained a config whose references
/// OUTNUMBERED its declarations.
///
/// # Scope — exactly the locations a slot can address, and no others
///
/// This walks the document's TABLES, building the same dotted paths
/// [`resolve_dotted_key`] resolves, and reports a string value that is a
/// whole-value environment reference whose path no slot's `config_key` names.
/// Two boundaries fall out of that, and BOTH are deliberate:
///
/// - **Arrays are not descended.** `resolve_dotted_key` addresses TOML tables
///   only — array indexing is out of its grammar — so a reference inside an
///   `[[tools]]` or `[[resources]]` entry is unnameable by ANY `config_key`.
///   Demanding a slot for it would be a demand no config author could satisfy.
///   This is also what keeps the gate off `london-tube.toml`'s two JS template
///   placeholders (`${line.id}` in a `[[tools]].script`, `${'victoria'}` in a
///   `[[resources]].content`): they are a different `${}` namespace entirely,
///   and a naive document-wide text scan would have flagged this crate's own
///   golden fixture.
/// - **Whole-value references only**, per the pinned grammar
///   (`tests/golden_fixtures/env_ref_grammar_v1.tsv`). `${A}-${B}` and
///   `${VAR}-suffix` are reject rows there, so an EMBEDDED reference is not
///   something any environment can fill through a slot, and this gate does not
///   pretend otherwise. A config wanting one filled must compose the whole
///   value in one variable — which is what the forward gate already says.
///
/// Malformed whole-value references (`${}`, `env:`, `${A}://${B}`) are OUT of
/// scope here: they are a different defect with a different fix — repair the
/// reference, not declare a slot — and the forward gate already names them
/// where a slot points at one.
///
/// # Naming the variable is safe HERE, unlike in the forward gate
///
/// [`validate_config_slot_placeholders`] names the key and never the value,
/// because the value it rejects may be a RESOLVED credential. This gate fires
/// only on values already proven to be references, so the "value" it reports is
/// a variable NAME — never a secret, and the single most useful thing to put in
/// the message.
///
/// # Errors
///
/// Returns [`PackageError::ConfigSlotViolation`] naming the first undeclared
/// key in lexicographic order, with the undeclared keys listed in the reason
/// (up to [`MAX_LISTED_UNDECLARED`], then a count of the rest) so a config with
/// several is fixed in one pass rather than one per pack.
///
/// Also returns [`PackageError::Serialize`] when `config_bytes` are not
/// parseable TOML — this entry point parses before it validates, so a caller
/// matching only on `ConfigSlotViolation` falls through on the commonest input
/// error of all.
///
/// [`PackageError::ConfigSlotViolation`]: crate::error::PackageError::ConfigSlotViolation
///
/// # Examples
///
/// ```
/// use pmcp_package::{validate_no_undeclared_env_refs, ConfigSlot, SlotType};
///
/// let config = br#"
/// [backend]
/// base_url = "${TFL_BASE_URL}"
/// "#;
///
/// // Refused: the config defers `backend.base_url` to the environment, but no
/// // slot tells a target environment to supply `TFL_BASE_URL`.
/// let err = validate_no_undeclared_env_refs(config, &[]).unwrap_err();
/// assert!(err.to_string().contains("backend.base_url"));
/// assert!(err.to_string().contains("TFL_BASE_URL"));
///
/// // Accepted once the slot is declared.
/// let slots = vec![ConfigSlot::new(SlotType::Endpoint {
///     name: "TFL_BASE_URL".to_string(),
///     tested_value: "https://api.tfl.gov.uk".to_string(),
/// })
/// .with_config_key("backend.base_url")];
/// assert!(validate_no_undeclared_env_refs(config, &slots).is_ok());
/// ```
pub fn validate_no_undeclared_env_refs(config_bytes: &[u8], slots: &[ConfigSlot]) -> Result<()> {
    validate_no_undeclared_env_refs_in(&parse_document(config_bytes)?, slots)
}

/// Document-taking half of [`validate_no_undeclared_env_refs`], so a caller
/// that runs several gates over the same config (`pack_server`) parses once.
///
/// # Errors
///
/// Per [`validate_no_undeclared_env_refs`].
pub(crate) fn validate_no_undeclared_env_refs_in(
    document: &toml::Value,
    slots: &[ConfigSlot],
) -> Result<()> {
    // Fail CLOSED, not open. Every in-tree caller sources `document` from
    // `parse_document`, and a TOML document always roots in a table, so this
    // arm is unreachable today. Returning `Ok(())` from it anyway would make
    // the one impossible input the one input this gate waves through — it
    // would report "no undeclared references" without having looked at
    // anything, which is the exact fail-open shape the gate exists to close.
    // `resolve_dotted_key` meets the same situation and raises a violation;
    // this matches it.
    let Some(root) = document.as_table() else {
        return Err(violation(
            DOCUMENT_LABEL,
            "the config document does not root in a TOML table, so no `config_key` can address \
             anything in it and no deferred value could be checked",
        ));
    };
    let declared: BTreeSet<&str> = slots
        .iter()
        .filter_map(|slot| slot.config_key.as_deref())
        .collect();
    let mut undeclared = BTreeMap::new();
    collect_undeclared_env_refs(root, "", &declared, &mut undeclared);
    let Some((first, _)) = undeclared.first_key_value() else {
        return Ok(());
    };
    Err(violation(first, undeclared_reason(&undeclared)))
}

/// Walk `table`, recording every SLOT-ADDRESSABLE key that holds a whole-value
/// environment reference `declared` does not name, as
/// `dotted key -> variable name`.
///
/// Addressable means what [`resolve_dotted_key`] can reach: a chain of
/// non-empty TOML bare keys through tables. Non-bare and empty keys are
/// skipped (a key whose literal name contains a dot is unaddressable by the
/// dotted grammar — note `is_bare_key("")` is `true`, so the emptiness check is
/// load-bearing rather than redundant) and arrays are not descended — see the
/// reasoning on [`validate_no_undeclared_env_refs`].
///
/// `declared` is applied HERE rather than to a fully-collected map afterwards,
/// so `out` holds only violations for its whole lifetime and its name is never
/// briefly a lie. `undeclared_agrees_with_what_resolve_dotted_key_can_address`
/// is what keeps this walk's notion of addressable in step with
/// [`resolve_dotted_key`]'s; the two are duals of one grammar with no shared
/// code, and a silent divergence would be fail-OPEN — the same shape as the bug
/// this gate exists to close.
///
/// Recursion is bounded by the document's own nesting depth, and that depth is
/// bounded by the `toml` dependency, which refuses to build a `Value` past its
/// own recursion limit — measured at 80 levels for `toml` 1.1.x, on both the
/// inline-table and the dotted/header forms, with `parse_document` returning
/// `Err` before this walk is ever entered.
///
/// State the bound as the DEPENDENCY PROPERTY it is. "The parser already
/// recursed through it" would not establish anything on its own: a parser
/// recursing does not bound a different walker with different frames, and
/// `toml`'s parser is event-based rather than a mirror of this walk. If a
/// future `toml` raises or removes that limit, this becomes the only recursive
/// consumer of untrusted config in the crate, and the failure mode is a stack
/// overflow — an abort, not a catchable panic — so re-measure it rather than
/// assume it.
fn collect_undeclared_env_refs(
    table: &toml::value::Table,
    prefix: &str,
    declared: &BTreeSet<&str>,
    out: &mut BTreeMap<String, String>,
) {
    for (key, value) in table {
        if key.is_empty() || !is_bare_key(key) {
            continue;
        }
        // The path is built INSIDE the arms that need it. Built before the
        // match, it is a heap allocation per visited key at every depth,
        // two thirds of which the match immediately drops (integers,
        // booleans, arrays, datetimes, and plain-literal strings — measured
        // at 15 of 23 on this crate's own golden fixture).
        match value {
            toml::Value::String(raw) => {
                let Some(name) = env_ref_name(raw) else {
                    continue;
                };
                let path = join_dotted(prefix, key);
                if !declared.contains(path.as_str()) {
                    out.insert(path, reportable_name(name).to_string());
                }
            },
            toml::Value::Table(nested) => {
                collect_undeclared_env_refs(nested, &join_dotted(prefix, key), declared, out);
            },
            _ => {},
        }
    }
}

/// The dotted path of `key` under `prefix` — the spelling
/// [`resolve_dotted_key`] parses back. One definition so the walker and the
/// test-module referee it is checked against cannot disagree on SPELLING while
/// the property test is busy checking they agree on REACH.
fn join_dotted(prefix: &str, key: &str) -> String {
    if prefix.is_empty() {
        key.to_string()
    } else {
        format!("{prefix}.{key}")
    }
}

/// The reason half of [`validate_no_undeclared_env_refs_in`]'s error: says what
/// breaks and lists the offending keys so one pack fixes them all.
///
/// Split out to keep the message string and its length bound out of the gate's
/// control flow. NOT a cognitive-complexity necessity: inlined, the gate sits
/// well inside CLAUDE.md's cap of 25.
///
/// The wording deliberately says the key "is not among the config keys the slot
/// list names" rather than "no `[[config_slots]]` entry declares it". This
/// function is reached from the exported
/// [`validate_no_undeclared_env_refs`], which is handed a `&[ConfigSlot]` and
/// never reads the document's own `[[config_slots]]` table — inside
/// `pack_server` the two are already proven equal by
/// [`validate_config_slot_agreement`], but a standalone caller passing `&[]`
/// against a fully-declared config would otherwise be told something false
/// about the file in front of it.
fn undeclared_reason(undeclared: &BTreeMap<String, String>) -> String {
    let listed = undeclared
        .iter()
        .take(MAX_LISTED_UNDECLARED)
        .map(|(key, name)| format!("{key} -> {name}"))
        .collect::<Vec<_>>()
        .join(", ");
    let elided = match undeclared.len().saturating_sub(MAX_LISTED_UNDECLARED) {
        0 => String::new(),
        n => format!(", and {n} more"),
    };
    format!(
        "the config defers this key to the environment, but it is not among the config keys the \
         slot list names — so the packed package under-reports what a target environment must \
         supply and the server cannot start where the variable is unset. Declare one slot per \
         deferred key ({listed}{elided}). Note only [[config_slots]] entries are read as \
         declarations; a slot must carry a `key` for this gate to see it"
    )
}

/// How many offending keys [`undeclared_reason`] spells out before eliding the
/// rest.
///
/// The list exists so one pack fixes them all, which a bounded prefix still
/// achieves — an author with twenty undeclared keys has a systemic problem the
/// first twenty already describe. Unbounded, the message grows with the
/// document: a config with 2000 references produced a measured 74 KB
/// `PackageError`, and every entry of it is text the config controls.
const MAX_LISTED_UNDECLARED: usize = 20;

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// The REAL config the reference OpenAPI server boots from, vendored
    /// byte-for-byte into this crate's fixtures (a drift guard in
    /// `tests/config_server.rs` fails if the copy diverges from its source).
    const LONDON_TUBE_TOML: &[u8] =
        include_bytes!("../../tests/golden_fixtures/config_server_london_tube_v1/london-tube.toml");

    fn endpoint_slot() -> ConfigSlot {
        ConfigSlot::new(SlotType::Endpoint {
            name: "TFL_BASE_URL".to_string(),
            tested_value: "https://api.tfl.gov.uk".to_string(),
        })
        .with_config_key("backend.base_url")
    }

    fn secret_slot() -> ConfigSlot {
        ConfigSlot::new(SlotType::Secret {
            name: "TFL_APP_KEY".to_string(),
        })
        .with_config_key("backend.auth.query_params.app_key")
    }

    fn auth_mode_slot() -> ConfigSlot {
        ConfigSlot::new(SlotType::AuthMode {
            name: "backend-auth-mode".to_string(),
            tested_value: "api_key".to_string(),
        })
        .with_config_key("backend.auth.type")
    }

    fn london_tube_package_slots() -> Vec<ConfigSlot> {
        vec![endpoint_slot(), secret_slot(), auth_mode_slot()]
    }

    fn expect_violation(err: PackageError) -> (String, String) {
        match err {
            PackageError::ConfigSlotViolation { key, reason } => (key, reason),
            other => panic!("expected ConfigSlotViolation, got: {other}"),
        }
    }

    /// Enumerate EVERY leaf string in `value` with a dotted path, descending
    /// arrays and non-bare keys alike — deliberately more permissive than
    /// either production walker, so it can act as the neutral referee in
    /// `undeclared_agrees_with_what_resolve_dotted_key_can_address`.
    fn naive_leaf_paths(value: &toml::Value, prefix: &str, out: &mut Vec<(String, String)>) {
        match value {
            toml::Value::String(raw) if !prefix.is_empty() => {
                out.push((prefix.to_string(), raw.clone()));
            },
            toml::Value::Table(table) => {
                for (key, nested) in table {
                    let path = if prefix.is_empty() {
                        key.clone()
                    } else {
                        format!("{prefix}.{key}")
                    };
                    naive_leaf_paths(nested, &path, out);
                }
            },
            toml::Value::Array(items) => {
                for (index, nested) in items.iter().enumerate() {
                    let path = if prefix.is_empty() {
                        index.to_string()
                    } else {
                        format!("{prefix}.{index}")
                    };
                    naive_leaf_paths(nested, &path, out);
                }
            },
            _ => {},
        }
    }

    /// Arbitrary TOML documents mixing the shapes this gate has to get right:
    /// accepted references, a malformed composition, plain literals, non-string
    /// values, nested tables, ARRAYS of tables (the `[[tools]]` shape), a
    /// non-bare key and the empty key.
    fn arbitrary_toml_document() -> impl Strategy<Value = toml::Value> {
        let leaf = prop_oneof![
            Just(toml::Value::String("${TFL_BASE_URL}".to_string())),
            Just(toml::Value::String("env:TFL_APP_KEY".to_string())),
            Just(toml::Value::String("${A}-${B}".to_string())),
            Just(toml::Value::String("https://api.tfl.gov.uk".to_string())),
            Just(toml::Value::Integer(7)),
            Just(toml::Value::Boolean(true)),
        ];
        // `my` is here for ONE reason: with `my.key` alone, the alias the
        // bi-implication's `raw == leaf` guard exists to handle — a quoted
        // `"my.key"` beside a table `my` holding `key`, two locations sharing
        // one dotted spelling — is unconstructible, and the guard is dead
        // code. Proven by mutation before it was added: rewriting the guard to
        // `|| true` left the property green at 50_000 cases.
        let key = prop_oneof![
            Just("a".to_string()),
            Just("backend".to_string()),
            Just("auth".to_string()),
            Just("my".to_string()),
            Just("key".to_string()),
            Just("my.key".to_string()),
            Just(String::new()),
        ];
        let table_of = |inner: BoxedStrategy<toml::Value>, key: BoxedStrategy<String>| {
            proptest::collection::btree_map(key, inner, 0..4)
                .prop_map(|entries| toml::Value::Table(entries.into_iter().collect()))
        };
        let tree = leaf.boxed().prop_recursive(4, 32, 4, move |inner| {
            prop_oneof![
                proptest::collection::vec(inner.clone(), 0..3).prop_map(toml::Value::Array),
                table_of(inner, key.clone().boxed()),
            ]
        });
        // The root of a TOML document is always a table.
        proptest::collection::btree_map(
            prop_oneof![
                Just("a".to_string()),
                Just("backend".to_string()),
                Just("my".to_string()),
                Just("my.key".to_string()),
            ],
            tree,
            0..4,
        )
        .prop_map(|entries| toml::Value::Table(entries.into_iter().collect()))
    }

    // --- Test 1: the real fixture parses to exactly its three declarations ---

    #[test]
    fn the_real_fixture_parses_to_its_three_declared_slots() {
        let declared = parse_declared_config_slots(LONDON_TUBE_TOML).unwrap();
        assert_eq!(declared.len(), 3, "declared slots were: {declared:?}");

        assert_eq!(declared[0].key, "backend.base_url");
        assert_eq!(declared[0].kind, "endpoint");
        assert_eq!(declared[0].name, "TFL_BASE_URL");
        assert_eq!(
            declared[0].tested_value.as_deref(),
            Some("https://api.tfl.gov.uk"),
            "the endpoint records the value it was tested against"
        );

        assert_eq!(declared[1].key, "backend.auth.query_params.app_key");
        assert_eq!(declared[1].kind, "secret");
        assert_eq!(declared[1].name, "TFL_APP_KEY");
        assert_eq!(
            declared[1].tested_value, None,
            "an identity-bearing slot structurally carries no tested value"
        );

        assert_eq!(declared[2].key, "backend.auth.type");
        assert_eq!(declared[2].kind, "auth_mode");
        assert_eq!(declared[2].name, "backend-auth-mode");
        assert_eq!(declared[2].tested_value.as_deref(), Some("api_key"));
    }

    // --- The ACCEPTED_KINDS const cannot drift from SlotType ---------------

    #[test]
    fn accepted_kinds_are_exactly_the_key_discriminators_of_the_config_slot_types() {
        // ACCEPTED_KINDS is a hand-written mirror of the three config-capable
        // SlotType discriminators. Nothing derives one from the other, so this
        // pin is what turns a fourth packable kind added on only ONE side into
        // a red test instead of a pack-time "unknown config-slot kind" for a
        // kind the type system already supports.
        let discriminators = [
            SlotType::Endpoint {
                name: String::new(),
                tested_value: String::new(),
            }
            .key()
            .0,
            SlotType::Secret {
                name: String::new(),
            }
            .key()
            .0,
            SlotType::AuthMode {
                name: String::new(),
                tested_value: String::new(),
            }
            .key()
            .0,
        ];
        assert_eq!(
            ACCEPTED_KINDS, discriminators,
            "ACCEPTED_KINDS must stay byte-identical to the SlotType::key() discriminators \
             of the config-capable kinds — update both together"
        );
    }

    // --- Test 2: no declaration table is legal, not an error ---------------

    #[test]
    fn a_config_with_no_declaration_table_parses_to_an_empty_vec() {
        let declared =
            parse_declared_config_slots(b"name = \"x\"\n[backend]\nkind = \"openapi\"\n")
                .expect("a config that declares nothing is legal");
        assert!(declared.is_empty());
    }

    // --- Agreement: the matched case, order-insensitively -----------------

    #[test]
    fn agreement_holds_when_both_sides_describe_the_same_three_slots() {
        let declared = parse_declared_config_slots(LONDON_TUBE_TOML).unwrap();
        validate_config_slot_agreement(&declared, &london_tube_package_slots()).unwrap();
    }

    #[test]
    fn agreement_compares_as_sets_so_declaration_order_is_not_load_bearing() {
        let declared = parse_declared_config_slots(LONDON_TUBE_TOML).unwrap();
        let reordered = vec![auth_mode_slot(), secret_slot(), endpoint_slot()];
        validate_config_slot_agreement(&declared, &reordered).unwrap();
    }

    // --- Test 4: declared in TOML, absent from the package -----------------

    #[test]
    fn a_declaration_with_no_matching_package_slot_names_that_key() {
        let declared = parse_declared_config_slots(LONDON_TUBE_TOML).unwrap();
        let missing_the_secret = vec![endpoint_slot(), auth_mode_slot()];
        let (key, reason) = expect_violation(
            validate_config_slot_agreement(&declared, &missing_the_secret).unwrap_err(),
        );
        assert_eq!(key, "backend.auth.query_params.app_key");
        assert!(reason.contains("absent from the package"), "was: {reason}");
    }

    // --- Test 5: invented by the package, absent from the TOML -------------

    #[test]
    fn a_package_slot_the_config_never_declares_names_that_key() {
        let declared = parse_declared_config_slots(LONDON_TUBE_TOML).unwrap();
        let mut invented = london_tube_package_slots();
        invented.push(
            ConfigSlot::new(SlotType::Secret {
                name: "INVENTED".to_string(),
            })
            .with_config_key("backend.auth.invented"),
        );
        let (key, reason) =
            expect_violation(validate_config_slot_agreement(&declared, &invented).unwrap_err());
        assert_eq!(key, "backend.auth.invented");
        assert!(reason.contains("absent from the config"), "was: {reason}");
    }

    // --- Test 6: same key, different kind ---------------------------------

    #[test]
    fn a_kind_disagreement_names_the_key_and_both_kinds() {
        let declared = parse_declared_config_slots(LONDON_TUBE_TOML).unwrap();
        let wrong_kind = vec![
            ConfigSlot::new(SlotType::Secret {
                name: "TFL_BASE_URL".to_string(),
            })
            .with_config_key("backend.base_url"),
            secret_slot(),
            auth_mode_slot(),
        ];
        let (key, reason) =
            expect_violation(validate_config_slot_agreement(&declared, &wrong_kind).unwrap_err());
        assert_eq!(key, "backend.base_url");
        assert!(reason.contains("endpoint"), "was: {reason}");
        assert!(reason.contains("secret"), "was: {reason}");
    }

    // --- Test 7: same key and kind, different name / tested_value ----------

    #[test]
    fn a_name_disagreement_names_the_key_and_the_field_but_not_the_values() {
        let declared = parse_declared_config_slots(LONDON_TUBE_TOML).unwrap();
        let wrong_name = vec![
            ConfigSlot::new(SlotType::Endpoint {
                name: "SOMETHING_ELSE".to_string(),
                tested_value: "https://api.tfl.gov.uk".to_string(),
            })
            .with_config_key("backend.base_url"),
            secret_slot(),
            auth_mode_slot(),
        ];
        let (key, reason) =
            expect_violation(validate_config_slot_agreement(&declared, &wrong_name).unwrap_err());
        assert_eq!(key, "backend.base_url");
        assert!(reason.contains("`name`"), "was: {reason}");
        assert!(
            !reason.contains("SOMETHING_ELSE"),
            "the message must name the FIELD, not echo the values; was: {reason}"
        );
    }

    #[test]
    fn a_tested_value_disagreement_names_the_key_and_the_field_but_not_the_values() {
        let declared = parse_declared_config_slots(LONDON_TUBE_TOML).unwrap();
        let wrong_tested = vec![
            ConfigSlot::new(SlotType::Endpoint {
                name: "TFL_BASE_URL".to_string(),
                tested_value: "https://sentinel.invalid/other".to_string(),
            })
            .with_config_key("backend.base_url"),
            secret_slot(),
            auth_mode_slot(),
        ];
        let (key, reason) =
            expect_violation(validate_config_slot_agreement(&declared, &wrong_tested).unwrap_err());
        assert_eq!(key, "backend.base_url");
        assert!(reason.contains("`tested_value`"), "was: {reason}");
        assert!(
            !reason.contains("sentinel.invalid"),
            "the message must not echo either value; was: {reason}"
        );
    }

    // --- Slots with no config_key do not participate ----------------------

    #[test]
    fn a_package_slot_with_no_config_key_does_not_participate_in_agreement() {
        let declared = parse_declared_config_slots(LONDON_TUBE_TOML).unwrap();
        let mut with_unkeyed = london_tube_package_slots();
        with_unkeyed.push(ConfigSlot::new(SlotType::LlmProvider {
            name: "primary".to_string(),
            tested_value: "anthropic".to_string(),
        }));
        validate_config_slot_agreement(&declared, &with_unkeyed).unwrap();
    }

    // --- Duplicates on either side ----------------------------------------

    #[test]
    fn the_same_key_declared_twice_in_the_config_is_a_violation() {
        let declared = vec![
            DeclaredConfigSlot {
                key: "backend.base_url".to_string(),
                kind: "endpoint".to_string(),
                name: "A".to_string(),
                tested_value: None,
            },
            DeclaredConfigSlot {
                key: "backend.base_url".to_string(),
                kind: "endpoint".to_string(),
                name: "B".to_string(),
                tested_value: None,
            },
        ];
        let (key, reason) = expect_violation(
            validate_config_slot_agreement(&declared, &london_tube_package_slots()).unwrap_err(),
        );
        assert_eq!(key, "backend.base_url");
        assert!(reason.contains("more than once"), "was: {reason}");
    }

    #[test]
    fn the_same_config_key_claimed_by_two_package_slots_is_a_violation() {
        let declared = parse_declared_config_slots(LONDON_TUBE_TOML).unwrap();
        let mut doubled = london_tube_package_slots();
        doubled.push(endpoint_slot());
        let (key, reason) =
            expect_violation(validate_config_slot_agreement(&declared, &doubled).unwrap_err());
        assert_eq!(key, "backend.base_url");
        assert!(reason.contains("more than one slot"), "was: {reason}");
    }

    // --- Test 9: kind is re-validated here, not trusted -------------------

    #[test]
    fn an_unknown_kind_names_the_key_and_the_accepted_kinds_without_echoing_it() {
        let config = br#"
[[config_slots]]
key = "backend.base_url"
kind = "endpont"
name = "TFL_BASE_URL"
"#;
        let (key, reason) = expect_violation(parse_declared_config_slots(config).unwrap_err());
        assert_eq!(key, "backend.base_url");
        assert!(reason.contains("endpoint"), "was: {reason}");
        assert!(reason.contains("secret"), "was: {reason}");
        assert!(reason.contains("auth_mode"), "was: {reason}");
        assert!(
            !reason.contains("endpont"),
            "the rejected discriminator is document content and must not be echoed; was: {reason}"
        );
    }

    // --- Malformed entries ------------------------------------------------

    #[test]
    fn a_declaration_missing_its_key_is_named_by_position() {
        let config = b"[[config_slots]]\nkind = \"secret\"\nname = \"A\"\n";
        let (key, reason) = expect_violation(parse_declared_config_slots(config).unwrap_err());
        assert_eq!(key, "config_slots[0]");
        assert!(reason.contains("`key` is required"), "was: {reason}");
    }

    #[test]
    fn a_declaration_missing_its_name_is_named_by_key() {
        let config = b"[[config_slots]]\nkey = \"backend.base_url\"\nkind = \"endpoint\"\n";
        let (key, reason) = expect_violation(parse_declared_config_slots(config).unwrap_err());
        assert_eq!(key, "backend.base_url");
        assert!(reason.contains("`name` is required"), "was: {reason}");
    }

    #[test]
    fn a_non_string_tested_value_is_a_violation() {
        let config =
            b"[[config_slots]]\nkey = \"k\"\nkind = \"endpoint\"\nname = \"N\"\ntested_value = 7\n";
        let (key, reason) = expect_violation(parse_declared_config_slots(config).unwrap_err());
        assert_eq!(key, "k");
        assert!(reason.contains("`tested_value` must be"), "was: {reason}");
    }

    #[test]
    fn a_config_slots_key_that_is_not_an_array_of_tables_is_a_violation() {
        let config = b"config_slots = \"not-an-array\"\n";
        let (key, _) = expect_violation(parse_declared_config_slots(config).unwrap_err());
        assert_eq!(key, "config_slots");
    }

    #[test]
    fn a_toml_syntax_error_is_reported_without_quoting_the_offending_line() {
        let config = b"backend = { api_key = \"super-secret-sentinel\"\n";
        let (key, reason) = expect_violation(parse_declared_config_slots(config).unwrap_err());
        assert_eq!(key, DOCUMENT_LABEL);
        assert!(
            !reason.contains("super-secret-sentinel"),
            "the parser's snippet must not reach the message; was: {reason}"
        );
    }

    #[test]
    fn non_utf8_config_bytes_are_an_error_not_a_panic() {
        let (key, _) =
            expect_violation(parse_declared_config_slots(&[0xff, 0xfe, 0x00]).unwrap_err());
        assert_eq!(key, DOCUMENT_LABEL);
    }

    // --- Test 10: never-panic property over arbitrary bytes ---------------

    proptest! {
        #[test]
        fn parse_declared_config_slots_never_panics_on_arbitrary_bytes(
            bytes in proptest::collection::vec(any::<u8>(), 0..512)
        ) {
            // The contract is total: every input yields Ok or Err, never an unwind.
            let _ = parse_declared_config_slots(&bytes);
        }

        #[test]
        fn parse_declared_config_slots_never_panics_on_arbitrary_text(
            text in "\\PC{0,200}"
        ) {
            let _ = parse_declared_config_slots(text.as_bytes());
        }
    }

    // ===================================================================
    // Task 2 — D-04 placeholder validation, scoped by the exhaustive
    // three-way slot split (D-17).
    // ===================================================================

    /// A distinctive value that must never appear in an error message. If it
    /// does, the validator is echoing config content and a real credential
    /// would leak the same way.
    const SENTINEL_CREDENTIAL: &str = "sentinel-leaked-credential";
    const SENTINEL_ENDPOINT: &str = "https://sentinel.invalid/leaked";

    fn assert_names_key_without_echoing(err: PackageError, key: &str, forbidden: &str) {
        let message = err.to_string();
        assert!(
            message.contains(key),
            "the error must name the config key; message was: {message}"
        );
        assert!(
            !message.contains(forbidden),
            "the error must NOT echo the offending value; message was: {message}"
        );
    }

    // --- Test 1: an environment reference at a value key packs ------------

    #[test]
    fn an_endpoint_slot_over_an_environment_reference_is_accepted() {
        let config = b"[backend]\nbase_url = \"${TFL_BASE_URL}\"\n";
        validate_config_slot_placeholders(config, &[endpoint_slot()]).unwrap();
    }

    #[test]
    fn the_env_colon_reference_form_is_accepted_too() {
        let config = b"[backend]\nbase_url = \"env:TFL_BASE_URL\"\n";
        validate_config_slot_placeholders(config, &[endpoint_slot()]).unwrap();
    }

    // --- Test 1b: a malformed reference is refused, naming the defect -----

    #[test]
    fn a_multi_placeholder_composition_is_refused_as_a_malformed_reference() {
        // Reference-SHAPED but resolvable by no environment: the interior
        // "name" would be `TFL_SCHEME}://${TFL_HOST`. Without the distinct
        // malformed arm this packed green and failed at every boot.
        let config = b"[backend]\nbase_url = \"${TFL_SCHEME}://${TFL_HOST}\"\n";
        let err = validate_config_slot_placeholders(config, &[endpoint_slot()]).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("malformed environment reference"),
            "the error must name the real defect, got: {msg}"
        );
        assert!(msg.contains("backend.base_url"), "must name the key: {msg}");
    }

    #[test]
    fn the_empty_brace_form_is_refused_as_a_malformed_reference_too() {
        let config = b"[backend]\nbase_url = \"${}\"\n";
        let err = validate_config_slot_placeholders(config, &[endpoint_slot()]).unwrap_err();
        assert!(
            err.to_string().contains("malformed environment reference"),
            "got: {err}"
        );
    }

    // --- Test 2: an endpoint holding a resolved literal is refused --------

    #[test]
    fn an_endpoint_slot_over_a_resolved_literal_is_refused_without_echoing_it() {
        let config = format!("[backend]\nbase_url = \"{SENTINEL_ENDPOINT}\"\n");
        let err =
            validate_config_slot_placeholders(config.as_bytes(), &[endpoint_slot()]).unwrap_err();
        assert_names_key_without_echoing(err, "backend.base_url", SENTINEL_ENDPOINT);
    }

    // --- Test 3: a credential holding a resolved literal is refused -------

    #[test]
    fn a_secret_slot_over_a_resolved_literal_is_refused_without_echoing_it() {
        let config = format!("[backend.auth.query_params]\napp_key = \"{SENTINEL_CREDENTIAL}\"\n");
        let err =
            validate_config_slot_placeholders(config.as_bytes(), &[secret_slot()]).unwrap_err();
        assert_names_key_without_echoing(
            err,
            "backend.auth.query_params.app_key",
            SENTINEL_CREDENTIAL,
        );
    }

    // --- Test 4: the auth-mode key is structurally exempt (D-17) ----------

    #[test]
    fn an_auth_mode_slot_over_a_baked_literal_is_accepted_because_it_is_structural() {
        // `AuthConfig` is internally tagged, so no placeholder form of this key
        // deserializes at all — the literal IS the only legal content.
        let config = b"[backend.auth]\ntype = \"api_key\"\n";
        validate_config_slot_placeholders(config, &[auth_mode_slot()]).unwrap();
    }

    // --- Test 4b: the auth-mode exemption is anchored, not unconditional ---

    #[test]
    fn an_auth_mode_slot_whose_config_key_resolves_to_nothing_is_a_violation() {
        // The declaration points at a key the config does not have — the same
        // "pointing at no key is a defect, not a pass" rule value slots get.
        let config = b"[backend]\nbase_url = \"${X}\"\n";
        let (key, reason) = expect_violation(
            validate_config_slot_placeholders(config, &[auth_mode_slot()]).unwrap_err(),
        );
        assert_eq!(key, "backend.auth.type");
        assert!(reason.contains("resolves to nothing"), "was: {reason}");
    }

    #[test]
    fn an_auth_mode_baked_literal_disagreeing_with_tested_value_is_refused_without_echo() {
        // The config ships `bearer` while the slot claims it was tested with
        // `api_key` — packing that records a false baseline for downstream
        // deviation classification, so it is refused. The baked discriminator
        // is document content and must not be echoed.
        let config = b"[backend.auth]\ntype = \"bearer\"\n";
        let err = validate_config_slot_placeholders(config, &[auth_mode_slot()]).unwrap_err();
        let message = err.to_string();
        assert!(message.contains("backend.auth.type"), "was: {message}");
        assert!(message.contains("`tested_value`"), "was: {message}");
        assert!(
            !message.contains("bearer"),
            "the baked literal is document content and must not be echoed; was: {message}"
        );
    }

    #[test]
    fn an_auth_mode_key_addressing_a_non_string_is_a_violation() {
        let config = b"[backend.auth]\ntype = 7\n";
        let (key, reason) = expect_violation(
            validate_config_slot_placeholders(config, &[auth_mode_slot()]).unwrap_err(),
        );
        assert_eq!(key, "backend.auth.type");
        assert!(reason.contains("non-string"), "was: {reason}");
    }

    #[test]
    fn an_auth_mode_slot_with_no_config_key_anchors_nothing_and_is_skipped() {
        // Agreement with a declaring config forces the key on; a bare package
        // slot with no key fills no config path, so there is nothing to check.
        let config = b"[backend.auth]\ntype = \"api_key\"\n";
        let slot = ConfigSlot::new(SlotType::AuthMode {
            name: "backend-auth-mode".to_string(),
            tested_value: "api_key".to_string(),
        });
        validate_config_slot_placeholders(config, &[slot]).unwrap();
    }

    // --- Test 5: config_key is conditional, not unconditionally skipped ---

    #[test]
    fn a_value_slot_with_no_config_key_is_a_violation_when_a_config_is_present() {
        let config = b"[backend]\nbase_url = \"${TFL_BASE_URL}\"\n";
        for slot in [
            ConfigSlot::new(SlotType::Secret {
                name: "TFL_APP_KEY".to_string(),
            }),
            ConfigSlot::new(SlotType::Endpoint {
                name: "TFL_BASE_URL".to_string(),
                tested_value: "https://api.tfl.gov.uk".to_string(),
            }),
        ] {
            let err =
                validate_config_slot_placeholders(config, std::slice::from_ref(&slot)).unwrap_err();
            let (key, reason) = expect_violation(err);
            assert_eq!(key, slot.slot.key().1);
            assert!(reason.contains("must name the config key"), "was: {reason}");
        }
    }

    #[test]
    fn a_non_config_slot_kind_with_no_config_key_is_skipped_not_rejected() {
        let config = b"[backend]\nbase_url = \"${TFL_BASE_URL}\"\n";
        let slots = vec![
            ConfigSlot::new(SlotType::LlmProvider {
                name: "primary".to_string(),
                tested_value: "anthropic".to_string(),
            }),
            ConfigSlot::new(SlotType::BudgetOverride {
                name: "cap".to_string(),
                tested_value: "10".to_string(),
            }),
            ConfigSlot::new(SlotType::OauthClient {
                name: "client".to_string(),
            }),
            ConfigSlot::new(SlotType::ChannelBinding {
                name: "notify".to_string(),
            }),
            ConfigSlot::new(SlotType::HumanRole {
                role: "approver".to_string(),
                description: "approves".to_string(),
                responsibilities: vec![],
                channel_hints: vec![],
            }),
        ];
        validate_config_slot_placeholders(config, &slots).unwrap();
    }

    // --- Test 6: a non-config variant that DOES carry a config_key --------

    #[test]
    fn a_non_config_slot_kind_carrying_a_config_key_is_a_violation() {
        let config = b"[backend]\nbase_url = \"${TFL_BASE_URL}\"\n";
        let cases = [
            ConfigSlot::new(SlotType::LlmProvider {
                name: "primary".to_string(),
                tested_value: "anthropic".to_string(),
            })
            .with_config_key("backend.base_url"),
            ConfigSlot::new(SlotType::HumanRole {
                role: "approver".to_string(),
                description: "approves".to_string(),
                responsibilities: vec![],
                channel_hints: vec![],
            })
            .with_config_key("backend.base_url"),
            ConfigSlot::new(SlotType::BudgetOverride {
                name: "cap".to_string(),
                tested_value: "10".to_string(),
            })
            .with_config_key("backend.base_url"),
            ConfigSlot::new(SlotType::OauthClient {
                name: "client".to_string(),
            })
            .with_config_key("backend.base_url"),
            ConfigSlot::new(SlotType::ChannelBinding {
                name: "notify".to_string(),
            })
            .with_config_key("backend.base_url"),
        ];
        for slot in cases {
            let err =
                validate_config_slot_placeholders(config, std::slice::from_ref(&slot)).unwrap_err();
            let (key, reason) = expect_violation(err);
            assert_eq!(key, "backend.base_url");
            assert!(
                reason.contains("no config-value semantics"),
                "was: {reason}"
            );
        }
    }

    // --- Test 7: a config_key naming nothing is a defect ------------------

    #[test]
    fn a_config_key_that_resolves_to_nothing_is_a_violation() {
        let config = b"[backend]\nother = \"x\"\n";
        let (key, reason) = expect_violation(
            validate_config_slot_placeholders(config, &[endpoint_slot()]).unwrap_err(),
        );
        assert_eq!(key, "backend.base_url");
        assert!(reason.contains("resolves to nothing"), "was: {reason}");
    }

    // --- Test 8: the config_key grammar, stated and enforced --------------

    #[test]
    fn every_malformed_config_key_is_a_named_violation_not_a_silent_pass() {
        let config = b"[backend]\nbase_url = \"${TFL_BASE_URL}\"\n";
        let cases: [(&str, &str); 6] = [
            ("", "is empty"),
            (".", "empty path component"),
            (".backend", "empty path component"),
            ("backend.", "empty path component"),
            ("backend..base_url", "empty path component"),
            ("backend.base_url.inner", "not a table"),
        ];
        for (config_key, expected_rule) in cases {
            let slot = ConfigSlot::new(SlotType::Endpoint {
                name: "TFL_BASE_URL".to_string(),
                tested_value: "https://api.tfl.gov.uk".to_string(),
            })
            .with_config_key(config_key);
            let (key, reason) =
                expect_violation(validate_config_slot_placeholders(config, &[slot]).unwrap_err());
            assert_eq!(key, config_key, "the error must name the offending key");
            assert!(
                reason.contains(expected_rule),
                "key {config_key:?} must state which rule it broke; was: {reason}"
            );
        }
    }

    #[test]
    fn a_quoted_or_indexed_config_key_is_rejected_rather_than_mis_resolved() {
        let config = b"[backend]\nbase_url = \"${TFL_BASE_URL}\"\n";
        for config_key in ["backend.\"base.url\"", "tools[0].path"] {
            let slot = ConfigSlot::new(SlotType::Endpoint {
                name: "N".to_string(),
                tested_value: "v".to_string(),
            })
            .with_config_key(config_key);
            let (_, reason) =
                expect_violation(validate_config_slot_placeholders(config, &[slot]).unwrap_err());
            assert!(reason.contains("bare key"), "was: {reason}");
        }
    }

    #[test]
    fn a_value_slot_key_addressing_a_non_string_is_a_violation() {
        let config = b"[backend]\nbase_url = 7\n";
        let (key, reason) = expect_violation(
            validate_config_slot_placeholders(config, &[endpoint_slot()]).unwrap_err(),
        );
        assert_eq!(key, "backend.base_url");
        assert!(reason.contains("must hold a string"), "was: {reason}");
    }

    // --- The env-reference grammar itself ---------------------------------

    #[test]
    fn is_env_reference_recognises_exactly_the_two_reference_forms() {
        assert!(is_env_reference("${TFL_BASE_URL}"));
        assert!(is_env_reference("env:TFL_BASE_URL"));
        // Malformed empty-name forms name no variable, so they are not
        // references a target environment could fill.
        assert!(!is_env_reference("${}"));
        assert!(!is_env_reference("env:"));
        // Unterminated, trailing text, plain literals, whitespace-wrapped.
        assert!(!is_env_reference("${TFL_BASE_URL"));
        assert!(!is_env_reference("${TFL_BASE_URL}-suffix"));
        // Multi-placeholder compositions and non-portable names are
        // reference-SHAPED but resolvable by no target environment.
        assert!(!is_env_reference("${TFL_SCHEME}://${TFL_HOST}"));
        assert!(!is_env_reference("${A}-${B}"));
        assert!(!is_env_reference("${TFL-HOST}"));
        // The explicit `env:` form keeps its any-non-empty-remainder rule —
        // the composition accident is a brace-form problem.
        assert!(is_env_reference("env:FOO}BAR"));
        assert!(is_malformed_env_reference("${TFL_SCHEME}://${TFL_HOST}"));
        assert!(is_malformed_env_reference("${}"));
        assert!(is_malformed_env_reference("env:"));
        assert!(!is_malformed_env_reference("https://api.tfl.gov.uk"));
        assert!(!is_malformed_env_reference("${TFL_BASE_URL}"));
        assert!(!is_env_reference("https://api.tfl.gov.uk"));
        assert!(!is_env_reference(""));
        assert!(!is_env_reference("  ${TFL_BASE_URL}  "));
    }

    // --- The real fixture passes both gates -------------------------------

    #[test]
    fn the_real_fixture_passes_placeholder_validation_for_all_three_slots() {
        validate_config_slot_placeholders(LONDON_TUBE_TOML, &london_tube_package_slots()).unwrap();
    }

    // --- Test 11: never-panic property (the FUZZ leg) ---------------------

    proptest! {
        /// FUZZ (CLAUDE.md ALWAYS): `pmcp-package` is workspace-excluded with its
        /// own `[workspace]` table, so a `cargo fuzz` target would need a second
        /// fuzz workspace outside every gate that runs today. A `proptest`
        /// never-panic property over the same newly-promoted TOML parse boundary
        /// buys the same guarantee INSIDE `make pmcp-package-gate`.
        #[test]
        fn validate_config_slot_placeholders_never_panics(
            bytes in proptest::collection::vec(any::<u8>(), 0..512),
            config_key in "\\PC{0,40}"
        ) {
            let slot = ConfigSlot::new(SlotType::Secret { name: "N".to_string() })
                .with_config_key(config_key);
            // Total on both axes: arbitrary config bytes (including non-UTF-8)
            // and arbitrary config keys (empty, dot-only, deeply dotted).
            let _ = validate_config_slot_placeholders(&bytes, &[slot]);
        }

        #[test]
        fn resolve_dotted_key_never_panics_on_arbitrary_dotted_keys(
            config_key in "[.a-zA-Z0-9_-]{0,40}"
        ) {
            let document: toml::Value =
                toml::from_str("[backend]\nbase_url = \"${X}\"\n[backend.auth]\ntype = \"t\"\n")
                    .unwrap();
            let _ = resolve_dotted_key(&document, &config_key);
        }

        /// THE COUPLING THAT NOTHING ELSE ENFORCES.
        ///
        /// `resolve_dotted_key` and `collect_undeclared_env_refs` are duals of
        /// ONE grammar with no shared code: the first parses a single
        /// user-supplied path ("is this addressable, and if not, precisely
        /// why"), the second enumerates every addressable location. They agree
        /// today on three separate facts — bare-key components (shared through
        /// `is_bare_key`), non-empty components, and tables-only-never-arrays,
        /// which is duplicated as a structural coincidence (`as_table()` on one
        /// side, a `match` arm on the other).
        ///
        /// A divergence would be silently fail-OPEN, which is the exact shape
        /// of the bug this gate exists to close: widen `resolve_dotted_key` to
        /// address array elements and a `config_key` gains reach the collector
        /// does not follow, so a `${VAR}` inside a `[[tools]]` entry becomes
        /// both slot-addressable and un-demanded. Every other test here stays
        /// green through that change.
        ///
        /// So the agreement is asserted as an executable bi-implication against
        /// a neutral referee (`naive_leaf_paths`, which descends everything):
        /// the collector reports a path IF AND ONLY IF `resolve_dotted_key` can
        /// address it AND it holds a whole-value reference.
        ///
        /// The `raw == leaf` guard on the second direction is not decoration:
        /// two distinct locations can share one dotted spelling (a quoted
        /// `"a.b"` key beside a table `a` holding `b`), and without it an alias
        /// resolving elsewhere would be read as a missed report.
        #[test]
        fn undeclared_agrees_with_what_resolve_dotted_key_can_address(
            document in arbitrary_toml_document()
        ) {
            let table = document
                .as_table()
                .expect("arbitrary_toml_document always roots in a table");
            let declared = BTreeSet::new();
            let mut found = BTreeMap::new();
            collect_undeclared_env_refs(table, "", &declared, &mut found);

            // Direction 1 — nothing over-reported: every path the collector
            // emits is one `resolve_dotted_key` resolves to that same
            // reference.
            for (path, name) in &found {
                let addressable = matches!(
                    resolve_dotted_key(&document, path),
                    Ok(toml::Value::String(raw))
                        if env_ref_name(raw).map(reportable_name) == Some(name.as_str())
                );
                prop_assert!(
                    addressable,
                    "collector emitted `{path}` -> `{name}`, which resolve_dotted_key does not \
                     address as that reference"
                );
            }

            // Direction 2 — nothing under-reported: every addressable location
            // holding a reference was emitted.
            let mut naive = Vec::new();
            naive_leaf_paths(&document, "", &mut naive);
            for (path, leaf) in naive {
                if env_ref_name(&leaf).is_none() {
                    continue;
                }
                let addresses_this_leaf = matches!(
                    resolve_dotted_key(&document, &path),
                    Ok(toml::Value::String(raw)) if *raw == leaf
                );
                if !addresses_this_leaf {
                    continue;
                }
                prop_assert!(
                    found.contains_key(&path),
                    "resolve_dotted_key addresses `{path}`, which holds the reference `{leaf}`, \
                     but the collector did not report it — the two walkers have diverged and the \
                     gate is now fail-open at that shape"
                );
            }
        }

        /// THE SUPPRESSION HALF, which the bi-implication above cannot see.
        ///
        /// `undeclared_agrees_with_what_resolve_dotted_key_can_address` runs
        /// the collector with `declared` EMPTY, so it pins which references
        /// exist and says nothing about which are forgiven. Widen the
        /// `!declared.contains(path)` test at the collector's `String` arm to
        /// match on the leaf key rather than the full dotted path — forgiving
        /// far too much — and it stays green, as do both `pack_server`-level
        /// integration tests. This is the property that fails.
        ///
        /// Declaring EXACTLY the paths the collector found must empty it, and
        /// declaring any strict subset must leave exactly the rest.
        #[test]
        fn declaring_a_path_suppresses_exactly_that_path_and_no_other(
            document in arbitrary_toml_document()
        ) {
            let table = document
                .as_table()
                .expect("arbitrary_toml_document always roots in a table");
            let mut all = BTreeMap::new();
            collect_undeclared_env_refs(table, "", &BTreeSet::new(), &mut all);

            let every: BTreeSet<&str> = all.keys().map(String::as_str).collect();
            let mut none_left = BTreeMap::new();
            collect_undeclared_env_refs(table, "", &every, &mut none_left);
            prop_assert!(
                none_left.is_empty(),
                "declaring every reported path must leave nothing, but {none_left:?} survived"
            );

            // Drop ONE declaration; exactly the dropped path must come back.
            if let Some(dropped) = all.keys().next().cloned() {
                let partial: BTreeSet<&str> = every
                    .iter()
                    .copied()
                    .filter(|path| *path != dropped)
                    .collect();
                let mut remaining = BTreeMap::new();
                collect_undeclared_env_refs(table, "", &partial, &mut remaining);
                let expected: BTreeMap<String, String> = all
                    .iter()
                    .filter(|(path, _)| **path == dropped)
                    .map(|(path, name)| (path.clone(), name.clone()))
                    .collect();
                prop_assert_eq!(
                    remaining,
                    expected,
                    "un-declaring `{}` must bring back exactly that one path",
                    dropped
                );
            }
        }

        /// FUZZ (CLAUDE.md ALWAYS), the axis that actually reaches the walk.
        ///
        /// `validate_no_undeclared_env_refs_never_panics` below drives the
        /// BYTE axis, which is totality on the parse leg and nothing more:
        /// random bytes essentially never form a content-bearing TOML
        /// document (measured over 200_000 samples of that exact strategy —
        /// ~0.4% valid UTF-8, ~0.2% parse, and every one of those the EMPTY
        /// document), so `collect_undeclared_env_refs`'s loop body runs zero
        /// times there. The empty keys, non-bare keys, arrays and nesting the
        /// walk has to survive live in `arbitrary_toml_document`, so this
        /// property feeds it that instead.
        #[test]
        fn validate_no_undeclared_env_refs_never_panics_on_arbitrary_documents(
            document in arbitrary_toml_document(),
            config_key in "\\PC{0,40}"
        ) {
            let slot = ConfigSlot::new(SlotType::Secret { name: "N".to_string() })
                .with_config_key(config_key);
            let _ = validate_no_undeclared_env_refs_in(&document, &[slot]);
            // And through the public byte entry point where the document
            // re-serializes (TOML forbids a value after a table, so some
            // generated shapes legitimately cannot round-trip).
            if let Ok(text) = toml::to_string(&document) {
                let _ = validate_no_undeclared_env_refs(text.as_bytes(), &[]);
            }
        }

        /// FUZZ, the BYTE axis: the public entry point parses before it
        /// validates, so it must be total on input that is not TOML at all.
        /// Deliberately NOT credited with covering the walk — see
        /// `validate_no_undeclared_env_refs_never_panics_on_arbitrary_documents`.
        #[test]
        fn validate_no_undeclared_env_refs_never_panics(
            bytes in proptest::collection::vec(any::<u8>(), 0..512),
            config_key in "\\PC{0,40}"
        ) {
            let slot = ConfigSlot::new(SlotType::Secret { name: "N".to_string() })
                .with_config_key(config_key);
            let _ = validate_no_undeclared_env_refs(&bytes, &[slot]);
        }

        /// The gate is EXACTLY the inverse of the forward one, so a document
        /// whose only reference sits at a declared key must always pass — for
        /// any variable name the grammar accepts and any bare-key path.
        #[test]
        fn a_declared_key_holding_any_valid_reference_always_passes(
            name in "[A-Za-z0-9_]{1,24}",
            key in "[a-z][a-z0-9_]{0,12}"
        ) {
            let document: toml::Value =
                toml::from_str(&format!("[backend]\n{key} = \"${{{name}}}\"\n")).unwrap();
            let config_key = format!("backend.{key}");
            let slot = ConfigSlot::new(SlotType::Secret { name })
                .with_config_key(&config_key);
            prop_assert!(validate_no_undeclared_env_refs_in(&document, &[slot]).is_ok());
        }

        /// The converse, and the property the reported bug violated: the SAME
        /// document with NO slot must always be refused, naming the key.
        #[test]
        fn an_undeclared_reference_is_always_refused_naming_its_key(
            name in "[A-Za-z0-9_]{1,24}",
            key in "[a-z][a-z0-9_]{0,12}"
        ) {
            let document: toml::Value =
                toml::from_str(&format!("[backend]\n{key} = \"${{{name}}}\"\n")).unwrap();
            let err = validate_no_undeclared_env_refs_in(&document, &[])
                .expect_err("an undeclared reference must never pack");
            let rendered = err.to_string();
            let expected_key = format!("backend.{key}");
            prop_assert!(rendered.contains(&expected_key));
            prop_assert!(rendered.contains(&name));
        }
    }

    // -----------------------------------------------------------------
    // Gate C — the CONFIG -> SLOT direction (undeclared environment refs)
    // -----------------------------------------------------------------

    /// The reported bug, reduced to its six-line form: a config that defers a
    /// value to the environment while declaring no slot at all. Both older
    /// gates accept it (iterating an empty slot list finds no violations),
    /// which is why the package it produced said "declares no config slots —
    /// nothing to fill" about a server that could not start without one.
    #[test]
    fn a_reference_with_no_slot_is_refused_naming_the_key_and_the_variable() {
        let config = b"[server]\nname = \"repro\"\n\n[backend]\nbase_url = \"${SOME_ENDPOINT}\"\n";
        // The gates that existed before this one both accept it.
        assert!(validate_config_slot_placeholders(config, &[]).is_ok());

        let err = validate_no_undeclared_env_refs(config, &[])
            .expect_err("a deferred value with no slot must not pack");
        let rendered = err.to_string();
        assert!(rendered.contains("backend.base_url"), "was: {rendered}");
        assert!(rendered.contains("SOME_ENDPOINT"), "was: {rendered}");
    }

    /// Declaring the slot is what makes it pack — the gate demands a
    /// declaration, never a particular value.
    #[test]
    fn a_reference_whose_slot_is_declared_is_accepted() {
        let config = b"[backend]\nbase_url = \"${TFL_BASE_URL}\"\n";
        assert!(validate_no_undeclared_env_refs(config, &[endpoint_slot()]).is_ok());
    }

    /// The `env:` form is the same grammar, so it is the same rule.
    #[test]
    fn the_env_prefixed_form_is_gated_identically_to_the_brace_form() {
        let config = b"[backend]\nbase_url = \"env:SOME_ENDPOINT\"\n";
        let err = validate_no_undeclared_env_refs(config, &[])
            .expect_err("`env:VAR` defers a value exactly as `${VAR}` does");
        assert!(err.to_string().contains("SOME_ENDPOINT"));
    }

    /// The fixture the feature was built against must stay green: all three of
    /// its slots are declared, so it has nothing undeclared. This is the
    /// regression guard for the gap's own cause — a fully self-consistent
    /// corpus that exercised only the direction that was gated.
    #[test]
    fn the_real_fixture_has_no_undeclared_references() {
        assert!(
            validate_no_undeclared_env_refs(LONDON_TUBE_TOML, &london_tube_package_slots()).is_ok()
        );
    }

    /// THE BOUNDARY THAT KEEPS THE GATE OFF ITS OWN FIXTURE. `london-tube.toml`
    /// carries two `${...}` occurrences that are JS TEMPLATE placeholders in a
    /// `[[tools]].script` and a `[[resources]].content` — a different `${}`
    /// namespace entirely. They live inside arrays of tables, which the dotted
    /// `config_key` grammar cannot address, so no slot could ever be written
    /// for them. A naive document-wide text scan would have flagged this
    /// crate's own golden fixture; this asserts the structural reason it does
    /// not.
    #[test]
    fn a_reference_inside_an_array_of_tables_is_not_demanded() {
        let config = br#"
[[tools]]
name = "t"
script = "await api.get(`/Line/${line.id}/Disruption`)"

[[tools]]
name = "whole-value"
script = "${NOT_ADDRESSABLE}"
"#;
        assert!(
            validate_no_undeclared_env_refs(config, &[]).is_ok(),
            "array elements are unaddressable by the dotted key grammar, so demanding a slot \
             for one would be a demand no config author could satisfy"
        );
    }

    /// The pinned grammar accepts a reference only as the WHOLE value
    /// (`env_ref_grammar_v1.tsv` rejects `${A}-${B}` and `${VAR}-suffix`), so
    /// an EMBEDDED reference is not something a slot can fill and this gate
    /// does not pretend otherwise. Documented rather than silently true: it is
    /// the one shape from the original report this gate cannot catch.
    #[test]
    fn an_embedded_reference_is_not_demanded_because_no_slot_could_fill_it() {
        let config = b"[backend]\nbase_url = \"${SCHEME}://${HOST}\"\nother = \"${VAR}-suffix\"\n";
        assert!(validate_no_undeclared_env_refs(config, &[]).is_ok());
    }

    /// ERROR HYGIENE (T-120-21). The `env:` form's grammar accepts ANY
    /// non-empty remainder, so the "variable name" this gate would otherwise
    /// echo is arbitrary document text. An author who writes
    /// `api_key = "env:sk-live-…"` believing `env:` is an encoding prefix must
    /// NOT have the credential printed by `cargo pmcp package save` and
    /// captured in CI logs. The key still says where the fix goes.
    #[test]
    fn an_env_prefixed_name_that_is_not_a_settable_variable_is_not_echoed() {
        let config = b"[backend]\napi_key = \"env:sk-live-DEADBEEF secret text\"\n";
        let (key, reason) =
            expect_violation(validate_no_undeclared_env_refs(config, &[]).unwrap_err());
        assert_eq!(key, "backend.api_key");
        assert!(
            !reason.contains("sk-live-DEADBEEF"),
            "the credential must never reach the message: {reason}"
        );
        assert!(reason.contains(UNREPORTABLE_NAME), "was: {reason}");
    }

    /// The same rule blocks message SPOOFING. `undeclared_reason` renders the
    /// violations as one comma-separated list, so an `env:` remainder holding
    /// `, ` would otherwise forge an extra entry — a second offending key that
    /// does not exist.
    #[test]
    fn an_env_prefixed_name_cannot_forge_extra_entries_in_the_list() {
        let config = b"[backend]\napi_key = \"env:A, backend.forged -> B\"\n";
        let (_, reason) =
            expect_violation(validate_no_undeclared_env_refs(config, &[]).unwrap_err());
        assert!(
            !reason.contains("backend.forged"),
            "a config value must not be able to fabricate a listed key: {reason}"
        );
    }

    /// A well-formed `env:NAME` still names its variable — the withholding is
    /// scoped to names an environment could not be told to set, not to the
    /// `env:` form as a whole.
    #[test]
    fn a_well_formed_env_prefixed_name_is_still_reported() {
        let config = b"[backend]\napi_key = \"env:TFL_APP_KEY\"\n";
        let (_, reason) =
            expect_violation(validate_no_undeclared_env_refs(config, &[]).unwrap_err());
        assert!(reason.contains("TFL_APP_KEY"), "was: {reason}");
    }

    /// The listed-keys prefix is BOUNDED: unbounded, a hostile or generated
    /// config controls both the length and the content of an error the CLI
    /// prints and callers log (measured: 2000 references produced a 74 KB
    /// message).
    #[test]
    fn the_listed_keys_are_capped_and_the_remainder_is_counted() {
        let mut config = String::from("[backend]\n");
        let total = MAX_LISTED_UNDECLARED + 5;
        for i in 0..total {
            config.push_str(&format!("k{i:03} = \"${{VAR{i:03}}}\"\n"));
        }
        let (_, reason) =
            expect_violation(validate_no_undeclared_env_refs(config.as_bytes(), &[]).unwrap_err());
        assert!(reason.contains("and 5 more"), "was: {reason}");
        assert!(
            !reason.contains(&format!("VAR{:03}", total - 1)),
            "the tail must be elided, not spelled out: {reason}"
        );
    }

    /// Malformed whole-value references are a different defect with a
    /// different fix — repair the reference, not declare a slot — so they are
    /// out of this gate's scope and stay the forward gate's business.
    #[test]
    fn a_malformed_whole_value_reference_is_out_of_scope() {
        let config = b"[backend]\nbase_url = \"${}\"\napi_key = \"env:\"\n";
        assert!(validate_no_undeclared_env_refs(config, &[]).is_ok());
    }

    /// A resolved literal is not a reference, so the gate is silent about it —
    /// baked credentials are the forward gate's job, and firing here would
    /// risk putting a value in a message.
    #[test]
    fn a_resolved_literal_never_triggers_the_gate() {
        let config = b"[backend]\nbase_url = \"https://api.tfl.gov.uk\"\n";
        assert!(validate_no_undeclared_env_refs(config, &[]).is_ok());
    }

    /// A config with several undeclared references is fixed in ONE pass: the
    /// error names the lexicographically first key and lists every one of
    /// them. Deterministic regardless of the TOML map's iteration order,
    /// because the walk collects into a `BTreeMap`.
    #[test]
    fn every_undeclared_reference_is_listed_in_one_error() {
        let config = br#"
[backend]
base_url = "${BT_ENDPOINT}"

[backend.auth]
client_id = "${BT_CLIENT_ID}"
client_secret = "${BT_CLIENT_SECRET}"

[code_mode]
token_secret = "${CODE_MODE_SECRET}"
"#;
        let err = validate_no_undeclared_env_refs(config, &[]).unwrap_err();
        let (key, reason) = expect_violation(err);
        assert_eq!(
            key, "backend.auth.client_id",
            "first in lexicographic order"
        );
        for expected in [
            "backend.auth.client_id -> BT_CLIENT_ID",
            "backend.auth.client_secret -> BT_CLIENT_SECRET",
            "backend.base_url -> BT_ENDPOINT",
            "code_mode.token_secret -> CODE_MODE_SECRET",
        ] {
            assert!(reason.contains(expected), "missing {expected} in: {reason}");
        }
    }

    /// Declaring SOME of them is not enough — the ones left over are still
    /// reported, and the satisfied one is not.
    #[test]
    fn a_partially_declared_config_reports_only_what_is_still_missing() {
        let config = b"[backend]\nbase_url = \"${TFL_BASE_URL}\"\napi_key = \"${TFL_APP_KEY}\"\n";
        let err = validate_no_undeclared_env_refs(config, &[endpoint_slot()]).unwrap_err();
        let rendered = err.to_string();
        assert!(rendered.contains("backend.api_key"), "was: {rendered}");
        assert!(
            !rendered.contains("backend.base_url"),
            "the declared key must not be reported: {rendered}"
        );
    }

    /// An inline table is a table, so the gate reaches through it — this is
    /// the shape the real fixture's `query_params = { app_key = "${...}" }`
    /// uses, and missing it would leave the secret path ungated.
    #[test]
    fn the_gate_reaches_through_an_inline_table() {
        let config = b"[backend.auth]\nquery_params = { app_key = \"${TFL_APP_KEY}\" }\n";
        let err = validate_no_undeclared_env_refs(config, &[]).unwrap_err();
        assert!(err
            .to_string()
            .contains("backend.auth.query_params.app_key"));
        assert!(validate_no_undeclared_env_refs(config, &[secret_slot()]).is_ok());
    }

    /// A key whose literal name contains a dot is unaddressable by the dotted
    /// grammar (`resolve_dotted_key` says so explicitly), so it is skipped for
    /// the same reason array elements are.
    #[test]
    fn a_non_bare_key_is_skipped_as_unaddressable() {
        let config = b"[backend]\n\"my.key\" = \"${SOME_VAR}\"\n";
        assert!(validate_no_undeclared_env_refs(config, &[]).is_ok());
    }

    /// Every accept row of the pinned grammar must yield its variable NAME, and
    /// every reject row `None`.
    ///
    /// Deliberately asserts `env_ref_name` ONLY. `is_env_reference` is now
    /// `env_ref_name(raw).is_some()`, so an `assert_eq!(is_env_reference(raw),
    /// expected.is_some())` beside the assertion below would expand to
    /// `env_ref_name(raw).is_some() == expected.is_some()` — strictly implied by
    /// it, and unable to fail on its own. The predicate is covered
    /// INDEPENDENTLY by `tests/config_server.rs`'s
    /// `is_env_reference_agrees_with_the_shared_grammar_table_on_every_row`,
    /// which reads the `.tsv` rather than this inlined copy.
    #[test]
    fn env_ref_name_yields_the_variable_for_every_accept_row() {
        for (raw, expected) in [
            ("${TFL_BASE_URL}", Some("TFL_BASE_URL")),
            ("env:TFL_APP_KEY", Some("TFL_APP_KEY")),
            ("${A}", Some("A")),
            ("env:A", Some("A")),
            ("env:FOO}BAR", Some("FOO}BAR")),
            ("https://api.tfl.gov.uk", None),
            ("the price is $5 per call", None),
            ("${}", None),
            ("env:", None),
            ("${TFL_BASE_URL", None),
            ("${TFL_BASE_URL}-suffix", None),
            ("  ${TFL_BASE_URL}  ", None),
            ("${TFL_SCHEME}://${TFL_HOST}", None),
            ("${A}-${B}", None),
            ("${TFL-HOST}", None),
            ("", None),
        ] {
            assert_eq!(env_ref_name(raw), expected, "env_ref_name({raw:?})");
        }
    }
}
