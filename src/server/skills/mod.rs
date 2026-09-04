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
//! # Transport reach: the two SEP-2640 methods are answered over streamable HTTP
//!
//! `skills/list` and `skills/get` are routed through the crate-private
//! `InternalClientRequest` classifier, which adds NO variant to the public
//! `ClientRequest` enum (a 2.x semver promise). The consequence a server author
//! must plan around is that the classifier's only production consumer is
//! `classify_http_ingress` in `src/server/streamable_http_server.rs`. Every other
//! transport reaches requests through the PUBLIC parse path, which maps an
//! internally-routed method to a method-not-found error — so these two methods
//! are served over streamable HTTP and are unavailable everywhere else. The
//! `resources/list` / `resources/read` surface below is transport-independent and
//! is unaffected; only the two RPC methods are narrowed.
//!
//! **Over stdio the failure is harsher than a method-not-found reply, and it was
//! measured rather than assumed.** The frame fails inside the transport's own
//! `parse_method_message`, which turns the parse error into a
//! `TransportError::InvalidMessage`; the server actor's receive arm logs that and
//! BREAKS its loop. The connection therefore tears down instead of answering.
//! `tests/skills_routing.rs::stdio_ingress_rejects_a_skills_list_frame` pins this,
//! with a `resources/list` control that must parse so the assertion cannot pass
//! against a transport that rejects everything.
//!
//! Widening the reach is a NON-semver-breaking follow-on owned by the next skills
//! phase of the v2.7 milestone (the classifier seam lives in `shared/` and is
//! transport-agnostic by design). It is nonetheless a bigger change than the
//! skills work itself: the server actor's request channel is typed
//! `(RequestId, Request)` over the PUBLIC request enum, so widening means changing
//! that channel's type or adding a second one.
//!
//! # Reach boundary: a `ServerCore` serves skill RESOURCES, not the skill METHODS
//!
//! The same limit in a second place, and a reader who knows only the stdio half
//! has an incomplete picture. A `ServerCore` built through `ServerCoreBuilder`
//! serves its registered skills through `resources/list` and `resources/read`, and
//! answers NEITHER skills method. The cause is the same typed ingress: a
//! `ServerCore`'s only request entry point is `ProtocolHandler::handle_request`,
//! which accepts the public `Request` enum, and this phase adds no variant to it.
//! The streamable-HTTP transport dispatches through the high-level `Server`, which
//! is where the two methods land.
//!
//! Widening this reach has a stated precondition: `ProtocolHandler` needs an
//! ingress that accepts an internally-routed request — the same change the stdio
//! deferral above needs. Adding delegates first produces code nothing can call,
//! which is exactly what Phase 112 deleted when it consolidated `server/discover`.
//! The boundary is enforced, not merely described:
//! `tests/skills_routing.rs::server_core_declares_no_skills_field_or_skills_method`
//! fails if a `ServerCore` skills delegate appears without that ingress.
//!
//! # What this phase deliberately did not implement, and who owns each
//!
//! Recorded here as prose, never as a code marker: `make check-todos` fails the
//! build on a self-admitted-debt comment anywhere in `src/`, and a deferral with
//! an owner is documentation rather than debt.
//!
//! | Deferred | Why it is legal to defer | Owner |
//! |---|---|---|
//! | Stdio (and every non-HTTP transport) reach for the two methods | The extension's methods are answered on the transport this SDK's remote deployments use; see the section above for the measured stdio behaviour | next skills phase, v2.7 |
//! | The `ServerCore` method reach | Same typed-ingress cause; a delegate added first would be unreachable code | same follow-on, gated on the `ProtocolHandler` ingress change |
//! | `resources/directory/read` | The capability declaration is an EMPTY object, which is exactly how SEP-2640 spells `directoryRead: false`. Declaring nothing and implementing nothing is conformant; the optional feature is simply off | a later v2.7 phase, if a host asks for it |
//! | A strict frontmatter mode (a `try_`-style constructor that ERRORS instead of excluding) | D-02 chose warn-and-exclude because 40+ existing `Skill::new(..)` call sites, both proptest strategies and this module's own doctests build frontmatter-less skills. A hard error is a behaviour break that belongs after the canonical surfaces are all conforming | a later v2.7 phase, after D-03's cleanup has settled |
//! | Cursor pagination for `skills/list` | D-11: a single page with no `nextCursor` is conformant — an absent cursor means the listing is complete — and the shipped handler already ignores its cursor argument | revisit when a registry with hundreds of skills exists |
//!
//! # Neither skills method is name-bearing under the v2 routing-header cross-check
//!
//! `Mcp-Name` carries the primary subject of a request for the methods that have
//! one (`tools/call` and `prompts/get` carry `name`, `resources/read` carries
//! `uri`, the `tasks/*` trio carries `taskId`). `skills/get` does take a `uri`, so
//! its absence from that table is a decision and not an oversight — and it is a
//! decision with a stated cost. Adding either method would require editing BOTH
//! `contracts/mcp-protocol-sdk-v1.yaml`'s method table AND the literal-contract
//! test beside `is_name_bearing_method` in
//! `src/server/streamable_http_server.rs`, so it belongs to a phase that owns the
//! header contract rather than to one that adds a method.
//! `tests/skills_routing.rs::neither_skills_method_is_routing_name_bearing`
//! resolves the property through the production seam rather than restating a
//! method list, so it stays true if the table moves.
//!
//! Byte-equal mirror of the doctest at the end of `pmcp-book/src/ch12-8-skills.md`.
//!
//! ```rust,no_run
//! use pmcp::server::skills::Skill;
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // SEP-2640 frontmatter. `name` MUST equal the final segment of the
//!     // resolved URI (`skill://hello-world/SKILL.md`) or the build is
//!     // rejected; a body with no frontmatter at all is excluded from
//!     // `skills/list` while remaining readable via `resources/read`.
//!     let greeting = Skill::new(
//!         "hello-world",
//!         "---\nname: hello-world\ndescription: A minimal skill\n---\n\n# Hello\nThis is a minimal skill.\n",
//!     );
//!     // The prompt surface serves the SKILL.md verbatim, frontmatter included.
//!     let prompt_text = greeting.as_prompt_text();
//!     assert!(prompt_text.starts_with("---\nname: hello-world\n"));
//!     assert!(prompt_text.contains("# Hello"));
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

/// Deterministic [`crate::server::workflow::SequentialWorkflow`] -> SEP-2640
/// [`Skill`] projection.
pub mod projection;

/// Reverse-domain key under `ServerCapabilities.extensions` advertising
/// SEP-2640 skill support. Set automatically when any skill is registered.
pub(crate) const SKILLS_EXTENSION_KEY: &str = "io.modelcontextprotocol/skills";

// Retirement note (Phase 125, plan 04): a synthesized `skill://` discovery
// index used to be minted here, enumerated by `resources/list` and served by
// `resources/read` against the agentskills.io discovery schema 0.2.0.
// It is GONE, deliberately and as a published behavior break (125-CONTEXT
// D-08, ROADMAP success criterion 2). Two reasons, either sufficient:
//   1. `skills/list` and `skills/get` are the discovery surface the SEP-2640
//      working group chose, and serving both meant advertising two surfaces,
//      one of which the draft does not define.
//   2. The synthesized URI violated the draft's own URI structure rule — its
//      name segment is not a skill name and it has no `/SKILL.md` sibling.
// Reading the retired URI now falls through to the ordinary unknown-URI error
// path in `SkillsHandler::read`; there is no special case for it, and adding
// one back would reintroduce exactly what was removed.
const SKILL_MD_MIME: &str = "text/markdown";

