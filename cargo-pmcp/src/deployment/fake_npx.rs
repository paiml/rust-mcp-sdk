//! Test-only recording stand-in for `npx`, so the exact `cdk` argv cargo-pmcp
//! spawns can be asserted without Node.js, the CDK CLI, or AWS credentials.
//!
//! The stand-in is a POSIX `sh` script (Unix only). Every invocation appends
//! its argv, space-joined, as one line of `calls.log`. `cdk list` / `cdk ls`
//! additionally prints a canned stdout and stderr and exits with a canned
//! status, and records whether `lib/stack.ts` existed (relative to the child's
//! working directory, which is the project's `deploy/` directory) and which
//! `SERVER_NAME` / `AWS_REGION` it saw. Every other invocation exits 0.
//! [`FakeNpx::listing_scaffold_app_ts`] instead answers `cdk list` from the
//! project's current `bin/app.ts`, the way CDK would for the scaffold.

use std::path::{Path, PathBuf};

/// A recording `npx` stand-in living in its own directory.
pub struct FakeNpx {
    script: PathBuf,
    log: PathBuf,
}

impl FakeNpx {
    /// Write the stand-in into `dir`. `cdk list` prints `list_stdout` on
    /// stdout and `list_stderr` on stderr, then exits with `list_exit`.
    pub fn new(dir: &Path, list_stdout: &str, list_stderr: &str, list_exit: i32) -> Self {
        std::fs::create_dir_all(dir).expect("create fake-npx dir");
        let out = dir.join("list.out");
        std::fs::write(&out, list_stdout).expect("write list.out");
        Self::with_list_command(
            dir,
            &format!("cat '{}'", out.display()),
            list_stderr,
            list_exit,
        )
    }

    /// Like [`Self::new`], but `cdk list` answers the way CDK would for the
    /// `deploy/bin/app.ts` scaffold: it prints `<serverName>-stack`, parsed
    /// from the `const serverName = '...';` line of the CURRENT `bin/app.ts`.
    pub fn listing_scaffold_app_ts(dir: &Path) -> Self {
        Self::with_list_command(
            dir,
            "sed -n \"s/^const serverName = '\\(.*\\)';\\$/\\1-stack/p\" bin/app.ts",
            "",
            0,
        )
    }

    fn with_list_command(
        dir: &Path,
        list_stdout_cmd: &str,
        list_stderr: &str,
        list_exit: i32,
    ) -> Self {
        std::fs::create_dir_all(dir).expect("create fake-npx dir");
        let log = dir.join("calls.log");
        let err = dir.join("list.err");
        std::fs::write(&err, list_stderr).expect("write list.err");
        let script = dir.join("npx");
        let body = format!(
            "#!/bin/sh\n\
             printf '%s\\n' \"$*\" >> '{log}'\n\
             if [ \"$1\" = cdk ] && {{ [ \"$2\" = list ] || [ \"$2\" = ls ]; }}; then\n\
             \x20 if [ -f lib/stack.ts ]; then echo 'probe stack.ts=present' >> '{log}'; \
             else echo 'probe stack.ts=missing' >> '{log}'; fi\n\
             \x20 echo \"probe SERVER_NAME=$SERVER_NAME AWS_REGION=$AWS_REGION\" >> '{log}'\n\
             \x20 {list_stdout_cmd}\n\
             \x20 cat '{err}' >&2\n\
             \x20 exit {list_exit}\n\
             fi\n\
             exit 0\n",
            log = log.display(),
            err = err.display(),
        );
        std::fs::write(&script, body).expect("write fake npx");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&script)
                .expect("stat fake npx")
                .permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&script, perms).expect("chmod fake npx");
        }
        Self { script, log }
    }

    /// Path to pass wherever cargo-pmcp would spawn `npx`.
    pub fn program(&self) -> &Path {
        &self.script
    }

    /// Every recorded line, in order: one argv line per invocation, plus the
    /// `probe ...` lines a `cdk list` invocation adds after its argv line.
    pub fn calls(&self) -> Vec<String> {
        std::fs::read_to_string(&self.log)
            .unwrap_or_default()
            .lines()
            .map(str::to_string)
            .collect()
    }

    /// The argv lines only (the `probe ...` lines filtered out).
    pub fn argv_lines(&self) -> Vec<String> {
        self.calls()
            .into_iter()
            .filter(|line| !line.starts_with("probe "))
            .collect()
    }

    /// True when any recorded argv line starts with `prefix`.
    pub fn ran(&self, prefix: &str) -> bool {
        self.argv_lines()
            .iter()
            .any(|line| line.starts_with(prefix))
    }
}
