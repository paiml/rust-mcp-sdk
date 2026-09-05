//! Spike 008: SEP-2640 drift check — shipped `skills` module vs the CURRENT
//! draft (branch `sep/skills-extension`, last push 2026-08-29).
//!
//! Spikes 001/002 validated (and Phase 80 shipped) the 2026-05-12 draft:
//! pure resources mapping, synthesized `skill://index.json` discovery index,
//! no new RPC methods. The current draft is a different animal: three
//! protocol methods (`skills/list`, `skills/get`, optional
//! `resources/directory/read`), verbatim-frontmatter entries, sha256+size
//! resource manifests, strict URI structure rules, format delegated to
//! agentskills.io, archive mode formally dead.
//!
//! Each step prints the wire evidence and a `✓` (still conforms) or
//! `❗ GAP` (drift) line. The verdict enumerates the fix list.

use pmcp::server::skills::{Skill, SkillReference, Skills};
use pmcp::types::protocol::ClientRequest;
use pmcp::RequestHandlerExtra;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const RULE: &str = "──────────────────────────────────────────────────────────";

const REFUNDS_SKILL_MD: &str = "---\nname: refunds\ndescription: Process customer refund requests per company policy\nlicense: Apache-2.0\n---\n# Refund Processing\n\n1. Look up the order with `get_order`.\n2. Check eligibility against `references/policy.md`.\n3. Draft the customer email from `examples/email.md`.\n4. Issue the refund with `issue_refund`.\n";

const POLICY_MD: &str = "# Refund Policy\n\n- 30 days, undamaged, original receipt.\n";
const EMAIL_MD: &str = "Subject: Your refund\n\nWe have processed your refund.\n";

fn print_banner() {
    println!("{RULE}");
    println!("Spike 008: SEP-2640 drift check (current draft vs shipped `skills`)");
    println!("{RULE}\n");
}

/// Minimal frontmatter parser for the spike's OWN fixtures (flat `k: v`
/// string pairs only). The SDK-level fix needs a real YAML decision — see
/// GAP #2 in the verdict. This is deliberately not that.
fn parse_frontmatter(body: &str) -> Option<serde_json::Map<String, Value>> {
    let rest = body.strip_prefix("---\n")?;
    let end = rest.find("\n---")?;
    let mut map = serde_json::Map::new();
    for line in rest[..end].lines() {
        let (k, v) = line.split_once(':')?;
        map.insert(k.trim().to_string(), Value::String(v.trim().to_string()));
    }
    Some(map)
}

fn digest_of(content: &str) -> String {
    let mut h = Sha256::new();
    h.update(content.as_bytes());
    format!("sha256:{:x}", h.finalize())
}

fn resource_entry(uri: &str, content: &str) -> Value {
    json!({ "uri": uri, "digest": digest_of(content), "size": content.len() })
}

/// Synthesize the CURRENT-draft `skills/list` entry for a skill, out-of-band
/// from the shipped types. `path` is the skill path the spike registered
/// (`Skill::resolved_path` is pub(crate), so the spike carries it itself).
fn synthesize_entry(skill: &Skill, path: &str) -> Value {
    let fm = parse_frontmatter(skill.body())
        .expect("spike fixtures always carry flat frontmatter");
    let skill_md_uri = format!("skill://{path}/SKILL.md");
    let mut resources = vec![resource_entry(&skill_md_uri, skill.body())];
    for r in skill.references() {
        resources.push(resource_entry(
            &format!("skill://{path}/{}", r.relative_path()),
            r.body(),
        ));
    }
    json!({ "uri": skill_md_uri, "frontmatter": Value::Object(fm), "resources": resources })
}

