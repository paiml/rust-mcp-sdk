//! SEP-2640 Agent Skills — `ResourceHandler`-served skill resources with
//! a parallel `PromptHandler` fallback for SEP-2640-blind hosts.
//!
//! Both surfaces are derived from one [`Skill`] value: the SKILL.md
//! content + each reference body is byte-equal whether the host fetches
//! via [`crate::server::ResourceHandler::list`]/[`crate::server::ResourceHandler::read`]
//! (SEP-2640) or via [`crate::server::PromptHandler::handle`] (legacy).
//!
//! Internal storage uses [`indexmap::IndexMap`] so resource ordering is
//! deterministic across runs — required for stable example output,
//! snapshot tests, and predictable host UX.
//!
//! Wire shape: reads return [`crate::types::Content::resource_with_text`]
//! (NOT [`crate::types::Content::text`]) so per-resource MIME types survive
//! the wire round-trip — reference files like `schema.graphql` keep their
//! `application/graphql` MIME type.
//!
//! Byte-equal mirror of the doctest at the end of `pmcp-book/src/ch12-8-skills.md`.
//!
//! ```rust,no_run
//! use pmcp::server::skills::Skill;
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let greeting = Skill::new("hello-world", "# Hello\nThis is a minimal skill.\n");
//!     let prompt_text = greeting.as_prompt_text();
//!     assert!(prompt_text.starts_with("# Hello"));
//!
//!     let _server = pmcp::Server::builder()
//!         .name("doctest-skills-demo")
//!         .version("0.1.0")
//!         .bootstrap_skill_and_prompt(greeting, "hello_prompt")
//!         .build()?;
//!     Ok(())
//! }
//! ```

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use indexmap::IndexMap;
use serde::Serialize;
use serde_json::json;

use crate::error::{Error, ErrorCode, Result};
use crate::server::cancellation::RequestHandlerExtra;
use crate::server::{PromptHandler, ResourceHandler};
use crate::types::content::Role;
use crate::types::{
    Content, GetPromptResult, ListResourcesResult, PromptMessage, ReadResourceResult, ResourceInfo,
};

/// Reverse-domain key under `ServerCapabilities.extensions` advertising
/// SEP-2640 skill support. Set automatically when any skill is registered.
pub(crate) const SKILLS_EXTENSION_KEY: &str = "io.modelcontextprotocol/skills";

/// Synthesized discovery-index URI; emitted in `resources/list` and
/// served from `resources/read`.
const SKILL_INDEX_URI: &str = "skill://index.json";
const SKILL_MD_MIME: &str = "text/markdown";
const INDEX_JSON_MIME: &str = "application/json";

/// Flip `ServerCapabilities` to advertise skills support. Called from
/// every builder method that accepts a skill or skill registry — keeping
/// the four call sites in sync via one function instead of inline copies.
pub(crate) fn set_skills_capabilities(caps: &mut crate::types::ServerCapabilities) {
    if caps.resources.is_none() {
        caps.resources = Some(crate::types::ResourceCapabilities {
            subscribe: Some(false),
            list_changed: Some(false),
        });
    }
    caps.extensions
        .get_or_insert_with(HashMap::new)
        .entry(SKILLS_EXTENSION_KEY.to_string())
        .or_insert_with(|| json!({}));
}

// ── Public types ──────────────────────────────────────────────────────

/// A supporting file within a skill's directory (SEP-2640 §4 directory model).
///
/// Carries the relative path (e.g. `references/schema.graphql`), the
/// per-resource MIME type, and the body. Validation against duplicate or
/// invalid paths happens at [`Skill::with_reference`] /
/// [`Skill::try_with_reference`] time so that the parent skill's existing
/// references can be consulted.
///
/// # Examples
///
/// ```rust
/// use pmcp::server::skills::SkillReference;
///
/// let r = SkillReference::new("references/api.md", "text/markdown", "...");
/// assert_eq!(r.relative_path(), "references/api.md");
/// assert_eq!(r.mime_type(), "text/markdown");
/// ```
#[derive(Clone, Debug)]
pub struct SkillReference {
    relative_path: String,
    mime_type: String,
    body: String,
}

impl SkillReference {
    /// Construct a reference. Validation happens at
    /// [`Skill::with_reference`] / [`Skill::try_with_reference`] time so
    /// duplicate-within-skill checks can use the parent's reference set.
    pub fn new(
        relative_path: impl Into<String>,
        mime_type: impl Into<String>,
        body: impl Into<String>,
    ) -> Self {
        Self {
            relative_path: relative_path.into(),
            mime_type: mime_type.into(),
            body: body.into(),
        }
    }

    /// Relative path within the skill's directory (e.g.
    /// `references/schema.graphql`).
    pub fn relative_path(&self) -> &str {
        &self.relative_path
    }

    /// Per-resource MIME type (e.g. `application/graphql`).
    pub fn mime_type(&self) -> &str {
        &self.mime_type
    }

    /// Reference body text.
    pub fn body(&self) -> &str {
        &self.body
    }
}

/// A single Agent Skill (SEP-2640).
///
/// `name` is required (derived from SKILL.md frontmatter); `body` is the
/// SKILL.md content with YAML frontmatter intact. Optional `path` overrides
/// the default `skill://<name>/SKILL.md` URI. Optional `references` carry
/// supporting files (`schema.graphql`, `examples.md`, etc.) addressable at
/// `skill://<path>/<relative_path>`.
///
/// # Examples
///
/// ```rust
/// use pmcp::server::skills::{Skill, SkillReference};
///
/// let s = Skill::new("refunds", "---\nname: refunds\ndescription: Issue refunds\n---\nBody")
///     .with_reference(SkillReference::new("references/policy.md", "text/markdown", "..."));
/// assert_eq!(s.name(), "refunds");
/// assert_eq!(s.resolved_description(), "Issue refunds");
/// ```
#[derive(Clone, Debug)]
pub struct Skill {
    name: String,
    body: String,
    path: Option<String>,
    description: String,
    references: Vec<SkillReference>,
}

impl Skill {
    /// Create a skill from its frontmatter `name` and full SKILL.md body.
    ///
    /// The `description:` frontmatter line is parsed eagerly so per-skill
    /// metadata reads (e.g. `resources/list`, the discovery index) avoid
    /// re-scanning the body on every request.
    pub fn new(name: impl Into<String>, body: impl Into<String>) -> Self {
        let body = body.into();
        let description = parse_frontmatter_description(&body).unwrap_or_default();
        Self {
            name: name.into(),
            body,
            path: None,
            description,
            references: Vec::new(),
        }
    }

    /// Override the URI path (default: `skill://<name>/SKILL.md`).
    #[must_use]
    pub fn with_path(mut self, path: impl Into<String>) -> Self {
        self.path = Some(path.into());
        self
    }

    /// Explicit description override (otherwise the frontmatter
    /// `description:` line is used).
    #[must_use]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// Append a reference. **Panics** on invalid `relative_path` — use
    /// [`Self::try_with_reference`] for fallible registration.
    ///
    /// Invalid inputs: empty, contains a null byte, exactly `"SKILL.md"`
    /// (collides with the canonical URI), contains a `..` segment, starts
    /// with `/`, contains `://`, or duplicates a `relative_path` already
    /// registered on this Skill.
    ///
    /// # Panics
    ///
    /// Panics if the reference's relative path violates any of the rules
    /// listed above. Use [`Self::try_with_reference`] to surface the same
    /// failures as `Result`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::server::skills::{Skill, SkillReference};
    ///
    /// let s = Skill::new("x", "body")
    ///     .with_reference(SkillReference::new("references/a.md", "text/markdown", "a"));
    /// assert_eq!(s.references().count(), 1);
    /// ```
    #[must_use]
    pub fn with_reference(self, reference: SkillReference) -> Self {
        match self.try_with_reference(reference) {
            Ok(s) => s,
            Err(e) => panic!("Skill::with_reference: {e}"),
        }
    }

    /// Append a reference, returning `Err` on invalid input — the fallible
    /// counterpart to [`Self::with_reference`] for runtime-dynamic
    /// registration where panicking is unacceptable.
    ///
    /// # Errors
    ///
    /// Returns `Err(pmcp::Error::Validation)` if the relative path is
    /// empty, contains a null byte, exactly `"SKILL.md"`, contains a `..`
    /// segment, starts with `/`, contains `://`, or duplicates an existing
    /// relative path on this skill.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::server::skills::{Skill, SkillReference};
    ///
    /// let ok = Skill::new("x", "body")
    ///     .try_with_reference(SkillReference::new("references/a.md", "text/markdown", "a"));
    /// assert!(ok.is_ok());
    ///
    /// let bad = Skill::new("x", "body")
    ///     .try_with_reference(SkillReference::new("", "text/markdown", "a"));
    /// assert!(bad.is_err());
    /// ```
    pub fn try_with_reference(mut self, reference: SkillReference) -> Result<Self> {
        validate_reference_path(&reference.relative_path, &self.references)?;
        self.references.push(reference);
        Ok(self)
    }

    /// Skill name (from frontmatter).
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Full SKILL.md body (frontmatter + recipe).
    pub fn body(&self) -> &str {
        &self.body
    }

    /// Iterate over registered references in insertion order.
    pub fn references(&self) -> impl Iterator<Item = &SkillReference> {
        self.references.iter()
    }

    /// Resolved description: explicit [`Self::with_description`] override
    /// if set, otherwise the `description:` line parsed from the SKILL.md
    /// frontmatter at construction time. Returns `""` if neither is set.
    pub fn resolved_description(&self) -> &str {
        &self.description
    }

    pub(crate) fn resolved_path(&self) -> &str {
        self.path.as_deref().unwrap_or(&self.name)
    }

    pub(crate) fn skill_md_uri(&self) -> String {
        format!("skill://{}/SKILL.md", self.resolved_path())
    }

    pub(crate) fn reference_uri(&self, relative_path: &str) -> String {
        format!("skill://{}/{}", self.resolved_path(), relative_path)
    }

    /// Synthesize the PROMPT surface — body followed by each reference
    /// inlined with labelled `--- <relative_path> ---` rules.
    ///
    /// This is the load-bearing dual-surface invariant: the value
    /// returned here is byte-equal to the concatenation of the SKILL.md
    /// body and every reference body read via the
    /// [`crate::server::ResourceHandler`] surface, with a trailing
    /// newline normalization applied to each segment.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pmcp::server::skills::{Skill, SkillReference};
    ///
    /// let s = Skill::new("x", "A")
    ///     .with_reference(SkillReference::new("ref1.md", "text/markdown", "refbody"));
    /// assert_eq!(s.as_prompt_text(), "A\n\n--- ref1.md ---\nrefbody\n");
    /// ```
    pub fn as_prompt_text(&self) -> String {
        let mut out = String::new();
        out.push_str(&self.body);
        if !self.body.ends_with('\n') {
            out.push('\n');
        }
        for r in &self.references {
            out.push_str("\n--- ");
            out.push_str(&r.relative_path);
            out.push_str(" ---\n");
            out.push_str(&r.body);
            if !r.body.ends_with('\n') {
                out.push('\n');
            }
        }
        out
    }
}

fn validate_reference_path(path: &str, existing: &[SkillReference]) -> Result<()> {
    if path.is_empty() {
        return Err(Error::validation(
            "SkillReference relative_path must not be empty",
        ));
    }
    if path.contains('\0') {
        return Err(Error::validation(
            "SkillReference relative_path must not contain null bytes",
        ));
    }
    if path == "SKILL.md" {
        return Err(Error::validation(
            "SkillReference relative_path 'SKILL.md' collides with the canonical SKILL.md URI",
        ));
    }
    if path.split('/').any(|seg| seg == "..") {
        return Err(Error::validation(format!(
            "SkillReference relative_path '{path}' must not contain '..' segments"
        )));
    }
    if path.starts_with('/') {
        return Err(Error::validation(format!(
            "SkillReference relative_path '{path}' must be relative (no leading '/')"
        )));
    }
    if path.contains("://") {
        return Err(Error::validation(format!(
            "SkillReference relative_path '{path}' must not contain a URI scheme"
        )));
    }
    if existing.iter().any(|r| r.relative_path == path) {
        return Err(Error::validation(format!(
            "SkillReference relative_path '{path}' is already registered on this Skill"
        )));
    }
    Ok(())
}

