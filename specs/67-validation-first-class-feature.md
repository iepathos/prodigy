---
number: 67
title: Validation as First-Class Workflow Feature
category: foundation
priority: high
status: draft
dependencies: [66]
created: 2025-01-14
---

# Specification 67: Validation as First-Class Workflow Feature

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: [66 - Variable Capture]

## Context

The whitepaper consistently shows `validate:` as a primary workflow feature:
```yaml
tasks:
  - name: "Modernize Python code"
    claude: "/modernize-python user.py"
    validate: "python -m py_compile user.py"

  - name: "Convert to TypeScript"
    claude: "/convert-to-typescript ${item}"
    validate: "tsc --noEmit ${item}"
```

Currently, validation is treated as a secondary concern or conflated with testing. The whitepaper clearly shows validation as a first-class feature for verifying command success beyond just exit codes.

## Objective

Implement validation as a first-class workflow feature that provides structured success criteria, enabling workflows to verify outcomes through custom validation commands and determine true success beyond simple exit codes.

## Requirements

### Functional Requirements
- Support `validate:` field on any workflow step
- Execute validation after main command succeeds
- Validation failure marks step as failed
- Support both shell and Claude validation commands
- Enable multiple validation commands per step
- Capture validation output for debugging
- Support validation-specific timeouts
- Enable conditional validation based on context
- Integration with retry logic (retry on validation failure)
- Validation metrics and reporting

### Non-Functional Requirements
- Clear separation between command and validation
- Minimal overhead when validation not specified
- Detailed validation failure messages
- Consistent behavior across all command types

## Acceptance Criteria

- [ ] `validate: "command"` executes after main command
- [ ] Step fails if validation fails (even if command succeeds)
- [ ] Multiple validations: `validate: ["test1", "test2"]`
- [ ] Shell validation: `validate: "npm test"`
- [ ] Claude validation: `validate: "claude: /check-quality"`
- [ ] Validation output captured in logs
- [ ] Retry triggered by validation failure
- [ ] `skip_validation: true` bypasses validation
- [ ] Validation metrics in workflow summary
- [ ] MapReduce items validated individually

## Technical Details

### Implementation Approach

1. **Enhanced Workflow Step with Validation**:
   ```rust
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct WorkflowStep {
       #[serde(flatten)]
       pub command: CommandType,

       /// Validation command(s) to verify success
       #[serde(skip_serializing_if = "Option::is_none")]
       pub validate: Option<ValidationSpec>,

       /// Skip validation even if specified
       #[serde(default)]
       pub skip_validation: bool,

       /// Validation-specific timeout
       #[serde(skip_serializing_if = "Option::is_none")]
       pub validation_timeout: Option<Duration>,

       /// Continue on validation failure
       #[serde(default)]
       pub ignore_validation_failure: bool,
   }

   #[derive(Debug, Clone, Serialize, Deserialize)]
   #[serde(untagged)]
   pub enum ValidationSpec {
       Single(String),
       Multiple(Vec<String>),
       Detailed(ValidationConfig),
   }

   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct ValidationConfig {
       /// Validation commands
       pub commands: Vec<ValidationCommand>,

       /// Success criteria
       #[serde(default)]
       pub success_criteria: SuccessCriteria,

       /// Maximum validation attempts
       #[serde(default = "default_validation_attempts")]
       pub max_attempts: u32,

       /// Delay between validation attempts
       #[serde(with = "duration_serde")]
       pub retry_delay: Duration,
   }

   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct ValidationCommand {
       #[serde(flatten)]
       pub command: CommandType,

       /// Expected output pattern
       #[serde(skip_serializing_if = "Option::is_none")]
       pub expect_output: Option<String>,

       /// Expected exit code
       #[serde(default)]
       pub expect_exit_code: i32,
   }
   ```

