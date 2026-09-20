//! The ONE human-text renderer the local read verbs share.
//!
//! `package load` reaches this file today; plan 05's `package pull` reaches the
//! same file through the library mount described below. Everything either verb
//! prints about a package it has just read is produced here, so the two cannot
//! drift into two different reports.
//!
//! # This file is compiled TWICE, from one source
//!
//! Genuinely surprising, so it is stated plainly rather than left to be
//! discovered:
//!
//! - into the BIN target as `commands::package::render`, which is how
//!   `load.rs` reaches it (`use super::render`);
//! - into the LIB target as `cargo_pmcp::package_render`, which is how this
//!   module's own unit tests, every file under `cargo-pmcp/tests/`, and plan
//!   05's lib-mounted pull pipeline reach it.
//!
//! That is TWO COMPILATIONS OF ONE SOURCE, not two implementations — the same
//! shape `kind.rs` and `artifact.rs` already live in. Do NOT "fix" it by having
//! one call the other across the lib/bin boundary: to the compiler the two
//! copies' types are distinct even though the source is identical. What proves
//! the two agree is behavioural, not structural — plan 05 asserts that `pull`
//! and `load` emit byte-identical reports.
//!
//! The mount is what makes this module testable at all. `cargo-pmcp/src/lib.rs`
//! declares no `commands` module, so a module declared ONLY in
//! `commands/package/mod.rs` exists in the bin tree and nowhere else — invisible
//! to `cargo test --lib` and unreachable from any integration test. Two
//! consequences follow and both are load-bearing: this file must stay
//! dependency-light — `pmcp_package` types and `std` ONLY, never the argument
//! parser, never the command layer's global-flags type, never anything under
//! `crate::commands` — and it must never name `super::`. The exact prohibition
//! is spelled out at the mount in `cargo-pmcp/src/lib.rs`; it is deliberately
//! not repeated verbatim here, because the acceptance grep that enforces it
//! scans THIS file for those symbols and would otherwise match its own
//! prohibition text instead of a real dependency.
//!
//! # One output shape, matching `inspect`'s visual vocabulary
//!
//! `inspect.rs`'s module header states the principle this module inherits: a
//! reader should never have to learn two output shapes. So there is exactly one
//! rendering here and no second machine-readable flag — a machine-readable
//! rendering is a clean follow-on if a consumer ever asks for one, not a surface
//! shipped on speculation.
//!
//! `inspect.rs`'s own renderers are deliberately NOT moved here. `inspect` is
//! shipped and its output is a surface people already read; this module matches
//! its VISUAL grammar instead — the same two-space indent, the same fixed-width
//! `label:` column, the same section headers — so a reader moving between
//! `inspect` and `load` sees one house style.
//!
//! **Colour is deliberately absent**, and that is the one place the visual match
//! is imperfect. Every function here returns a `String` rather than printing,
//! which is what makes the determinism property testable and what lets `load`
//! and `pull` gate printing on their own quiet flag without duplicating any
//! layout logic. A `String` carrying ANSI escapes would make that determinism
//! depend on whether stdout happened to be a terminal, i.e. on the environment
//! rather than on the inputs — so the escapes are simply not emitted.

use std::fmt::Write as _;

use pmcp_package::oci::UnpackedAttestation;
use pmcp_package::reference::ComponentType;
use pmcp_package::{
    classify_slots, required_slots, ComponentRef, ConfigSlot, SlotClass, SlotType, SuppliedBy,
};

/// Width of the `label:` column, matching `inspect.rs`'s `field` helper so the
/// two commands line up visually.
///
/// `pub(crate)` so `inspect.rs` READS this one rather than repeating the
/// literal: the coupling above is deliberate, and while it existed as two
/// independent `14`s nothing failed when one of them moved — no test compares
/// `inspect`'s output to `load`'s (the only cross-verb pin, in
/// `package_portability_contract.rs`, compares `pull` to `load`, and both of
/// those come through THIS file).
pub(crate) const LABEL_WIDTH: usize = 14;

/// Maximum rendered length of an ATTACKER-CONTROLLED string.
///
/// 72 rather than a round number for a measured reason: a well-formed
/// `sha256:<64 hex>` subject is exactly 71 characters, so a legitimate claim is
/// never clipped while a hostile annotation carrying megabytes cannot flood the
/// terminal.
const UNTRUSTED_MAX: usize = 72;