/// Flip `ServerCapabilities` to advertise skills support. Called from
/// every builder method that accepts a skill or skill registry — keeping
/// the four call sites in sync via one function instead of inline copies.
///
/// # What the EMPTY declaration object means
///
/// `json!({})` is not a placeholder and is not an omission. SEP-2640's extension
/// object carries optional feature flags, of which `directoryRead` is the one
/// this SDK does not implement, and an absent flag is exactly how the draft
/// spells "off". So the empty object legitimately declares
/// `directoryRead: false`, and it is the honest declaration for a server that
/// serves `resources/read` per file and has no `resources/directory/read`.
///
/// # What declaring the extension COMMITS the server to
///
/// Declaring `io.modelcontextprotocol/skills` makes both `skills/list` and
/// `skills/get` MANDATORY for a conforming server. Phase 125 implements both, so
/// the declaration is honest in a way it was not before — but only over
/// streamable HTTP. See this module's transport-reach section for why, and for
/// the measured stdio behaviour.
///
/// # The uncomfortable half, said at the site where it matters
///
/// This function runs at BUILD time, before a transport has been chosen. A server
/// built here and then run over stdio therefore declares an extension whose two
/// mandatory methods it cannot answer, and a host that trusts the declaration and
/// calls `skills/list` gets a torn-down connection rather than a listing. That is
/// the largest accepted product risk of the phase that implemented these methods,
/// and it is recorded here because the declaration site is where an operator will
/// be standing when it bites.
///
/// **A stdio operator should expect:** the skills' SKILL.md and reference files to
/// be fully served through `resources/list` and `resources/read` (that surface is
/// transport-independent), and the two RPC methods to be unavailable.
///
/// Closing the gap needs one of two changes, named here so the next phase
/// inherits options rather than a complaint:
///
/// 1. **Widen the transport reach** so every transport can answer an
///    internally-routed method. Non-semver-breaking, and the same change the
///    stdio and `ServerCore` deferrals both need.
/// 2. **Defer capability resolution until a transport is known**, so the
///    declaration can be conditional. Larger: it changes WHEN capabilities are
///    computed, which every builder path depends on.
///
/// Narrowing the declaration silently was considered and rejected: a server that
/// implements the methods but hides the extension is unusable by a conforming
/// host on the transport where it does work.
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
    /// metadata reads (e.g. `resources/list`, the prompt-surface projection)
    /// avoid re-scanning the body on every request.
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
    /// The final segment of the skill's URI path differs from
    /// [`Skill::name`] — spike gap 4a.
    ///
    /// A WARNING, never a rejection. ROADMAP success criterion 3 scopes the
    /// hard reject to the FRONTMATTER name (gap 4c, see [`validate_names`]),
    /// and three in-repo constructions plus `pmcp-book`'s taught
    /// `.with_path("team/topic")` exercise deliberately use a path whose final
    /// segment differs from the constructor name for reasons unrelated to
    /// naming. The skill is still listed.
    NameMismatch {
        /// The skill's SKILL.md URI.
        uri: String,
        /// The final `/` segment of the resolved path.
        uri_segment: String,
        /// The name the skill was constructed with.
        skill_name: String,
    },
    /// The frontmatter carries a `name` key whose value is not a string
    /// (`name: 42`, `name: true`, `name: [a]`).
    ///
    /// A WARNING, never a rejection — [`validate_names`] exempts a non-string
    /// `name` deliberately, and a test pins that. But the exemption was SILENT,
    /// and silence is what made it a hole: it is the one shape in which an
    /// emitted entry carries a `frontmatter.name` that cannot equal its URI
    /// segment while NEITHER the name-identity reject NOR
    /// [`Self::NameMismatch`] fires (the latter compares the CONSTRUCTOR name,
    /// which in this shape matches). SEP-2640 makes that mismatch a mandatory
    /// host-side refusal, so the operator is told at build time.
    FrontmatterNameNotAString {
        /// The skill's SKILL.md URI.
        uri: String,
        /// The JSON spelling of what the author actually wrote.
        found: String,
        /// The final `/` segment of the resolved path the name is compared to.
        uri_segment: String,
    },
    /// The skill crosses one of the SEP-2640 Limits bounds — see
    /// [`exceeds_skill_limits`] for what this guard is and is not.
    ///
    /// A WARNING, never a rejection: ROADMAP success criterion 3 says "warns".
    /// The skill is still emitted as an entry.
    LimitExceeded {
        /// The over-limit skill's SKILL.md URI.
        uri: String,
        /// Which bound was crossed, and by what measurement.
        breach: SkillLimitBreach,
    },
}

impl SkillDiagnostic {
    /// The SKILL.md URI of the skill this finding is about.
    pub(crate) fn uri(&self) -> &str {
        match self {
            Self::FrontmatterAbsent { uri }
            | Self::FrontmatterInvalid { uri, .. }
            | Self::FrontmatterNotAMapping { uri, .. }
            | Self::NameMismatch { uri, .. }
            | Self::FrontmatterNameNotAString { uri, .. }
            | Self::LimitExceeded { uri, .. } => uri,
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
            Self::NameMismatch {
                uri,
                uri_segment,
                skill_name,
            } => format!(
                "skill {uri} is still listed, but its URI's final segment '{uri_segment}' differs \
                 from its constructed name '{skill_name}'. SEP-2640 hosts key a skill by its URI; \
                 this is a warning because a deliberate `with_path` override is legitimate."
            ),
            Self::FrontmatterNameNotAString {
                uri,
                found,
                uri_segment,
            } => format!(
                "skill {uri} is still listed, but its frontmatter `name` is {found}, not a \
                 string. The SEP-2640 name-identity rule compares that value against the URI \
                 segment '{uri_segment}' and a non-string can never equal it, so a conforming \
                 host will REFUSE this skill. Quote the name or write it as a plain scalar."
            ),
            Self::LimitExceeded { uri, breach } => format!(
                "skill {uri} is still listed, but it exceeds a SEP-2640 Limits bound: {}",
                breach.describe()
            ),
        }
    }
}

/// Which SEP-2640 Limits bound a skill crossed, and by what measurement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SkillLimitBreach {
    /// More than [`MAX_SKILL_RESOURCES`] entries in the skill's manifest,
    /// counting its own `SKILL.md`. Carries the actual count.
    TooManyResources(usize),
    /// The manifest's `size` values sum above [`MAX_SKILL_TOTAL_BYTES`].
    /// Carries the actual total.
    TooManyBytes(u64),
}

impl SkillLimitBreach {
    fn describe(&self) -> String {
        match self {
            Self::TooManyResources(count) => format!(
                "{count} resource entries against a limit of {MAX_SKILL_RESOURCES} (SKILL.md \
                 included in the count)"
            ),
            Self::TooManyBytes(total) => format!(
                "{total} total bytes against a limit of {MAX_SKILL_TOTAL_BYTES} (16 MiB), summed \
                 over the manifest's `size` values"
            ),
        }
    }
}

/// SEP-2640 Limits: at most 512 resource entries per skill, counting the
/// skill's own `SKILL.md`.
const MAX_SKILL_RESOURCES: usize = 512;

/// SEP-2640 Limits: at most 16 MiB of total file size per skill, summed over
/// the manifest's `size` values.
const MAX_SKILL_TOTAL_BYTES: u64 = 16_777_216;

