//! Stack-identity guard for the aws-lambda `npx cdk deploy` path.
//!
//! cargo-pmcp names, reports, reads outputs for, and destroys an aws-lambda
//! deployment as `{[server] name}-stack`. But the stack `npx cdk` acts on is
//! whatever `deploy/bin/app.ts` declares, and `deploy init` writes the server
//! name into app.ts once, as a literal. After a `[server] name` rename the two
//! disagree. Before this guard, `cdk deploy` (run with no stack argument)
//! silently updated the OLD stack in place, and `cdk destroy <new>-stack`
//! matched no stack and still exited 0 (aws-cdk issue #27179), so `destroy`
//! reported success while the stack kept running.
//!
//! The guard asks the CDK app itself which stacks it declares (`cdk list`,
//! with the same environment as the deploy) and refuses before any
//! `CloudFormation` change unless the expected stack is among them. The deploy
//! then names that stack explicitly. Debug session `cargo-pmcp-deploy-targets`.
//!
//! Since cargo-pmcp 0.28.0 an unmodified `deploy/bin/app.ts` follows
//! `[server] name` on its own (when `deploy/lib/stack.ts` is unmodified too),
//! so this guard refuses only hand-edited app.ts/stack.ts combinations, and
//! `destroy` no longer goes through `npx cdk` at all: it deletes the stack
//! with `CloudFormation` directly (`targets::aws_lambda::teardown`).

use anyhow::{bail, Context, Result};
use std::io::Write;
use std::process::Command;

/// One stack as `cdk list` reports it.
///
/// Newer CDK CLIs print `id (stackName)` when a stack's `CloudFormation` name
/// differs from its construct path. Older ones print the construct path only,
/// in which case `stack_name` is `None`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredStack {
    /// The construct path, which is what `cdk deploy <name>` selects on.
    pub id: String,
    /// The `CloudFormation` stack name, when `cdk list` printed one.
    pub stack_name: Option<String>,
}

impl DeclaredStack {
    /// True when this is exactly the stack `expected` names: the construct path
    /// equals it and, when CDK printed a `CloudFormation` name, so does that.
    pub fn is(&self, expected: &str) -> bool {
        self.id == expected && self.stack_name.as_deref().is_none_or(|n| n == expected)
    }

    /// The `CloudFormation` stack this entry deploys to: the printed stack
    /// name when there is one, otherwise the construct path (they are equal
    /// for a top-level stack with no explicit `stackName`).
    fn cloudformation_name(&self) -> &str {
        self.stack_name.as_deref().unwrap_or(&self.id)
    }

    fn display(&self) -> String {
        self.stack_name
            .as_ref()
            .map_or_else(|| self.id.clone(), |name| format!("{} ({name})", self.id))
    }
}

/// The stack name cargo-pmcp deploys, reports and destroys for a server.
pub fn expected_stack_name(server_name: &str) -> String {
    format!("{server_name}-stack")
}