/// Everything the report renders about one package, in primitive terms.
///
/// Deliberately built from `pmcp_package` types and `&str` only — never from a
/// command-layer type. `load` (bin) and `pull` (lib) each assemble one of these
/// from whatever their own unpack step produced, and hand it to
/// [`render_report`]. That is the seam that makes ONE renderer serve both verbs
/// rather than two renderers that merely look alike.
#[derive(Debug)]
pub struct PackageReport<'a> {
    /// The package kind's lowercase label (`agent`/`team`/`server`/`workflow`).
    pub kind: &'a str,
    /// The package's declared name.
    pub name: &'a str,
    /// The package's declared version.
    pub version: &'a str,
    /// The package's identity digest, derived locally over the manifest blob.
    pub digest: &'a str,
    /// Where the layout was materialized, rendered verbatim.
    pub destination: &'a str,
    /// Every config slot the package declares.
    pub slots: &'a [ConfigSlot],
    /// Every component reference the package holds. Empty for a server
    /// package, which has no `ComponentRef` field at all.
    pub components: &'a [ComponentRef],
    /// The platform-issued attestation, if the package carried one.
    pub attestation: Option<&'a UnpackedAttestation>,
}

/// Render the whole report for one loaded package.
///
/// The single entry point both verbs call, so byte-identical inputs produce
/// byte-identical output on either side by construction.
#[must_use]
pub fn render_report(report: &PackageReport<'_>) -> String {
    let mut out = String::new();
    out.push_str(&render_identity(report));
    out.push_str(&render_required_slots(report.slots));
    out.push_str(&render_host_supplied_slots(report.slots));
    out.push_str(&render_component_pins(report.components));
    out.push_str(&render_carriage(report.attestation));
    out
}

/// Render the package's own identity block: kind, name, version, digest and
/// where it landed.
#[must_use]
pub fn render_identity(report: &PackageReport<'_>) -> String {
    let mut out = String::new();
    section(&mut out, "Package");
    field(&mut out, "  ", "Kind", report.kind);
    field(&mut out, "  ", "Name", &untrusted(report.name));
    field(&mut out, "  ", "Version", &untrusted(report.version));
    // The package's IDENTITY, derived locally over the manifest blob's own
    // bytes — never read out of the archive and called derived.
    field(&mut out, "  ", "Digest", report.digest);
    field(&mut out, "  ", "Layout", report.destination);
    out
}

/// Render the inventory of slots a target environment must fill.
///
/// The enumerator is [`required_slots`], and that choice is not
/// interchangeable. `detect_deviation` — the neighbouring function a reader may
/// reach for — answers a DIFFERENT question: it compares one already-known
/// tested/proposed pair and returns `None` for every identity-bearing slot by
/// design, which makes it structurally incapable of ever naming a credential.
/// This rendering asks the ENUMERATION question ("what must the target
/// environment supply?"), so it must use the enumerator. Do not "improve" this
/// by switching.
///
/// # Two strings that are never the same string
///
/// A slot's `name` is the ENVIRONMENT VARIABLE the target environment sets;
/// `config_key` is the dotted CONFIG PATH the resolved value is written to.
/// They are labelled distinctly and never substituted for one another — a
/// variable name derived from a config path (`BACKEND.BASE_URL`) is one no
/// environment can portably set.
///
/// # Ordering
///
/// Exactly the order [`required_slots`] returns: by `SlotType::key()`, with a
/// stable sort that preserves the relative order of equal-keyed duplicates.
/// Nothing is re-sorted here.
#[must_use]
pub fn render_required_slots(slots: &[ConfigSlot]) -> String {
    let mut out = String::new();
    section(&mut out, "Required slots");

    let required = required_slots(slots);
    if required.is_empty() {
        let _ = writeln!(
            out,
            "  This package declares no config slots — nothing to fill."
        );
        return out;
    }

    let _ = writeln!(
        out,
        "  The target environment must supply a value for each entry below."
    );
    for (position, entry) in required.iter().enumerate() {
        let (kind, name) = entry.slot.key();
        let class = match entry.class {
            SlotClass::IdentityBearing => "identity-bearing (a credential or binding)",
            SlotClass::BehaviorRelevant => "behavior-relevant (changes what the tools do)",
        };
        let _ = writeln!(out, "\n  [{}] {kind}", position + 1);
        // A `HumanRole` names a ROLE a person fills, not a variable any
        // environment can set, so it is labelled honestly rather than forced
        // into the environment-variable column.
        let name_label = match entry.slot {
            SlotType::HumanRole { .. } => "Role",
            _ => "Env var",
        };
        field(&mut out, "      ", name_label, &untrusted(name));
        field(&mut out, "      ", "Class", class);
        match entry.config_key.as_deref() {
            Some(key) => field(&mut out, "      ", "Config path", &untrusted(key)),
            // Explicitly stated rather than left blank: an empty value reads as
            // a missing render, whereas "fills no config key" is a fact.
            None => field(&mut out, "      ", "Config path", "fills no config key"),
        }
        // Rendered ONLY for the behavior-relevant family. An identity-bearing
        // slot has no value field in the type at all, so there is nothing here
        // to leak — the absence is structural, not a filter that could be
        // forgotten.
        if let Some(tested) = entry.slot.tested_value() {
            field(&mut out, "      ", "Tested value", &untrusted(tested));
        }
    }
    out
}

