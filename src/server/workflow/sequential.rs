//! Sequential workflow implementation
//!
//! Orchestrates multiple workflow steps in sequence with data flow validation.

use super::{
    error::WorkflowError,
    newtypes::{ArgName, BindingName},
    prompt_content::InternalPromptMessage,
    workflow_step::WorkflowStep,
};
use crate::types::PromptArgumentType;
use indexmap::IndexMap;
use smallvec::SmallVec;

/// A sequential workflow that executes steps in order
#[derive(Clone, Debug)]
pub struct SequentialWorkflow {
    /// Workflow name
    name: String,
    /// Workflow description
    description: String,
    /// Required prompt arguments
    /// `IndexMap` for deterministic iteration
    arguments: IndexMap<ArgName, ArgumentSpec>,
    /// Workflow steps in execution order
    /// `SmallVec` optimized for 2-4 steps
    steps: SmallVec<[WorkflowStep; 3]>,
    /// Instruction messages for the workflow
    /// `SmallVec` optimized for 2-3 instructions
    instructions: SmallVec<[InternalPromptMessage; 3]>,
    /// Whether this workflow should be backed by a task for durable progress tracking.
    /// When true and a task router is configured on the server, the workflow will be
    /// wrapped in a [`TaskWorkflowPromptHandler`](super::TaskWorkflowPromptHandler)
    /// that creates a task on invocation.
    task_support: bool,
}

/// Specification for a prompt argument
#[derive(Clone, Debug)]
pub struct ArgumentSpec {
    /// Argument description
    pub description: String,
    /// Whether the argument is required
    pub required: bool,
    /// Type hint for the argument (PMCP extension)
    ///
    /// When set, string arguments will be validated and converted to the
    /// appropriate type before being passed to tool calls.
    pub arg_type: Option<PromptArgumentType>,
}