fn step_1_new_methods_unrouteable() {
    println!("{RULE}");
    println!("Step 1 — the three new methods are unrouteable in pmcp today");
    println!("{RULE}");

    // Control: a method pmcp DOES implement parses into ClientRequest.
    let control = serde_json::from_value::<ClientRequest>(
        json!({ "method": "resources/list", "params": {} }),
    );
    assert!(control.is_ok(), "control resources/list must parse");
    println!("  ✓ control: {{\"method\":\"resources/list\"}} parses into ClientRequest");

    for method in ["skills/list", "skills/get", "resources/directory/read"] {
        let r = serde_json::from_value::<ClientRequest>(
            json!({ "method": method, "params": {} }),
        );
        assert!(r.is_err(), "{method} must NOT parse — no variant exists");
        println!("  ✓ {{\"method\":\"{method}\"}} fails to parse → server answers -32601");
    }

    println!("\n  ❗ GAP #1 (CRITICAL): current draft §Capability Declaration:");
    println!("     \"Declaring the extension itself commits the server to");
    println!("      skills/list and skills/get.\"  pmcp AUTO-DECLARES the");
    println!("     extension key whenever a skill is registered");
    println!("     (set_skills_capabilities, src/server/skills.rs:66) but");
    println!("     routes neither method. A conforming host calls skills/list");
    println!("     right after initialize and gets method-not-found.");
    println!("     Fix path: the ServerDiscoverRequest pattern — crate-private");
    println!("     InternalClientRequest + classify_internal_method routing");
    println!("     (documented at src/types/protocol/mod.rs:583) so the public");
    println!("     exhaustive ClientRequest enum gains NO variant (2.x promise).\n");
}

async fn step_2_shipped_surface_and_index_drift() -> anyhow::Result<()> {
    println!("{RULE}");
    println!("Step 2 — shipped resource surface: baseline holds, index is drift");
    println!("{RULE}");

    let refunds = Skill::new("refunds", REFUNDS_SKILL_MD)
        .with_reference(SkillReference::new(
            "references/policy.md",
            "text/markdown",
            POLICY_MD,
        ))
        .with_reference(SkillReference::new(
            "examples/email.md",
            "text/markdown",
            EMAIL_MD,
        ));
    let handler = Skills::new().add(refunds).into_handler()?;

    let listed = handler
        .list(None, RequestHandlerExtra::default())
        .await?;
    let uris: Vec<&str> = listed.resources.iter().map(|r| r.uri.as_str()).collect();
    println!("  resources/list → {uris:?}");

    assert!(uris.contains(&"skill://refunds/SKILL.md"));
    println!("  ✓ SKILL.md listed and (below) readable — the draft's baseline");
    assert!(!uris.iter().any(|u| u.contains("references/")));
    assert!(!uris.iter().any(|u| u.contains("examples/")));
    println!("  ✓ supporting files not enumerated (still fine: they need not be listed)");

    let read = handler
        .read("skill://refunds/SKILL.md", RequestHandlerExtra::default())
        .await?;
    let wire = serde_json::to_value(&read)?;
    let text = wire
        .pointer("/contents/0/text")
        .and_then(Value::as_str)
        .expect("SKILL.md read returns text content");
    assert_eq!(text, REFUNDS_SKILL_MD);
    println!("  ✓ resources/read returns SKILL.md byte-identical (baseline conforms)");

    let ref_read = handler
        .read(
            "skill://refunds/references/policy.md",
            RequestHandlerExtra::default(),
        )
        .await?;
    let ref_wire = serde_json::to_value(&ref_read)?;
    assert_eq!(
        ref_wire.pointer("/contents/0/text").and_then(Value::as_str),
        Some(POLICY_MD)
    );
    println!("  ✓ supporting file readable at skill://refunds/references/policy.md");

    assert!(uris.contains(&"skill://index.json"));
    let idx = handler
        .read("skill://index.json", RequestHandlerExtra::default())
        .await?;
    let idx_wire = serde_json::to_value(&idx)?;
    let idx_text = idx_wire
        .pointer("/contents/0/text")
        .and_then(Value::as_str)
        .unwrap_or("");
    println!("  legacy index.json content (what retiring it loses): {idx_text}");
    println!("\n  ❗ GAP #3 (MAJOR): shipped module synthesizes skill://index.json");
    println!("     as the discovery surface. The current draft has NO index");
    println!("     resource — the WG rationale explicitly chose \"a method over");
    println!("     an index resource\"; discovery is skills/list. Worse, the URI");
    println!("     itself violates the draft's structure rule: a skill path's");
    println!("     final segment must be a skill name whose SKILL.md exists —");
    println!("     'index.json' is not a skill, and skill://index.json/SKILL.md");
    println!("     does not exist. Fix: retire the index (or gate it behind a");
    println!("     legacy option) when skills/list lands.\n");
    Ok(())
}