/// Render the slots the HOST or RUNTIME injects — the ones no operator fills.
///
/// # Why this section exists
///
/// [`required_slots`] deliberately excludes these: asking an operator to supply
/// a value the platform injects is wrong, and a package that demanded them
/// would be unfillable. But excluding them from the DEMAND must not exclude
/// them from the RECORD. A slot nobody is asked for and nothing displays is
/// invisible, and a reader cannot tell "this package needs nothing here" from
/// "this package never said" — which is precisely the ambiguity
/// [`SuppliedBy`] was introduced to remove.
///
/// So this section is not decoration; it is the other half of the filter. It is
/// omitted entirely when there is nothing to report, so an ordinary
/// all-operator-supplied package renders exactly as it did before.
///
/// # Complement by construction
///
/// Both sections are built from one [`classify_slots`] list split on one
/// predicate, so every declared slot appears in exactly one of them. This is
/// deliberately NOT a second independent filter over `slots` — two predicates
/// could drift apart, and a slot that fell out of both would be invisible in
/// the exact way this section exists to prevent.
///
/// # Ordering
///
/// Exactly [`classify_slots`]' order, which is `required_slots`' order. Nothing
/// is re-sorted here.
#[must_use]
pub fn render_host_supplied_slots(slots: &[ConfigSlot]) -> String {
    let mut out = String::new();

    let host_supplied: Vec<_> = classify_slots(slots)
        .into_iter()
        .filter(|entry| !entry.supplied_by.is_operator_supplied())
        .collect();

    // Nothing to say: omit the heading rather than print an empty section, so a
    // package with no host-supplied slots renders byte-identically to before.
    if host_supplied.is_empty() {
        return out;
    }

    section(&mut out, "Supplied by the host at deploy time");
    let _ = writeln!(
        out,
        "  Listed for the record — no operator action is required for these."
    );
    for (position, entry) in host_supplied.iter().enumerate() {
        let (kind, name) = entry.slot.key();
        let source = match entry.supplied_by {
            SuppliedBy::Platform => "platform (injected by the host at deploy time)",
            SuppliedBy::Runtime => "runtime (injected by the execution environment)",
            // `SuppliedBy` is `#[non_exhaustive]`, so this arm is required.
            // `Environment` cannot reach it (filtered out above), which leaves
            // only a variant added after this code was written. Say exactly
            // that rather than guessing it resembles one of the two above — a
            // wrong label here would misreport who fills a slot.
            _ => "an unrecognized source — this package may need a newer CLI",
        };
        let _ = writeln!(out, "\n  [{}] {kind}", position + 1);
        let name_label = match entry.slot {
            SlotType::HumanRole { .. } => "Role",
            _ => "Env var",
        };
        field(&mut out, "      ", name_label, &untrusted(name));
        field(&mut out, "      ", "Supplied by", source);
        match entry.config_key.as_deref() {
            Some(key) => field(&mut out, "      ", "Config path", &untrusted(key)),
            None => field(&mut out, "      ", "Config path", "fills no config key"),
        }
        // Shown because `supplied_by` is ORTHOGONAL to `kind`: a
        // platform-supplied endpoint stays deviation-visible, and
        // `detect_deviation` compares against exactly this value. Omitting it
        // would hide the baseline of a comparison the package still makes.
        // Identity-bearing slots structurally have none, so nothing prints.
        if let Some(tested) = entry.slot.tested_value() {
            field(&mut out, "      ", "Tested value", &untrusted(tested));
        }
    }
    out
}