// ── SEP-2640 entry manifests (`skills/list` / `skills/get`) ───────────

/// One file in a skill's `resources` manifest: the URI a host fetches, the
/// SHA-256 digest of the bytes it will receive, and their length.
///
/// Produced only by [`Skills::entries`]; see that method's rustdoc for the
/// STABLE-vs-unstable boundary on this type's serialized shape.
///
/// # Security
///
/// The digest is **not** an integrity boundary. SEP-2640 is explicit that
/// digests are unsigned and are supplied by the same server that supplies the
/// content, so a host MUST NOT treat a digest match as authentication of the
/// bytes. It detects drift between a listing and a later read; it proves nothing
/// about origin.
///
/// # Examples
///
/// ```rust
/// # #[cfg(feature = "skills")] {
/// use pmcp::server::skills::{Skill, Skills};
///
/// let entries = Skills::new()
///     .add(Skill::new("x", "---\nname: x\ndescription: d\n---\nbody\n"))
///     .entries()
///     .expect("entries build");
/// let manifest = entries[0].resources();
/// assert_eq!(manifest[0].uri(), "skill://x/SKILL.md");
/// assert!(manifest[0].digest().starts_with("sha256:"));
/// # }
/// ```
#[derive(Clone, Debug, Serialize)]
#[non_exhaustive]
pub struct SkillResourceRef {
    uri: String,
    digest: String,
    size: usize,
}

impl SkillResourceRef {
    /// The `skill://` URI a host fetches this file from via `resources/read`.
    pub fn uri(&self) -> &str {
        &self.uri
    }

    /// `sha256:` followed by 64 lowercase hexadecimal characters.
    ///
    /// See this type's `# Security` section: this is a drift detector, not an
    /// integrity guarantee.
    pub fn digest(&self) -> &str {
        &self.digest
    }

    /// Length of the served bytes, in bytes.
    pub fn size(&self) -> usize {
        self.size
    }
}

/// One entry of a SEP-2640 `skills/list` / `skills/get` result: the skill's
/// `SKILL.md` URI, its frontmatter rendered VERBATIM as JSON, and the complete
/// manifest of the files the skill is made of.
///
/// Produced only by [`Skills::entries`]; see that method's rustdoc for the
/// STABLE-vs-unstable boundary on this type's serialized shape and for the
/// disclosure warning on `frontmatter`.
#[derive(Clone, Debug, Serialize)]
#[non_exhaustive]
pub struct SkillEntry {
    uri: String,
    frontmatter: serde_json::Value,
    resources: Vec<SkillResourceRef>,
}

impl SkillEntry {
    /// The skill's canonical `skill://<path>/SKILL.md` URI.
    pub fn uri(&self) -> &str {
        &self.uri
    }

    /// The skill's YAML frontmatter, rendered verbatim as a JSON object —
    /// every field the author wrote, never a curated subset.
    ///
    /// SEP-2640 requires a host to compare this object field-by-field against
    /// the frontmatter of the `SKILL.md` it fetches and to REFUSE the skill on
    /// any discrepancy, which is why nothing here is synthesized or normalized.
    pub fn frontmatter(&self) -> &serde_json::Value {
        &self.frontmatter
    }

    /// The skill's COMPLETE file manifest: `SKILL.md` first, then every
    /// registered reference in registration order.
    pub fn resources(&self) -> &[SkillResourceRef] {
        &self.resources
    }
}

/// A build-time finding about ONE registered skill, produced by
/// [`Skills::entries_with_diagnostics`] and warned by [`Skills::entries`].
///
/// Crate-private on purpose: it adds no public API surface while making every
/// warn path directly assertable from the in-module test block, which is what a
/// test of "the exclusion produces a warning naming the skill" needs in order to
/// be a measurement rather than a claim.
///
/// Which findings exist, and how they are partitioned, is explicitly NOT part of
/// the stable surface — see [`Skills::entries`]'s stability boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
// Why: every variant here is about the frontmatter block, so the shared prefix
// is the accurate name rather than a stutter — and dropping it would leave
// `Absent` / `Invalid` / `NotAMapping`, which say nothing about WHAT is absent.
#[allow(clippy::enum_variant_names)]
pub(crate) enum SkillDiagnostic {
    /// The SKILL.md carries no `---`-delimited frontmatter block at all. The
    /// skill is EXCLUDED from `skills/list` (D-02) and stays readable via
    /// `resources/read`.
    FrontmatterAbsent {
        /// The excluded skill's SKILL.md URI.
        uri: String,
    },
    /// A frontmatter block IS present but is unterminated or is not valid YAML.
    /// EXCLUDED, and deliberately a DIFFERENT finding from
    /// [`Self::FrontmatterAbsent`]: reporting a typo as a missing block sends
    /// the author to add one that is already there.
    FrontmatterInvalid {
        /// The excluded skill's SKILL.md URI.
        uri: String,
        /// The parser's own message.
        reason: String,
    },
    /// A frontmatter block IS present and IS valid YAML, but is a sequence or a
    /// scalar rather than a mapping. EXCLUDED.
    FrontmatterNotAMapping {
        /// The excluded skill's SKILL.md URI.
        uri: String,
        /// Which non-mapping shape was found.
        reason: String,
    },
}

impl SkillDiagnostic {
    /// The SKILL.md URI of the skill this finding is about.
    pub(crate) fn uri(&self) -> &str {
        match self {
            Self::FrontmatterAbsent { uri }
            | Self::FrontmatterInvalid { uri, .. }
            | Self::FrontmatterNotAMapping { uri, .. } => uri,
        }
    }

    /// The operator-facing warning text. Each case says what it actually is:
    /// an author reading these must be able to tell an absent block from a
    /// broken one without opening the source.
    pub(crate) fn message(&self) -> String {
        match self {
            Self::FrontmatterAbsent { uri } => format!(
                "skill {uri} is excluded from skills/list: its SKILL.md carries NO frontmatter \
                 block. Add a `---`-delimited YAML block at the top of the file; the skill stays \
                 readable via resources/read either way."
            ),
            Self::FrontmatterInvalid { uri, reason } => format!(
                "skill {uri} is excluded from skills/list: a frontmatter block IS present but is \
                 unusable ({reason}). This is a broken block, not a missing one."
            ),
            Self::FrontmatterNotAMapping { uri, reason } => format!(
                "skill {uri} is excluded from skills/list: its frontmatter block is valid YAML but \
                 is not a mapping ({reason}). SEP-2640 entries need top-level `key: value` pairs."
            ),
        }
    }
}

/// Collection of skills + auto-generated discovery index. Lifted into a
/// [`crate::server::ResourceHandler`] impl via [`Skills::into_handler`].
///
/// `Clone` is required so the builder's `try_skills` can probe duplicates
/// by cloning the registry before storing it (consume-by-value
/// `into_handler` API).
///
/// # Examples
///
/// ```rust
/// use pmcp::server::skills::{Skill, Skills};
///
/// let registry = Skills::new()
///     .add(Skill::new("a", "body-a"))
///     .add(Skill::new("b", "body-b"));
/// assert_eq!(registry.skill_md_uris(), vec![
///     "skill://a/SKILL.md".to_string(),
///     "skill://b/SKILL.md".to_string(),
/// ]);
/// ```
#[derive(Default, Clone, Debug)]
pub struct Skills {
    skills: Vec<Skill>,
}