fn step_3_entry_synthesis_data_sufficiency() {
    println!("{RULE}");
    println!("Step 3 — a conforming skills/list entry is fully derivable from Skill");
    println!("{RULE}");

    let refunds = Skill::new("refunds", REFUNDS_SKILL_MD)
        .with_reference(SkillReference::new(
            "references/policy.md",
            "text/markdown",
            POLICY_MD,
        ))
        .with_reference(SkillReference::new(
            "examples/email.md",
            "text/markdown",
            EMAIL_MD,
        ));

    let entry = synthesize_entry(&refunds, "refunds");
    println!("{}", serde_json::to_string_pretty(&entry).unwrap());

    // Draft rules, asserted against the synthesized entry:
    let fm = entry.get("frontmatter").unwrap();
    assert_eq!(fm.get("name").and_then(Value::as_str), Some("refunds"));
    assert!(fm.get("description").is_some());
    assert_eq!(
        fm.get("license").and_then(Value::as_str),
        Some("Apache-2.0"),
        "verbatim rule: EVERY authored field passes through, not a curated subset"
    );
    println!("  ✓ frontmatter rendered verbatim (name, description, license all present)");

    let resources = entry.get("resources").unwrap().as_array().unwrap();
    assert_eq!(resources.len(), 3, "SKILL.md itself + 2 references — complete");
    assert_eq!(resources[0].get("uri"), entry.get("uri"));
    println!("  ✓ resources manifest complete, includes SKILL.md's own entry first");

    for r in resources {
        let d = r.get("digest").and_then(Value::as_str).unwrap();
        assert!(d.starts_with("sha256:") && d.len() == 7 + 64);
        assert!(d[7..].chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
        assert!(r.get("size").and_then(Value::as_u64).unwrap() > 0);
    }
    println!("  ✓ digests are sha256:{{64 lowercase hex}}, sizes are byte lengths");

    let uri = entry.get("uri").and_then(Value::as_str).unwrap();
    let final_seg = uri
        .strip_suffix("/SKILL.md")
        .unwrap()
        .rsplit('/')
        .next()
        .unwrap();
    assert_eq!(final_seg, fm.get("name").and_then(Value::as_str).unwrap());
    println!("  ✓ final <skill-path> segment equals frontmatter.name");

    println!("\n  ❗ GAP #2 (MAJOR): pmcp has NO API to produce this entry — no");
    println!("     digest/size computation, no verbatim-frontmatter rendering.");
    println!("     But every input lives in Skill already (name, body, refs),");
    println!("     so the gap is API surface, not data model: add");
    println!("     Skills::entries() computed at into_handler()/build() time.");
    println!("     Open decision: verbatim frontmatter→JSON needs real YAML");
    println!("     (nested maps, lists). serde_yaml is deprecated/archived —");
    println!("     candidates: serde_yaml_ng / serde-yml / saphyr, or keep the");
    println!("     shipped line-parser and document a flat-frontmatter limit.\n");
}

fn step_4_uri_and_identity_validation_gaps() {
    println!("{RULE}");
    println!("Step 4 — URI structure & frontmatter identity: silently violable");
    println!("{RULE}");

    // 4a. Nested organizational prefix, name as final segment: conforms.
    let nested = Skill::new("refunds", REFUNDS_SKILL_MD).with_path("acme/billing/refunds");
    let uris = Skills::new().add(nested).skill_md_uris();
    assert_eq!(uris, vec!["skill://acme/billing/refunds/SKILL.md".to_string()]);
    println!("  ✓ with_path(\"acme/billing/refunds\") → nested prefix conforms");

    // 4b. with_path whose final segment is NOT the name: accepted silently.
    let bad_path = Skill::new("refunds", REFUNDS_SKILL_MD).with_path("acme/billing");
    let uris = Skills::new().add(bad_path).skill_md_uris();
    assert_eq!(uris, vec!["skill://acme/billing/SKILL.md".to_string()]);
    println!("  ❗ GAP #4a (MINOR): with_path(\"acme/billing\") on a skill named");
    println!("     'refunds' produces skill://acme/billing/SKILL.md — final");
    println!("     segment 'billing' ≠ name 'refunds'. Draft: MUST be equal");
    println!("     (name is recoverable from URI alone). No validation fires.");

    // 4c. Draft-LEGAL name collision: two skills named 'refunds' at
    // different paths must coexist (names are labels, URIs identify).
    let collision = Skills::new()
        .add(Skill::new("refunds", REFUNDS_SKILL_MD).with_path("acme/billing/refunds"))
        .add(Skill::new("refunds", REFUNDS_SKILL_MD).with_path("acme/support/refunds"))
        .into_handler();
    assert!(collision.is_ok(), "same-name different-path skills are legal");
    println!("  ✓ same-name skills at different paths coexist (draft: names are");
    println!("    labels, not identifiers — hosts disambiguate by path)");

    // 4d. Constructor name vs body frontmatter name: accepted silently.
    let mismatched = Skill::new("something-else", REFUNDS_SKILL_MD);
    assert_eq!(mismatched.name(), "something-else");
    println!("  ❗ GAP #4c (MINOR): Skill::new(\"something-else\", body-whose-");
    println!("     frontmatter-says-name:refunds) is accepted. Draft: entry");
    println!("     frontmatter MUST be identical to SKILL.md frontmatter, and");
    println!("     hosts verify field-by-field — a conforming host would refuse");
    println!("     to load this skill. Validate at construction or build().\n");
}

fn step_5_limits() {
    println!("{RULE}");
    println!("Step 5 — per-skill limits (512 files / 16 MiB) are entry-checkable");
    println!("{RULE}");

    let refunds = Skill::new("refunds", REFUNDS_SKILL_MD)
        .with_reference(SkillReference::new(
            "references/policy.md",
            "text/markdown",
            POLICY_MD,
        ));
    let entry = synthesize_entry(&refunds, "refunds");
    let resources = entry.get("resources").unwrap().as_array().unwrap();
    let count = resources.len();
    let total: u64 = resources
        .iter()
        .map(|r| r.get("size").and_then(Value::as_u64).unwrap())
        .sum();
    println!("  fixture skill: {count} resources, {total} bytes (limits: 512 / 16,777,216)");
    assert!(count <= 512 && total <= 16_777_216);
    println!("  ✓ both limits checkable from the manifest alone, before any fetch");
    println!("  ❗ GAP #5 (MINOR): draft SDK guidance says warn when a registered");
    println!("     skill exceeds the limits; pmcp has no check. One guard at");
    println!("     into_handler() once entries exist.\n");
}

fn print_verdict() {
    println!("{RULE}");
    println!("VERDICT");
    println!("{RULE}");
    println!(
        r#"
The shipped `skills` module conforms to a draft that no longer exists.
Baseline serving still conforms (SKILL.md + supporting files readable via
resources/read, byte-identical; capability key + SEP-2133 shape correct;
archive-mode exclusion now vindicated — formally moved to Deferred
Features). But the module is NON-CONFORMANT with the current draft on the
one thing it advertises: declaring io.modelcontextprotocol/skills commits
the server to skills/list + skills/get, and neither exists.

  #   sev       gap                              fix
  1   CRITICAL  skills/list + skills/get         InternalClientRequest
                unrouteable while capability      classifier route (pattern:
                is auto-declared                  protocol/mod.rs:583) +
                                                  Skills answers both
  2   MAJOR     no entry manifest API             Skills::entries(): verbatim
                (frontmatter JSON, sha256,        frontmatter + digests/sizes
                size)                             computed at build time;
                                                  YAML dep decision needed
  3   MAJOR     skill://index.json index is       retire index (or legacy-
                nonstandard + violates URI        gate) when skills/list
                structure rules                   lands
  4   MINOR     with_path / Skill::new don't      validate final-segment ==
                enforce name identity rules       frontmatter name at build()
  5   MINOR     no 512-file / 16 MiB limit        warn at into_handler()
                warning
  6   INFO      resources/directory/read          defer; current `{{}}`
                unimplemented                     declaration = false, valid
  7   INFO      no client wrappers                client.list_skills() /
                (list_skills/get_skill/           get_skill() /
                read_skill_uri)                   read_skill_uri()

Interim question for the maintainer: until #1 lands, either implement the
two methods (small — the registry already holds every input, step 3 proved
sufficiency) or STOP auto-declaring the extension key. Declaring-but-not-
implementing is the one state the draft makes indefensible.

Positioning input (feeds spikes 009-011): the draft's host-integration
sketch (read_skill keyed by server+uri, origin tagging, content-bound
approval) is written for HOSTS — and pmcp-agent IS a host when it consumes
skills as instructions. Spike 010 must apply the draft's security section,
not just fetch bytes.
"#
    );
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    print_banner();
    step_1_new_methods_unrouteable();
    step_2_shipped_surface_and_index_drift().await?;
    step_3_entry_synthesis_data_sufficiency();
    step_4_uri_and_identity_validation_gaps();
    step_5_limits();
    print_verdict();
    Ok(())
}