/// The SEP-2640 Limits bounds check, as a PURE predicate over two
/// already-computed totals. Both bounds are INCLUSIVE: exactly 512 entries and
/// exactly 16,777,216 bytes are within limits.
///
/// # What this guard IS, and what it is NOT
///
/// It is an OPERATOR DIAGNOSTIC. Entry synthesis must retain and parse a
/// skill's bodies before it can compute their byte total, so this check fires
/// strictly AFTER the allocation it would have to prevent in order to be a
/// denial-of-service mitigation. It is not one, and pmcp does not claim it as
/// one. What it genuinely provides is growth named at build time, in the
/// operator's own log, before a host meets it.
///
/// The control that actually bounds allocation from an untrusted peer is the
/// streamable-HTTP transport's collected-body cap
/// (`DEFAULT_MAX_COLLECTED_BODY_BYTES`), which this module does not change. A
/// skill registry is assembled by the SERVER AUTHOR from compiled-in or
/// locally-read files, never supplied by a caller, so the residual risk here is
/// bounded by who controls the input rather than by this predicate.
///
/// Writing that down matters: a threat register that overstates a mitigation is
/// worse than one that records an accepted gap, because it stops anyone looking
/// again.
fn exceeds_skill_limits(count: usize, total_bytes: u64) -> Option<SkillLimitBreach> {
    if count > MAX_SKILL_RESOURCES {
        return Some(SkillLimitBreach::TooManyResources(count));
    }
    if total_bytes > MAX_SKILL_TOTAL_BYTES {
        return Some(SkillLimitBreach::TooManyBytes(total_bytes));
    }
    None
}

/// Collection of skills, with two projections.
///
/// Lifted into a [`crate::server::ResourceHandler`] impl via
/// [`Skills::into_handler`], and projected to SEP-2640 `skills/list` entries via
/// [`Skills::entries`]. It synthesizes no discovery index — that surface
/// (`skill://index.json`) was retired in Phase 125 plan 04 in favour of the
/// `skills/list` and `skills/get` methods.
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
    /// treated as a partial answer. See `skill_resource_manifest` for the
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
    /// Returns `Err(pmcp::Error::Validation)` when a skill's frontmatter
    /// `name` disagrees with the final segment of its URI path — the SEP-2640
    /// name-identity rule, checked through the same `validate_names` function
    /// [`Self::into_handler`] runs. Every offender is named in one message.
    ///
    /// Crossing a SEP-2640 Limits bound is deliberately NOT an error: it warns
    /// (see `exceeds_skill_limits`), and the skill is still emitted.
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
        let artifacts = self.build_artifacts();
        // The SAME validation function `into_handler` runs, over artifacts of
        // the same shape, so a registry can never produce entries it would
        // then refuse to serve.
        validate_names(&artifacts)?;
        // ...and the SAME duplicate-URI rule. Without this the claim above was
        // false in one direction: `into_handler` rejected a registry with two
        // skills at one URI while `entries()` happily returned two entries
        // carrying that URI, so the public projection could emit a `skills/list`
        // payload the server that produced it refuses to build.
        validate_unique_uris(&artifacts)?;
        Ok(entries_from_artifacts(artifacts))
    }

    /// The single per-build parse pass; see [`SkillBuildArtifact`].
    fn build_artifacts(&self) -> Vec<SkillBuildArtifact> {
        self.skills.iter().map(build_artifact).collect()
    }

    /// The same pass WITHOUT the manifest, for callers that only validate names.
    /// See `build_artifact_inner` for why the split exists.
    fn build_name_artifacts(&self) -> Vec<SkillBuildArtifact> {
        self.skills.iter().map(build_name_artifact).collect()
    }

    /// Both build products from ONE artifact pass.
    ///
    /// [`Self::entries`] and [`Self::into_handler`] each run `build_artifacts`,
    /// so calling them in sequence — which is exactly what a server build does
    /// — parses every skill's YAML twice and SHA-256s every SKILL.md and every
    /// reference body twice. Nothing needs the second pass: `validate_names`
    /// reads only `frontmatter["name"]` and `uri_segment`, and the handler is
    /// built from `self.skills` alone.
    ///
    /// This is what [`SkillBuildArtifact`]'s "every consumer reads this
    /// instead" claim means at the build seam — the artifacts are computed
    /// here and both consumers read the SAME vector, rather than each
    /// recomputing its own copy. The public `entries` / `into_handler` remain
    /// as they were for callers holding only one of the two.
    ///
    /// # Errors
    ///
    /// Returns `Err(pmcp::Error::Validation)` on a name-identity offender or a
    /// duplicate URI — the same two conditions, checked by the same functions,
    /// that the separate entry points return.
    pub(crate) fn finalize(self) -> Result<FinalizedSkills> {
        let artifacts = self.build_artifacts();
        validate_names(&artifacts)?;
        let (entries, diagnostics) = entries_from_artifacts(artifacts);
        let handler = self.build_handler()?;
        Ok((handler, entries, diagnostics))
    }

    /// Flatten the registry into a [`crate::server::ResourceHandler`].
    ///
    /// Returns `Err` on:
    /// - A skill whose frontmatter `name` disagrees with the final segment of
    ///   its URI path (see `validate_names`).
    /// - Two skills resolving to the same `skill_md_uri()`.
    /// - Two skills' reference URIs colliding.
    ///
    /// Name identity is checked through the SAME `validate_names` function
    /// [`Self::entries`] runs, so a registry can never produce entries it would
    /// then refuse to serve, nor the reverse.
    ///
    /// Insertion order is preserved via [`indexmap::IndexMap`] so
    /// `resources/list` output is deterministic across runs.
    ///
    /// # Errors
    ///
    /// Returns `Err(pmcp::Error::Validation)` listing every name-identity
    /// offender, or every duplicate URI detected. No silent overwrites.
    pub fn into_handler(self) -> Result<Arc<dyn ResourceHandler>> {
        // The NAME-ONLY pass: `validate_names` reads `frontmatter["name"]` and
        // `uri_segment` and nothing else, while the full pass SHA-256s every
        // SKILL.md and every reference body. `try_skills` probes through this
        // method on every registration call, so hashing here made a registry
        // assembled in K calls cost K passes over its accumulated bodies.
        // Duplicate URIs are still caught, by `build_handler` below.
        validate_names(&self.build_name_artifacts())?;
        self.build_handler()
    }

    /// The duplicate-URI half of [`Self::into_handler`], without the name
    /// validation — split out so [`Self::finalize`] can run `validate_names`
    /// once over artifacts it already holds instead of rebuilding them.
    ///
    /// # Errors
    ///
    /// Returns `Err(pmcp::Error::Validation)` listing every duplicate
    /// `SKILL.md` or reference URI. No silent overwrites.
    fn build_handler(self) -> Result<Arc<dyn ResourceHandler>> {
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
        // The two maps are populated independently, so a collision BETWEEN them
        // is invisible to both loops above. It is reachable: `SkillsHandler::read`
        // consults `skill_md` first, so a reference registered at, say,
        // `sub/SKILL.md` on skill `a` is permanently shadowed by a second skill
        // at `.with_path("a/sub")` — and, worse, `skill_resource_manifest`
        // publishes skill `a`'s digest for a URI that serves the OTHER skill's
        // bytes, which a conforming SEP-2640 host must refuse. `validate_reference_path`
        // anticipated the class but rejects only the bare `"SKILL.md"`, so any
        // nested `<dir>/SKILL.md` slipped through.
        let mut dup_cross: Vec<String> = references
            .keys()
            .filter(|uri| skill_md.contains_key(*uri))
            .cloned()
            .collect();
        dup_cross.sort();
        if !dup_skill.is_empty() || !dup_ref.is_empty() || !dup_cross.is_empty() {
            let mut msg = String::from("Skills::into_handler: duplicate URI(s):");
            if !dup_skill.is_empty() {
                msg.push_str(&format!(" SKILL.md=[{}]", dup_skill.join(", ")));
            }
            if !dup_ref.is_empty() {
                msg.push_str(&format!(" references=[{}]", dup_ref.join(", ")));
            }
            if !dup_cross.is_empty() {
                msg.push_str(&format!(
                    " a reference collides with another skill's SKILL.md=[{}]",
                    dup_cross.join(", ")
                ));
            }
            return Err(Error::validation(msg));
        }
        Ok(Arc::new(SkillsHandler::new(skill_md, references)))
    }
}

