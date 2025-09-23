---
number: 102B
title: Executor Pure Function Extraction - Phase 2
category: foundation
priority: critical
status: draft
dependencies: [101, 102A]
created: 2025-09-23
---

# Specification 102B: Executor Pure Function Extraction - Phase 2

## Context

This is Phase 2 of decomposing the monolithic executor. With clean interfaces defined in Phase 1 (102A), we can now extract pure functions that don't depend on mutable state. This is the safest refactoring as pure functions are easy to test and have no side effects.

Current issues:
- Many utility functions are embedded as methods that don't actually need `self`
- Validation, interpolation, and formatting logic is mixed with stateful operations
- Hard to unit test these functions in isolation

## Objective

Extract all pure functions from `workflow/executor.rs` into focused modules, reducing the file size by ~30% while improving testability and reusability.

## Requirements

### Functional Requirements
- Extract validation logic to `workflow/validation_engine.rs`
- Extract interpolation helpers to `workflow/interpolation_helpers.rs`
- Extract step building logic to `workflow/step_builder.rs`
- Extract formatting utilities to `workflow/formatters.rs`
- All functions must be pure (no side effects, deterministic)
- Maintain backward compatibility with thin wrapper methods

### Non-Functional Requirements
- Each new module under 300 lines
- Functions average under 20 lines
- 100% test coverage for extracted functions
- No performance regression

## Acceptance Criteria

- [ ] `executor.rs` reduced by at least 1,500 lines
- [ ] Created 4 new focused modules for pure functions
- [ ] All extracted functions have unit tests
- [ ] All existing tests pass without modification
- [ ] Performance benchmarks show no regression
- [ ] Each function has clear documentation

## Technical Details

### Module Structure

```rust
// workflow/validation_engine.rs (new ~250 lines)
pub mod validation_engine {
    use anyhow::{anyhow, Result};
    use super::types::*;

    /// Validate workflow configuration
    pub fn validate_workflow_config(workflow: &ExtendedWorkflowConfig) -> Result<()> {
        // Extracted from WorkflowExecutor::validate_workflow_config
        if workflow.name.is_empty() {
            return Err(anyhow!("Workflow name cannot be empty"));
        }
        // ... validation logic
        Ok(())
    }

    /// Check if step should be executed based on conditions
    pub fn should_execute_step(
        step: &WorkflowStep,
        context: &VariableContext,
        dry_run: bool,
    ) -> Result<bool> {
        // Check when clause
        if let Some(when_clause) = &step.when {
            evaluate_condition(when_clause, context)
        } else {
            Ok(true)
        }
    }

    /// Validate step requirements
    pub fn validate_step_requirements(
        step: &WorkflowStep,
        available_commands: &HashSet<String>,
    ) -> Result<()> {
        // Check command availability
        // Validate timeout ranges
        // Check capture output configuration
        Ok(())
    }

    /// Validate commit requirements for a step
    pub fn validate_commit_requirement(
        step: &WorkflowStep,
        head_before: &str,
        head_after: &str,
    ) -> Result<()> {
        if step.commit_required && head_before == head_after {
            return Err(anyhow!("Step requires commit but no changes detected"));
        }
        Ok(())
    }
}

// workflow/interpolation_helpers.rs (new ~200 lines)
pub mod interpolation_helpers {
    use regex::Regex;
    use once_cell::sync::Lazy;

    static BRACED_VAR_REGEX: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"\$\{([^}]+)\}").expect("Invalid regex"));

    static UNBRACED_VAR_REGEX: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"\$([A-Za-z_][A-Za-z0-9_]*)").expect("Invalid regex"));

    /// Extract variable names from template
    pub fn extract_variable_names(template: &str) -> Vec<String> {
        let mut vars = Vec::new();

        for cap in BRACED_VAR_REGEX.captures_iter(template) {
            vars.push(cap[1].to_string());
        }

        for cap in UNBRACED_VAR_REGEX.captures_iter(template) {
            vars.push(cap[1].to_string());
        }

        vars
    }

    /// Build interpolation context from various sources
    pub fn build_interpolation_context(
        env_vars: &HashMap<String, String>,
        iteration_vars: &HashMap<String, String>,
        captured_outputs: &HashMap<String, String>,
        git_context: &HashMap<String, String>,
    ) -> InterpolationContext {
        InterpolationContext {
            environment: env_vars.clone(),
            iteration: iteration_vars.clone(),
            captured: captured_outputs.clone(),
            git: git_context.clone(),
        }
    }

    /// Format variable value for display (with masking)
    pub fn format_variable_value(
        value: &str,
        sensitive_patterns: &[Regex],
        max_length: usize,
    ) -> String {
        let masked = mask_sensitive_data(value, sensitive_patterns);
        truncate_with_ellipsis(&masked, max_length)
    }

    fn mask_sensitive_data(value: &str, patterns: &[Regex]) -> String {
        let mut result = value.to_string();
        for pattern in patterns {
            result = pattern.replace_all(&result, "***").to_string();
        }
        result
    }
}

// workflow/step_builder.rs (new ~300 lines)
pub mod step_builder {
    use super::types::*;

    /// Convert normalized step to workflow step
    pub fn build_workflow_step(
        normalized: &NormalizedStep,
        index: usize,
        defaults: &StepDefaults,
    ) -> WorkflowStep {
        WorkflowStep {
            command_type: determine_command_type(&normalized.command),
            timeout: normalized.timeout.or(defaults.timeout),
            capture_output: normalized.capture_output.unwrap_or(defaults.capture_output),
            when: normalized.when.clone(),
            commit_required: normalized.commit_required.unwrap_or(false),
            on_failure: build_on_failure_config(normalized.on_failure.as_ref()),
            on_success: normalized.on_success.as_ref().map(|s| Box::new(build_workflow_step(s, 0, defaults))),
            allow_failure: normalized.allow_failure.unwrap_or(false),
        }
    }

    /// Determine command type from command string
    pub fn determine_command_type(command: &str) -> CommandType {
        if command.starts_with("claude:") {
            CommandType::Claude(command[7..].trim().to_string())
        } else if command.starts_with("shell:") {
            CommandType::Shell(command[6..].trim().to_string())
        } else if command.starts_with("test:") {
            CommandType::Test(command[5..].trim().to_string())
        } else {
            CommandType::Legacy(command.to_string())
        }
    }

    /// Build error handler configuration
    pub fn build_on_failure_config(on_failure: Option<&OnFailureSpec>) -> Option<OnFailureConfig> {
        on_failure.map(|spec| {
            OnFailureConfig {
                strategy: determine_strategy(&spec.strategy),
                max_retries: spec.max_retries.unwrap_or(3),
                handler: spec.handler.as_ref().map(|h| build_handler_step(h)),
            }
        })
    }

    /// Get display name for a step
    pub fn get_step_display_name(step: &WorkflowStep) -> String {
        match &step.command_type {
            CommandType::Claude(cmd) => format!("claude: {}", cmd),
            CommandType::Shell(cmd) => format!("shell: {}", truncate_command(cmd, 50)),
            CommandType::Test(cmd) => format!("test: {}", cmd),
            CommandType::GoalSeek(config) => format!("goal_seek: {}", config.description),
            CommandType::Foreach(config) => format!("foreach: {} items", config.items.len()),
            CommandType::Handler { description, .. } => description.clone(),
            CommandType::Legacy(cmd) => cmd.clone(),
        }
    }
}

// workflow/formatters.rs (new ~150 lines)
pub mod formatters {
    use std::time::Duration;

    /// Format duration for display
    pub fn format_duration(duration: Duration) -> String {
        let secs = duration.as_secs();
        if secs < 60 {
            format!("{}s", secs)
        } else if secs < 3600 {
            format!("{}m {}s", secs / 60, secs % 60)
        } else {
            format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
        }
    }

    /// Format error message with context
    pub fn format_step_error(
        step_name: &str,
        error: &str,
        stderr: &str,
        exit_code: Option<i32>,
    ) -> String {
        let mut msg = format!("Step '{}' failed", step_name);

        if let Some(code) = exit_code {
            msg.push_str(&format!(" with exit code {}", code));
        }

        if !error.is_empty() {
            msg.push_str(&format!(": {}", error));
        }

        if !stderr.is_empty() {
            msg.push_str(&format!("\nStderr: {}", truncate_output(stderr, 500)));
        }

        msg
    }

    /// Truncate output for display
    pub fn truncate_output(output: &str, max_lines: usize) -> String {
        let lines: Vec<&str> = output.lines().collect();
        if lines.len() <= max_lines {
            output.to_string()
        } else {
            let truncated: Vec<&str> = lines.iter().take(max_lines).copied().collect();
            format!("{}\n... ({} more lines)", truncated.join("\n"), lines.len() - max_lines)
        }
    }

    /// Format environment variable for safe logging
    pub fn format_env_var_for_logging(key: &str, value: &str) -> String {
        // Mask sensitive environment variables
        if is_sensitive_var(key) {
            format!("{}=***", key)
        } else if value.len() > 100 {
            format!("{}={}...", key, &value[..50])
        } else {
            format!("{}={}", key, value)
        }
    }

    fn is_sensitive_var(key: &str) -> bool {
        let key_upper = key.to_uppercase();
        key_upper.contains("TOKEN") ||
        key_upper.contains("SECRET") ||
        key_upper.contains("PASSWORD") ||
        key_upper.contains("KEY") ||
        key_upper.contains("CREDENTIAL")
    }
}
```