/// Render what THIS PACKAGE records about each of its component references.
///
/// # Three states, never two
///
/// - a `Range` was declared and is not resolved in this package;
/// - a `Pinned` whose `resolved_from` is `Some(range)` shows the declared range
///   alongside the resolved version and digest;
/// - a `Pinned` whose `resolved_from` is `None` was pinned directly, and the
///   declared range CANNOT BE REPORTED.
///
/// That third state is not a formatting nicety. `PinnedRef::resolved_from`'s
/// own documentation names this phase and states the obligation verbatim:
///
/// > Anything building skew reporting on this field — Phase 123's dev-to-prod
/// > import check is the named one — MUST treat `None` as "cannot report" and
/// > NEVER as "no skew". Reading an absent fact as a positive claim is precisely
/// > the failure this field exists to prevent.
///
/// # Ordering
///
/// Sorted by component NAME, with component TYPE as the tiebreak (`server <
/// agent < team`, the declaration order `ComponentType`'s `Ord` derives). The
/// sort is stable, so two references agreeing on both keys keep their relative
/// input order.
#[must_use]
pub fn render_component_pins(components: &[ComponentRef]) -> String {
    let mut out = String::new();
    if components.is_empty() {
        // A server package has no `ComponentRef` field at all, so the section
        // is genuinely inapplicable rather than empty. Rendering an empty
        // heading would invite the reader to wonder what was omitted.
        return out;
    }
    section(&mut out, "Component pins");
    let _ = writeln!(
        out,
        "  What THIS PACKAGE records about each component it references."
    );
    // The boundary D-14 draws, stated in the output rather than only in a
    // decision record: an operator who does not see it will assume the CLI
    // checked something it structurally cannot check, because the comparison
    // needs deployed-state knowledge nothing offline has.
    let _ = writeln!(
        out,
        "  Comparing these against what a target environment actually runs is\n  \
         `import`'s job, platform-side. Nothing below was derived from any\n  \
         environment — only from this package."
    );

    let mut ordered: Vec<&ComponentRef> = components.iter().collect();
    // Stable, so two references agreeing on BOTH keys keep their input order.
    ordered.sort_by(|a, b| {
        a.name()
            .cmp(b.name())
            .then(a.component_type().cmp(&b.component_type()))
    });

    for (position, component) in ordered.iter().enumerate() {
        let _ = writeln!(
            out,
            "\n  [{}] {} {}",
            position + 1,
            component_type_label(component.component_type()),
            untrusted(component.name())
        );
        match component {
            ComponentRef::Range { range, .. } => {
                field(&mut out, "      ", "Declared", &range.to_string());
                field(
                    &mut out,
                    "      ",
                    "Resolved",
                    "not resolved in this package",
                );
            },
            ComponentRef::Pinned(pinned) => {
                match &pinned.resolved_from {
                    Some(range) => field(&mut out, "      ", "Declared", &range.to_string()),
                    // The third state, and the reason this renderer exists.
                    // Reading an absent fact as a positive claim is precisely
                    // the failure `resolved_from` was added to prevent, so the
                    // wording says what is UNKNOWN and says nothing about
                    // whether the pin and the declaration agree.
                    None => field(
                        &mut out,
                        "      ",
                        "Declared",
                        "CANNOT REPORT — pinned directly, so this package records no \
                         declared range",
                    ),
                }
                field(&mut out, "      ", "Resolved", &pinned.version.to_string());
                field(&mut out, "      ", "Digest", pinned.digest.as_str());
            },
        }
    }
    out
}

/// The lowercase label for a component type, for the pin section's headings.
fn component_type_label(component_type: ComponentType) -> &'static str {
    match component_type {
        ComponentType::Server => "server",
        ComponentType::Agent => "agent",
        ComponentType::Team => "team",
    }
}

