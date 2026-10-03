//! Test-only recording stand-ins for `docker` and `gcloud`, so the exact argv
//! the Cloud Run deploy spawns can be asserted with no Docker daemon, no
//! Google Cloud SDK and no GCP project (debug session
//! `cargo-pmcp-deploy-targets`, PR-D; same idea as `deployment::fake_npx`).
//!
//! Each stand-in is a POSIX `sh` script (Unix only). Every invocation appends
//! `<program> <argv...>` as one line of a shared `calls.log`. A [`Reply`]
//! whose prefix matches the space-joined argv prints its canned stdout and
//! stderr and exits with its status; anything else exits 0 silently.

use std::path::{Path, PathBuf};

/// A canned answer for invocations whose argv starts with `prefix`.
pub struct Reply {
    /// `docker` or `gcloud`.
    pub program: &'static str,
    /// Matched against the start of the space-joined argv.
    pub prefix: &'static str,
    /// Printed on stdout.
    pub stdout: &'static str,
    /// Printed on stderr.
    pub stderr: &'static str,
    /// Exit status.
    pub exit: i32,
}

/// `gcloud auth list` reports an active account.
pub const ACTIVE_ACCOUNT: Reply = Reply {
    program: "gcloud",
    prefix: "auth list",
    stdout: "operator@example.com\n",
    stderr: "",
    exit: 0,
};

/// `gcloud run services describe` reports this service URL.
pub const SERVICE_URL: &str = "https://svc-abc123-uc.a.run.app";

/// The `describe` reply carrying [`SERVICE_URL`].
pub const DESCRIBE_URL: Reply = Reply {
    program: "gcloud",
    prefix: "run services describe",
    stdout: "https://svc-abc123-uc.a.run.app\n",
    stderr: "",
    exit: 0,
};

/// Recording `docker` + `gcloud` stand-ins living in one directory.
pub struct FakeCloud {
    docker: PathBuf,
    gcloud: PathBuf,
    log: PathBuf,
}

impl FakeCloud {
    /// Write both stand-ins into `dir`. `replies` are tried in order; the
    /// first whose program and prefix match answers.
    ///
    /// # Panics
    ///
    /// Panics when a file cannot be written.
    pub fn new(dir: &Path, replies: &[Reply]) -> Self {
        std::fs::create_dir_all(dir).expect("create fake-cloud dir");
        let log = dir.join("calls.log");
        let docker = write_script(dir, "docker", &log, replies);
        let gcloud = write_script(dir, "gcloud", &log, replies);
        Self {
            docker,
            gcloud,
            log,
        }
    }

    /// The `docker` stand-in.
    pub fn docker(&self) -> &Path {
        &self.docker
    }

    /// The `gcloud` stand-in.
    pub fn gcloud(&self) -> &Path {
        &self.gcloud
    }

    /// Every recorded `<program> <argv...>` line, in order.
    pub fn calls(&self) -> Vec<String> {
        std::fs::read_to_string(&self.log)
            .unwrap_or_default()
            .lines()
            .map(str::to_string)
            .collect()
    }

    /// True when a recorded line starts with `prefix`.
    pub fn ran(&self, prefix: &str) -> bool {
        self.calls().iter().any(|line| line.starts_with(prefix))
    }
}

fn write_script(dir: &Path, program: &str, log: &Path, replies: &[Reply]) -> PathBuf {
    let mut cases = String::new();
    for (index, reply) in replies.iter().filter(|r| r.program == program).enumerate() {
        let out = dir.join(format!("{program}-{index}.out"));
        let err = dir.join(format!("{program}-{index}.err"));
        std::fs::write(&out, reply.stdout).expect("write reply stdout");
        std::fs::write(&err, reply.stderr).expect("write reply stderr");
        cases.push_str(&format!(
            "  '{prefix}'*) cat '{out}'; cat '{err}' >&2; exit {exit} ;;\n",
            prefix = reply.prefix,
            out = out.display(),
            err = err.display(),
            exit = reply.exit,
        ));
    }
    let script = dir.join(program);
    let body = format!(
        "#!/bin/sh\nprintf '%s\\n' \"{program} $*\" >> '{log}'\ncase \"$*\" in\n{cases}esac\nexit 0\n",
        log = log.display(),
    );
    std::fs::write(&script, body).expect("write fake program");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&script)
            .expect("stat fake program")
            .permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script, perms).expect("chmod fake program");
    }
    script
}