### Migration Strategy

1. **Extract functions incrementally**:
```rust
// In executor.rs - before
impl WorkflowExecutor {
    fn validate_workflow_config(workflow: &ExtendedWorkflowConfig) -> Result<()> {
        // ... 50 lines of validation logic
    }
}

// In executor.rs - after
impl WorkflowExecutor {
    fn validate_workflow_config(workflow: &ExtendedWorkflowConfig) -> Result<()> {
        // Delegate to pure function
        validation_engine::validate_workflow_config(workflow)
    }
}
```

2. **Update call sites gradually**:
```rust
// Can update internal calls to use the module directly
let is_valid = validation_engine::should_execute_step(&step, &context, self.dry_run)?;

// Public API remains unchanged
self.validate_workflow_config(workflow)?;
```

## Implementation Steps

1. Create `workflow/validation_engine.rs` with validation functions
2. Create `workflow/interpolation_helpers.rs` with string processing
3. Create `workflow/step_builder.rs` with step construction logic
4. Create `workflow/formatters.rs` with display formatting
5. Update executor.rs to delegate to these modules
6. Add comprehensive tests for each module

## Testing Strategy

- Unit tests for each pure function with property-based testing
- Test edge cases and error conditions
- Benchmark key functions to ensure performance
- Integration tests remain unchanged

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| Breaking existing functionality | Keep wrapper methods, extensive testing |
| Circular dependencies | Pure functions have no dependencies on executor |
| Performance regression | Benchmark before and after |
| Code duplication | Share common types through workflow/types.rs |

## Success Metrics

- executor.rs reduced to under 4,000 lines
- Each pure function has at least 3 unit tests
- Zero test failures
- Performance within 2% of baseline
- Improved code coverage (target: 90%+)

## Documentation Requirements

- Document each module's purpose
- Provide examples for common use cases
- Document any behavioral differences from original code