/// Everything one build pass learns about ONE skill, computed exactly once.
///
/// Before this existed, a skill's YAML could be parsed three or more times per
/// server build: name validation ran from both entry synthesis and handler
/// construction, and the builder's finalization calls `entries()` before
/// `into_handler()`. That is invisible on a three-skill fixture and linear in
/// registry size on a real one. Every consumer now reads this instead.
struct SkillBuildArtifact {
    /// The skill's canonical `skill://<path>/SKILL.md` URI.
    uri: String,
    /// The final `/` segment of the resolved path — what SEP-2640 name
    /// identity is checked against.
    uri_segment: String,
    /// `Some` only when the frontmatter parsed to a mapping; `None` means the
    /// skill is excluded and `diagnostics` says why.
    frontmatter: Option<serde_json::Value>,
    /// The complete `resources` manifest.
    resources: Vec<SkillResourceRef>,
    /// Every non-fatal finding about this skill.
    diagnostics: Vec<SkillDiagnostic>,
}

/// The final `/`-separated segment of a resolved skill path.
fn final_path_segment(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// The ONE build pass. Every YAML parse in this module's production path
/// happens here; [`Skills::entries_with_diagnostics`], [`validate_names`] and
/// [`Skills::into_handler`] all consume the result rather than re-deriving from
/// `Skill` bodies.
///
/// `with_manifest` selects how much of the pass runs. The manifest is the
/// EXPENSIVE half — one SHA-256 over the SKILL.md and over every reference body
/// — and [`validate_names`] reads neither it nor the byte totals derived from
/// it, so the name-only callers skip it. That matters because
/// [`crate::server::ServerBuilder::try_skills`] probes with
/// `into_handler()` on EVERY registration call: hashing there made a registry
/// assembled in K calls cost K full passes over its accumulated bodies. When
/// `with_manifest` is false the artifact's `resources` is empty and no
/// [`SkillDiagnostic::LimitExceeded`] can be produced, so such artifacts must
/// never reach [`entries_from_artifacts`].
fn build_artifact_inner(skill: &Skill, with_manifest: bool) -> SkillBuildArtifact {
    let uri = skill.skill_md_uri();
    let uri_segment = final_path_segment(skill.resolved_path()).to_string();
    let mut diagnostics = Vec::new();

    let frontmatter = match parse_frontmatter_value(skill.body()) {
        FrontmatterParse::Parsed(value) => Some(value),
        FrontmatterParse::Absent => {
            diagnostics.push(SkillDiagnostic::FrontmatterAbsent { uri: uri.clone() });
            None
        },
        FrontmatterParse::Invalid(reason) => {
            diagnostics.push(SkillDiagnostic::FrontmatterInvalid {
                uri: uri.clone(),
                reason,
            });
            None
        },
        FrontmatterParse::NotAMapping(reason) => {
            diagnostics.push(SkillDiagnostic::FrontmatterNotAMapping {
                uri: uri.clone(),
                reason,
            });
            None
        },
    };

    // A frontmatter `name` that is present but not a string is EXEMPT from the
    // reject in `validate_names` (a test pins that), so it warns here instead.
    // Without this it was the one shape where an entry shipped a name that
    // cannot match its URI segment and nothing said so.
    if let Some(found) = frontmatter
        .as_ref()
        .and_then(|fm| fm.get("name"))
        .filter(|name| !name.is_string())
    {
        diagnostics.push(SkillDiagnostic::FrontmatterNameNotAString {
            uri: uri.clone(),
            found: found.to_string(),
            uri_segment: uri_segment.clone(),
        });
    }

    // Gap 4a: a constructor-name mismatch WARNS. See `SkillDiagnostic::NameMismatch`
    // for why it is not the reject — the reject is scoped to the frontmatter name.
    if uri_segment != skill.name() {
        diagnostics.push(SkillDiagnostic::NameMismatch {
            uri: uri.clone(),
            uri_segment: uri_segment.clone(),
            skill_name: skill.name().to_string(),
        });
    }

    let resources = if with_manifest {
        let rows = skill_resource_manifest(skill);
        let total_bytes = rows.iter().fold(0u64, |acc, row| {
            acc.saturating_add(u64::try_from(row.size).unwrap_or(u64::MAX))
        });
        if let Some(breach) = exceeds_skill_limits(rows.len(), total_bytes) {
            diagnostics.push(SkillDiagnostic::LimitExceeded {
                uri: uri.clone(),
                breach,
            });
        }
        rows
    } else {
        Vec::new()
    };

    SkillBuildArtifact {
        uri,
        uri_segment,
        frontmatter,
        resources,
        diagnostics,
    }
}

/// The full pass, manifest included — what entry synthesis needs.
fn build_artifact(skill: &Skill) -> SkillBuildArtifact {
    build_artifact_inner(skill, true)
}

/// The name-identity half of the pass, with no SHA-256 over any body.
fn build_name_artifact(skill: &Skill) -> SkillBuildArtifact {
    build_artifact_inner(skill, false)
}

/// Everything one artifact pass yields: the resource handler, the SEP-2640
/// entries, and the build-time diagnostics for skills that were excluded.
///
/// Named because [`Skills::finalize`] returns all three and the bare tuple
/// trips `clippy::type_complexity` — which `make lint` did NOT see until the
/// gate grew a `skills`-featured lint leg, since it pins `--features "full"`
/// and `skills` is in neither `full` nor `full-v2`.
pub(crate) type FinalizedSkills = (
    Arc<dyn ResourceHandler>,
    Vec<SkillEntry>,
    Vec<SkillDiagnostic>,
);

/// Drain a completed artifact pass into its entries and its diagnostics.
///
/// Shared by [`Skills::entries_with_diagnostics`] and [`Skills::finalize`] so
/// the exclusion semantics have exactly ONE implementation: a skill whose
/// frontmatter did not parse contributes no entry and carries its diagnostic
/// out, whichever build path drained it.
fn entries_from_artifacts(
    artifacts: Vec<SkillBuildArtifact>,
) -> (Vec<SkillEntry>, Vec<SkillDiagnostic>) {
    let mut entries = Vec::with_capacity(artifacts.len());
    let mut diagnostics = Vec::new();
    for artifact in artifacts {
        diagnostics.extend(artifact.diagnostics);
        // The emitted object comes from the ONE build-pass parse and from
        // NOTHING else. It is deliberately never reconstructed from
        // `Skill::name()` or from the resolved description accessor:
        // `with_description` is an explicit OVERRIDE, so that accessor can
        // legitimately differ from the SKILL.md's authored `description:`
        // line, while SEP-2640 requires the emitted object to be identical
        // to the file's, field for field.
        if let Some(frontmatter) = artifact.frontmatter {
            entries.push(SkillEntry {
                uri: artifact.uri,
                frontmatter,
                resources: artifact.resources,
            });
        }
    }
    (entries, diagnostics)
}

/// Every URI an artifact pass would serve appears exactly once.
///
/// The projection twin of [`Skills::build_handler`]'s duplicate check, over the
/// same URI population: each artifact's manifest names its own `SKILL.md` first
/// and then every reference, which is precisely the key set the handler builds.
/// Running it from [`Skills::entries_with_diagnostics`] is what makes that
/// method's "a registry can never produce entries it would then refuse to
/// serve" comment true in BOTH directions — before it, `into_handler` rejected
/// a two-skills-one-URI registry while `entries()` happily returned two entries
/// carrying that URI.
///
/// Reads `artifact.resources`, so it is meaningful only over a FULL artifact
/// pass; the name-only pass leaves that vector empty by design.
///
/// # Errors
///
/// Returns `Err(pmcp::Error::Validation)` naming every repeated URI, sorted so
/// the message is stable across runs.
fn validate_unique_uris(artifacts: &[SkillBuildArtifact]) -> Result<()> {
    let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let mut dups: Vec<&str> = Vec::new();
    for row in artifacts.iter().flat_map(|a| a.resources.iter()) {
        if !seen.insert(row.uri()) {
            dups.push(row.uri());
        }
    }
    if dups.is_empty() {
        return Ok(());
    }
    dups.sort_unstable();
    dups.dedup();
    Err(Error::validation(format!(
        "Skills::entries: duplicate URI(s): [{}]",
        dups.join(", ")
    )))
}

/// SEP-2640 name identity (spike gap 4c, ROADMAP success criterion 3): a
/// skill whose frontmatter carries a string `name` MUST have that name equal
/// the final segment of its URI path.
///
/// # Deliberately conditional on frontmatter being present
///
/// A skill with no frontmatter, or with frontmatter carrying no `name` key, is
/// never rejected by this rule regardless of its path. That scoping is the
/// criterion's own — it names the FRONTMATTER name — and it is what keeps the
/// duplicate-URI tests, the `skills_strategy_with_refs` proptest and
/// `pmcp-book`'s taught `.with_path("team/topic")` exercise working: none of
/// them carries frontmatter, and all three deliberately use a path whose final
/// segment differs from the constructor name. The unconditional form (gap 4a)
/// ships as [`SkillDiagnostic::NameMismatch`]; promoting IT to a reject belongs
/// with the strict-frontmatter-mode work, once canonical surfaces are cleaned.
///
/// # A `name` that is PRESENT but not a string is exempt — and now WARNED
///
/// `name: 42` and `name: true` are valid YAML producing a `Value` that is not a
/// string, so `as_str()` yields `None` and this rule skips them — which
/// `the_name_rule_never_touches_a_skill_without_a_frontmatter_name` pins as
/// intended behaviour. The exemption stays; its SILENCE does not. Unannounced,
/// it let criterion 3's reject be defeated by writing the name as a scalar of
/// the wrong type: the entry shipped with a `frontmatter.name` that cannot
/// possibly equal its URI segment, which SEP-2640 makes a mandatory host-side
/// refusal, and no diagnostic fired either — [`SkillDiagnostic::NameMismatch`]
/// compares the CONSTRUCTOR name, which in that shape matches. The exemption
/// therefore carries [`SkillDiagnostic::FrontmatterNameNotAString`] so an
/// operator learns at build time instead of from a host.
///
/// Every offender is collected before erroring, matching
/// [`Skills::into_handler`]'s duplicate-URI style: an author with two bad
/// skills should learn about both in one build.
///
/// # Errors
///
/// Returns `Err(pmcp::Error::Validation)` naming every offending URI with its
/// frontmatter name and the URI segment that disagrees with it.
fn validate_names(artifacts: &[SkillBuildArtifact]) -> Result<()> {
    let mut offenders: Vec<String> = Vec::new();
    for artifact in artifacts {
        let Some(name) = artifact
            .frontmatter
            .as_ref()
            .and_then(|fm| fm.get("name"))
            .and_then(serde_json::Value::as_str)
        else {
            continue;
        };
        if name != artifact.uri_segment {
            offenders.push(format!(
                "{} (frontmatter name '{}', URI segment '{}')",
                artifact.uri, name, artifact.uri_segment
            ));
        }
    }
    if offenders.is_empty() {
        return Ok(());
    }
    Err(Error::validation(format!(
        "Skills: frontmatter `name` must equal the final segment of the skill's URI path: [{}]",
        offenders.join(", ")
    )))
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
}

impl SkillsHandler {
    fn new(
        skill_md: IndexMap<String, Skill>,
        references: IndexMap<String, (String, String)>,
    ) -> Self {
        // Exactly one entry per registered SKILL.md, in registration order,
        // and nothing else. No synthesized discovery entry is appended —
        // see the retirement note at the top of this file.
        let list_resources: Vec<ResourceInfo> = skill_md
            .values()
            .map(|s| {
                ResourceInfo::new(s.skill_md_uri(), s.name().to_string())
                    .with_description(s.resolved_description())
                    .with_mime_type(SKILL_MD_MIME)
            })
            .collect();
        Self {
            list_resources,
            skill_md,
            references,
        }
    }
}

#[async_trait]
impl ResourceHandler for SkillsHandler {
    async fn list(
        &self,
        _cursor: Option<String>,
        _extra: RequestHandlerExtra,
    ) -> Result<ListResourcesResult> {
        // Per SEP-2640 §9: list emits SKILL.md entries ONLY. Reference URIs
        // are never enumerated, and no discovery entry is synthesized —
        // discovery is `skills/list` / `skills/get`.
        Ok(ListResourcesResult::new(self.list_resources.clone()))
    }

    async fn read(&self, uri: &str, _extra: RequestHandlerExtra) -> Result<ReadResourceResult> {
        // resource_with_text (not Content::text) preserves the per-resource
        // URI + MIME on the wire — required so a reference like
        // schema.graphql round-trips with `application/graphql`.
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
    // Borrows: it reads two projections off the skill and stores Strings, so
    // taking ownership only forced every call site to `.clone()` a whole
    // registry entry it still needed afterwards.
    pub(crate) fn new(skill: &Skill) -> Self {
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
        // The USER handler's result is the base, because it is the only one of
        // the two that can carry pagination or caching state. Building on the
        // skills result instead discarded `next_cursor`, `ttl_ms` and
        // `cache_scope` — `SkillsHandler::list` returns a bare
        // `ListResourcesResult::new(..)`, which sets all three to `None` — so a
        // paginating `.resources(...)` handler had its cursor silently dropped
        // and the client concluded the listing was complete after page one.
        let mut combined = self.other.list(cursor.clone(), extra).await?;
        // Skills are ONE complete page with no cursor of their own, so they
        // belong only on the caller's first page. Emitting them unconditionally
        // (as `self.skills.list(None, ..)` did) repeated every skill URI on
        // every page of the user handler's pagination.
        if cursor.is_none() {
            // SkillsHandler::list ignores cursor + extra — pass owned defaults
            // so the user handler above could take the real ones by move.
            let skills = self
                .skills
                .list(None, RequestHandlerExtra::default())
                .await?;
            let mut resources = skills.resources;
            resources.append(&mut combined.resources);
            combined.resources = resources;
        }
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
/// encoded here directly.
///
/// A nibble table rather than `write!(out, "{byte:02x}")`: the formatter path
/// runs the full `core::fmt` machinery (width, fill, padding) 32 times per
/// digest, and this runs once per SKILL.md and once per reference body on every
/// server build. Indexing a 16-byte table pins the same lowercase, zero-padded
/// guarantee the format string did — every byte yields exactly two characters
/// from `[0-9a-f]` — without the `fmt::Write` import or the discarded `Result`.
fn sha256_digest_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};

    const HEX: &[u8; 16] = b"0123456789abcdef";

    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();

    let mut out = String::with_capacity("sha256:".len() + 64);
    out.push_str("sha256:");
    for byte in &digest {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
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
        // Exactly one entry per registered SKILL.md and nothing else —
        // the synthesized discovery entry was retired in Phase 125 plan 04.
        assert_eq!(list.resources.len(), 2);
        assert_eq!(list.resources[0].uri, "skill://a/SKILL.md");
        assert_eq!(list.resources[1].uri, "skill://b/SKILL.md");
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
            assert_eq!(list.resources.len(), 3);
            assert_eq!(list.resources[0].uri, "skill://zeta/SKILL.md");
            assert_eq!(list.resources[1].uri, "skill://alpha/SKILL.md");
            assert_eq!(list.resources[2].uri, "skill://mu/SKILL.md");
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
        // The retired discovery entry must not reappear: asserting ABSENCE
        // rather than deleting the check keeps a regression detectable.
        let retired_count = list
            .resources
            .iter()
            .filter(|r| r.uri == "skill://index.json")
            .count();
        assert_eq!(retired_count, 0, "retired discovery entry reappeared");
        assert_eq!(list.resources.len(), 1, "SKILL.md only");
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

    // ── Test 1.12 (the retired discovery URI is no longer served) ─────
    /// This test REPLACES `test_1_12_skills_handler_read_index_returns_resource_with_text`,
    /// which asserted the synthesized discovery index came back as an
    /// `application/json` resource. Deleting that test would have been a
    /// silent coverage loss; replacing it pins the decision instead.
    ///
    /// The retired URI takes the ORDINARY unknown-URI path — the same error
    /// any unregistered URI gets. There is deliberately no special case for
    /// it, so a reintroduced short-circuit fails here. Note the handler-level
    /// code is `METHOD_NOT_FOUND` (-32601); the streamable-HTTP dispatch tail
    /// re-wraps it, so a caller on the WIRE sees -32603 carrying -32601 inside
    /// the message (measured in 125-02, correcting D-06's single-level phrasing).
    #[tokio::test]
    async fn test_1_12_skills_handler_read_retired_index_uri_is_unknown() {
        let s = Skill::new("a", "").with_reference(SkillReference::new(
            "references/r.md",
            "text/markdown",
            "x",
        ));
        let handler = Skills::new().add(s).into_handler().unwrap();
        let err = handler
            .read("skill://index.json", extra())
            .await
            .expect_err("the retired discovery URI must no longer be served");
        // Byte-identical to the unknown-URI error any other stranger gets.
        let control = handler
            .read("skill://totally-unregistered/SKILL.md", extra())
            .await
            .expect_err("control: an unregistered URI must error");
        match (&err, &control) {
            (
                Error::Protocol {
                    code, message: m1, ..
                },
                Error::Protocol {
                    code: cc,
                    message: m2,
                    ..
                },
            ) => {
                assert_eq!(*code, ErrorCode::METHOD_NOT_FOUND);
                assert_eq!(code, cc, "retired URI must take the ordinary path");
                assert!(m1.contains("skill://index.json"), "m1 = {m1}");
                assert!(m2.contains("skill://totally-unregistered"), "m2 = {m2}");
                // Same error SHAPE, differing only in the echoed URI.
                assert_eq!(
                    m1.replace("skill://index.json", "URI"),
                    m2.replace("skill://totally-unregistered/SKILL.md", "URI")
                );
            },
            other => panic!("expected two Error::Protocol values, got {other:?}"),
        }
        // Control that the handler is not simply refusing everything.
        assert!(handler.read("skill://a/SKILL.md", extra()).await.is_ok());
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
        let handler = SkillPromptHandler::new(&skill);
        let result = handler.handle(HashMap::new(), extra()).await.unwrap();
        assert_eq!(result.messages.len(), 1);
        assert_eq!(result.messages[0].role, Role::User);
        match &result.messages[0].content {
            Content::Text { text } => assert_eq!(text, &skill.as_prompt_text()),
            other => panic!("expected Content::Text, got {other:?}"),
        }
    }

    // ── The module doctest's assertions, actually EXECUTED ────────────
    /// The module doctest at the top of this file (byte-mirrored into
    /// `pmcp-book/src/ch12-8-skills.md`, per the rule at line 18) is
    /// `rust,no_run`: `cargo test --doc` and `mdbook test` COMPILE it but
    /// never RUN it, so its two `assert!`s are unexecuted claims. A reader
    /// who copies the snippet does run them.
    ///
    /// This test executes exactly those assertions against exactly that
    /// body, so the canonical snippet cannot silently become false. It is
    /// the reason Phase 125 plan 04 could change the assertion from
    /// `starts_with("# Hello")` — which the frontmatter block D-03 requires
    /// makes false — with evidence rather than with reasoning.
    #[test]
    fn the_module_doctest_assertions_actually_hold() {
        let greeting = Skill::new(
            "hello-world",
            "---\nname: hello-world\ndescription: A minimal skill\n---\n\n# Hello\nThis is a minimal skill.\n",
        );
        let prompt_text = greeting.as_prompt_text();
        assert!(prompt_text.starts_with("---\nname: hello-world\n"));
        assert!(prompt_text.contains("# Hello"));

        // The assertion the doctest USED to make, now false — recorded so
        // a future reverter sees why it changed rather than "fixing" it back.
        assert!(!prompt_text.starts_with("# Hello"));

        // D-03: the snippet must produce a CONFORMING entry, not one the
        // D-02 exclusion path drops. That is the whole point of giving it
        // frontmatter, and it is checkable here.
        let entries = Skills::new()
            .add(greeting)
            .entries()
            .expect("frontmatter name equals the URI's final segment");
        assert_eq!(
            entries.len(),
            1,
            "the canonical snippet must be discoverable"
        );
        assert_eq!(entries[0].uri(), "skill://hello-world/SKILL.md");
        assert_eq!(
            entries[0]
                .frontmatter()
                .get("name")
                .and_then(|v| v.as_str()),
            Some("hello-world")
        );
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
        // Skills first (SKILL.md only = 1), then other (1).
        assert_eq!(list.resources.len(), 2);
        assert_eq!(list.resources[0].uri, "skill://a/SKILL.md");
        assert_eq!(list.resources[1].uri, "docs://handbook");
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
        // The retired discovery URI is deliberately NOT seeded here — it is
        // no longer readable, so including it would make the property assert
        // that an error is a well-formed resource.
        let mut uris: Vec<String> = Vec::new();
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
        assert_eq!(list.resources.len(), 2);
        assert_eq!(list.resources[0].uri, "skill://a/SKILL.md");
        assert_eq!(list.resources[1].uri, "skill://b/SKILL.md");
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

    // ── Entry synthesis is total over arbitrary bodies (Phase 125, plan 05) ──

    /// The stable-toolchain half of the CLAUDE.md ALWAYS/FUZZ requirement:
    /// `fuzz/fuzz_targets/fuzz_skill_entry.rs` drives the same path under
    /// libFuzzer, but the fuzz crate is workspace-EXCLUDED, needs a nightly
    /// toolchain, and runs only on `fuzz.yml`'s schedule. This function is the
    /// same invariant asserted where `make test-skills` — and therefore
    /// `make quality-gate` — can see it on every run.
    ///
    /// Drives ONE arbitrary body through the whole build path and asserts the
    /// outcome is always one of the two documented shapes: an entry with a
    /// well-formed digest and a byte-accurate size, or an exclusion diagnostic.
    /// A panic, an uppercase digest, or a size that disagrees with the body all
    /// fail here.
    ///
    /// Returns `true` when the body produced an ENTRY (as opposed to a
    /// legitimate exclusion or name-identity reject), so callers can assert
    /// anti-vacuity: the digest and size branches below are only evidence if
    /// something actually reaches them.
    fn assert_entry_synthesis_is_total(body: &str) -> bool {
        let registry = Skills::new().add(Skill::new("fuzzed", body));

        // `into_handler` runs the SAME `validate_names`; for a ONE-skill registry
        // the duplicate-URI rule cannot fire, so the two MUST agree on
        // acceptance. A disagreement is a registry that lists what it refuses to
        // serve, or the reverse.
        let handler_ok = Skills::new()
            .add(Skill::new("fuzzed", body))
            .into_handler()
            .is_ok();

        let Ok((entries, diagnostics)) = registry.entries_with_diagnostics() else {
            assert!(
                !handler_ok,
                "entries() rejected but into_handler() accepted; both run the same \
                 validate_names over artifacts of the same shape"
            );
            return false;
        };
        assert!(
            handler_ok,
            "entries() accepted but into_handler() rejected the same one-skill registry"
        );

        // Exactly one outcome per skill: an entry, or a diagnostic saying why not.
        assert_eq!(
            entries.len()
                + diagnostics
                    .iter()
                    .filter(|d| matches!(
                        d,
                        SkillDiagnostic::FrontmatterAbsent { .. }
                            | SkillDiagnostic::FrontmatterInvalid { .. }
                            | SkillDiagnostic::FrontmatterNotAMapping { .. }
                    ))
                    .count(),
            1,
            "a one-skill registry yields either ONE entry or ONE exclusion \
             diagnostic, never both and never neither (body: {body:?})"
        );

        for entry in &entries {
            assert!(
                entry.frontmatter().is_object(),
                "an emitted frontmatter is always a JSON object; a scalar or a \
                 sequence takes the exclusion path (body: {body:?})"
            );
            let manifest = entry.resources();
            assert_eq!(manifest.len(), 1, "no references were registered");
            assert_eq!(manifest[0].uri(), entry.uri());
            assert_eq!(
                manifest[0].size(),
                body.len(),
                "the SKILL.md row's size must be the body's byte length (body: {body:?})"
            );
            let hex = manifest[0]
                .digest()
                .strip_prefix("sha256:")
                .unwrap_or_else(|| panic!("digest must carry the sha256: prefix (body: {body:?})"));
            assert_eq!(
                hex.len(),
                64,
                "digest hex is 64 characters (body: {body:?})"
            );
            assert!(
                hex.bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
                "digest hex must be LOWERCASE — an uppercase rendering is a silent \
                 host-side comparison failure (body: {body:?})"
            );
        }
        !entries.is_empty()
    }

    /// The four malformed shapes a generator essentially never produces, and
    /// which are precisely what a real hand-authored SKILL.md hits.
    ///
    /// These are named explicitly rather than left to the proptest above them
    /// because `"\\PC*"` will not stumble onto a leading BOM followed by a `---`
    /// delimiter, nor onto an unterminated block, in any realistic number of
    /// cases — and each one exercises a DIFFERENT arm of
    /// [`parse_frontmatter_value`].
    #[test]
    fn entry_synthesis_survives_the_named_malformed_frontmatter_shapes() {
        let cases: &[(&str, &str)] = &[
            (
                "leading BOM then a real block",
                "\u{FEFF}---\nname: fuzzed\ndescription: d\n---\n\n# Body\n",
            ),
            ("a lone --- line", "---\n"),
            ("unterminated block", "---\nname: fuzzed\ndescription: d\n"),
            (
                "frontmatter parses to a sequence",
                "---\n- a\n- b\n---\n\nbody\n",
            ),
            (
                "frontmatter parses to a scalar",
                "---\njust a scalar\n---\n\nbody\n",
            ),
            ("empty body", ""),
            (
                "a markdown horizontal rule, not frontmatter",
                "# Title\n\n---\n\nbody\n",
            ),
            (
                "CRLF block",
                "---\r\nname: fuzzed\r\ndescription: d\r\n---\r\n\r\nbody\r\n",
            ),
            (
                "a YAML alias reference",
                "---\nname: fuzzed\na: &x [1]\nb: *x\n---\n\nbody\n",
            ),
            (
                "a duplicate YAML key",
                "---\nname: fuzzed\nname: fuzzed\n---\n\nbody\n",
            ),
            ("a tab-indented block", "---\n\tname: fuzzed\n---\n\nbody\n"),
            (
                "a NUL byte inside the block",
                "---\nname: fuzz\u{0}ed\n---\n\nbody\n",
            ),
        ];

        // Anti-vacuity: assert the case set is non-empty before quantifying over
        // it, so a future edit that empties the array cannot pass silently.
        assert!(
            cases.len() >= 4,
            "the named-shape set must not shrink below the four the plan requires"
        );

        let mut produced_entries = 0usize;
        for (label, body) in cases {
            if assert_entry_synthesis_is_total(body) {
                produced_entries += 1;
            }
            // Exclusion is not deletion (D-02): whatever the frontmatter did,
            // the skill still resolves to a servable handler unless the
            // name-identity rule — which both entry points share — rejected it.
            let handler = Skills::new()
                .add(Skill::new("fuzzed", *body))
                .into_handler();
            assert!(
                handler.is_ok()
                    || Skills::new()
                        .add(Skill::new("fuzzed", *body))
                        .entries()
                        .is_err(),
                "case `{label}`: into_handler failed for a reason entries() did not share"
            );
        }

        // Anti-vacuity: the digest, size and object-shape assertions inside the
        // helper only run for a case that yields an ENTRY. If every named shape
        // took the exclusion path, this test would pass having checked only that
        // nothing panicked.
        assert!(
            produced_entries >= 3,
            "only {produced_entries} of the named shapes produced an entry; the \
             digest and size assertions are barely exercised"
        );
    }

    proptest! {
        /// Arbitrary UTF-8 as a SKILL.md body never panics entry synthesis.
        ///
        /// THREE generated shapes per case, and each reaches a different arm:
        ///
        /// - `raw` almost never opens with `---`, so it exercises the delimiter
        ///   scan and the `Absent` path.
        /// - `framed` puts the arbitrary text where the YAML DOCUMENT goes, so
        ///   `serde_yaml` itself — the only third-party parser this module
        ///   reaches from author-supplied bytes — is driven on every case
        ///   instead of only when the generator happens to emit a delimiter.
        ///   Arbitrary text is usually a YAML scalar, so this mostly lands on
        ///   the `NotAMapping` exclusion.
        /// - `valued` puts the arbitrary text where a FIELD VALUE goes, as a
        ///   JSON-quoted string, so the frontmatter is always a mapping and the
        ///   digest / size / verbatim-object assertions are reached on EVERY
        ///   case. Without this third shape those assertions would run on
        ///   almost no generated input and the property would be near-vacuous
        ///   for the half of the invariant that is about emitted entries.
        #[test]
        fn prop_entry_synthesis_never_panics_on_arbitrary_bodies(raw in "\\PC*") {
            assert_entry_synthesis_is_total(&raw);
            assert_entry_synthesis_is_total(&format!("---\n{raw}\n---\n\n# Body\n"));

            // JSON string encoding, NOT Rust's `{:?}`. YAML 1.2 is a superset of
            // JSON, so a `serde_json` string literal is always a valid YAML
            // double-quoted scalar. Rust's Debug escape is NOT: it renders a
            // non-ASCII char as `\u{1e000}`, which YAML rejects (it wants
            // `\U0001E000`). Measured — that exact input was the first failure
            // this property produced, and it was the framing that was wrong, not
            // the parser.
            let encoded = serde_json::to_string(&raw).expect("a String always encodes as JSON");
            let valued = format!("---\nname: fuzzed\nfree: {encoded}\n---\n\n# Body\n");
            prop_assert!(
                assert_entry_synthesis_is_total(&valued),
                "the `valued` shape must always yield an entry — it is what keeps \
                 the digest and size assertions non-vacuous"
            );
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

    // ── Name identity + SEP Limits (Phase 125, plan 03, task 3) ────────

    /// Gap 4c: a frontmatter `name` that disagrees with the URI's final segment
    /// is REJECTED, by BOTH build-time entry points, with a message naming the
    /// URI and both names.
    #[test]
    fn frontmatter_name_identity_is_rejected_by_entries_and_into_handler() {
        let body = "---\nname: refunds\ndescription: d\n---\nbody\n";
        let build = || Skills::new().add(Skill::new("refunds", body).with_path("acme/billing"));

        match build().entries() {
            Err(Error::Validation(msg)) => {
                assert!(msg.contains("skill://acme/billing/SKILL.md"), "msg = {msg}");
                assert!(msg.contains("refunds"), "the frontmatter name: {msg}");
                assert!(msg.contains("billing"), "the URI segment: {msg}");
            },
            other => panic!("expected Err(Validation) from entries(), got {other:?}"),
        }

        match build().into_handler() {
            Err(Error::Validation(msg)) => {
                assert!(msg.contains("skill://acme/billing/SKILL.md"), "msg = {msg}");
            },
            Err(other) => panic!("expected Validation, got {other:?}"),
            Ok(_) => panic!("expected Err from into_handler() — the two must agree"),
        }
    }

    /// The name rule is conditional on a frontmatter `name` being present. No
    /// frontmatter, no `name` key, or a non-string `name` are all exempt
    /// regardless of the path — that scoping is what keeps the duplicate-URI
    /// tests and the proptest strategies working.
    #[test]
    fn the_name_rule_never_touches_a_skill_without_a_frontmatter_name() {
        let cases = [
            ("", "no frontmatter at all"),
            ("---\ndescription: d\n---\nbody\n", "no `name` key"),
            (
                "---\nname: 42\ndescription: d\n---\nbody\n",
                "non-string name",
            ),
        ];
        for (body, why) in cases {
            let registry =
                || Skills::new().add(Skill::new("a", body).with_path("totally/other/place"));
            assert!(registry().entries().is_ok(), "entries() rejected {why}");
            assert!(
                registry().into_handler().is_ok(),
                "into_handler() rejected {why}"
            );
        }
    }

    /// Every offender is collected before erroring — an author with two bad
    /// skills learns about both in one build.
    #[test]
    fn two_name_mismatches_produce_one_error_naming_both() {
        let err = Skills::new()
            .add(Skill::new(
                "a",
                "---\nname: alpha\ndescription: d\n---\nb\n",
            ))
            .add(Skill::new("b", "---\nname: beta\ndescription: d\n---\nb\n"))
            .entries()
            .expect_err("both skills violate name identity");
        let Error::Validation(msg) = err else {
            panic!("expected Validation");
        };
        assert!(msg.contains("skill://a/SKILL.md"), "msg = {msg}");
        assert!(msg.contains("skill://b/SKILL.md"), "msg = {msg}");
        assert!(msg.contains("alpha"), "msg = {msg}");
        assert!(msg.contains("beta"), "msg = {msg}");
    }

    /// Gap 4a: a CONSTRUCTOR-name mismatch whose frontmatter name still matches
    /// the URI segment WARNS and is still listed. Promoting this to a reject
    /// would break three in-repo constructions and a taught book exercise.
    #[test]
    fn constructor_name_mismatch_warns_rather_than_rejects() {
        let body = "---\nname: billing\ndescription: d\n---\nbody\n";
        let (entries, diagnostics) = Skills::new()
            .add(Skill::new("refunds", body).with_path("billing"))
            .entries_with_diagnostics()
            .expect("gap 4a warns, it does not reject");

        assert_eq!(entries.len(), 1, "the skill is still listed");
        let SkillDiagnostic::NameMismatch {
            uri,
            uri_segment,
            skill_name,
        } = &diagnostics[0]
        else {
            panic!("expected NameMismatch, got {:?}", diagnostics[0]);
        };
        assert_eq!(uri, "skill://billing/SKILL.md");
        assert_eq!(uri_segment, "billing");
        assert_eq!(skill_name, "refunds");

        assert!(
            Skills::new()
                .add(Skill::new("refunds", body).with_path("billing"))
                .into_handler()
                .is_ok(),
            "and into_handler agrees — a warning is not a refusal"
        );
    }

    /// The PURE bounds predicate, exercised at exactly the bounds and one past
    /// each. Allocates no skill bodies at all — building a 16 MiB registry to
    /// prove an inclusive comparison would add real time to every future run of
    /// the suite for no additional signal (R-23).
    #[test]
    fn exceeds_skill_limits_bounds_are_inclusive() {
        assert_eq!(exceeds_skill_limits(0, 0), None);
        assert_eq!(
            exceeds_skill_limits(MAX_SKILL_RESOURCES, MAX_SKILL_TOTAL_BYTES),
            None,
            "512 entries and 16,777,216 bytes exactly are WITHIN limits"
        );
        assert_eq!(
            exceeds_skill_limits(MAX_SKILL_RESOURCES + 1, 0),
            Some(SkillLimitBreach::TooManyResources(513))
        );
        assert_eq!(
            exceeds_skill_limits(0, MAX_SKILL_TOTAL_BYTES + 1),
            Some(SkillLimitBreach::TooManyBytes(16_777_217))
        );
        // The literals and the constants agree — a silent retune of either
        // would break here rather than at a host.
        assert_eq!(MAX_SKILL_RESOURCES, 512);
        assert_eq!(MAX_SKILL_TOTAL_BYTES, 16_777_216);
    }

    /// The predicate is WIRED into the real synthesis path, not merely correct
    /// in isolation: an over-COUNT registry produces the diagnostic naming its
    /// URI and its count, and is STILL emitted as an entry.
    #[test]
    fn an_over_count_registry_produces_a_wired_limit_diagnostic() {
        let make = |refs: usize| {
            let mut skill = Skill::new("big", "---\nname: big\ndescription: d\n---\nb\n");
            for i in 0..refs {
                skill = skill.with_reference(SkillReference::new(
                    format!("references/r{i}.md"),
                    "text/markdown",
                    "x",
                ));
            }
            Skills::new().add(skill)
        };

        // 512 references + the skill's own SKILL.md = 513 rows, one past the bound.
        let (entries, diagnostics) = make(MAX_SKILL_RESOURCES)
            .entries_with_diagnostics()
            .expect("an over-limit skill is a WARNING, never a rejection");
        assert_eq!(entries.len(), 1, "still listed");
        assert_eq!(entries[0].resources().len(), MAX_SKILL_RESOURCES + 1);
        let found = diagnostics
            .iter()
            .find_map(|d| match d {
                SkillDiagnostic::LimitExceeded { uri, breach } => Some((uri.as_str(), *breach)),
                _ => None,
            })
            .expect("the over-count skill must produce a limit diagnostic");
        assert_eq!(found.0, "skill://big/SKILL.md");
        assert_eq!(found.1, SkillLimitBreach::TooManyResources(513));

        // Control at exactly the bound: 511 references + SKILL.md = 512 rows.
        let (entries, diagnostics) = make(MAX_SKILL_RESOURCES - 1)
            .entries_with_diagnostics()
            .expect("entries build");
        assert_eq!(entries[0].resources().len(), MAX_SKILL_RESOURCES);
        assert!(
            !diagnostics
                .iter()
                .any(|d| matches!(d, SkillDiagnostic::LimitExceeded { .. })),
            "512 rows exactly is WITHIN limits, got {diagnostics:?}"
        );
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