/// Render what the package carries by way of an attestation — all three states.
///
/// The verdict is read from `SubjectVerdict::matches()` and never re-derived
/// here: a CLI-side digest comparison would be a second implementation, free to
/// drift from `inspect`'s.
///
/// `claimed`, `issuer` and `payload_type` are ATTACKER-CONTROLLED strings read
/// off layer annotations. They are rendered as bounded, escaped DATA and are
/// never joined onto a path or interpreted.
#[must_use]
pub fn render_carriage(attestation: Option<&UnpackedAttestation>) -> String {
    let mut out = String::new();
    section(&mut out, "Attestation");
    let Some(attestation) = attestation else {
        // Said explicitly rather than rendered as silence, so "carries no
        // attestation" can never be confused with "this build does not know
        // about attestations".
        field(&mut out, "  ", "Carriage", "none (package is unattested)");
        return out;
    };
    field(&mut out, "  ", "Issuer", &untrusted(&attestation.issuer));
    field(
        &mut out,
        "  ",
        "Subject",
        &untrusted(&attestation.subject.claimed),
    );
    field(
        &mut out,
        "  ",
        "Payload type",
        &untrusted(&attestation.payload_type),
    );
    // Read from `SubjectVerdict::matches()`. A CLI-side digest comparison would
    // be a second implementation of the same rule, free to drift from
    // `inspect`'s — this asks the crate that derived the digest.
    if attestation.subject.matches() {
        field(&mut out, "  ", "Verdict", "subject matches this package");
    } else {
        field(
            &mut out,
            "  ",
            "Verdict",
            "SUBJECT MISMATCH — this attestation is not about this package",
        );
        // The claim and the reality side by side. That juxtaposition IS the
        // diagnostic; either line alone tells the operator nothing.
        field(
            &mut out,
            "  ",
            "Actual",
            attestation.subject.unattested_digest.as_str(),
        );
    }
    out
}

/// Append a section header, matching `inspect.rs`'s `header` shape.
fn section(out: &mut String, title: &str) {
    let _ = writeln!(out, "\n{title}");
}

/// Append a `  label:        value` line, matching `inspect.rs`'s `field` shape.
fn field(out: &mut String, indent: &str, label: &str, value: &str) {
    let _ = writeln!(
        out,
        "{indent}{:<width$} {value}",
        format!("{label}:"),
        width = LABEL_WIDTH
    );
}