impl SequentialWorkflow {
    /// Create a new sequential workflow
    ///
    /// # Example
    /// ```
    /// use pmcp::server::workflow::SequentialWorkflow;
    ///
    /// let workflow = SequentialWorkflow::new(
    ///     "content_workflow",
    ///     "Create and review content"
    /// );
    /// ```
    #[must_use]
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            arguments: IndexMap::new(),
            steps: SmallVec::new(),
            instructions: SmallVec::new(),
            task_support: false,
        }
    }

    /// Add a prompt argument (chainable)
    ///
    /// This adds a string-typed argument. For numeric or boolean arguments,
    /// use [`typed_argument`](Self::typed_argument) instead.
    ///
    /// # Example
    /// ```
    /// use pmcp::server::workflow::SequentialWorkflow;
    ///
    /// let workflow = SequentialWorkflow::new("workflow", "description")
    ///     .argument("topic", "The topic to write about", true);
    /// ```
    #[must_use]
    pub fn argument(
        mut self,
        name: impl Into<ArgName>,
        description: impl Into<String>,
        required: bool,
    ) -> Self {
        self.arguments.insert(
            name.into(),
            ArgumentSpec {
                description: description.into(),
                required,
                arg_type: None, // Default to string (no type hint)
            },
        );
        self
    }

    /// Add a typed prompt argument (chainable)
    ///
    /// This adds an argument with a type hint. The type hint enables:
    /// - Validation of string arguments before processing
    /// - Automatic conversion to the correct JSON type for tool calls
    /// - Better UX in MCP clients (appropriate input widgets)
    ///
    /// # Example
    /// ```
    /// use pmcp::server::workflow::SequentialWorkflow;
    /// use pmcp::types::PromptArgumentType;
    ///
    /// let workflow = SequentialWorkflow::new("calculator", "Calculate something")
    ///     .typed_argument("x", "First number", true, PromptArgumentType::Number)
    ///     .typed_argument("y", "Second number", true, PromptArgumentType::Number)
    ///     .typed_argument("verbose", "Show steps", false, PromptArgumentType::Boolean);
    /// ```
    #[must_use]
    pub fn typed_argument(
        mut self,
        name: impl Into<ArgName>,
        description: impl Into<String>,
        required: bool,
        arg_type: PromptArgumentType,
    ) -> Self {
        self.arguments.insert(
            name.into(),
            ArgumentSpec {
                description: description.into(),
                required,
                arg_type: Some(arg_type),
            },
        );
        self
    }

    /// Add a workflow step (chainable)
    ///
    /// # Example
    /// ```
    /// use pmcp::server::workflow::{SequentialWorkflow, WorkflowStep, ToolHandle};
    ///
    /// let workflow = SequentialWorkflow::new("workflow", "description")
    ///     .step(WorkflowStep::new("step1", ToolHandle::new("create_content")));
    /// ```
    #[must_use]
    pub fn step(mut self, step: WorkflowStep) -> Self {
        self.steps.push(step);
        self
    }

    /// Add an instruction message (chainable)
    ///
    /// # Example
    /// ```
    /// use pmcp::server::workflow::{SequentialWorkflow, InternalPromptMessage};
    ///
    /// let workflow = SequentialWorkflow::new("workflow", "description")
    ///     .instruction(InternalPromptMessage::system("Process the content carefully"));
    /// ```
    #[must_use]
    pub fn instruction(mut self, message: InternalPromptMessage) -> Self {
        self.instructions.push(message);
        self
    }

    /// Enable task support for this workflow.
    ///
    /// When task support is enabled and a task router is configured on the server
    /// (via [`ServerCoreBuilder::with_task_store()`](crate::server::builder::ServerCoreBuilder::with_task_store)),
    /// invoking this workflow prompt will create a task and track step progress
    /// in task variables.
    ///
    /// If task support is enabled but no task router is configured, the builder
    /// will return an error at build time.
    ///
    /// # Example
    /// ```
    /// use pmcp::server::workflow::{SequentialWorkflow, WorkflowStep, ToolHandle};
    ///
    /// let workflow = SequentialWorkflow::new("deploy", "Deploy a service")
    ///     .step(WorkflowStep::new("validate", ToolHandle::new("validate_config")))
    ///     .step(WorkflowStep::new("deploy", ToolHandle::new("deploy_service")))
    ///     .with_task_support(true);
    ///
    /// assert!(workflow.has_task_support());
    /// ```
    #[must_use]
    pub fn with_task_support(mut self, enabled: bool) -> Self {
        self.task_support = enabled;
        self
    }

    /// Returns whether this workflow has task support enabled.
    pub fn has_task_support(&self) -> bool {
        self.task_support
    }

    /// Get workflow name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get workflow description
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Get arguments
    pub fn arguments(&self) -> &IndexMap<ArgName, ArgumentSpec> {
        &self.arguments
    }

    /// Get steps
    pub fn steps(&self) -> &[WorkflowStep] {
        &self.steps
    }

    /// Get instructions
    pub fn instructions(&self) -> &[InternalPromptMessage] {
        &self.instructions
    }

    /// Project this workflow into a SEP-2640 [`Skill`] (D-01).
    ///
    /// The returned skill's `SKILL.md` body is rendered from this workflow's
    /// own introspection surface, so the skill a host reads and the prompt a
    /// host runs are one content rendered twice — they cannot drift, because
    /// the SDK owns the renderer.
    ///
    /// The skill's name is this workflow's name normalized to the
    /// agentskills alphabet (`refund_flow` becomes `refund-flow`), and no URI
    /// path override is set, so the frontmatter `name` and the final segment of
    /// `skill://{name}/SKILL.md` agree by construction.
    ///
    /// # Infallible by design
    ///
    /// This method never fails and never panics (D-02, D-15). When a workflow
    /// name normalizes to nothing legal it emits a `tracing::warn!` on the
    /// `mcp.skills` target and substitutes a deterministic `workflow-{8 hex}`
    /// slug; when the description is empty it substitutes a deterministic legal
    /// one. This deliberately DIVERGES from [`Skill::with_reference`]'s
    /// `Err(e) => panic!` arm — restoring a panic here would reintroduce Phase
    /// 125's WR-03 finding. A caller that wants those conditions to be hard
    /// errors, and wants the warnings as data, uses the fallible builder path
    /// instead.
    ///
    /// The rendered text is not semver-stable; see
    /// [`crate::server::skills::projection`] for the re-pinning contract.
    ///
    /// [`Skill`]: crate::server::skills::Skill
    /// [`Skill::with_reference`]: crate::server::skills::Skill::with_reference
    ///
    /// # Examples
    ///
    /// ```rust
    /// # #[cfg(feature = "skills")] {
    /// use pmcp::server::workflow::SequentialWorkflow;
    ///
    /// let workflow = SequentialWorkflow::new("refund_flow", "Process a refund");
    /// let skill = workflow.as_skill();
    ///
    /// assert_eq!(skill.name(), "refund-flow");
    /// assert!(skill.body().starts_with("---\nname: \"refund-flow\"\n"));
    /// # }
    /// ```
    #[cfg(feature = "skills")]
    #[must_use]
    pub fn as_skill(&self) -> crate::server::skills::Skill {
        crate::server::skills::projection::project(self)
    }

    /// Validate the workflow
    ///
    /// Checks:
    /// - All steps reference valid bindings
    /// - No circular dependencies
    /// - All prompt arguments referenced by steps are defined
    ///
    /// # Binding behavior
    /// Steps can only be referenced by their explicit binding names set via `.bind()`.
    /// Steps without bindings cannot have their outputs referenced by later steps.
    pub fn validate(&self) -> Result<(), WorkflowError> {
        let mut available_bindings = Vec::new();

        // Validate each step in sequence
        for step in &self.steps {
            // Validate step can access required bindings
            step.validate(&available_bindings)?;

            // Add this step's binding to available bindings (if it has one)
            // Only explicit bindings can be referenced by later steps
            if let Some(binding) = step.binding() {
                available_bindings.push(binding.clone());
            }
        }

        // Validate all prompt arguments referenced in steps are defined
        for step in &self.steps {
            for (_, source) in step.arguments() {
                if let super::data_source::DataSource::PromptArg(arg_name) = source {
                    if !self.arguments.contains_key(arg_name) {
                        return Err(WorkflowError::InvalidMapping {
                            step: step.name().to_string(),
                            reason: format!("References undefined prompt argument '{}'", arg_name),
                        });
                    }
                }
            }
        }

        Ok(())
    }

    /// Get all bindings that will be available after executing all steps
    pub fn output_bindings(&self) -> Vec<BindingName> {
        self.steps
            .iter()
            .filter_map(|step| step.binding().cloned())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::workflow::{dsl::*, ToolHandle};
    use serde_json::json;

    #[test]
    fn test_sequential_workflow_creation() {
        let workflow = SequentialWorkflow::new("content_workflow", "Create content");
        assert_eq!(workflow.name(), "content_workflow");
        assert_eq!(workflow.description(), "Create content");
        assert!(workflow.arguments().is_empty());
        assert!(workflow.steps().is_empty());
    }

    #[test]
    fn test_sequential_workflow_with_arguments() {
        let workflow = SequentialWorkflow::new("workflow", "description")
            .argument("topic", "The topic", true)
            .argument("style", "Writing style", false);

        assert_eq!(workflow.arguments().len(), 2);
        assert!(workflow.arguments().contains_key(&ArgName::new("topic")));
        assert!(workflow.arguments().contains_key(&ArgName::new("style")));
    }

    #[test]
    fn test_sequential_workflow_with_steps() {
        let workflow = SequentialWorkflow::new("workflow", "description")
            .step(WorkflowStep::new("step1", ToolHandle::new("create")).bind("content"))
            .step(WorkflowStep::new("step2", ToolHandle::new("review")).bind("review"));

        assert_eq!(workflow.steps().len(), 2);
    }

    #[test]
    fn test_sequential_workflow_with_instructions() {
        let workflow = SequentialWorkflow::new("workflow", "description")
            .instruction(InternalPromptMessage::system("Be concise"))
            .instruction(InternalPromptMessage::system("Be accurate"));

        assert_eq!(workflow.instructions().len(), 2);
    }

    #[test]
    fn test_sequential_workflow_validation_success() {
        let workflow = SequentialWorkflow::new("workflow", "description")
            .argument("topic", "The topic", true)
            .step(
                WorkflowStep::new("step1", ToolHandle::new("create"))
                    .arg("topic", prompt_arg("topic"))
                    .bind("content"),
            )
            .step(
                WorkflowStep::new("step2", ToolHandle::new("review"))
                    .arg("content", from_step("content"))  // Reference binding, not step name
                    .bind("review"),
            );

        assert!(workflow.validate().is_ok());
    }

    #[test]
    fn test_sequential_workflow_validation_unknown_binding() {
        let workflow = SequentialWorkflow::new("workflow", "description").step(
            WorkflowStep::new("step1", ToolHandle::new("review"))
                .arg("content", from_step("nonexistent")),
        );

        let result = workflow.validate();
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            WorkflowError::UnknownBinding { .. }
        ));
    }

    #[test]
    fn test_sequential_workflow_validation_undefined_prompt_arg() {
        let workflow = SequentialWorkflow::new("workflow", "description").step(
            WorkflowStep::new("step1", ToolHandle::new("create"))
                .arg("topic", prompt_arg("undefined_arg")),
        );

        let result = workflow.validate();
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            WorkflowError::InvalidMapping { .. }
        ));
    }

    #[test]
    fn test_sequential_workflow_output_bindings() {
        let workflow = SequentialWorkflow::new("workflow", "description")
            .step(WorkflowStep::new("step1", ToolHandle::new("create")).bind("content"))
            .step(WorkflowStep::new("step2", ToolHandle::new("review")).bind("review"))
            .step(
                WorkflowStep::new("step3", ToolHandle::new("format")), // No binding
            );

        let bindings = workflow.output_bindings();
        assert_eq!(bindings.len(), 2);
        assert_eq!(bindings[0].as_str(), "content");
        assert_eq!(bindings[1].as_str(), "review");
    }

    #[test]
    fn test_sequential_workflow_complete_example() {
        let workflow =
            SequentialWorkflow::new("content_creation", "Create, review, and publish content")
                .argument("topic", "The topic to write about", true)
                .argument("target_audience", "Target audience", false)
                .instruction(InternalPromptMessage::system(
                    "Create high-quality content following these steps",
                ))
                .step(
                    WorkflowStep::new("create", ToolHandle::new("create_content"))
                        .arg("topic", prompt_arg("topic"))
                        .arg("audience", prompt_arg("target_audience"))
                        .bind("draft"),
                )
                .step(
                    WorkflowStep::new("review", ToolHandle::new("review_content"))
                        .arg("content", from_step("draft"))  // Reference binding name
                        .arg("criteria", constant(json!(["grammar", "clarity", "tone"])))
                        .bind("review_result"),
                )
                .step(
                    WorkflowStep::new("publish", ToolHandle::new("publish_content"))
                        .arg("content", field("draft", "text"))  // Reference binding name
                        .arg("metadata", field("review_result", "metadata")), // Reference binding name
                );

        assert!(workflow.validate().is_ok());
        assert_eq!(workflow.arguments().len(), 2);
        assert_eq!(workflow.steps().len(), 3);
        assert_eq!(workflow.instructions().len(), 1);
    }

    #[test]
    fn test_sequential_workflow_clone() {
        let workflow = SequentialWorkflow::new("workflow", "description")
            .argument("arg", "description", true)
            .step(WorkflowStep::new("step1", ToolHandle::new("tool")));

        let cloned = workflow.clone();
        assert_eq!(cloned.name(), workflow.name());
        assert_eq!(cloned.arguments().len(), workflow.arguments().len());
    }

    #[test]
    fn test_sequential_workflow_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<SequentialWorkflow>();
    }

    #[test]
    fn test_argument_spec_required() {
        let spec = ArgumentSpec {
            description: "Test arg".to_string(),
            required: true,
            arg_type: None,
        };
        assert!(spec.required);
    }

    #[test]
    fn test_argument_spec_optional() {
        let spec = ArgumentSpec {
            description: "Test arg".to_string(),
            required: false,
            arg_type: None,
        };
        assert!(!spec.required);
    }

    #[test]
    fn test_task_support_defaults_to_false() {
        let workflow = SequentialWorkflow::new("workflow", "description");
        assert!(!workflow.has_task_support());
    }

    #[test]
    fn test_with_task_support_enabled() {
        let workflow = SequentialWorkflow::new("workflow", "description").with_task_support(true);
        assert!(workflow.has_task_support());
    }

    #[test]
    fn test_with_task_support_disabled() {
        let workflow = SequentialWorkflow::new("workflow", "description")
            .with_task_support(true)
            .with_task_support(false);
        assert!(!workflow.has_task_support());
    }

    #[test]
    fn test_task_support_preserved_on_clone() {
        let workflow = SequentialWorkflow::new("workflow", "description").with_task_support(true);
        let cloned = workflow.clone();
        assert!(cloned.has_task_support());
    }

    #[test]
    fn test_sequential_workflow_step_without_binding_cannot_be_referenced() {
        // Step without binding cannot be referenced
        let workflow = SequentialWorkflow::new("workflow", "description")
            .step(
                WorkflowStep::new("step1", ToolHandle::new("create")), // No .bind() - output cannot be referenced
            )
            .step(
                WorkflowStep::new("step2", ToolHandle::new("review"))
                    .arg("content", from_step("step1")), // Try to reference step1
            );

        let result = workflow.validate();
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            WorkflowError::UnknownBinding { .. }
        ));
    }
}
