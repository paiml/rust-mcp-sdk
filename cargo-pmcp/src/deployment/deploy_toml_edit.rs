//! Surgical edits to an existing `.pmcp/deploy.toml`.
//!
//! `deploy init` keeps an existing deploy.toml (debug session
//! `cargo-pmcp-deploy-targets`, findings #3/A4). When it must still change one
//! value (`--name`, a different `--target-type`) or add a section the target
//! needs, it edits the text in place instead of re-serializing the whole
//! config, so comments, key order, formatting and keys cargo-pmcp does not
//! model all survive. Every edit is verified by re-parsing: the edited file
//! must differ from the original in exactly the intended value, otherwise the
//! edit is rejected and the caller refuses rather than guessing.

/// `value` as a TOML basic-string literal (quoted and escaped).
pub fn toml_string_literal(value: &str) -> String {
    toml::Value::String(value.to_string()).to_string()
}

/// The name of a `[table]` header line (already trimmed), or `None` for an
/// array-of-tables header (`[[...]]`) or a line that is not a header.
fn table_header_name(trimmed: &str) -> Option<&str> {
    if trimmed.starts_with("[[") {
        return None;
    }
    let inner = trimmed.strip_prefix('[')?;
    let end = inner.find(']')?;
    Some(inner[..end].trim())
}

/// The bare key of a `key = value` line (already trimmed), or `None` for a
/// comment, a blank line, a quoted or dotted key, or a continuation line.
fn line_key(trimmed: &str) -> Option<&str> {
    if trimmed.starts_with('#') {
        return None;
    }
    let (key, _) = trimmed.split_once('=')?;
    let key = key.trim();
    let bare = !key.is_empty()
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    bare.then_some(key)
}

/// Index of the `key = ...` line inside `[table]`, if there is one.
fn find_key_line(lines: &[&str], table: &str, key: &str) -> Option<usize> {
    let mut in_table = false;
    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_table = table_header_name(trimmed) == Some(table);
            continue;
        }
        if in_table && line_key(trimmed) == Some(key) {
            return Some(index);
        }
    }
    None
}

/// True when `edited` parses to exactly `original` with `[table] key` set to
/// `value` and nothing else changed.
fn only_that_value_changed(
    original: &str,
    edited: &str,
    table: &str,
    key: &str,
    value: &str,
) -> bool {
    let (Ok(mut before), Ok(after)) = (
        toml::from_str::<toml::Table>(original),
        toml::from_str::<toml::Table>(edited),
    ) else {
        return false;
    };
    let Some(section) = before.get_mut(table).and_then(toml::Value::as_table_mut) else {
        return false;
    };
    section.insert(key.to_string(), toml::Value::String(value.to_string()));
    before == after
}

/// Set `key = "value"` inside `[table]` by replacing that one line.
///
/// Returns `None` (and the caller must not write anything) when `[table]` has
/// no plain `key = ...` line (a dotted key, an inline table, a key spread over
/// several lines), or when the edited text does not re-parse to the original
/// with only that value changed. A trailing comment on the replaced line is
/// not kept.
pub fn set_string_in_table(text: &str, table: &str, key: &str, value: &str) -> Option<String> {
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let index = find_key_line(&lines, table, key)?;
    let line = lines[index];
    let indent = &line[..line.len() - line.trim_start().len()];
    let newline = if line.ends_with("\r\n") {
        "\r\n"
    } else if line.ends_with('\n') {
        "\n"
    } else {
        ""
    };
    let replacement = format!("{indent}{key} = {}{newline}", toml_string_literal(value));
    let edited: String = lines
        .iter()
        .enumerate()
        .map(|(i, l)| if i == index { replacement.as_str() } else { l })
        .collect();
    only_that_value_changed(text, &edited, table, key, value).then_some(edited)
}