impl Skills {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self { skills: Vec::new() }
    }

    /// Append a skill to the registry.
    #[must_use]
    #[allow(clippy::should_implement_trait)] // builder-style consumer; not a std::ops::Add impl
    pub fn add(mut self, skill: Skill) -> Self {
        self.skills.push(skill);
        self
    }

    /// Concatenate another registry onto this one — the builder
    /// accumulator uses this on repeated `.skills(...)` calls so each
    /// call adds to (rather than replaces) prior registrations.
    #[must_use]
    pub fn merge(mut self, other: Self) -> Self {
        self.skills.extend(other.skills);
        self
    }

    /// Snapshot of all registered SKILL.md URIs in registration order.
    ///
    /// Reference URIs are NOT included — they're readable via
    /// `resources/read` but never enumerated (SEP-2640 §9).
    pub fn skill_md_uris(&self) -> Vec<String> {
        self.skills.iter().map(Skill::skill_md_uri).collect()
    }

    /// Synthesize the SEP-2640 `skills/list` entry for every registered skill
    /// that carries parseable YAML frontmatter.
    ///
    /// Takes `&self` deliberately: [`Self::into_handler`] is public API returning
    /// `Result<Arc<dyn ResourceHandler>>` and CONSUMES `self`, so widening it to
    /// a tuple would be a semver-MAJOR break on a shipped method. Call this
    /// first, `into_handler()` second — which is exactly what the crate-private
    /// builder finalization does, so both build paths get entries from one place.
    ///
    /// A skill whose body carries NO frontmatter block, or whose block is present
    /// but malformed, is EXCLUDED from the returned vector rather than
    /// synthesized. SEP-2640 permits a partial listing; it does not permit an
    /// entry whose `frontmatter` disagrees with the served `SKILL.md`, and a
    /// synthesized `{name, description}` for an unannotated skill is a guaranteed
    /// host-side rejection. Such skills stay readable via `resources/read`.
    ///
    /// Each exclusion emits exactly ONE `tracing::warn!` on the `mcp.skills`
    /// target naming the excluded skill's URI and its case-specific reason, so
    /// an operator sees a partial listing being formed rather than discovering
    /// it from a host. The three failure shapes — no block, a broken block, and
    /// a block that is valid YAML but not a mapping — each report themselves as
    /// what they are.
    ///
    /// A future strict variant may REJECT frontmatter-less skills at build time
    /// instead of excluding them. That is deliberately not the default today:
    /// this crate's own doctests, both in-module proptest strategies and dozens
    /// of shipped `Skill::new(...)` call sites construct frontmatter-less
    /// skills, and hard-erroring on them would be a breaking change to shipped,
    /// documented usage. Promotion belongs with the canonical-surface cleanup,
    /// not here.
    ///
    /// # The manifest is COMPLETE, and its digests are the served bytes
    ///
    /// Each entry's `resources` names the skill's own `SKILL.md` first and then
    /// EVERY registered reference, in registration order. A conforming host
    /// fetches every file named there and compares it against the row, so a
    /// manifest that listed only `SKILL.md` would be rejected rather than
    /// treated as a partial answer. See [`skill_resource_manifest`] for the
    /// invariant that keeps a row and its `resources/read` in agreement by
    /// construction.
    ///
    /// # `frontmatter` comes from the FILE, never from the overridable accessor
    ///
    /// The emitted object is produced by the single crate-private YAML parse and
    /// by nothing else. [`Skill::with_description`] is an explicit override, so
    /// [`Skill::resolved_description`] can legitimately differ from the
    /// `description:` line the SKILL.md actually carries — and SEP-2640 makes a
    /// field-by-field mismatch between this object and the fetched file a
    /// mandatory host-side refusal. Reconstructing any part of it from `Skill`'s
    /// own fields would therefore ship a server that looks conformant and is
    /// unusable.
    ///
    /// # What is STABLE about the returned values, and what is not
    ///
    /// These types are designed backwards from a wire format, so the boundary is
    /// written down rather than left to be guessed.
    ///
    /// **STABLE — a conforming host reads every one of these, so changing any is
    /// a wire break whatever Rust semver says about it:**
    ///
    /// - the serialized key sets, exactly `uri` / `frontmatter` / `resources` on
    ///   [`SkillEntry`] and `uri` / `digest` / `size` on [`SkillResourceRef`];
    /// - the digest format — `sha256:` followed by 64 LOWERCASE hex characters;
    /// - `size` measured in BYTES of the served content;
    /// - entry order equal to REGISTRATION order;
    /// - the `SKILL.md`-first position in each `resources` manifest.
    ///
    /// **NOT stable:** which non-conforming skills are excluded rather than
    /// rejected, and the crate-private diagnostic set behind the exclusion, which
    /// may be re-partitioned freely.
    ///
    /// # Security — `frontmatter` is disclosed verbatim to every caller
    ///
    /// SEP-2640 REQUIRES verbatim emission, so pmcp performs no redaction. A
    /// secret placed in a `SKILL.md` frontmatter block is therefore returned to
    /// every `skills/list` caller — where before this method existed it required
    /// a deliberate `resources/read` of that skill's body. Treat frontmatter as
    /// public data (ASVS V8).
    ///
    /// # Errors
    ///
    /// Returns `Err(pmcp::Error::Validation)` if the registry cannot produce a
    /// well-formed entry set. No condition currently reaches that arm; the
    /// build-time name-identity and limits validation that will is added by a
    /// later plan, and the signature is fixed now so adding it is not a
    /// source-breaking change for callers.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # #[cfg(feature = "skills")] {
    /// use pmcp::server::skills::{Skill, SkillReference, Skills};
    ///
    /// let body = "---\nname: refunds\ndescription: Issue refunds\nlicense: Apache-2.0\n---\n\
    ///             # Refunds\n";
    /// let policy = "# Policy\n";
    /// let entries = Skills::new()
    ///     .add(Skill::new("refunds", body).with_reference(SkillReference::new(
    ///         "references/policy.md",
    ///         "text/markdown",
    ///         policy,
    ///     )))
    ///     .entries()
    ///     .expect("entries build");
    ///
    /// assert_eq!(entries.len(), 1);
    /// assert_eq!(entries[0].uri(), "skill://refunds/SKILL.md");
    /// // Verbatim: the non-required `license` field survives untouched.
    /// assert_eq!(entries[0].frontmatter()["license"], "Apache-2.0");
    ///
    /// // The manifest is COMPLETE: SKILL.md first, then every reference.
    /// let manifest = entries[0].resources();
    /// assert_eq!(manifest.len(), 2);
    /// assert_eq!(manifest[0].uri(), "skill://refunds/SKILL.md");
    /// assert_eq!(manifest[0].size(), body.len());
    /// assert!(manifest[0].digest().starts_with("sha256:"));
    /// assert_eq!(manifest[0].digest().len(), "sha256:".len() + 64);
    /// assert_eq!(manifest[1].uri(), "skill://refunds/references/policy.md");
    /// assert_eq!(manifest[1].size(), policy.len());
    ///
    /// // A skill with NO frontmatter is excluded, not synthesized.
    /// assert!(Skills::new()
    ///     .add(Skill::new("bare", "just a body"))
    ///     .entries()
    ///     .expect("entries build")
    ///     .is_empty());
    /// # }
    /// ```
    pub fn entries(&self) -> Result<Vec<SkillEntry>> {
        let (entries, diagnostics) = self.entries_with_diagnostics()?;
        for diagnostic in &diagnostics {
            tracing::warn!(
                target: "mcp.skills",
                uri = %diagnostic.uri(),
                "{}",
                diagnostic.message()
            );
        }
        Ok(entries)
    }

    /// [`Self::entries`] with the build-time findings returned instead of
    /// logged — the crate-private form every test of the exclusion semantics
    /// drives, so "a warning is emitted naming the skill" is assertable rather
    /// than merely claimed.
    ///
    /// A skill whose frontmatter yields anything other than
    /// [`FrontmatterParse::Parsed`] contributes NO entry and exactly ONE
    /// diagnostic, and each of the three failure shapes maps to its OWN
    /// [`SkillDiagnostic`] variant. None of them is an error: 40+ existing
    /// `Skill::new(...)` call sites, the module doctests and both proptest
    /// strategies construct frontmatter-less skills, and D-02 chose a partial
    /// listing over breaking them.
    ///
    /// A `{name, description}` object is never synthesized for an excluded
    /// skill. SEP-2640 makes a field-by-field mismatch between an entry's
    /// `frontmatter` and the fetched SKILL.md a mandatory host-side refusal, so
    /// a synthesized entry ships a server that looks conformant and is unusable.
    ///
    /// # Errors
    ///
    /// Returns `Err(pmcp::Error::Validation)` if the registry cannot produce a
    /// well-formed entry set. See [`Self::entries`]'s `# Errors` section.
    pub(crate) fn entries_with_diagnostics(
        &self,
    ) -> Result<(Vec<SkillEntry>, Vec<SkillDiagnostic>)> {
        let mut entries = Vec::with_capacity(self.skills.len());
        let mut diagnostics = Vec::new();
        for skill in &self.skills {
            let uri = skill.skill_md_uri();
            // The emitted object comes from `parse_frontmatter_value` and from
            // NOTHING else. It is deliberately never reconstructed from
            // `Skill::name()` or from the resolved description accessor:
            // `with_description` is an explicit OVERRIDE, so that accessor can
            // legitimately differ from the SKILL.md's authored `description:`
            // line, while SEP-2640 requires the emitted object to be identical
            // to the file's, field for field.
            let frontmatter = match parse_frontmatter_value(skill.body()) {
                FrontmatterParse::Parsed(value) => value,
                FrontmatterParse::Absent => {
                    diagnostics.push(SkillDiagnostic::FrontmatterAbsent { uri });
                    continue;
                },
                FrontmatterParse::Invalid(reason) => {
                    diagnostics.push(SkillDiagnostic::FrontmatterInvalid { uri, reason });
                    continue;
                },
                FrontmatterParse::NotAMapping(reason) => {
                    diagnostics.push(SkillDiagnostic::FrontmatterNotAMapping { uri, reason });
                    continue;
                },
            };
            entries.push(SkillEntry {
                uri,
                frontmatter,
                resources: skill_resource_manifest(skill),
            });
        }
        Ok((entries, diagnostics))
    }

    /// Flatten the registry into a [`crate::server::ResourceHandler`].
    ///
    /// Returns `Err` on:
    /// - Two skills resolving to the same `skill_md_uri()`.
    /// - Two skills' reference URIs colliding.
    ///
    /// Insertion order is preserved via [`indexmap::IndexMap`] so
    /// `resources/list` output is deterministic across runs.
    ///
    /// # Errors
    ///
    /// Returns `Err(pmcp::Error::Validation)` listing every duplicate URI
    /// detected. No silent overwrites.
    pub fn into_handler(self) -> Result<Arc<dyn ResourceHandler>> {
        let mut skill_md: IndexMap<String, Skill> = IndexMap::with_capacity(self.skills.len());
        let mut references: IndexMap<String, (String, String)> = IndexMap::new();
        let mut dup_skill: Vec<String> = Vec::new();
        let mut dup_ref: Vec<String> = Vec::new();
        for skill in self.skills {
            for r in &skill.references {
                let uri = skill.reference_uri(&r.relative_path);
                match references.entry(uri) {
                    indexmap::map::Entry::Occupied(e) => dup_ref.push(e.key().clone()),
                    indexmap::map::Entry::Vacant(e) => {
                        e.insert((r.mime_type.clone(), r.body.clone()));
                    },
                }
            }
            let uri = skill.skill_md_uri();
            match skill_md.entry(uri) {
                indexmap::map::Entry::Occupied(e) => dup_skill.push(e.key().clone()),
                indexmap::map::Entry::Vacant(e) => {
                    e.insert(skill);
                },
            }
        }
        if !dup_skill.is_empty() || !dup_ref.is_empty() {
            let mut msg = String::from("Skills::into_handler: duplicate URI(s):");
            if !dup_skill.is_empty() {
                msg.push_str(&format!(" SKILL.md=[{}]", dup_skill.join(", ")));
            }
            if !dup_ref.is_empty() {
                msg.push_str(&format!(" references=[{}]", dup_ref.join(", ")));
            }
            return Err(Error::validation(msg));
        }
        Ok(Arc::new(SkillsHandler::new(skill_md, references)))
    }
}

/// The complete SEP-2640 `resources` manifest for ONE skill: its own
/// `SKILL.md` first, then every registered reference in REGISTRATION order.
///
/// # The manifest cannot disagree with the served bytes
///
/// Every `digest` and `size` below is computed from the exact same `&str` that
/// [`SkillsHandler::read`] returns for the exact same URI —
/// [`Skill::body`] for the `SKILL.md` row and [`SkillReference::body`] for each
/// reference row, keyed by [`Skill::skill_md_uri`] and
/// [`Skill::reference_uri`] respectively. There is no second source of bytes to
/// drift from, so a manifest that disagrees with a later `resources/read` is
/// unconstructible rather than merely untested.
///
/// A conforming host fetches EVERY file named here and compares what it reads
/// against this row, which is why the manifest is complete rather than
/// `SKILL.md`-only: an incomplete manifest is a host-side rejection, not a
/// graceful degradation.
fn skill_resource_manifest(skill: &Skill) -> Vec<SkillResourceRef> {
    let mut rows = Vec::with_capacity(1 + skill.references.len());
    let body = skill.body();
    rows.push(SkillResourceRef {
        uri: skill.skill_md_uri(),
        digest: sha256_digest_hex(body.as_bytes()),
        size: body.len(),
    });
    for r in &skill.references {
        let ref_body = r.body();
        rows.push(SkillResourceRef {
            uri: skill.reference_uri(r.relative_path()),
            digest: sha256_digest_hex(ref_body.as_bytes()),
            size: ref_body.len(),
        });
    }
    rows
}

// ── Internal handler types ────────────────────────────────────────────

/// Internal [`crate::server::ResourceHandler`] impl synthesized by
/// [`Skills::into_handler`]. The registry is immutable post-construction,
/// so list/read responses are precomputed once and cloned per request.
pub(crate) struct SkillsHandler {
    list_resources: Vec<ResourceInfo>,
    skill_md: IndexMap<String, Skill>,
    references: IndexMap<String, (String, String)>, // uri -> (mime, body)
    index_json: String,
}

impl SkillsHandler {
    fn new(
        skill_md: IndexMap<String, Skill>,
        references: IndexMap<String, (String, String)>,
    ) -> Self {
        let mut list_resources: Vec<ResourceInfo> = skill_md
            .values()
            .map(|s| {
                ResourceInfo::new(s.skill_md_uri(), s.name().to_string())
                    .with_description(s.resolved_description())
                    .with_mime_type(SKILL_MD_MIME)
            })
            .collect();
        list_resources.push(
            ResourceInfo::new(SKILL_INDEX_URI, "index")
                .with_description("Skill discovery index (SEP-2640 §9)")
                .with_mime_type(INDEX_JSON_MIME),
        );
        let index_json = build_discovery_index_json(&skill_md);
        Self {
            list_resources,
            skill_md,
            references,
            index_json,
        }
    }
}

fn build_discovery_index_json(skill_md: &IndexMap<String, Skill>) -> String {
    let entries: Vec<_> = skill_md
        .values()
        .map(|s| {
            json!({
                "name": s.name(),
                "type": "skill-md",
                "description": s.resolved_description(),
                "url": s.skill_md_uri(),
            })
        })
        .collect();
    serde_json::to_string_pretty(&json!({
        "$schema": "https://schemas.agentskills.io/discovery/0.2.0/schema.json",
        "skills": entries,
    }))
    .expect("static JSON object — to_string_pretty cannot fail")
}

#[async_trait]
impl ResourceHandler for SkillsHandler {
    async fn list(
        &self,
        _cursor: Option<String>,
        _extra: RequestHandlerExtra,
    ) -> Result<ListResourcesResult> {
        // Per SEP-2640 §9: list emits SKILL.md entries + the discovery
        // index ONLY. Reference URIs are never enumerated.
        Ok(ListResourcesResult::new(self.list_resources.clone()))
    }

    async fn read(&self, uri: &str, _extra: RequestHandlerExtra) -> Result<ReadResourceResult> {
        // resource_with_text (not Content::text) preserves the per-resource
        // URI + MIME on the wire — required so a reference like
        // schema.graphql round-trips with `application/graphql`.
        if uri == SKILL_INDEX_URI {
            return Ok(ReadResourceResult::new(vec![Content::resource_with_text(
                uri,
                self.index_json.clone(),
                INDEX_JSON_MIME,
            )]));
        }
        if let Some(skill) = self.skill_md.get(uri) {
            return Ok(ReadResourceResult::new(vec![Content::resource_with_text(
                uri,
                skill.body().to_string(),
                SKILL_MD_MIME,
            )]));
        }
        if let Some((mime, body)) = self.references.get(uri) {
            return Ok(ReadResourceResult::new(vec![Content::resource_with_text(
                uri,
                body.clone(),
                mime.clone(),
            )]));
        }
        Err(Error::protocol(
            ErrorCode::METHOD_NOT_FOUND,
            format!("Skill resource not found: {uri}"),
        ))
    }
}