2. **Validation Executor**:
   ```rust
   pub struct ValidationExecutor {
       command_executor: Arc<CommandExecutor>,
       metrics: Arc<ValidationMetrics>,
   }

   impl ValidationExecutor {
       pub async fn validate_step(
           &self,
           step: &WorkflowStep,
           context: &ExecutionContext,
       ) -> Result<ValidationResult> {
           // Skip if validation not needed
           if step.skip_validation || step.validate.is_none() {
               return Ok(ValidationResult::Skipped);
           }

           let validation_spec = step.validate.as_ref().unwrap();
           let start_time = Instant::now();

           // Parse validation spec
           let commands = self.parse_validation_spec(validation_spec)?;

           // Execute validation commands
           let mut all_passed = true;
           let mut results = Vec::new();

           for (idx, cmd) in commands.iter().enumerate() {
               info!("Running validation {}/{}", idx + 1, commands.len());

               let result = self.execute_validation_command(cmd, context).await?;

               if !result.passed {
                   all_passed = false;
                   if !step.ignore_validation_failure {
                       error!("Validation failed: {}", result.message);
                       break;
                   }
               }

               results.push(result);
           }

           // Record metrics
           self.metrics.record_validation(
               &step.name(),
               all_passed,
               start_time.elapsed(),
           );

           Ok(ValidationResult {
               passed: all_passed,
               results,
               duration: start_time.elapsed(),
           })
       }

       async fn execute_validation_command(
           &self,
           cmd: &ValidationCommand,
           context: &ExecutionContext,
       ) -> Result<SingleValidationResult> {
           // Execute the validation command
           let output = match &cmd.command {
               CommandType::Shell(shell_cmd) => {
                   self.command_executor.execute_shell(shell_cmd, context).await?
               }
               CommandType::Claude(claude_cmd) => {
                   self.command_executor.execute_claude(claude_cmd, context).await?
               }
               _ => return Err(anyhow!("Unsupported validation command type")),
           };

           // Check success criteria
           let mut passed = output.exit_code == cmd.expect_exit_code;

           if let Some(expected_output) = &cmd.expect_output {
               if !output.stdout.contains(expected_output) {
                   passed = false;
               }
           }

           Ok(SingleValidationResult {
               passed,
               message: if passed {
                   "Validation passed".to_string()
               } else {
                   format!("Expected exit code {}, got {}",
                          cmd.expect_exit_code, output.exit_code)
               },
               output: output.stdout,
               exit_code: output.exit_code,
           })
       }
   }
   ```

3. **Integration with Step Execution**:
   ```rust
   impl StepExecutor {
       pub async fn execute_with_validation(
           &self,
           step: &WorkflowStep,
           context: &mut ExecutionContext,
       ) -> Result<StepResult> {
           // Execute main command
           let command_result = self.execute_command(step, context).await?;

           // If command failed, skip validation
           if !command_result.success {
               return Ok(command_result);
           }

           // Run validation if specified
           let validation_result = self.validation_executor
               .validate_step(step, context)
               .await?;

           // Combine results
           Ok(StepResult {
               success: command_result.success && validation_result.passed,
               command_result: Some(command_result),
               validation_result: Some(validation_result),
               ..Default::default()
           })
       }
   }
   ```

### Architecture Changes
- Add `ValidationExecutor` to execution pipeline
- Enhance `StepResult` to include validation results
- Integrate validation with retry logic
- Add validation metrics collection
- Update progress tracking for validation phase

### Data Structures
```yaml
# Example workflow with validation
tasks:
  - name: "Refactor module"
    claude: "/refactor complex_module.py"
    validate: "python -m pytest tests/test_complex_module.py"
    retry: 3  # Retry if validation fails

  - name: "Update API"
    shell: "generate-api-client.sh"
    validate:
      - "npm run test:api"
      - "npm run lint:api"
      - "claude: /verify-api-compatibility"

  - name: "Deploy service"
    shell: "./deploy.sh"
    validate:
      commands:
        - shell: "curl -f https://api.example.com/health"
          expect_exit_code: 0
        - shell: "check-deployment.sh"
          expect_output: "Deployment successful"
      max_attempts: 5
      retry_delay: 10s
```

## Dependencies

- **Prerequisites**: [66 - Variable Capture] for validation output handling
- **Affected Components**:
  - `src/cook/workflow/validation.rs` - Core validation logic
  - `src/cook/execution/` - Integration with executors
  - `src/config/workflow.rs` - Validation configuration
- **External Dependencies**: None

## Testing Strategy

- **Unit Tests**:
  - Validation command parsing
  - Success criteria evaluation
  - Validation retry logic
  - Metrics collection
- **Integration Tests**:
  - End-to-end validation flow
  - Multiple validation commands
  - Validation failure handling
  - Integration with retry system
- **Scenario Tests**:
  - Build validation workflows
  - Test suite validation
  - Deployment verification
  - Data integrity checks

## Documentation Requirements

- **Code Documentation**: Document validation execution flow
- **User Documentation**:
  - Validation guide with examples
  - Success criteria configuration
  - Common validation patterns
  - Validation vs testing guidance
- **Architecture Updates**: Add validation to step execution diagram

## Implementation Notes

- Validation should be clearly distinguished from testing
- Consider caching validation results for idempotency
- Support parallel validation for multiple commands
- Enable validation-only dry runs
- Future: ML-based validation for complex scenarios

## Migration and Compatibility

- Workflows without validation work unchanged
- No breaking changes to existing workflows
- Clear migration from ad-hoc validation to structured
- Backwards compatible with current validation attempts