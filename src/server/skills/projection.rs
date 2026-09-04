//! Deterministic [`SequentialWorkflow`] -> SEP-2640 [`Skill`] projection.
//!
//! A workflow already carries everything a manual runner needs — a name, a
//! description, argument specs, and an ordered list of steps naming tools and
//! bindings. This module renders that same content as an agentskills-legal
//! `SKILL.md` body, so the served skill and the served prompt are one content
//! rendered twice rather than two documents that can drift apart (D-03).
//!
//! [`SequentialWorkflow`]: crate::server::workflow::SequentialWorkflow
//! [`Skill`]: crate::server::skills::Skill
//!
//! # The rendered text is NOT semver-stable
//!
//! The exact bytes this module produces are pinned by a golden test so that no
//! change is accidental, but they are explicitly **not** part of the crate's
//! semver contract: the render may change on any minor bump (D-14). It changes
//! with a CHANGELOG entry every time, because the bytes become the `sha256`
//! digest published in the skill's `skills/list` entry, and a consumer that
//! pinned that digest must re-pin. A digest mismatch is a fatal pre-loop
//! revocation for such a consumer, not a warning — so a silent render change is
//! a supply-chain event, not a cosmetic one.
//!
//! # Two workflow accessors are excluded from the render on purpose
//!
//! [`SequentialWorkflow::has_task_support`] and [`WorkflowStep::is_retryable`]
//! are server-execution mechanics with no manual analogue: a human or a client
//! LLM following the rendered procedure by hand neither schedules a task nor
//! retries a step the way the server's own executor does. Rendering them would
//! put facts in the body that the reader cannot act on. They are excluded
//! deliberately, and a test asserts they do not appear (D-11).
//!
//! [`SequentialWorkflow::has_task_support`]: crate::server::workflow::SequentialWorkflow::has_task_support
//! [`WorkflowStep::is_retryable`]: crate::server::workflow::WorkflowStep::is_retryable

#[cfg(test)]
mod tests {}