/// [`crate::server::PromptHandler`] impl that returns the
/// [`Skill::as_prompt_text`] body as a single user message.
///
/// The dual-surface invariant: the prompt body is byte-equal to the
/// concatenated SKILL.md + reference reads. Pointer-style prompts
/// (returning a `skill://` URI the host cannot fetch) are prohibited —
/// hosts that don't yet speak SEP-2640 need the content inlined.
pub(crate) struct SkillPromptHandler {
    prompt_text: String,
    description: String,
}

impl SkillPromptHandler {
    pub(crate) fn new(skill: Skill) -> Self {
        let prompt_text = skill.as_prompt_text();
        let description = skill.resolved_description().to_string();
        Self {
            prompt_text,
            description,
        }
    }
}

#[async_trait]
impl PromptHandler for SkillPromptHandler {
    async fn handle(
        &self,
        _args: HashMap<String, String>,
        _extra: RequestHandlerExtra,
    ) -> Result<GetPromptResult> {
        // Plain Content::text is correct here — this is a PromptMessage,
        // not a resource read. The resource_with_text shape applies only
        // to ResourceHandler::read.
        let message = PromptMessage::new(Role::User, Content::text(self.prompt_text.clone()));
        Ok(GetPromptResult::new(
            vec![message],
            Some(self.description.clone()),
        ))
    }
}

/// URI-prefix-routing composite [`crate::server::ResourceHandler`].
///
/// Constructed exactly once per server, in the builder's `.build()`
/// finalization step — never nested.
pub(crate) struct ComposedResources {
    pub(crate) skills: Arc<dyn ResourceHandler>,
    pub(crate) other: Arc<dyn ResourceHandler>,
}

#[async_trait]
impl ResourceHandler for ComposedResources {
    async fn list(
        &self,
        cursor: Option<String>,
        extra: RequestHandlerExtra,
    ) -> Result<ListResourcesResult> {
        // SkillsHandler::list ignores cursor + extra — pass owned defaults
        // so the user handler can take the real ones by move.
        let mut combined = self
            .skills
            .list(None, RequestHandlerExtra::default())
            .await?;
        let extra_other = self.other.list(cursor, extra).await?;
        combined.resources.extend(extra_other.resources);
        Ok(combined)
    }

    async fn read(&self, uri: &str, extra: RequestHandlerExtra) -> Result<ReadResourceResult> {
        if uri.starts_with("skill://") {
            self.skills.read(uri, extra).await
        } else {
            self.other.read(uri, extra).await
        }
    }
}

// ── Frontmatter parsing (internal) ───────────────────────────────────

fn parse_frontmatter_description(body: &str) -> Option<String> {
    // Strip UTF-8 BOM so frontmatter authored on Windows still parses;
    // `str::lines()` already handles both \n and \r\n line endings.
    let body = body.strip_prefix('\u{FEFF}').unwrap_or(body);
    let mut in_frontmatter = false;
    for line in body.lines().take(40) {
        if line == "---" {
            if in_frontmatter {
                break;
            }
            in_frontmatter = true;
            continue;
        }
        if in_frontmatter {
            if let Some(rest) = line.strip_prefix("description: ") {
                return Some(rest.trim().to_string());
            }
        }
    }
    None
}

/// The FOUR outcomes of reading a SKILL.md's YAML frontmatter block.
///
/// Four, not two, on purpose. Collapsing any of the failures into `Absent` would
/// make a BROKEN canonical skill indistinguishable from a deliberately
/// unannotated one, and every build-time diagnostic in [`SkillDiagnostic`] is
/// built on the distinction: an author whose block has a YAML typo is told about
/// the typo rather than told to add a block that is already there.
///
/// The split between [`Self::Invalid`] and [`Self::NotAMapping`] is the same
/// argument one level down — "your YAML does not parse" and "your YAML parses
/// but is a list" send an author to two different edits.
#[derive(Debug)]
enum FrontmatterParse {
    /// The body carries no `---`-delimited block at its start.
    Absent,
    /// The block parsed to a YAML mapping, rendered here as a JSON object.
    Parsed(serde_json::Value),
    /// A block IS present but is unterminated or is not valid YAML. Carries a
    /// human-readable diagnostic including the parser's own message.
    Invalid(String),
    /// A block IS present and IS valid YAML, but is a sequence or a scalar
    /// rather than the mapping SEP-2640 entries require.
    NotAMapping(String),
}

/// The ONE crate-private function that touches `serde_yaml` (D-04).
///
/// Every other line of this module is YAML-library-agnostic, so swapping to a
/// maintained fork later is a change to this function body and nothing else.
///
/// # Where the block must be
///
/// The opening `---` must be the FIRST line of the body (after an optional UTF-8
/// BOM), which is what agentskills.io requires. This is deliberately STRICTER
/// than the shipped [`parse_frontmatter_description`] scanner beside it, which
/// accepts the first `---` anywhere in the leading 40 lines: under that looser
/// rule an ordinary markdown horizontal rule in an UNANNOTATED skill would open
/// a phantom block and turn `Absent` into `Invalid` — exactly the confusion the
/// three-way return exists to avoid.
///
/// A trailing `\r` is stripped from each candidate delimiter line BEFORE the
/// `---` comparison, so a CRLF-authored SKILL.md behaves identically to an LF one
/// (`str::lines` already does this; the explicit trim is belt-and-braces and
/// documents the intent).
fn parse_frontmatter_value(body: &str) -> FrontmatterParse {
    // Strip the UTF-8 BOM, exactly as the description scanner does.
    let body = body.strip_prefix('\u{FEFF}').unwrap_or(body);
    let mut lines = body.lines();
    match lines.next() {
        Some(first) if first.trim_end_matches('\r') == "---" => {},
        _ => return FrontmatterParse::Absent,
    }

    let mut block = String::new();
    let mut terminated = false;
    for line in lines {
        let line = line.trim_end_matches('\r');
        if line == "---" {
            terminated = true;
            break;
        }
        block.push_str(line);
        block.push('\n');
    }
    if !terminated {
        return FrontmatterParse::Invalid(
            "frontmatter block opens with `---` but is never closed by a `---` line".to_string(),
        );
    }

    match serde_yaml::from_str::<serde_json::Value>(&block) {
        Ok(serde_json::Value::Object(map)) => {
            FrontmatterParse::Parsed(serde_json::Value::Object(map))
        },
        Ok(other) => FrontmatterParse::NotAMapping(format!(
            "frontmatter must be a YAML mapping, got {}",
            match other {
                serde_json::Value::Array(_) => "a sequence",
                serde_json::Value::Null => "an empty document",
                serde_json::Value::Bool(_) | serde_json::Value::Number(_) => "a scalar",
                serde_json::Value::String(_) => "a string scalar",
                serde_json::Value::Object(_) => unreachable!("matched by the arm above"),
            }
        )),
        Err(e) => FrontmatterParse::Invalid(format!("frontmatter is not valid YAML: {e}")),
    }
}