/// Append `section` (a complete `[table]` block) at the end of `text`,
/// separated by one blank line. Existing content is kept byte-for-byte.
pub fn append_section(text: &str, section: &str) -> String {
    let mut out = text.to_string();
    if !out.is_empty() {
        if !out.ends_with('\n') {
            out.push('\n');
        }
        out.push('\n');
    }
    out.push_str(section);
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEPLOY_TOML: &str = "\
# operator comment: keep me
[target]
type = \"aws-lambda\"
version = \"1.0.0\"

[aws]
region = \"eu-west-2\"   # London

[server]
name = \"acme-forecast\"
memory_mb = 1024

[environment]
MODEL_URL = \"s3://bucket/model\"
name = \"not-the-server-name\"
";

    #[test]
    fn sets_the_server_name_and_keeps_everything_else_byte_for_byte() {
        let edited = set_string_in_table(DEPLOY_TOML, "server", "name", "acme").expect("edit");
        assert_eq!(
            edited,
            DEPLOY_TOML.replace("name = \"acme-forecast\"", "name = \"acme\""),
        );
    }

    #[test]
    fn only_the_key_inside_the_named_table_is_touched() {
        let edited = set_string_in_table(DEPLOY_TOML, "environment", "name", "x")
            .expect("edit [environment]");
        assert!(edited.contains("name = \"acme-forecast\""), "{edited}");
        assert!(edited.contains("\nname = \"x\"\n"), "{edited}");
    }

    #[test]
    fn sets_the_target_type() {
        let edited = set_string_in_table(DEPLOY_TOML, "target", "type", "pmcp-run").expect("edit");
        assert!(edited.contains("type = \"pmcp-run\""), "{edited}");
        assert!(edited.starts_with("# operator comment: keep me\n"));
    }

    #[test]
    fn escapes_the_value() {
        let edited = set_string_in_table(DEPLOY_TOML, "server", "name", "a\"b").expect("edit");
        let parsed: toml::Table = toml::from_str(&edited).expect("parses");
        assert_eq!(parsed["server"]["name"].as_str(), Some("a\"b"));
    }

    #[test]
    fn keeps_crlf_line_endings() {
        let crlf = DEPLOY_TOML.replace('\n', "\r\n");
        let edited = set_string_in_table(&crlf, "server", "name", "acme").expect("edit");
        assert!(edited.contains("name = \"acme\"\r\n"), "{edited:?}");
    }

    #[test]
    fn refuses_when_the_table_or_key_is_missing() {
        assert_eq!(set_string_in_table(DEPLOY_TOML, "gcp", "region", "x"), None);
        assert_eq!(
            set_string_in_table(DEPLOY_TOML, "server", "binary", "x"),
            None
        );
    }

    #[test]
    fn refuses_dotted_and_inline_forms_instead_of_guessing() {
        let dotted = "server.name = \"old\"\n";
        assert_eq!(set_string_in_table(dotted, "server", "name", "new"), None);
        let inline = "server = { name = \"old\" }\n";
        assert_eq!(set_string_in_table(inline, "server", "name", "new"), None);
    }

    /// A `name = ...` line inside a multi-line string must not be edited:
    /// the re-parse check sees that the edit changed something else.
    #[test]
    fn refuses_a_lookalike_line_inside_a_multiline_string() {
        let tricky = "[server]\ndescription = \"\"\"\nname = \"inside\"\n\"\"\"\n";
        assert_eq!(set_string_in_table(tricky, "server", "name", "new"), None);
    }

    #[test]
    fn array_of_tables_headers_end_the_section() {
        let text = "[server]\nname = \"a\"\n[[widgets]]\nname = \"w\"\n";
        let edited = set_string_in_table(text, "server", "name", "b").expect("edit");
        assert_eq!(
            edited,
            "[server]\nname = \"b\"\n[[widgets]]\nname = \"w\"\n"
        );
    }

    #[test]
    fn append_section_keeps_existing_content_and_separates_with_a_blank_line() {
        assert_eq!(
            append_section("[server]\nname = \"a\"", "[aws]\nregion = \"x\"\n"),
            "[server]\nname = \"a\"\n\n[aws]\nregion = \"x\"\n"
        );
        assert_eq!(append_section("", "[aws]\n"), "[aws]\n");
    }

    #[test]
    fn toml_string_literal_round_trips() {
        for value in ["plain", "with \"quotes\"", "back\\slash", "uni-çø", ""] {
            let doc = format!("k = {}\n", toml_string_literal(value));
            let parsed: toml::Table = toml::from_str(&doc).expect("parses");
            assert_eq!(parsed["k"].as_str(), Some(value));
        }
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        /// Setting `[server] name` on any generated deploy.toml either fails
        /// cleanly or changes exactly that value: every other parsed value,
        /// and every other line, is unchanged.
        #[test]
        fn setting_the_name_changes_exactly_one_value(
            old in "[a-z][a-z0-9-]{0,20}",
            new in "[a-zA-Z][a-zA-Z0-9-]{0,30}",
            region in "[a-z]{2}-[a-z]{4,9}-[1-3]",
            env_key in "[A-Z][A-Z_]{0,10}",
            env_val in "[ -~]{0,20}",
        ) {
            let text = format!(
                "[target]\ntype = \"aws-lambda\"\n\n[aws]\nregion = {}\n\n[server]\nname = {}\n\n[environment]\n{} = {}\n",
                toml_string_literal(&region),
                toml_string_literal(&old),
                env_key,
                toml_string_literal(&env_val),
            );
            let edited = set_string_in_table(&text, "server", "name", &new).expect("plain layout edits");
            let before: Vec<&str> = text.lines().collect();
            let after: Vec<&str> = edited.lines().collect();
            prop_assert_eq!(before.len(), after.len());
            let changed = (0..before.len()).filter(|i| before[*i] != after[*i]).count();
            prop_assert!(changed <= 1);
            let parsed: toml::Table = toml::from_str(&edited).expect("parses");
            prop_assert_eq!(parsed["server"]["name"].as_str(), Some(new.as_str()));
            prop_assert_eq!(parsed["environment"][&env_key].as_str(), Some(env_val.as_str()));
        }

        /// Arbitrary input never panics, and any edit that IS returned
        /// parses with the requested value in place.
        #[test]
        fn editing_arbitrary_text_is_total(text in "[\\PC\\n\\r]{0,300}", value in "[ -~]{0,20}") {
            if let Some(edited) = set_string_in_table(&text, "server", "name", &value) {
                let parsed: toml::Table = toml::from_str(&edited).expect("verified edit parses");
                prop_assert_eq!(parsed["server"]["name"].as_str(), Some(value.as_str()));
            }
        }
    }
}
