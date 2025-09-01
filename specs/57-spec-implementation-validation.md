---
number: 57
title: Spec Implementation Validation System
category: foundation
priority: high
status: draft
dependencies: []
created: 2025-01-01
---

# Specification 57: Spec Implementation Validation System

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: None

## Context

When using LLM-based agents like Claude to implement specifications through workflows, there's no reliable signal to determine if a specification has been fully implemented. LLMs may partially implement complex specifications due to context limits, misunderstanding requirements, or focusing on certain aspects while missing others. Currently, MMM's workflow system can detect and retry on test/lint failures, but cannot detect incomplete implementations that still compile and pass tests.

This creates a gap where specifications may be marked as "complete" when critical requirements are missing, leading to technical debt and requiring manual review to identify gaps. We need a mechanism to validate that specifications have been fully implemented before considering them complete.

## Objective

Add a validation mechanism to MMM workflows that can verify specification completeness after implementation, providing structured feedback about missing requirements and enabling automatic retry or gap-filling operations to achieve full specification coverage.

## Requirements

### Functional Requirements
- Add `validate` configuration option to workflow steps
- Support multiple validation strategies (spec coverage, test generation, self-assessment)
- Capture structured validation output (JSON format with completion percentage and gaps)
- Enable retry logic based on validation results
- Support both gap-filling and full re-implementation strategies
- Integrate with existing `capture_output` mechanism
- Allow custom validation commands per workflow
- Provide detailed gap analysis in validation output
- Support validation of both Claude and shell command outputs

### Non-Functional Requirements
- Validation should add minimal overhead (<5 seconds per check)
- Support async validation for long-running checks
- Maintain backward compatibility with existing workflows
- Clear error messages when validation fails
- Extensible architecture for adding new validation types

## Acceptance Criteria

- [ ] `validate` field added to `WorkflowStep` struct
- [ ] Validation runs after successful command execution
- [ ] JSON schema validation for structured output
- [ ] `on_incomplete` handler triggered when validation fails
- [ ] Retry mechanism re-runs validation after each fix attempt
- [ ] Variables `${validation.gaps}` and `${validation.completion}` available in context
- [ ] Support for multiple validation types (spec_coverage, test_coverage, self_assessment)
- [ ] Integration tests covering validation scenarios
- [ ] Documentation updated with validation examples
- [ ] Example workflows demonstrating validation usage

## Technical Details

### Implementation Approach

1. **Extend WorkflowStep Structure**:
```rust
pub struct WorkflowStep {
    // ... existing fields ...
    
    #[serde(default)]
    pub validate: Option<ValidationConfig>,
}

pub struct ValidationConfig {
    pub validation_type: ValidationType,
    pub command: String,
    pub expected_schema: Option<JsonSchema>,
    pub on_incomplete: Option<OnIncompleteConfig>,
}

pub enum ValidationType {
    SpecCoverage,
    TestCoverage, 
    SelfAssessment,
    Custom(String),
}

pub struct OnIncompleteConfig {
    pub strategy: CompletionStrategy,
    pub command: CommandType,
    pub max_attempts: u32,
    pub fail_workflow: bool,
}

pub enum CompletionStrategy {
    PatchGaps,    // Try to fill missing pieces
    RetryFull,    // Re-run full implementation
    Interactive,  // Ask user for guidance
}
```

2. **Validation Flow**:
```yaml
- claude: "/mmm-implement-spec $ARG"
  commit_required: true
  validate:
    type: "spec_coverage"
    command: "/mmm-validate-spec ${ARG}"
    expected_schema:
      type: "object"
      required: ["completion_percentage", "gaps", "implemented", "missing"]
    on_incomplete:
      strategy: "patch_gaps"
      claude: "/mmm-complete-gaps --spec ${ARG} --gaps ${validation.gaps}"
      max_attempts: 2
      fail_workflow: true
```

3. **Validation Output Format**:
```json
{
  "completion_percentage": 85.5,
  "status": "incomplete",
  "implemented": [
    "API endpoints for user management",
    "Database schema migrations",
    "Basic authentication"
  ],
  "missing": [
    "Role-based access control",
    "Password reset functionality"
  ],
  "gaps": {
    "rbac": {
      "description": "Role-based access control not implemented",
      "location": "src/auth/",
      "severity": "critical"
    },
    "password_reset": {
      "description": "Password reset endpoints missing",
      "location": "src/api/users/",
      "severity": "high"
    }
  }
}
```

### Architecture Changes

1. **Workflow Executor Enhancement**:
   - Add validation step after command execution
   - Implement retry loop for validation failures
   - Capture and parse validation output
   - Update workflow context with validation results

2. **Command Registry Updates**:
   - Add validation commands to registry
   - Support for validation-specific attributes
   - Integration with structured output parsing

