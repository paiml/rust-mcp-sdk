pub mod add;
pub mod app;
pub mod connect;
pub mod deploy;
pub mod dev;
pub mod landing;
pub mod loadtest;
pub mod new;
pub mod preview;
pub mod schema;
pub mod secret;
pub mod test;
pub mod validate;

/// Global CLI flags shared across all commands.
///
/// Constructed in `main()` from top-level CLI args and passed to every
/// command handler. The `no_color` field reflects the *resolved* value
/// (CLI flag OR `NO_COLOR` env OR non-TTY), so downstream code can use
/// it directly without re-checking the environment.
///
/// The `quiet` field reflects the *resolved* value after verbose-wins-over-quiet
/// precedence: if both `--verbose` and `--quiet` are passed, quiet is disabled.
#[derive(Clone, Debug)]
pub struct GlobalFlags {
    /// Enable verbose output for debugging.
    ///
    /// Currently used to resolve quiet precedence (verbose wins over quiet).
    /// Will be used by individual commands for detailed output in future phases.
    #[allow(dead_code)]
    pub verbose: bool,
    /// Suppress colored output (resolved: flag, env, or non-TTY).
    pub no_color: bool,
    /// Suppress all non-error output.
    ///
    /// When active, only error messages (from `anyhow::bail!`, `eprintln!("Error: ...")`)
    /// and explicitly requested output (schema export, test results, secret values) are shown.
    /// All informational, decorative, success, warning, and progress output is suppressed.
    pub quiet: bool,
}

impl GlobalFlags {
    /// Returns true if decorative output should be shown (i.e. not in quiet mode).
    pub fn should_output(&self) -> bool {
        !self.quiet
    }
}