/// `sha256:` + 64 LOWERCASE hex characters over `bytes`, the SEP-2640 digest
/// format.
///
/// The whole-value `{:x}` formatter does NOT compile on this workspace's stack:
/// MEASURED at plan time, there is no `LowerHex` impl anywhere in `sha2-0.11.0`,
/// `digest-0.11.2` or `crypto-common-0.2.2`. The finalized bytes are therefore
/// folded with the per-byte width-2 formatter, which is also what pins the
/// lowercase-and-zero-padded guarantee the format string requires.
fn sha256_digest_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    use std::fmt::Write as _;

    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();

    let mut out = String::with_capacity("sha256:".len() + 64);
    out.push_str("sha256:");
    for byte in digest.iter() {
        // Infallible: writing to a String cannot fail.
        let _ = write!(out, "{byte:02x}");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn extra() -> RequestHandlerExtra {
        RequestHandlerExtra::default()
    }

    // ── Test 1.1 ──────────────────────────────────────────────────────
    #[test]
    fn test_1_1_skill_new_and_builders() {
        let s = Skill::new("foo", "body");
        assert_eq!(s.name(), "foo");
        assert_eq!(s.body(), "body");
        assert_eq!(s.references().count(), 0);
        assert_eq!(s.resolved_description(), "");

        let s = s
            .with_path("p")
            .with_description("d")
            .with_reference(SkillReference::new(
                "references/x.md",
                "text/markdown",
                "ref body",
            ));
        assert_eq!(s.resolved_path(), "p");
        assert_eq!(s.resolved_description(), "d");
        assert_eq!(s.references().count(), 1);
    }

    // ── Test 1.2 ──────────────────────────────────────────────────────
    #[test]
    fn test_1_2_skill_md_uri_default_and_override() {
        let s = Skill::new("foo", "");
        assert_eq!(s.skill_md_uri(), "skill://foo/SKILL.md");
        let s = s.with_path("acme/refunds");
        assert_eq!(s.skill_md_uri(), "skill://acme/refunds/SKILL.md");
    }

    // ── Test 1.3 ──────────────────────────────────────────────────────
    #[test]
    fn test_1_3_skill_reference_uri_resolution() {
        let s = Skill::new("x", "").with_reference(SkillReference::new(
            "references/a.md",
            "text/markdown",
            "...",
        ));
        assert_eq!(
            s.reference_uri("references/a.md"),
            "skill://x/references/a.md"
        );

        let s = s.with_path("y/z");
        assert_eq!(
            s.reference_uri("references/a.md"),
            "skill://y/z/references/a.md"
        );
    }

    // ── Test 1.4 ──────────────────────────────────────────────────────
    #[test]
    fn test_1_4_as_prompt_text_no_references() {
        let s = Skill::new("x", "---\nname: x\n---\nbody");
        assert_eq!(s.as_prompt_text(), "---\nname: x\n---\nbody\n");
    }

    // ── Test 1.5 ──────────────────────────────────────────────────────
    #[test]
    fn test_1_5_as_prompt_text_with_references() {
        let s = Skill::new("x", "A").with_reference(SkillReference::new(
            "ref1.md",
            "text/markdown",
            "refbody",
        ));
        assert_eq!(s.as_prompt_text(), "A\n\n--- ref1.md ---\nrefbody\n");

        let s = Skill::new("x", "A")
            .with_reference(SkillReference::new("r1.md", "text/markdown", "b1"))
            .with_reference(SkillReference::new("r2.md", "text/markdown", "b2"));
        assert_eq!(
            s.as_prompt_text(),
            "A\n\n--- r1.md ---\nb1\n\n--- r2.md ---\nb2\n"
        );
    }

    // ── Test 1.6 ──────────────────────────────────────────────────────
    #[test]
    fn test_1_6_resolved_description_frontmatter_parsing() {
        let s = Skill::new("x", "---\nname: x\ndescription: hello\n---\nbody");
        assert_eq!(s.resolved_description(), "hello");

        let s = Skill::new("x", "---\nname: x\ndescription: hello\n---\nbody")
            .with_description("override");
        assert_eq!(s.resolved_description(), "override");

        let s = Skill::new("x", "no frontmatter");
        assert_eq!(s.resolved_description(), "");
    }

    // ── Test 1.6a (CRLF) ──────────────────────────────────────────────
    #[test]
    fn test_1_6a_parse_frontmatter_crlf() {
        let s = Skill::new("x", "---\r\nname: x\r\ndescription: hello\r\n---\r\nbody");
        assert_eq!(s.resolved_description(), "hello");
    }

    // ── Test 1.6b (UTF-8 BOM) ─────────────────────────────────────────
    #[test]
    fn test_1_6b_parse_frontmatter_utf8_bom() {
        let s = Skill::new("x", "\u{FEFF}---\nname: x\ndescription: hello\n---\nbody");
        assert_eq!(s.resolved_description(), "hello");
    }

    // ── Test 1.7 ──────────────────────────────────────────────────────
    #[tokio::test]
    async fn test_1_7_skills_into_handler_happy_path() {
        let handler = Skills::new()
            .add(Skill::new("a", ""))
            .add(Skill::new("b", ""))
            .into_handler()
            .unwrap();
        let list = handler.list(None, extra()).await.unwrap();
        assert_eq!(list.resources.len(), 3);
        assert_eq!(list.resources[0].uri, "skill://a/SKILL.md");
        assert_eq!(list.resources[1].uri, "skill://b/SKILL.md");
        assert_eq!(list.resources[2].uri, "skill://index.json");
        // No references in the list.
        for r in &list.resources {
            assert!(!r.uri.contains("/references/"));
        }
    }

    // ── Test 1.7a (registration order) ────────────────────────────────
    #[tokio::test]
    async fn test_1_7a_skills_into_handler_preserves_registration_order() {
        for _ in 0..10 {
            let handler = Skills::new()
                .add(Skill::new("zeta", ""))
                .add(Skill::new("alpha", ""))
                .add(Skill::new("mu", ""))
                .into_handler()
                .unwrap();
            let list = handler.list(None, extra()).await.unwrap();
            assert_eq!(list.resources.len(), 4);
            assert_eq!(list.resources[0].uri, "skill://zeta/SKILL.md");
            assert_eq!(list.resources[1].uri, "skill://alpha/SKILL.md");
            assert_eq!(list.resources[2].uri, "skill://mu/SKILL.md");
            assert_eq!(list.resources[3].uri, "skill://index.json");
        }
    }

    // ── Test 1.8 ──────────────────────────────────────────────────────
    #[test]
    fn test_1_8_skills_into_handler_duplicate_skill_md_uri_rejected() {
        match Skills::new()
            .add(Skill::new("refunds", "a"))
            .add(Skill::new("refunds", "b"))
            .into_handler()
        {
            Err(Error::Validation(msg)) => {
                assert!(msg.contains("skill://refunds/SKILL.md"), "msg = {msg}");
            },
            Err(other) => panic!("expected Validation, got {other:?}"),
            Ok(_) => panic!("expected Err for duplicate names"),
        }

        // Different names colliding via path.
        match Skills::new()
            .add(Skill::new("a", "").with_path("p"))
            .add(Skill::new("b", "").with_path("p"))
            .into_handler()
        {
            Err(Error::Validation(msg)) => assert!(msg.contains("skill://p/SKILL.md")),
            Err(other) => panic!("expected Validation, got {other:?}"),
            Ok(_) => panic!("expected Err for colliding paths"),
        }
    }

    // ── Test 1.8a (cross-skill reference URI duplicates) ──────────────
    #[test]
    fn test_1_8a_skills_into_handler_duplicate_reference_uri_rejected() {
        let s1 = Skill::new("a", "").with_reference(SkillReference::new(
            "references/shared.md",
            "text/markdown",
            "x",
        ));
        let s2 = Skill::new("b", "")
            .with_path("a")
            .with_reference(SkillReference::new(
                "references/shared.md",
                "text/markdown",
                "y",
            ));
        match Skills::new().add(s1).add(s2).into_handler() {
            Err(Error::Validation(msg)) => {
                assert!(
                    msg.contains("skill://a/references/shared.md"),
                    "msg = {msg}"
                );
                assert!(msg.contains("references="), "msg = {msg}");
            },
            Err(other) => panic!("expected Validation, got {other:?}"),
            Ok(_) => panic!("expected Err for colliding reference URIs"),
        }
    }

    // ── Test 1.9 ──────────────────────────────────────────────────────
    #[tokio::test]
    async fn test_1_9_skills_handler_list_excludes_references() {
        let s = Skill::new("a", "")
            .with_reference(SkillReference::new(
                "references/r1.md",
                "text/markdown",
                "1",
            ))
            .with_reference(SkillReference::new(
                "references/r2.md",
                "text/markdown",
                "2",
            ));
        let handler = Skills::new().add(s).into_handler().unwrap();
        let list = handler.list(None, extra()).await.unwrap();
        let skill_md_count = list
            .resources
            .iter()
            .filter(|r| r.uri == "skill://a/SKILL.md")
            .count();
        assert_eq!(skill_md_count, 1);
        for r in &list.resources {
            assert!(!r.uri.contains("/references/"), "leaked: {}", r.uri);
        }
        let index_count = list
            .resources
            .iter()
            .filter(|r| r.uri == "skill://index.json")
            .count();
        assert_eq!(index_count, 1);
    }

    // ── Test 1.10 (wire shape SKILL.md) ───────────────────────────────
    #[tokio::test]
    async fn test_1_10_skills_handler_read_skill_md_returns_resource_with_text() {
        let handler = Skills::new()
            .add(Skill::new("a", "the body"))
            .into_handler()
            .unwrap();
        let res = handler.read("skill://a/SKILL.md", extra()).await.unwrap();
        assert_eq!(res.contents.len(), 1);
        match &res.contents[0] {
            Content::Resource {
                uri,
                text,
                mime_type,
                ..
            } => {
                assert_eq!(uri, "skill://a/SKILL.md");
                assert_eq!(text.as_deref(), Some("the body"));
                assert_eq!(mime_type.as_deref(), Some("text/markdown"));
            },
            other => panic!("expected Content::Resource, got {other:?}"),
        }
    }

    // ── Test 1.11 (wire shape reference) ──────────────────────────────
    #[tokio::test]
    async fn test_1_11_skills_handler_read_reference_carries_per_resource_mime() {
        let s = Skill::new("a", "").with_reference(SkillReference::new(
            "references/schema.graphql",
            "application/graphql",
            "schema { query: Q }",
        ));
        let handler = Skills::new().add(s).into_handler().unwrap();
        let res = handler
            .read("skill://a/references/schema.graphql", extra())
            .await
            .unwrap();
        match &res.contents[0] {
            Content::Resource {
                uri,
                text,
                mime_type,
                ..
            } => {
                assert_eq!(uri, "skill://a/references/schema.graphql");
                assert_eq!(text.as_deref(), Some("schema { query: Q }"));
                assert_eq!(mime_type.as_deref(), Some("application/graphql"));
            },
            other => panic!("expected Content::Resource, got {other:?}"),
        }
    }

    // ── Test 1.12 (wire shape index) ──────────────────────────────────
    #[tokio::test]
    async fn test_1_12_skills_handler_read_index_returns_resource_with_text() {
        let s = Skill::new("a", "").with_reference(SkillReference::new(
            "references/r.md",
            "text/markdown",
            "x",
        ));
        let handler = Skills::new().add(s).into_handler().unwrap();
        let res = handler.read("skill://index.json", extra()).await.unwrap();
        match &res.contents[0] {
            Content::Resource {
                uri,
                text,
                mime_type,
                ..
            } => {
                assert_eq!(uri, "skill://index.json");
                assert_eq!(mime_type.as_deref(), Some("application/json"));
                let parsed: serde_json::Value =
                    serde_json::from_str(text.as_deref().unwrap()).unwrap();
                assert!(parsed.get("$schema").is_some());
                assert!(parsed.get("skills").is_some());
                let arr = parsed["skills"].as_array().unwrap();
                assert_eq!(arr.len(), 1);
                // Reference entries MUST NOT appear in the discovery index.
                let serialized = serde_json::to_string(&parsed).unwrap();
                assert!(!serialized.contains("references/r.md"));
            },
            other => panic!("expected Content::Resource, got {other:?}"),
        }
    }

    // ── Test 1.13 ─────────────────────────────────────────────────────
    #[tokio::test]
    async fn test_1_13_skills_handler_read_unknown_uri_method_not_found() {
        let handler = Skills::new()
            .add(Skill::new("a", "body"))
            .into_handler()
            .unwrap();
        let err = handler
            .read("skill://nonexistent/SKILL.md", extra())
            .await
            .expect_err("unknown URI must error");
        match err {
            Error::Protocol { code, .. } => assert_eq!(code, ErrorCode::METHOD_NOT_FOUND),
            other => panic!("expected Protocol, got {other:?}"),
        }

        let err = handler
            .read("skill://a/references/missing.md", extra())
            .await
            .expect_err("unknown reference must error");
        match err {
            Error::Protocol { code, .. } => assert_eq!(code, ErrorCode::METHOD_NOT_FOUND),
            other => panic!("expected Protocol, got {other:?}"),
        }
    }

    // ── Test 1.14 ─────────────────────────────────────────────────────
    #[tokio::test]
    async fn test_1_14_skill_prompt_handler_returns_byte_equal_text() {
        let skill = Skill::new("x", "A").with_reference(SkillReference::new(
            "ref1.md",
            "text/markdown",
            "refbody",
        ));
        let handler = SkillPromptHandler::new(skill.clone());
        let result = handler.handle(HashMap::new(), extra()).await.unwrap();
        assert_eq!(result.messages.len(), 1);
        assert_eq!(result.messages[0].role, Role::User);
        match &result.messages[0].content {
            Content::Text { text } => assert_eq!(text, &skill.as_prompt_text()),
            other => panic!("expected Content::Text, got {other:?}"),
        }
    }

    // ── Test 1.15 ─────────────────────────────────────────────────────
    struct DocsHandler;

    #[async_trait]
    impl ResourceHandler for DocsHandler {
        async fn read(&self, uri: &str, _extra: RequestHandlerExtra) -> Result<ReadResourceResult> {
            Ok(ReadResourceResult::new(vec![Content::text(format!(
                "DOCS:{uri}"
            ))]))
        }

        async fn list(
            &self,
            _cursor: Option<String>,
            _extra: RequestHandlerExtra,
        ) -> Result<ListResourcesResult> {
            Ok(ListResourcesResult::new(vec![ResourceInfo::new(
                "docs://handbook",
                "handbook",
            )]))
        }
    }

    #[tokio::test]
    async fn test_1_15_composed_resources_uri_prefix_routing() {
        let skills: Arc<dyn ResourceHandler> = Skills::new()
            .add(Skill::new("a", "skill-a"))
            .into_handler()
            .unwrap();
        let other: Arc<dyn ResourceHandler> = Arc::new(DocsHandler);
        let composed = ComposedResources { skills, other };

        let res = composed.read("skill://a/SKILL.md", extra()).await.unwrap();
        match &res.contents[0] {
            Content::Resource { uri, .. } => assert_eq!(uri, "skill://a/SKILL.md"),
            other => panic!("expected Content::Resource, got {other:?}"),
        }

        let res = composed.read("docs://handbook", extra()).await.unwrap();
        match &res.contents[0] {
            Content::Text { text } => assert_eq!(text, "DOCS:docs://handbook"),
            other => panic!("expected Content::Text, got {other:?}"),
        }

        let res = composed.read("ftp://foo", extra()).await.unwrap();
        match &res.contents[0] {
            Content::Text { text } => assert_eq!(text, "DOCS:ftp://foo"),
            other => panic!("expected Content::Text, got {other:?}"),
        }
    }

    // ── Test 1.16 ─────────────────────────────────────────────────────
    #[tokio::test]
    async fn test_1_16_composed_resources_list_concatenates_skills_first() {
        let skills: Arc<dyn ResourceHandler> = Skills::new()
            .add(Skill::new("a", ""))
            .into_handler()
            .unwrap();
        let other: Arc<dyn ResourceHandler> = Arc::new(DocsHandler);
        let composed = ComposedResources { skills, other };
        let list = composed.list(None, extra()).await.unwrap();
        // Skills first (SKILL.md + index = 2), then other (1).
        assert_eq!(list.resources.len(), 3);
        assert_eq!(list.resources[0].uri, "skill://a/SKILL.md");
        assert_eq!(list.resources[1].uri, "skill://index.json");
        assert_eq!(list.resources[2].uri, "docs://handbook");
    }

    // ── Test 1.17 (property: no reference ever listed) ────────────────
    fn skill_strategy() -> impl Strategy<Value = Skill> {
        let name = "[a-z]{1,8}";
        let ref_strategy = (
            "ref_[a-z]{1,6}\\.md",
            Just("text/markdown".to_string()),
            "[a-zA-Z]{1,12}",
        )
            .prop_map(|(p, m, b)| SkillReference::new(p, m, b));
        (
            name,
            "[a-zA-Z]{0,20}",
            proptest::collection::vec(ref_strategy, 0..=5),
        )
            .prop_map(|(name, body, refs)| {
                let mut s = Skill::new(name, body);
                // De-duplicate within a single skill by relative_path.
                let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
                for r in refs {
                    if seen.insert(r.relative_path().to_string()) {
                        s = s.with_reference(r);
                    }
                }
                s
            })
    }

    // Strategy that preserves references but uniquifies skill paths so
    // every reference URI is globally unique.
    fn skills_strategy_with_refs() -> impl Strategy<Value = Vec<Skill>> {
        proptest::collection::vec(skill_strategy(), 1..=10).prop_map(|skills| {
            skills
                .into_iter()
                .enumerate()
                .map(|(i, s)| {
                    let new_path = format!("p{i}");
                    let mut rebuilt = Skill::new(s.name().to_string(), s.body().to_string())
                        .with_path(new_path)
                        .with_description(s.resolved_description());
                    for r in s.references() {
                        rebuilt = rebuilt.with_reference(SkillReference::new(
                            r.relative_path(),
                            r.mime_type(),
                            r.body(),
                        ));
                    }
                    rebuilt
                })
                .collect()
        })
    }

    proptest! {
        #[test]
        fn prop_1_17_no_reference_ever_listed(skills in skills_strategy_with_refs()) {
            let mut registry = Skills::new();
            for s in skills {
                registry = registry.add(s);
            }
            // Skip inputs that produce duplicate URIs — covered by Test 1.18.
            let Ok(handler) = registry.into_handler() else { return Ok(()); };
            let rt = tokio::runtime::Runtime::new().unwrap();
            let list = rt.block_on(handler.list(None, RequestHandlerExtra::default())).unwrap();
            for r in &list.resources {
                prop_assert!(!r.uri.contains("/references/"), "leaked: {}", r.uri);
            }
        }
    }

    // ── Test 1.18 (property: duplicate URI always rejected) ───────────
    proptest! {
        #[test]
        fn prop_1_18_duplicate_uri_always_rejected(
            name in "[a-z]{1,6}",
            body_a in "[a-zA-Z]{0,12}",
            body_b in "[a-zA-Z]{0,12}",
        ) {
            // Same name → same skill_md_uri → always Err.
            let result = Skills::new()
                .add(Skill::new(name.clone(), body_a))
                .add(Skill::new(name, body_b))
                .into_handler();
            prop_assert!(result.is_err());
        }

        #[test]
        fn prop_1_18b_distinct_names_always_ok(
            name_a in "[a-z]{1,6}",
            name_b in "[a-z]{7,12}",
        ) {
            // Disjoint name lengths guarantee distinct names.
            prop_assume!(name_a != name_b);
            let result = Skills::new()
                .add(Skill::new(name_a, ""))
                .add(Skill::new(name_b, ""))
                .into_handler();
            prop_assert!(result.is_ok());
        }
    }

    // ── Test 1.19 (property: as_prompt_text byte-equal concat) ────────
    proptest! {
        #[test]
        fn prop_1_19_as_prompt_text_byte_equal_concat(skill in skill_strategy()) {
            // Manually concatenate the expected output.
            let mut expected = String::new();
            expected.push_str(skill.body());
            if !skill.body().ends_with('\n') {
                expected.push('\n');
            }
            for r in skill.references() {
                expected.push_str("\n--- ");
                expected.push_str(r.relative_path());
                expected.push_str(" ---\n");
                expected.push_str(r.body());
                if !r.body().ends_with('\n') {
                    expected.push('\n');
                }
            }
            prop_assert_eq!(skill.as_prompt_text(), expected);
        }
    }

    // ── Test 1.19a (property: read responses always have URI + MIME) ──
    fn collect_all_uris(skills: &[Skill]) -> Vec<String> {
        let mut uris: Vec<String> = vec!["skill://index.json".to_string()];
        for s in skills {
            uris.push(s.skill_md_uri());
            for r in s.references() {
                uris.push(s.reference_uri(r.relative_path()));
            }
        }
        uris
    }

    fn assert_read_response_has_uri_and_mime(
        contents: &[Content],
        expected_uri: &str,
    ) -> std::result::Result<(), proptest::test_runner::TestCaseError> {
        prop_assert_eq!(contents.len(), 1);
        match &contents[0] {
            Content::Resource {
                uri,
                text,
                mime_type,
                ..
            } => {
                prop_assert_eq!(uri, expected_uri);
                prop_assert!(text.is_some(), "text missing for {}", expected_uri);
                prop_assert!(mime_type.is_some(), "mime missing for {}", expected_uri);
                Ok(())
            },
            other => {
                prop_assert!(false, "expected Content::Resource, got {:?}", other);
                Ok(())
            },
        }
    }

    proptest! {
        #[test]
        fn prop_1_19a_read_responses_always_have_uri_and_mime(skills in skills_strategy_with_refs()) {
            let mut registry = Skills::new();
            for s in skills.clone() {
                registry = registry.add(s);
            }
            let Ok(handler) = registry.into_handler() else { return Ok(()); };
            let rt = tokio::runtime::Runtime::new().unwrap();
            let uris = collect_all_uris(&skills);
            for uri in uris {
                let Ok(res) = rt.block_on(handler.read(&uri, RequestHandlerExtra::default())) else { continue; };
                assert_read_response_has_uri_and_mime(&res.contents, &uri)?;
            }
        }
    }

    // ── Test 1.20 (with_reference validation panics) ──────────────────
    #[test]
    #[should_panic(expected = "must not be empty")]
    fn test_1_20_with_reference_panic_empty() {
        let _ =
            Skill::new("x", "b").with_reference(SkillReference::new("", "text/markdown", "body"));
    }

    #[test]
    #[should_panic(expected = "SKILL.md")]
    fn test_1_20_with_reference_panic_skill_md_collision() {
        let _ = Skill::new("x", "b").with_reference(SkillReference::new(
            "SKILL.md",
            "text/markdown",
            "body",
        ));
    }

    #[test]
    #[should_panic(expected = "..")]
    fn test_1_20_with_reference_panic_dotdot() {
        let _ = Skill::new("x", "b").with_reference(SkillReference::new(
            "../escape.md",
            "text/markdown",
            "body",
        ));
    }

    #[test]
    #[should_panic(expected = "leading")]
    fn test_1_20_with_reference_panic_absolute() {
        let _ = Skill::new("x", "b").with_reference(SkillReference::new(
            "/abs/path.md",
            "text/markdown",
            "body",
        ));
    }

    #[test]
    #[should_panic(expected = "URI scheme")]
    fn test_1_20_with_reference_panic_scheme() {
        let _ = Skill::new("x", "b").with_reference(SkillReference::new(
            "http://example.com/x",
            "text/markdown",
            "body",
        ));
    }

    #[test]
    #[should_panic(expected = "already registered")]
    fn test_1_20_with_reference_panic_duplicate_within_skill() {
        let _ = Skill::new("x", "b")
            .with_reference(SkillReference::new("a.md", "text/markdown", "body1"))
            .with_reference(SkillReference::new("a.md", "text/markdown", "body2"));
    }

    // ── Test 1.20a (try_with_reference returns Err) ───────────────────
    #[test]
    fn test_1_20a_try_with_reference_returns_err() {
        let invalid = [
            "",
            "SKILL.md",
            "../escape.md",
            "/abs/path.md",
            "http://example.com/x",
        ];
        for p in invalid {
            let res = Skill::new("x", "b").try_with_reference(SkillReference::new(
                p,
                "text/markdown",
                "body",
            ));
            assert!(res.is_err(), "expected Err for path = {p:?}");
            assert!(matches!(res.unwrap_err(), Error::Validation(_)));
        }
        // Duplicate within skill.
        let res = Skill::new("x", "b")
            .try_with_reference(SkillReference::new("a.md", "text/markdown", "1"))
            .and_then(|s| s.try_with_reference(SkillReference::new("a.md", "text/markdown", "2")));
        assert!(res.is_err());

        // Ok case.
        let res = Skill::new("x", "b").try_with_reference(SkillReference::new(
            "references/ok.md",
            "text/markdown",
            "body",
        ));
        assert!(res.is_ok());
    }

    // ── Test 1.21 (Skills::merge) ─────────────────────────────────────
    #[tokio::test]
    async fn test_1_21_skills_merge_concatenates() {
        let combined = Skills::new()
            .add(Skill::new("a", ""))
            .merge(Skills::new().add(Skill::new("b", "")));
        let handler = combined.into_handler().unwrap();
        let list = handler.list(None, extra()).await.unwrap();
        assert_eq!(list.resources.len(), 3);
        assert_eq!(list.resources[0].uri, "skill://a/SKILL.md");
        assert_eq!(list.resources[1].uri, "skill://b/SKILL.md");
        assert_eq!(list.resources[2].uri, "skill://index.json");
    }

    // ── SEP-2640 entry synthesis (Phase 125, plan 01) ─────────────────

    /// `parse_frontmatter_value` distinguishes ABSENT from PARSED — the happy
    /// pair, on both LF and CRLF bodies (RESEARCH A2/A4).
    #[test]
    fn parse_frontmatter_value_absent_and_parsed() {
        // No delimiter block at all → Absent.
        assert!(matches!(
            parse_frontmatter_value("just a body\nwith no frontmatter\n"),
            FrontmatterParse::Absent
        ));
        // An empty body is Absent, not Invalid.
        assert!(matches!(
            parse_frontmatter_value(""),
            FrontmatterParse::Absent
        ));
        // A markdown horizontal rule that is NOT at the start does not open a
        // phantom block — this is why the opening delimiter must be line 1.
        assert!(matches!(
            parse_frontmatter_value("# Title\n\n---\n\nmore prose\n\n---\n"),
            FrontmatterParse::Absent
        ));

        // LF frontmatter, including a NESTED field the shipped line scanner
        // cannot represent.
        let lf =
            "---\nname: refunds\ndescription: Issue refunds\nmetadata:\n  tier: gold\n---\n# R\n";
        let FrontmatterParse::Parsed(value) = parse_frontmatter_value(lf) else {
            panic!("LF frontmatter must parse");
        };
        assert_eq!(value["name"], "refunds");
        assert_eq!(value["description"], "Issue refunds");
        assert_eq!(value["metadata"]["tier"], "gold");

        // CRLF must behave IDENTICALLY.
        let crlf = lf.replace('\n', "\r\n");
        let FrontmatterParse::Parsed(crlf_value) = parse_frontmatter_value(&crlf) else {
            panic!("CRLF frontmatter must parse");
        };
        assert_eq!(
            crlf_value, value,
            "CRLF and LF must produce the same object"
        );

        // A leading UTF-8 BOM is stripped, as it is for the description scanner.
        let bommed = format!("\u{FEFF}{lf}");
        assert!(matches!(
            parse_frontmatter_value(&bommed),
            FrontmatterParse::Parsed(_)
        ));
    }

    /// `parse_frontmatter_value` reports INVALID — never `Absent` — for a block
    /// that is present but unusable (R-04). Three distinct causes.
    #[test]
    fn parse_frontmatter_value_invalid_is_not_absent() {
        // 1. Unterminated block.
        let FrontmatterParse::Invalid(reason) =
            parse_frontmatter_value("---\nname: x\ndescription: d\n")
        else {
            panic!("an unterminated block must be Invalid, never Absent");
        };
        assert!(
            reason.contains("never closed"),
            "diagnostic must name the cause, got: {reason}"
        );

        // 2. Present, terminated, but not valid YAML.
        let FrontmatterParse::Invalid(reason) =
            parse_frontmatter_value("---\nname: [unclosed\n---\nbody\n")
        else {
            panic!("a malformed YAML block must be Invalid");
        };
        assert!(
            reason.contains("not valid YAML"),
            "diagnostic must name the cause, got: {reason}"
        );

        // 3. Valid YAML that is a SEQUENCE, not a mapping.
        //
        // The bound VARIANT moved from `Invalid` to `NotAMapping` when plan
        // 125-03 split the failure shapes (R-20); the ASSERTION below is
        // unchanged, because the diagnostic text is the same and it is the text
        // an author reads.
        let FrontmatterParse::NotAMapping(reason) =
            parse_frontmatter_value("---\n- one\n- two\n---\nbody\n")
        else {
            panic!("a YAML sequence must be NotAMapping");
        };
        assert!(
            reason.contains("must be a YAML mapping"),
            "diagnostic must name the cause, got: {reason}"
        );
    }

    /// The digest format, and the reason it is not `format!("{:x}", ..)`.
    #[test]
    fn sha256_digest_hex_is_prefixed_lowercase_64_hex() {
        // The published SHA-256 of the empty input.
        assert_eq!(
            sha256_digest_hex(b""),
            "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        let d = sha256_digest_hex(b"abc");
        assert_eq!(d.len(), "sha256:".len() + 64);
        let hex = d
            .strip_prefix("sha256:")
            .expect("carries the sha256: prefix");
        assert!(
            hex.chars()
                .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)),
            "digest must be 64 LOWERCASE hex characters, got {hex}"
        );
    }

    /// One frontmatter-bearing skill yields exactly one conforming entry.
    #[test]
    fn entries_synthesizes_one_conforming_entry() {
        let body =
            "---\nname: refunds\ndescription: Issue refunds\nlicense: Apache-2.0\n---\n# Refunds\n";
        let entries = Skills::new()
            .add(Skill::new("refunds", body))
            .entries()
            .expect("entries build");

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].uri(), "skill://refunds/SKILL.md");
        // Verbatim: all three authored fields, including the non-required one.
        assert_eq!(entries[0].frontmatter()["name"], "refunds");
        assert_eq!(entries[0].frontmatter()["description"], "Issue refunds");
        assert_eq!(entries[0].frontmatter()["license"], "Apache-2.0");

        let manifest = entries[0].resources();
        assert_eq!(manifest.len(), 1, "SKILL.md is the only manifest row here");
        assert_eq!(manifest[0].uri(), "skill://refunds/SKILL.md");
        assert_eq!(manifest[0].size(), body.len());
        assert_eq!(manifest[0].digest(), sha256_digest_hex(body.as_bytes()));
    }

    /// Skills with no frontmatter, or a broken block, are EXCLUDED rather than
    /// synthesized — and the exclusion does not disturb registration order for
    /// the skills that do conform.
    #[test]
    fn entries_excludes_frontmatter_less_and_malformed_skills() {
        let good_a = "---\nname: a\ndescription: da\n---\nbody-a\n";
        let good_b = "---\nname: b\ndescription: db\n---\nbody-b\n";
        let entries = Skills::new()
            .add(Skill::new("a", good_a))
            .add(Skill::new("bare", "no frontmatter here"))
            .add(Skill::new("broken", "---\nname: [unclosed\n---\nbody\n"))
            .add(Skill::new("b", good_b))
            .entries()
            .expect("entries build");

        let uris: Vec<&str> = entries.iter().map(SkillEntry::uri).collect();
        assert_eq!(
            uris,
            vec!["skill://a/SKILL.md", "skill://b/SKILL.md"],
            "entry order equals REGISTRATION order, with non-conformers dropped"
        );

        // An EMPTY registry is an empty entry set, not an error.
        assert!(Skills::new().entries().expect("entries build").is_empty());
    }

    /// The serialized shape is the wire shape: exactly the declared key sets.
    #[test]
    fn entry_serialization_emits_exactly_the_wire_keys() {
        let entries = Skills::new()
            .add(Skill::new("x", "---\nname: x\ndescription: d\n---\nbody\n"))
            .entries()
            .expect("entries build");
        let value = serde_json::to_value(&entries[0]).expect("entry serializes");

        let obj = value.as_object().expect("an entry is a JSON object");
        let mut keys: Vec<&str> = obj.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(keys, vec!["frontmatter", "resources", "uri"]);

        let row = value["resources"][0].as_object().expect("a manifest row");
        let mut row_keys: Vec<&str> = row.keys().map(String::as_str).collect();
        row_keys.sort_unstable();
        assert_eq!(row_keys, vec!["digest", "size", "uri"]);
        assert!(
            value["resources"][0]["size"].is_u64(),
            "size is a byte count"
        );
    }

    // ── Complete manifests + verbatim frontmatter (Phase 125, plan 03) ──

    /// A two-reference skill yields a THREE-row manifest: SKILL.md at index 0,
    /// then the references in REGISTRATION order.
    #[test]
    fn entries_manifest_lists_skill_md_first_then_every_reference() {
        let body = "---\nname: refunds\ndescription: Issue refunds\n---\n# Refunds\n";
        let policy = "# Policy\n\nRefund within 30 days.\n";
        let email = "Dear customer,\n";
        let entries = Skills::new()
            .add(
                Skill::new("refunds", body)
                    .with_reference(SkillReference::new(
                        "references/policy.md",
                        "text/markdown",
                        policy,
                    ))
                    .with_reference(SkillReference::new(
                        "examples/email.md",
                        "text/markdown",
                        email,
                    )),
            )
            .entries()
            .expect("entries build");

        let manifest = entries[0].resources();
        assert_eq!(manifest.len(), 3, "1 SKILL.md + 2 references");
        assert_eq!(manifest[0].uri(), "skill://refunds/SKILL.md");
        assert_eq!(manifest[1].uri(), "skill://refunds/references/policy.md");
        assert_eq!(manifest[2].uri(), "skill://refunds/examples/email.md");

        assert_eq!(manifest[0].size(), body.len());
        assert_eq!(manifest[1].size(), policy.len());
        assert_eq!(manifest[2].size(), email.len());
        assert_eq!(manifest[0].digest(), sha256_digest_hex(body.as_bytes()));
        assert_eq!(manifest[1].digest(), sha256_digest_hex(policy.as_bytes()));
        assert_eq!(manifest[2].digest(), sha256_digest_hex(email.as_bytes()));
    }

    /// Entry order across a multi-skill registry equals REGISTRATION order —
    /// the `IndexMap` contract, with no response-time sort.
    #[test]
    fn entries_preserve_registration_order_across_skills() {
        let fm = |n: &str| format!("---\nname: {n}\ndescription: d\n---\nbody\n");
        let entries = Skills::new()
            .add(Skill::new("zeta", fm("zeta")))
            .add(Skill::new("alpha", fm("alpha")))
            .add(Skill::new("mu", fm("mu")))
            .entries()
            .expect("entries build");
        let uris: Vec<&str> = entries.iter().map(SkillEntry::uri).collect();
        assert_eq!(
            uris,
            vec![
                "skill://zeta/SKILL.md",
                "skill://alpha/SKILL.md",
                "skill://mu/SKILL.md"
            ]
        );
    }

    /// Frontmatter is emitted VERBATIM: a nested mapping stays a JSON object, a
    /// list stays a JSON array, and a non-required scalar survives untouched.
    #[test]
    fn entries_frontmatter_is_verbatim_including_nested_and_list_fields() {
        let body = "---\n\
                    name: refunds\n\
                    description: Issue refunds\n\
                    license: Apache-2.0\n\
                    keywords:\n  - billing\n  - support\n\
                    metadata:\n  tier: gold\n  owner:\n    team: payments\n\
                    ---\n# Refunds\n";
        let entries = Skills::new()
            .add(Skill::new("refunds", body))
            .entries()
            .expect("entries build");
        let fm = entries[0].frontmatter();

        let obj = fm.as_object().expect("frontmatter is a JSON object");
        let mut keys: Vec<&str> = obj.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            vec!["description", "keywords", "license", "metadata", "name"],
            "all five authored keys survive"
        );

        assert_eq!(fm["license"], "Apache-2.0");
        assert_eq!(
            fm["keywords"],
            serde_json::json!(["billing", "support"]),
            "a list-valued field is a JSON array"
        );
        assert_eq!(
            fm["metadata"],
            serde_json::json!({"tier": "gold", "owner": {"team": "payments"}}),
            "a nested mapping is a JSON object, arbitrarily deep"
        );
    }

    /// `with_description` is an OVERRIDE of the resolved description; it must
    /// never leak into the emitted frontmatter, which SEP-2640 requires to be
    /// byte-faithful to the SKILL.md the host will fetch.
    #[test]
    fn entries_frontmatter_ignores_the_with_description_override() {
        let body = "---\nname: x\ndescription: AUTHORED\n---\nbody\n";
        let skill = Skill::new("x", body).with_description("OVERRIDE");
        assert_eq!(
            skill.resolved_description(),
            "OVERRIDE",
            "the override is live on the accessor"
        );

        let entries = Skills::new().add(skill).entries().expect("entries build");
        assert_eq!(
            entries[0].frontmatter()["description"],
            "AUTHORED",
            "the emitted object comes from the FILE, not from the override"
        );
    }

    /// An LF-authored SKILL.md and its CRLF twin produce identical frontmatter
    /// JSON — the existing CRLF lock, now asserted at the entry level.
    #[test]
    fn entries_frontmatter_is_identical_for_lf_and_crlf() {
        let lf =
            "---\nname: widget\ndescription: Build widgets\nmetadata:\n  tier: gold\n---\n# W\n";
        let crlf = lf.replace('\n', "\r\n");

        let lf_entries = Skills::new()
            .add(Skill::new("widget", lf))
            .entries()
            .expect("entries build");
        let crlf_entries = Skills::new()
            .add(Skill::new("widget", crlf))
            .entries()
            .expect("entries build");

        assert_eq!(
            lf_entries[0].frontmatter(),
            crlf_entries[0].frontmatter(),
            "line endings must not reach the emitted frontmatter"
        );
    }

    /// Property: for EVERY generated registry, every manifest row's digest is
    /// `sha256:` + 64 lowercase hex, its `size` is the length of the bytes the
    /// `ResourceHandler` actually serves for that URI, and the manifest holds
    /// exactly `1 + references().count()` rows.
    ///
    /// The served bytes are read back THROUGH the handler rather than
    /// re-derived from the `Skill`, so the property cannot pass by both sides
    /// making the same mistake (T-125-14).
    ///
    /// The generator is the existing [`skills_strategy_with_refs`] with a
    /// conforming frontmatter block prepended to each body — without one every
    /// generated skill takes the D-02 exclusion path and the property would hold
    /// vacuously over an empty entry set, which the anti-vacuity assertion below
    /// refuses.
    fn annotate_with_frontmatter(skills: &[Skill]) -> Vec<Skill> {
        skills
            .iter()
            .map(|s| {
                // The frontmatter `name` must equal the final URI segment, which
                // `skills_strategy_with_refs` rewrote to `p{i}`.
                let path = s.resolved_path().to_string();
                let body = format!(
                    "---\nname: {path}\ndescription: generated\n---\n{}\n",
                    s.body()
                );
                let mut rebuilt = Skill::new(s.name().to_string(), body).with_path(path);
                for r in s.references() {
                    rebuilt = rebuilt.with_reference(SkillReference::new(
                        r.relative_path(),
                        r.mime_type(),
                        r.body(),
                    ));
                }
                rebuilt
            })
            .collect()
    }

    proptest! {
        #[test]
        fn prop_manifest_rows_are_the_bytes_the_handler_serves(
            skills in skills_strategy_with_refs(),
        ) {
            let annotated = annotate_with_frontmatter(&skills);
            let mut registry = Skills::new();
            for s in &annotated {
                registry = registry.add(s.clone());
            }

            let entries = registry.entries().expect("entries build");
            prop_assert_eq!(
                entries.len(),
                annotated.len(),
                "anti-vacuity: every annotated skill must yield an entry"
            );

            let handler = registry.into_handler().expect("unique p{i} paths cannot collide");
            let rt = tokio::runtime::Runtime::new().unwrap();

            for (entry, skill) in entries.iter().zip(annotated.iter()) {
                prop_assert_eq!(
                    entry.resources().len(),
                    1 + skill.references().count(),
                    "manifest is SKILL.md plus every reference"
                );
                for row in entry.resources() {
                    let hex = row.digest().strip_prefix("sha256:");
                    prop_assert!(hex.is_some(), "digest must carry the sha256: prefix");
                    let hex = hex.unwrap();
                    prop_assert_eq!(hex.len(), 64);
                    prop_assert!(
                        hex.chars().all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)),
                        "digest hex must be 64 LOWERCASE hex characters"
                    );

                    let read = rt
                        .block_on(handler.read(row.uri(), RequestHandlerExtra::default()))
                        .expect("every manifest URI must be readable");
                    let served = match &read.contents[0] {
                        Content::Resource { text, .. } => {
                            text.clone().expect("skills handler always emits text")
                        },
                        other => panic!("expected Content::Resource, got {other:?}"),
                    };
                    prop_assert_eq!(row.size(), served.len());
                    prop_assert_eq!(row.digest(), sha256_digest_hex(served.as_bytes()));
                }
            }
        }
    }

    // ── D-02 warn-and-exclude diagnostics (Phase 125, plan 03) ─────────

    /// A mixed registry yields exactly ONE entry and exactly ONE diagnostic:
    /// the frontmatter-bearing skill is listed, the bare one is named.
    #[test]
    fn entries_with_diagnostics_excludes_one_and_names_it() {
        let (entries, diagnostics) = Skills::new()
            .add(Skill::new(
                "good",
                "---\nname: good\ndescription: d\n---\nbody\n",
            ))
            .add(Skill::new("bare", "# Bare\n\nNo frontmatter.\n"))
            .entries_with_diagnostics()
            .expect("entries build");

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].uri(), "skill://good/SKILL.md");
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].uri(), "skill://bare/SKILL.md");
        assert!(
            diagnostics[0].message().contains("skill://bare/SKILL.md"),
            "the warning must NAME the excluded skill, got: {}",
            diagnostics[0].message()
        );
    }

    /// A body with NO delimited block produces the frontmatter-ABSENT
    /// diagnostic, and the warning says the block is missing.
    #[test]
    fn entries_diagnose_an_absent_frontmatter_block() {
        let (entries, diagnostics) = Skills::new()
            .add(Skill::new("bare", "# Bare\n\nNo frontmatter.\n"))
            .entries_with_diagnostics()
            .expect("entries build");
        assert!(entries.is_empty());
        assert!(
            matches!(diagnostics[0], SkillDiagnostic::FrontmatterAbsent { .. }),
            "got {:?}",
            diagnostics[0]
        );
        assert!(diagnostics[0].message().contains("NO frontmatter"));
    }

    /// A block that is present but unterminated, or present but unparseable,
    /// produces the frontmatter-INVALID diagnostic — never the absent one —
    /// and the warning carries the parser's own message (R-20).
    #[test]
    fn entries_diagnose_an_invalid_frontmatter_block() {
        for (body, expected_reason) in [
            ("---\nname: x\ndescription: d\n", "never closed"),
            ("---\nname: [unclosed\n---\nbody\n", "not valid YAML"),
        ] {
            let (entries, diagnostics) = Skills::new()
                .add(Skill::new("x", body))
                .entries_with_diagnostics()
                .expect("entries build");
            assert!(entries.is_empty());
            let SkillDiagnostic::FrontmatterInvalid { uri, reason } = &diagnostics[0] else {
                panic!("expected FrontmatterInvalid, got {:?}", diagnostics[0]);
            };
            assert_eq!(uri, "skill://x/SKILL.md");
            assert!(
                reason.contains(expected_reason),
                "reason must carry the parser's message, got: {reason}"
            );
            assert!(
                diagnostics[0].message().contains("not a missing one"),
                "the warning must distinguish a BROKEN block from an absent one"
            );
        }
    }

    /// A block that parses to a YAML sequence or a scalar produces the
    /// NOT-A-MAPPING diagnostic (R-20).
    #[test]
    fn entries_diagnose_a_non_mapping_frontmatter_block() {
        for body in [
            "---\n- one\n- two\n---\nbody\n",
            "---\njust a scalar\n---\nbody\n",
            "---\n\n---\nbody\n",
        ] {
            let (entries, diagnostics) = Skills::new()
                .add(Skill::new("x", body))
                .entries_with_diagnostics()
                .expect("entries build");
            assert!(entries.is_empty(), "body = {body:?}");
            let SkillDiagnostic::FrontmatterNotAMapping { uri, reason } = &diagnostics[0] else {
                panic!(
                    "expected FrontmatterNotAMapping for {body:?}, got {:?}",
                    diagnostics[0]
                );
            };
            assert_eq!(uri, "skill://x/SKILL.md");
            assert!(reason.contains("must be a YAML mapping"), "got: {reason}");
        }
    }

    /// The three exclusion shapes are DISTINCT variants, not one collapsed
    /// case. A test that only checked "a diagnostic was produced" would pass
    /// against the collapsed design this replaces (R-20).
    #[test]
    fn the_three_frontmatter_diagnostics_are_distinct_variants() {
        let of = |body: &str| {
            Skills::new()
                .add(Skill::new("x", body))
                .entries_with_diagnostics()
                .expect("entries build")
                .1
                .remove(0)
        };
        let absent = of("# Bare\n");
        let invalid = of("---\nname: [unclosed\n---\nbody\n");
        let not_mapping = of("---\n- one\n---\nbody\n");

        assert!(matches!(absent, SkillDiagnostic::FrontmatterAbsent { .. }));
        assert!(matches!(
            invalid,
            SkillDiagnostic::FrontmatterInvalid { .. }
        ));
        assert!(matches!(
            not_mapping,
            SkillDiagnostic::FrontmatterNotAMapping { .. }
        ));

        // Pairwise distinct — same URI, three different findings.
        assert_ne!(absent, invalid);
        assert_ne!(invalid, not_mapping);
        assert_ne!(absent, not_mapping);
        assert_ne!(absent.message(), invalid.message());
        assert_ne!(invalid.message(), not_mapping.message());
        assert_ne!(absent.message(), not_mapping.message());
    }

    /// YAML robustness (R-25): an anchor/alias block resolves to the aliased
    /// value in the emitted JSON, and a twenty-level nested block either parses
    /// or diagnoses. Neither may panic, loop, or exhaust the stack.
    #[test]
    fn entries_survive_yaml_anchors_and_deep_nesting() {
        let aliased = "---\nname: x\ndescription: d\ndefaults: &shared\n  tier: gold\n\
                       metadata: *shared\n---\nbody\n";
        let (entries, diagnostics) = Skills::new()
            .add(Skill::new("x", aliased))
            .entries_with_diagnostics()
            .expect("entries build");
        assert!(diagnostics.is_empty(), "got {diagnostics:?}");
        assert_eq!(
            entries[0].frontmatter()["metadata"]["tier"],
            "gold",
            "the alias must RESOLVE in the emitted JSON"
        );

        // Twenty levels deep.
        let mut deep = String::from("---\nname: x\ndescription: d\ndeep:\n");
        for level in 1..=20u32 {
            let indent = "  ".repeat(level as usize);
            deep.push_str(&format!("{indent}l{level}:\n"));
        }
        deep.push_str(&format!("{}leaf: bottom\n", "  ".repeat(21)));
        deep.push_str("---\nbody\n");

        let (entries, diagnostics) = Skills::new()
            .add(Skill::new("x", deep))
            .entries_with_diagnostics()
            .expect("entries build");
        // EITHER outcome is acceptable — a panic or a hang is not.
        assert_eq!(
            entries.len() + diagnostics.len(),
            1,
            "a deep block must parse or diagnose, exactly once"
        );
    }

    /// A registry of ONLY frontmatter-less skills yields an empty entry vector
    /// and `Ok` — an empty listing is SEP-legal ("MAY return an empty or
    /// partial listing"), never an error.
    #[test]
    fn entries_on_an_all_bare_registry_is_ok_and_empty() {
        let result = Skills::new()
            .add(Skill::new("a", "body-a"))
            .add(Skill::new("b", "body-b"))
            .add(Skill::new("c", ""))
            .entries();
        let entries = result.expect("an all-bare registry is Ok, not Err");
        assert!(entries.is_empty());
    }

    /// A capturing `tracing` subscriber counts the WARN events `entries()`
    /// actually emits (R-24).
    ///
    /// Without this, "emits a warning" is a contractual claim nothing checks:
    /// the diagnostics are asserted elsewhere, but the loop that turns them
    /// into warnings could be deleted and every other test would stay green.
    /// Proven by construction — the assertion is on CAPTURED events, not on the
    /// diagnostic count.
    mod warn_capture {
        use std::sync::{Arc, Mutex};

        /// A minimal thread-local `tracing` subscriber, hand-written against
        /// `tracing` itself rather than pulled from `tracing-subscriber`, so
        /// this test carries no feature gate beyond the module's own. Mirrors
        /// the shipped idiom in `crate::testing`'s `capture` module.
        pub(super) struct WarnCollector {
            pub(super) events: Arc<Mutex<Vec<String>>>,
        }

        #[derive(Default)]
        struct Fields {
            text: String,
        }

        impl Fields {
            fn push(&mut self, name: &str, value: &str) {
                use std::fmt::Write as _;
                let _ = write!(self.text, "{name}={value} ");
            }
        }

        impl tracing::field::Visit for Fields {
            fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
                self.push(field.name(), value);
            }

            fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
                self.push(field.name(), &format!("{value:?}"));
            }
        }

        impl tracing::Subscriber for WarnCollector {
            fn enabled(&self, _metadata: &tracing::Metadata<'_>) -> bool {
                true
            }

            fn new_span(&self, _span: &tracing::span::Attributes<'_>) -> tracing::span::Id {
                tracing::span::Id::from_u64(1)
            }

            fn record(&self, _span: &tracing::span::Id, _values: &tracing::span::Record<'_>) {}

            fn record_follows_from(&self, _span: &tracing::span::Id, _follows: &tracing::span::Id) {
            }

            fn event(&self, event: &tracing::Event<'_>) {
                if *event.metadata().level() != tracing::Level::WARN {
                    return;
                }
                let mut fields = Fields::default();
                event.record(&mut fields);
                if let Ok(mut events) = self.events.lock() {
                    events.push(fields.text);
                }
            }

            fn enter(&self, _span: &tracing::span::Id) {}

            fn exit(&self, _span: &tracing::span::Id) {}
        }
    }

    #[test]
    fn entries_emits_exactly_one_warn_event_per_diagnostic() {
        let registry = Skills::new()
            .add(Skill::new(
                "good",
                "---\nname: good\ndescription: d\n---\nb\n",
            ))
            .add(Skill::new("bare", "no frontmatter"))
            .add(Skill::new("broken", "---\nname: [unclosed\n---\nb\n"))
            .add(Skill::new("seq", "---\n- one\n---\nb\n"));

        let (entries, diagnostics) = registry.entries_with_diagnostics().expect("entries build");
        assert_eq!(entries.len(), 1);
        assert_eq!(diagnostics.len(), 3);

        let events = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let collector = warn_capture::WarnCollector {
            events: std::sync::Arc::clone(&events),
        };
        let logged = tracing::subscriber::with_default(collector, || {
            registry.entries().expect("entries build")
        });
        assert_eq!(logged.len(), 1, "the wrapper returns only the entries");

        let captured = events.lock().unwrap().clone();
        assert_eq!(
            captured.len(),
            diagnostics.len(),
            "exactly one WARN per diagnostic, captured: {captured:?}"
        );
        for diagnostic in &diagnostics {
            assert!(
                captured.iter().any(|e| e.contains(diagnostic.uri())),
                "no captured WARN names {}; captured: {captured:?}",
                diagnostic.uri()
            );
        }
    }

    /// `with_path` moves the URI, and the manifest follows it.
    #[test]
    fn entries_honor_with_path() {
        let entries = Skills::new()
            .add(
                Skill::new("refunds", "---\nname: refunds\ndescription: d\n---\nb\n")
                    .with_path("acme/billing/refunds"),
            )
            .entries()
            .expect("entries build");
        assert_eq!(entries[0].uri(), "skill://acme/billing/refunds/SKILL.md");
        assert_eq!(
            entries[0].resources()[0].uri(),
            "skill://acme/billing/refunds/SKILL.md"
        );
    }
}