/// Bound and neutralize an untrusted string before it reaches a terminal.
///
/// Two things happen, and both matter. Length is clipped to [`UNTRUSTED_MAX`]
/// so a hostile annotation cannot flood the output. Control characters —
/// including ESC — are rendered as escapes rather than emitted, because an ANSI
/// sequence smuggled through an annotation could otherwise repaint the terminal
/// and forge a verdict line the renderer never wrote.
pub(crate) fn untrusted(s: &str) -> String {
    let mut out = String::with_capacity(s.len().min(UNTRUSTED_MAX));
    let mut clipped = false;
    for (taken, ch) in s.chars().enumerate() {
        if taken == UNTRUSTED_MAX {
            clipped = true;
            break;
        }
        if ch.is_control() {
            let _ = write!(out, "\\u{{{:04x}}}", ch as u32);
        } else {
            out.push(ch);
        }
    }
    if clipped {
        out.push('…');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use pmcp_package::oci::SubjectVerdict;
    use pmcp_package::{ManifestDigest, PinnedRef};

    fn secret_slot() -> ConfigSlot {
        ConfigSlot::new(SlotType::Secret {
            name: "TFL_APP_KEY".to_string(),
        })
        .with_config_key("backend.auth.query_params.app_key")
    }

    fn endpoint_slot() -> ConfigSlot {
        ConfigSlot::new(SlotType::Endpoint {
            name: "TFL_BASE_URL".to_string(),
            tested_value: "https://api.tfl.gov.uk".to_string(),
        })
        .with_config_key("backend.base_url")
    }

    fn digest_of(seed: &[u8]) -> ManifestDigest {
        ManifestDigest::from_bytes(seed)
    }

    fn range_ref() -> ComponentRef {
        ComponentRef::Range {
            name: "triage-agent".to_string(),
            range: semver::VersionReq::parse("^1.2").unwrap(),
            component_type: ComponentType::Agent,
        }
    }

    fn pinned_with_range() -> ComponentRef {
        ComponentRef::Pinned(PinnedRef {
            name: "london-tube".to_string(),
            component_type: ComponentType::Server,
            version: semver::Version::parse("1.3.0").unwrap(),
            digest: digest_of(b"london-tube-1.3.0"),
            resolved_from: Some(semver::VersionReq::parse("^1.2").unwrap()),
        })
    }

    fn pinned_without_range() -> ComponentRef {
        ComponentRef::Pinned(PinnedRef {
            name: "support-team".to_string(),
            component_type: ComponentType::Team,
            version: semver::Version::parse("2.0.0").unwrap(),
            digest: digest_of(b"support-team-2.0.0"),
            resolved_from: None,
        })
    }

    fn attestation(claimed: String, unattested: ManifestDigest) -> UnpackedAttestation {
        UnpackedAttestation {
            bytes: b"opaque".to_vec(),
            subject: SubjectVerdict {
                claimed,
                unattested_digest: unattested,
            },
            issuer: "https://issuer.test.invalid/pmcp-run".to_string(),
            payload_type: "application/vnd.test.attestation-payload".to_string(),
        }
    }

    /// R1.2: a host-supplied slot is EXCLUDED from "Required slots" but still
    /// appears, under its own heading, naming who supplies it.
    ///
    /// This is the pair of assertions that matters: absent from the demand,
    /// present in the record. Asserting only the first would pass on the
    /// invisibility bug.
    #[test]
    fn a_host_supplied_slot_leaves_the_required_list_but_is_still_rendered() {
        let platform_endpoint = endpoint_slot().with_supplied_by(SuppliedBy::Platform);
        let slots = vec![secret_slot(), platform_endpoint];

        let required = render_required_slots(&slots);
        let host = render_host_supplied_slots(&slots);

        // Absent from the demand: the operator is not asked for it.
        assert!(
            !required.contains("TFL_BASE_URL"),
            "a platform-injected slot must not be demanded of the operator: {required}"
        );
        assert!(required.contains("TFL_APP_KEY"), "{required}");

        // Present in the record, and attributed.
        assert!(
            host.contains("Supplied by the host at deploy time"),
            "the section heading is missing: {host}"
        );
        assert!(
            host.contains("TFL_BASE_URL"),
            "the excluded slot vanished entirely — this is the invisibility bug: {host}"
        );
        assert!(host.contains("Supplied by:"), "{host}");
        assert!(host.contains("platform"), "{host}");
    }

    /// The section is omitted entirely when nothing is host-supplied, so an
    /// ordinary package's report is unchanged by this feature.
    #[test]
    fn an_all_operator_supplied_package_renders_no_host_section() {
        let rendered = render_host_supplied_slots(&[secret_slot(), endpoint_slot()]);
        assert!(
            rendered.is_empty(),
            "an all-operator-supplied package must render no extra section: {rendered}"
        );
    }

    /// Every declared slot reaches exactly one of the two sections. A slot in
    /// neither is invisible; a slot in both would be double-counted.
    #[test]
    fn the_two_sections_together_account_for_every_slot() {
        let slots = vec![
            secret_slot(),
            endpoint_slot().with_supplied_by(SuppliedBy::Platform),
        ];
        let required = render_required_slots(&slots);
        let host = render_host_supplied_slots(&slots);

        for name in ["TFL_APP_KEY", "TFL_BASE_URL"] {
            let in_required = required.contains(name);
            let in_host = host.contains(name);
            assert!(
                in_required ^ in_host,
                "{name} must appear in exactly one section \
                 (required={in_required}, host={in_host})"
            );
        }
    }

    /// Behavior 1: one line per required slot, with the ENVIRONMENT VARIABLE
    /// and the CONFIG PATH under DIFFERENT labels — and a secret slot carrying
    /// no value, because an identity-bearing slot structurally has none.
    #[test]
    fn a_required_slot_renders_its_variable_and_config_path_under_different_labels() {
        let rendered = render_required_slots(&[secret_slot(), endpoint_slot()]);

        assert!(
            rendered.contains("Env var:"),
            "the variable name needs its own label: {rendered}"
        );
        assert!(
            rendered.contains("Config path:"),
            "the config path needs its own label: {rendered}"
        );
        assert!(rendered.contains("TFL_APP_KEY"), "{rendered}");
        assert!(
            rendered.contains("backend.auth.query_params.app_key"),
            "{rendered}"
        );

        // The two strings must not be conflated: the config path must never be
        // rendered where the variable name belongs.
        for line in rendered.lines() {
            if line.contains("Env var:") {
                assert!(
                    !line.contains('.'),
                    "a dotted config path was rendered as a variable name: {line}"
                );
            }
        }

        // The secret slot's block carries no value at all.
        let secret_block = rendered
            .split("Env var:")
            .find(|chunk| chunk.contains("TFL_APP_KEY"))
            .expect("the secret slot is rendered");
        assert!(
            !secret_block.contains("Tested value:"),
            "an identity-bearing slot must render no value: {secret_block}"
        );
    }

    /// Behavior 2: a declared range says it was declared and is unresolved.
    #[test]
    fn a_declared_range_renders_as_declared_but_unresolved() {
        let rendered = render_component_pins(&[range_ref()]);
        assert!(rendered.contains("triage-agent"), "{rendered}");
        assert!(rendered.contains("^1.2"), "{rendered}");
        assert!(
            rendered.contains("not resolved"),
            "a range must say it is unresolved: {rendered}"
        );
    }

    /// Behavior 3: a pin that recorded its range shows BOTH the declared range
    /// and the resolved version plus digest.
    #[test]
    fn a_pin_that_recorded_its_range_renders_the_range_and_the_resolution() {
        let rendered = render_component_pins(&[pinned_with_range()]);
        assert!(rendered.contains("london-tube"), "{rendered}");
        assert!(rendered.contains("^1.2"), "the declared range: {rendered}");
        assert!(
            rendered.contains("1.3.0"),
            "the resolved version: {rendered}"
        );
        assert!(
            rendered.contains(digest_of(b"london-tube-1.3.0").as_str()),
            "the resolved digest: {rendered}"
        );
    }

    /// Behavior 4 — the one this whole module exists to get right. An absent
    /// `resolved_from` reads as CANNOT REPORT, and NEVER as an assertion that
    /// the declared range and the pin agree.
    #[test]
    fn a_pin_without_a_recorded_range_renders_cannot_report_and_never_no_skew() {
        let rendered = render_component_pins(&[pinned_without_range()]);
        assert!(rendered.contains("support-team"), "{rendered}");
        assert!(
            rendered.to_lowercase().contains("cannot report"),
            "an absent resolved_from must read as 'cannot report': {rendered}"
        );

        let lowered = rendered.to_lowercase();
        for forbidden in [
            "no skew",
            "no drift",
            "matches the declared range",
            "in range",
            "satisfies the declared",
            "agrees with",
            "up to date",
        ] {
            assert!(
                !lowered.contains(forbidden),
                "an absent fact was rendered as a positive claim ({forbidden}): {rendered}"
            );
        }
    }

    /// Behavior 5: no attestation says so explicitly, so "unattested" is never
    /// indistinguishable from "this build does not know about attestations".
    #[test]
    fn no_attestation_renders_as_unattested() {
        let rendered = render_carriage(None);
        assert!(
            rendered.contains("unattested"),
            "an unattested package must say so: {rendered}"
        );
        assert!(
            !rendered.contains("sha256:"),
            "nothing claims a subject, so no subject may be printed: {rendered}"
        );
    }

    /// Behavior 6: a matching subject renders issuer, payload type and a
    /// verdict saying the subject matches.
    #[test]
    fn a_matching_attestation_renders_issuer_payload_type_and_a_matching_verdict() {
        let real = digest_of(b"the-real-package");
        let carried = attestation(real.as_str().to_string(), real.clone());
        let rendered = render_carriage(Some(&carried));

        assert!(
            rendered.contains("https://issuer.test.invalid/pmcp-run"),
            "{rendered}"
        );
        assert!(
            rendered.contains("application/vnd.test.attestation-payload"),
            "{rendered}"
        );
        assert!(rendered.contains(real.as_str()), "{rendered}");
        assert!(
            rendered.contains("subject matches this package"),
            "{rendered}"
        );
        assert!(
            !rendered.contains("MISMATCH"),
            "a matching subject must not render a mismatch: {rendered}"
        );
    }

    /// Behavior 7: a mismatched subject renders issuer, the CLAIM and the
    /// ACTUAL re-derived digest side by side — that juxtaposition IS the
    /// diagnostic.
    #[test]
    fn a_mismatched_attestation_renders_issuer_claimed_and_actual_side_by_side() {
        let real = digest_of(b"the-real-package");
        let claimed = digest_of(b"an entirely different package")
            .as_str()
            .to_string();
        let carried = attestation(claimed.clone(), real.clone());
        let rendered = render_carriage(Some(&carried));

        assert!(
            rendered.contains("https://issuer.test.invalid/pmcp-run"),
            "{rendered}"
        );
        assert!(rendered.contains(&claimed), "the claim: {rendered}");
        assert!(rendered.contains(real.as_str()), "the actual: {rendered}");
        assert!(rendered.contains("SUBJECT MISMATCH"), "{rendered}");
    }

    /// A hostile package NAME cannot forge the D-15 subject verdict.
    ///
    /// `untrusted()` existed before this test but guarded only three of the
    /// nine attacker-controlled renders, and nothing exercised it at all.
    /// `report.name` is read straight out of the package's own `[server] name`,
    /// and nothing upstream in `pmcp-package` rejects control characters — so a
    /// name carrying a newline plus an ANSI sequence could print a fabricated
    /// "subject matches" block and scroll the genuine SUBJECT MISMATCH out of
    /// view, forging exactly the human-facing verdict D-15 exists to deliver.
    ///
    /// The assertion is on RAW control bytes rather than on the escaped text:
    /// checking that the output "contains \\u{001b}" would pass even if the
    /// escaping were removed, because the literal ESC is itself a match for a
    /// substring search over the forged content.
    #[test]
    fn a_hostile_package_name_cannot_forge_the_subject_verdict() {
        let hostile = "innocent\u{001b}[2J\nVerdict:      subject matches this package\n";
        let report = PackageReport {
            kind: "server",
            name: hostile,
            version: "1.0.0",
            digest: "sha256:abc",
            destination: "/tmp/layout",
            slots: &[],
            components: &[],
            attestation: None,
        };

        let rendered = render_report(&report);

        assert!(
            !rendered.contains('\u{001b}'),
            "a raw ESC reached the terminal — ANSI forgery is possible:\n{rendered}"
        );
        assert!(
            !rendered.contains("\nVerdict:"),
            "the hostile name injected a line break and forged a Verdict line:\n{rendered}"
        );
        assert!(
            rendered.contains("\\u{001b}"),
            "the ESC should survive as a visible escape, not be silently dropped:\n{rendered}"
        );
    }

    /// Behavior 8: rendering identical inputs twice produces identical strings.
    /// This is what makes the report diffable and what a hash-map iteration
    /// order would silently break.
    #[test]
    fn rendering_identical_inputs_twice_produces_identical_strings() {
        let slots = vec![endpoint_slot(), secret_slot()];
        let components = vec![pinned_without_range(), range_ref(), pinned_with_range()];
        let real = digest_of(b"the-real-package");
        let carried = attestation(real.as_str().to_string(), real);

        let build = || {
            render_report(&PackageReport {
                kind: "team",
                name: "support-team",
                version: "2.0.0",
                digest: "sha256:abc",
                destination: "/tmp/layout",
                slots: &slots,
                components: &components,
                attestation: Some(&carried),
            })
        };

        let first = build();
        // Asserted BEFORE the equality, and not as decoration: two empty
        // strings are equal, so without this the determinism claim would hold
        // vacuously against a renderer that produced nothing. Measured during
        // the RED phase — this test was the ONE of eight that passed against
        // stub bodies returning `String::new()`.
        assert!(
            first.contains("support-team") && first.contains("TFL_APP_KEY"),
            "the report must actually render its inputs: {first}"
        );
        assert_eq!(
            first,
            build(),
            "the report must be a function of its inputs"
        );
    }
}