3. **Context Management**:
   - Store validation results in workflow context
   - Make validation data available to subsequent commands
   - Persist validation state for resume capability

### Data Structures

```rust
pub struct ValidationResult {
    pub completion_percentage: f64,
    pub status: ValidationStatus,
    pub implemented: Vec<String>,
    pub missing: Vec<String>,
    pub gaps: HashMap<String, GapDetail>,
    pub raw_output: String,
}

pub struct GapDetail {
    pub description: String,
    pub location: Option<String>,
    pub severity: Severity,
    pub suggested_fix: Option<String>,
}

pub enum ValidationStatus {
    Complete,
    Incomplete,
    Failed,
    Skipped,
}
```

### APIs and Interfaces

1. **Validation Command Interface**:
```bash
/mmm-validate-spec <spec_id> [--format json] [--verbose]
```

2. **Gap Completion Interface**:
```bash
/mmm-complete-gaps --spec <spec_id> --gaps <json_gaps> [--strategy patch|full]
```

3. **Workflow Configuration**:
```yaml
validate:
  type: "spec_coverage|test_coverage|self_assessment|custom"
  command: "<validation_command>"
  threshold: 95  # Optional completion threshold (default: 100)
  timeout: 30    # Optional timeout in seconds
  on_incomplete:
    strategy: "patch_gaps|retry_full|interactive"
    command: "<remediation_command>"
    max_attempts: 3
    fail_workflow: true
```

## Dependencies

- **Prerequisites**: None
- **Affected Components**: 
  - `src/cook/workflow/executor.rs` - Main validation logic
  - `src/config/command.rs` - Validation configuration structures
  - `src/cook/execution/mod.rs` - Integration with execution flow
- **External Dependencies**: 
  - `serde_json` for JSON parsing
  - `jsonschema` for schema validation (optional)

## Testing Strategy

- **Unit Tests**: 
  - Validation result parsing
  - Retry logic with different strategies
  - Context variable interpolation
  - Schema validation

- **Integration Tests**:
  - Full workflow with validation
  - Multiple validation attempts
  - Gap-filling scenarios
  - Validation timeout handling

- **Performance Tests**:
  - Validation overhead measurement
  - Large validation output handling
  - Concurrent validation execution

- **User Acceptance**:
  - Example workflows with different validation types
  - Clear error messages on validation failure
  - Documentation with common patterns

## Documentation Requirements

- **Code Documentation**:
  - Document all new validation structures
  - Examples in validation command implementations
  - Clear comments on retry logic

- **User Documentation**:
  - Add validation section to workflow documentation
  - Provide example workflows for common scenarios
  - Document validation output format
  - Best practices for writing validation commands

- **Architecture Updates**:
  - Update workflow execution flow diagram
  - Document validation integration points
  - Add sequence diagrams for retry logic

## Implementation Notes

1. **Backward Compatibility**: Validation is optional - existing workflows continue to work without modification.

2. **Incremental Adoption**: Start with simple validation types, add more sophisticated strategies over time.

3. **Error Handling**: Validation failures should be clearly distinguished from execution failures.

4. **Performance**: Cache validation results to avoid redundant checks during retries.

5. **Extensibility**: Design validation system to easily add new validation types without core changes.

6. **Debugging**: Provide verbose mode to see detailed validation process for troubleshooting.

## Migration and Compatibility

- No breaking changes to existing workflows
- Validation is opt-in per workflow step
- Existing `on_failure` handlers continue to work
- Can combine validation with existing error handling
- Migration guide for adding validation to existing workflows

## Example Workflows

### Basic Spec Validation
```yaml
- claude: "/mmm-implement-spec $ARG"
  validate:
    type: "spec_coverage"
    command: "/mmm-validate-spec ${ARG}"
    on_incomplete:
      claude: "/mmm-complete-spec --spec ${ARG} --gaps ${validation.gaps}"
      max_attempts: 2
```

### Test-Driven Validation
```yaml
- claude: "/mmm-generate-tests $ARG"
  capture_output: "test_list"
  
- claude: "/mmm-implement-spec $ARG"
  validate:
    type: "test_coverage"
    command: "cargo test ${test_list}"
    on_incomplete:
      strategy: "retry_full"
      claude: "/mmm-implement-spec $ARG --focus ${validation.missing}"
      max_attempts: 3
```

### Self-Assessment with User Confirmation
```yaml
- claude: "/mmm-implement-feature $ARG"
  validate:
    type: "self_assessment"
    command: "/mmm-check-completeness ${ARG}"
    threshold: 90
    on_incomplete:
      strategy: "interactive"
      prompt: "Implementation ${validation.completion}% complete. Continue?"
```