/// Remove ANSI CSI escape sequences (`ESC [ ... final-byte`), so a colorized
/// `cdk list` (e.g. under `FORCE_COLOR`) parses the same as a plain one.
fn strip_ansi(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' && chars.peek() == Some(&'[') {
            chars.next();
            // Parameter and intermediate bytes run until a final byte in @..~.
            for c in chars.by_ref() {
                if ('@'..='~').contains(&c) {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Parse one `cdk list` line. Blank lines yield `None`.
fn parse_line(line: &str) -> Option<DeclaredStack> {
    let line = strip_ansi(line);
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    let split = line
        .strip_suffix(')')
        .and_then(|inner| inner.rsplit_once(" ("))
        .filter(|(id, name)| !id.trim().is_empty() && !name.is_empty());
    Some(match split {
        Some((id, name)) => DeclaredStack {
            id: id.trim().to_string(),
            stack_name: Some(name.to_string()),
        },
        None => DeclaredStack {
            id: line.to_string(),
            stack_name: None,
        },
    })
}

/// Parse `cdk list` stdout: one declared stack per non-blank line.
pub fn parse_cdk_list_output(stdout: &str) -> Vec<DeclaredStack> {
    stdout.lines().filter_map(parse_line).collect()
}

/// `[server] name` value that would make cargo-pmcp target the single
/// declared stack, when there is exactly one and it follows the scaffold's
/// `{name}-stack` shape.
fn suggested_server_name(declared: &[DeclaredStack]) -> Option<&str> {
    match declared {
        [only] => only
            .cloudformation_name()
            .strip_suffix("-stack")
            .filter(|name| !name.is_empty()),
        _ => None,
    }
}

fn declared_list(declared: &[DeclaredStack]) -> String {
    if declared.is_empty() {
        return "(none reported by `cdk list`)".to_string();
    }
    declared
        .iter()
        .map(DeclaredStack::display)
        .collect::<Vec<_>>()
        .join(", ")
}

fn keep_existing_hint(declared: &[DeclaredStack]) -> String {
    suggested_server_name(declared).map_or_else(
        || "set `[server] name` in .pmcp/deploy.toml to match a declared stack".to_string(),
        |name| format!("set `[server] name = \"{name}\"` in .pmcp/deploy.toml"),
    )
}

fn deploy_recovery(server_name: &str, declared: &[DeclaredStack], region: &str) -> String {
    let expected = expected_stack_name(server_name);
    let old_stack_note = match declared {
        [only] => format!(
            " `{old}` keeps running until you delete it: \
             aws cloudformation delete-stack --stack-name {old} --region {region}",
            old = only.cloudformation_name(),
        ),
        _ => String::new(),
    };
    format!(
        "Nothing was deployed; CloudFormation is unchanged.\n\
         To recover, choose one:\n  \
         - Keep deploying the existing stack: {keep}.\n  \
         - Deploy under the new name: set `const serverName = '{server_name}';` in \
         deploy/bin/app.ts, and the same serverName in deploy/lib/stack.ts if you customized \
         it (an unmodified stack.ts is regenerated for you), then re-run. This creates a NEW \
         stack, `{expected}`.{old_stack_note}",
        keep = keep_existing_hint(declared),
    )
}

/// The refusal text for a mismatch: names the expected and the declared
/// stacks and says how to recover.
pub fn mismatch_message(server_name: &str, declared: &[DeclaredStack], region: &str) -> String {
    let expected = expected_stack_name(server_name);
    format!(
        "refusing to run `cdk deploy`: deploy/bin/app.ts does not declare the stack \
         cargo-pmcp targets.\n  \
         expected: {expected}  (from `[server] name = \"{server_name}\"` in .pmcp/deploy.toml)\n  \
         declared: {declared}\n\
         {recovery}",
        declared = declared_list(declared),
        recovery = deploy_recovery(server_name, declared, region),
    )
}

/// Pure guard predicate: `Ok` iff the declared stacks include
/// `{server_name}-stack`, otherwise an error carrying [`mismatch_message`].
pub fn check_declared(server_name: &str, declared: &[DeclaredStack], region: &str) -> Result<()> {
    let expected = expected_stack_name(server_name);
    if declared.iter().any(|stack| stack.is(&expected)) {
        return Ok(());
    }
    bail!("{}", mismatch_message(server_name, declared, region))
}

/// Run `list_cmd` (a `cdk list` child in deploy/, built with the same
/// environment as the `cdk deploy` it guards) and refuse unless the app
/// declares `{server_name}-stack`. Fails closed: if `cdk list` cannot run or
/// exits non-zero, the stack cannot be verified and the deploy is refused.
pub fn ensure_app_declares_stack(
    mut list_cmd: Command,
    server_name: &str,
    region: &str,
) -> Result<()> {
    let expected = expected_stack_name(server_name);
    print!("   Verifying the CDK app declares stack {expected}...");
    std::io::stdout().flush()?;

    let output = match list_cmd.output() {
        Ok(output) => output,
        Err(e) => {
            println!(" ❌");
            return Err(e)
                .context("Failed to run `npx cdk list`. Make sure Node.js and npm are installed");
        },
    };
    if !output.status.success() {
        println!(" ❌");
        bail!(
            "refusing to run `cdk deploy`: could not verify which stacks deploy/bin/app.ts \
             declares (`npx cdk list` failed: {status}).\n\
             cdk list stderr:\n{stderr}\n\
             CloudFormation is unchanged. Make `npx cdk list` succeed in deploy/, then re-run.",
            status = output.status,
            stderr = String::from_utf8_lossy(&output.stderr).trim(),
        );
    }

    let declared = parse_cdk_list_output(&String::from_utf8_lossy(&output.stdout));
    let verdict = check_declared(server_name, &declared, region);
    println!("{}", if verdict.is_ok() { " ✅" } else { " ❌" });
    verdict
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stack(id: &str) -> DeclaredStack {
        DeclaredStack {
            id: id.to_string(),
            stack_name: None,
        }
    }

    #[test]
    fn expected_stack_name_appends_stack_suffix() {
        assert_eq!(expected_stack_name("acme"), "acme-stack");
    }

    #[test]
    fn parse_plain_ids_one_per_line() {
        let parsed = parse_cdk_list_output("acme-stack\n\nother-stack\r\n");
        assert_eq!(parsed, vec![stack("acme-stack"), stack("other-stack")]);
    }

    #[test]
    fn parse_display_name_with_cloudformation_name() {
        let parsed = parse_cdk_list_output("Prod/Web (Prod-Web)\n");
        assert_eq!(
            parsed,
            vec![DeclaredStack {
                id: "Prod/Web".to_string(),
                stack_name: Some("Prod-Web".to_string()),
            }]
        );
    }

    #[test]
    fn parse_strips_ansi_color() {
        let parsed = parse_cdk_list_output("\u{1b}[1macme-stack\u{1b}[22m\n");
        assert_eq!(parsed, vec![stack("acme-stack")]);
    }

    /// Only `ESC [` opens an escape sequence: a bare `[` and a bare ESC are
    /// ordinary characters and survive.
    #[test]
    fn strip_ansi_only_consumes_esc_bracket_sequences() {
        assert_eq!(strip_ansi("acme[1]-stack"), "acme[1]-stack");
        assert_eq!(strip_ansi("\u{1b}acme"), "\u{1b}acme");
        assert_eq!(strip_ansi("a\u{1b}[31mb\u{1b}[0mc"), "abc");
    }

    #[test]
    fn parse_keeps_unbalanced_parentheses_as_an_id() {
        assert_eq!(parse_cdk_list_output("(x)\n"), vec![stack("(x)")]);
        assert_eq!(parse_cdk_list_output("a ()\n"), vec![stack("a ()")]);
    }

    #[test]
    fn check_accepts_exact_match_among_several() {
        let declared = vec![stack("shared-vpc"), stack("acme-stack")];
        assert!(check_declared("acme", &declared, "us-east-1").is_ok());
    }

    /// Boundary neighbors of the expected name: one character more, one
    /// less, a different case, and a prefix/suffix all refuse.
    #[test]
    fn check_refuses_near_misses() {
        for near in [
            "acme-stack2",
            "acme-stac",
            "Acme-stack",
            "xacme-stack",
            "acme-stack-dev",
            "acme",
            "Stage/acme-stack",
        ] {
            let declared = vec![stack(near)];
            assert!(
                check_declared("acme", &declared, "us-east-1").is_err(),
                "{near} must not satisfy acme-stack"
            );
        }
    }

    #[test]
    fn check_refuses_matching_id_with_a_different_cloudformation_name() {
        let declared = vec![DeclaredStack {
            id: "acme-stack".to_string(),
            stack_name: Some("old-server-stack".to_string()),
        }];
        assert!(check_declared("acme", &declared, "us-east-1").is_err());
    }

    #[test]
    fn check_accepts_matching_id_with_the_same_cloudformation_name() {
        let declared = vec![DeclaredStack {
            id: "acme-stack".to_string(),
            stack_name: Some("acme-stack".to_string()),
        }];
        assert!(check_declared("acme", &declared, "us-east-1").is_ok());
    }

    #[test]
    fn check_refuses_when_nothing_is_declared() {
        let err =
            check_declared("acme", &[], "us-east-1").expect_err("no declared stacks must refuse");
        assert!(err.to_string().contains("none reported"), "{err}");
    }

    #[test]
    fn deploy_message_names_both_stacks_and_both_recoveries() {
        let msg = mismatch_message("acme", &[stack("old-server-stack")], "us-east-1");
        assert!(msg.contains("refusing to run `cdk deploy`"), "{msg}");
        assert!(msg.contains("expected: acme-stack"), "{msg}");
        assert!(msg.contains("declared: old-server-stack"), "{msg}");
        assert!(msg.contains("CloudFormation is unchanged"), "{msg}");
        assert!(msg.contains("`[server] name = \"old-server\"`"), "{msg}");
        assert!(msg.contains("const serverName = 'acme';"), "{msg}");
        assert!(
            !msg.contains("--regenerate-stack"),
            "must not advise overwriting a customized stack.ts: {msg}"
        );
        assert!(
            msg.contains(
                "aws cloudformation delete-stack --stack-name old-server-stack --region us-east-1"
            ),
            "{msg}"
        );
    }

    /// When CDK prints a `CloudFormation` name that differs from the construct
    /// path, recovery advice points at the stack that actually exists.
    #[test]
    fn recovery_uses_the_cloudformation_name_when_printed() {
        let declared = vec![DeclaredStack {
            id: "acme-stack".to_string(),
            stack_name: Some("old-server-stack".to_string()),
        }];
        let msg = mismatch_message("acme", &declared, "us-east-1");
        assert!(
            msg.contains("declared: acme-stack (old-server-stack)"),
            "{msg}"
        );
        assert!(msg.contains("`[server] name = \"old-server\"`"), "{msg}");
        assert!(
            msg.contains("delete-stack --stack-name old-server-stack --region us-east-1"),
            "{msg}"
        );
    }

    #[test]
    fn suggestion_needs_exactly_one_scaffold_shaped_stack() {
        assert_eq!(suggested_server_name(&[stack("old-stack")]), Some("old"));
        assert_eq!(suggested_server_name(&[stack("-stack")]), None);
        assert_eq!(suggested_server_name(&[stack("custom")]), None);
        assert_eq!(
            suggested_server_name(&[stack("a-stack"), stack("b-stack")]),
            None
        );
        assert_eq!(suggested_server_name(&[]), None);
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    /// Server names in the shape that yields a valid `CloudFormation` stack
    /// name once `-stack` is appended.
    fn arb_server_name() -> impl Strategy<Value = String> {
        "[a-zA-Z][a-zA-Z0-9-]{0,40}"
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]

        /// The guard accepts exactly when `{name}-stack` is declared,
        /// regardless of how many other stacks surround it or where it sits.
        #[test]
        fn accepts_iff_expected_is_declared(
            name in arb_server_name(),
            others in proptest::collection::vec("[a-zA-Z][a-zA-Z0-9-]{0,40}", 0..6),
            insert_at in any::<prop::sample::Index>(),
            include in any::<bool>(),
        ) {
            let expected = expected_stack_name(&name);
            let mut ids: Vec<String> = others.into_iter().filter(|o| *o != expected).collect();
            if include {
                let at = insert_at.index(ids.len() + 1);
                ids.insert(at, expected);
            }
            let stdout = ids.join("\n");
            let declared = parse_cdk_list_output(&stdout);
            let verdict = check_declared(&name, &declared, "us-east-1");
            prop_assert_eq!(verdict.is_ok(), include);
        }

        /// A refusal always names the expected stack and every declared
        /// stack.
        #[test]
        fn refusal_names_expected_and_every_declared_stack(
            name in arb_server_name(),
            others in proptest::collection::vec("[a-zA-Z][a-zA-Z0-9-]{0,40}", 0..6),
        ) {
            let expected = expected_stack_name(&name);
            let declared: Vec<DeclaredStack> = others
                .into_iter()
                .filter(|o| *o != expected)
                .map(|id| DeclaredStack { id, stack_name: None })
                .collect();
            let err = check_declared(&name, &declared, "us-east-1")
                .expect_err("expected stack is absent");
            let msg = err.to_string();
            let expected_line = format!("expected: {}", expected);
            prop_assert!(msg.contains(&expected_line));
            for stack in &declared {
                prop_assert!(msg.contains(&stack.id));
            }
        }

        /// One-character edits of the expected name (append, drop last,
        /// flip the case of the first letter) never satisfy the guard.
        #[test]
        fn one_character_neighbors_never_match(
            name in arb_server_name(),
            extra in "[a-zA-Z0-9-]",
        ) {
            let expected = expected_stack_name(&name);
            let mut flipped = expected.clone();
            let first = flipped.remove(0);
            let first = if first.is_ascii_uppercase() {
                first.to_ascii_lowercase()
            } else {
                first.to_ascii_uppercase()
            };
            flipped.insert(0, first);
            let neighbors = [
                format!("{expected}{extra}"),
                expected[..expected.len() - 1].to_string(),
                flipped,
            ];
            for neighbor in neighbors {
                let declared = parse_cdk_list_output(&neighbor);
                prop_assert!(
                    check_declared(&name, &declared, "us-east-1").is_err(),
                    "{} must not satisfy {}", neighbor, expected
                );
            }
        }

        /// Arbitrary `cdk list` output never panics the parser, and every
        /// parsed id is non-empty and free of surrounding whitespace.
        #[test]
        fn parser_is_total_and_yields_trimmed_ids(stdout in "[\\PC\\n\\r\\x1b]{0,400}") {
            for stack in parse_cdk_list_output(&stdout) {
                prop_assert!(!stack.id.is_empty());
                prop_assert_eq!(stack.id.trim(), stack.id.as_str());
            }
        }
    }
}
