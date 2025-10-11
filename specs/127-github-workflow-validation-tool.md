---
number: 127
title: GitHub Workflow Validation Tool
category: testing
priority: high
status: draft
dependencies: [126]
created: 2025-10-11
---

# Specification 127: GitHub Workflow Validation Tool

**Category**: testing
**Priority**: high
**Status**: draft
**Dependencies**: [126 - GitHub Workflow Template System]

## Context

Even with standardized workflow templates (Spec 126), developers can still introduce inconsistencies when customizing workflows or creating new ones. Manual verification against checklists is error-prone and often skipped during rapid development.

The WORKFLOW_SETUP_ISSUES.md document identified several categories of issues that could be detected automatically:
- Incorrect workflow filenames (e.g., `docs.yml` vs `deploy-docs.yml`)
- Wrong GitHub Actions (e.g., `actions/deploy-pages` vs `peaceiris/actions-gh-pages`)
- Missing path filters leading to wasted CI resources
- Incorrect permissions (e.g., `pages: write` vs `contents: write`)
- Missing pull request validation triggers

These issues are mechanical and can be detected through static analysis of workflow YAML files.

## Objective

Create an automated validation tool integrated into prodigy's CI/CD pipeline that checks GitHub Actions workflow files against established standards and best practices. The tool should provide clear, actionable feedback when violations are detected and prevent non-compliant workflows from being merged.

## Requirements

### Functional Requirements

- **Static Analysis**: Parse and validate YAML workflow files in `.github/workflows/`
- **Rule Engine**: Support configurable rules for different workflow types
- **Standard Validation**: Enforce standards from Spec 126 templates
- **Naming Conventions**: Validate workflow filenames match conventions
- **Action Validation**: Verify correct GitHub Actions are used for specific tasks
- **Path Filter Validation**: Ensure appropriate path filters exist
- **Permission Validation**: Check that correct permissions are granted
- **Trigger Validation**: Verify both push and PR triggers where appropriate
- **Reporting**: Generate clear, actionable error messages with line numbers
- **CI Integration**: Run automatically on workflow file changes in PRs

### Non-Functional Requirements

- **Performance**: Validation should complete in < 5 seconds for typical repositories
- **Accuracy**: Zero false positives for workflows following templates
- **Extensibility**: Easy to add new validation rules
- **Usability**: Error messages should guide developers to correct solutions
- **Portability**: Works in GitHub Actions, locally, and in prodigy workflows

## Acceptance Criteria

- [ ] Command-line tool `prodigy validate-workflows` implemented
- [ ] Validates workflow filenames against naming conventions
- [ ] Detects incorrect GitHub Actions for documentation deployment
- [ ] Identifies missing or incorrect path filters
- [ ] Checks for proper permissions configuration
- [ ] Verifies presence of PR validation triggers
- [ ] Provides line numbers and context for violations
- [ ] Exit code 0 for valid workflows, non-zero for violations
- [ ] Configuration file support (`.prodigy/workflow-validation.yml`)
- [ ] GitHub Action workflow created for automated validation
- [ ] Successfully detects all issues documented in WORKFLOW_SETUP_ISSUES.md
- [ ] Documentation includes guide for adding custom validation rules
- [ ] Local pre-commit hook example provided

## Technical Details

### Implementation Approach

**Phase 1: Core Validation Engine**
1. Implement YAML parser using `serde_yaml`
2. Create rule engine supporting pluggable validators
3. Implement standard rules for documentation workflows
4. Add reporting mechanism with rich error messages

**Phase 2: CLI Tool**
1. Add `validate-workflows` subcommand to prodigy CLI
2. Support recursive directory scanning
3. Implement configuration file loading
4. Add verbose output mode for debugging

**Phase 3: CI Integration**
1. Create GitHub Action workflow for validation
2. Add pre-commit hook example
3. Document integration with existing CI/CD pipelines

### Architecture

```rust
// Core validation types
pub struct WorkflowValidator {
    rules: Vec<Box<dyn ValidationRule>>,
    config: ValidationConfig,
}

pub trait ValidationRule {
    fn name(&self) -> &str;
    fn validate(&self, workflow: &WorkflowFile) -> Vec<Violation>;
    fn severity(&self) -> Severity;
}

pub struct WorkflowFile {
    path: PathBuf,
    content: serde_yaml::Value,
    raw_content: String,
}

pub struct Violation {
    rule: String,
    severity: Severity,
    message: String,
    line: Option<usize>,
    suggestion: Option<String>,
}

pub enum Severity {
    Error,      // Must be fixed
    Warning,    // Should be fixed
    Info,       // Nice to have
}
```

### Standard Validation Rules

**Rule: DocumentationWorkflowNaming**
```rust
// Validates: Documentation workflows must be named deploy-docs.yml
impl ValidationRule for DocumentationWorkflowNaming {
    fn validate(&self, workflow: &WorkflowFile) -> Vec<Violation> {
        let is_docs_workflow = self.is_documentation_workflow(workflow);
        let filename = workflow.path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        if is_docs_workflow && filename != "deploy-docs.yml" {
            vec![Violation {
                rule: "documentation-workflow-naming".into(),
                severity: Severity::Error,
                message: format!(
                    "Documentation workflow should be named 'deploy-docs.yml', found '{}'",
                    filename
                ),
                line: None,
                suggestion: Some("Rename to deploy-docs.yml".into()),
            }]
        } else {
            vec![]
        }
    }
}
```

**Rule: DocumentationDeploymentAction**
```rust
// Validates: Must use peaceiris/actions-gh-pages for deployment
impl ValidationRule for DocumentationDeploymentAction {
    fn validate(&self, workflow: &WorkflowFile) -> Vec<Violation> {
        let uses_wrong_action = self.uses_action(
            workflow,
            &["actions/deploy-pages", "actions/upload-pages-artifact"]
        );

        if uses_wrong_action && !self.uses_action(workflow, &["peaceiris/actions-gh-pages"]) {
            vec![Violation {
                rule: "documentation-deployment-action".into(),
                severity: Severity::Error,
                message: "Use peaceiris/actions-gh-pages@v4 for documentation deployment".into(),
                line: self.find_action_line(workflow, "actions/deploy-pages"),
                suggestion: Some(
                    "Replace with: uses: peaceiris/actions-gh-pages@v4".into()
                ),
            }]
        } else {
            vec![]
        }
    }
}
```

**Rule: PathFiltersRequired**
```rust
// Validates: Workflows should have path filters to avoid unnecessary runs
impl ValidationRule for PathFiltersRequired {
    fn validate(&self, workflow: &WorkflowFile) -> Vec<Violation> {
        let has_push_trigger = workflow.content["on"]["push"].is_mapping();
        let has_path_filter = workflow.content["on"]["push"]["paths"].is_sequence();

        if has_push_trigger && !has_path_filter {
            vec![Violation {
                rule: "path-filters-required".into(),
                severity: Severity::Warning,
                message: "Push trigger should include path filters to avoid unnecessary runs".into(),
                line: self.find_trigger_line(workflow, "push"),
                suggestion: Some(r#"Add:
    paths:
      - 'book/**'
      - '.github/workflows/deploy-docs.yml'"#.into()),
            }]
        } else {
            vec![]
        }
    }
}
```

**Rule: PullRequestValidation**
```rust
// Validates: Workflows should validate PRs, not just pushes
impl ValidationRule for PullRequestValidation {
    fn validate(&self, workflow: &WorkflowFile) -> Vec<Violation> {
        let has_push = workflow.content["on"]["push"].is_mapping();
        let has_pr = workflow.content["on"]["pull_request"].is_mapping();

        if has_push && !has_pr && self.should_validate_prs(workflow) {
            vec![Violation {
                rule: "pull-request-validation".into(),
                severity: Severity::Warning,
                message: "Workflow should validate pull requests, not just pushes".into(),
                line: self.find_on_line(workflow),
                suggestion: Some(r#"Add pull_request trigger:
  pull_request:
    branches: [main, master]
    paths:
      - 'book/**'"#.into()),
            }]
        } else {
            vec![]
        }
    }
}
```

**Rule: CorrectPermissions**
```rust
// Validates: Documentation workflows need contents: write, not pages: write
impl ValidationRule for CorrectPermissions {
    fn validate(&self, workflow: &WorkflowFile) -> Vec<Violation> {
        let is_docs_workflow = self.is_documentation_workflow(workflow);
        let perms = &workflow.content["permissions"];

        if is_docs_workflow && perms["pages"].as_str() == Some("write") {
            vec![Violation {
                rule: "correct-permissions".into(),
                severity: Severity::Error,
                message: "Documentation deployment needs 'contents: write', not 'pages: write'".into(),
                line: self.find_permissions_line(workflow),
                suggestion: Some("Change to: contents: write".into()),
            }]
        } else {
            vec![]
        }
    }
}
```

### Configuration File Format

```yaml
# .prodigy/workflow-validation.yml
version: 1

# Global settings
settings:
  fail_on_warnings: false
  exclude_paths:
    - '.github/workflows/deprecated/**'

# Rule configuration
rules:
  documentation-workflow-naming:
    enabled: true
    severity: error

  documentation-deployment-action:
    enabled: true
    severity: error
    allowed_actions:
      - peaceiris/actions-gh-pages@v4

  path-filters-required:
    enabled: true
    severity: warning
    exceptions:
      - workflow_dispatch  # Manual triggers don't need path filters

  pull-request-validation:
    enabled: true
    severity: warning

  correct-permissions:
    enabled: true
    severity: error

# Custom rules (future enhancement)
custom_rules:
  - name: require-timeout
    description: All jobs should specify a timeout
    pattern: 'jobs.*.timeout-minutes'
    severity: warning
```

### CLI Interface

```bash
# Validate all workflows in current repository
prodigy validate-workflows

# Validate specific workflow file
prodigy validate-workflows .github/workflows/deploy-docs.yml

# Validate with custom config
prodigy validate-workflows --config .prodigy/custom-validation.yml

# Verbose output showing all checks
prodigy validate-workflows --verbose

# Show available rules
prodigy validate-workflows --list-rules

# Validate and auto-fix where possible
prodigy validate-workflows --fix

# Output format options
prodigy validate-workflows --format json
prodigy validate-workflows --format github-actions  # For CI annotations
```

### Output Format Examples

**Console Output (Default)**
```
Validating GitHub Actions workflows...

✗ .github/workflows/docs.yml
  Error [documentation-workflow-naming]
  Documentation workflow should be named 'deploy-docs.yml', found 'docs.yml'
  Suggestion: Rename to deploy-docs.yml

  Error [documentation-deployment-action] Line 23
  Use peaceiris/actions-gh-pages@v4 for documentation deployment
  Found: uses: actions/deploy-pages@v4
  Suggestion: Replace with: uses: peaceiris/actions-gh-pages@v4

  Warning [path-filters-required] Line 5
  Push trigger should include path filters to avoid unnecessary runs
  Suggestion: Add:
    paths:
      - 'book/**'
      - '.github/workflows/deploy-docs.yml'

✓ .github/workflows/ci.yml

Summary:
  2 files checked
  1 file with violations
  2 errors, 1 warning
```

**GitHub Actions Output**
```
::error file=.github/workflows/docs.yml,line=23::Use peaceiris/actions-gh-pages@v4 for documentation deployment
::warning file=.github/workflows/docs.yml,line=5::Push trigger should include path filters
```

**JSON Output**
```json
{
  "version": "1.0",
  "timestamp": "2025-10-11T10:30:00Z",
  "summary": {
    "files_checked": 2,
    "files_with_violations": 1,
    "errors": 2,
    "warnings": 1,
    "infos": 0
  },
  "violations": [
    {
      "file": ".github/workflows/docs.yml",
      "rule": "documentation-deployment-action",
      "severity": "error",
      "line": 23,
      "message": "Use peaceiris/actions-gh-pages@v4 for documentation deployment",
      "suggestion": "Replace with: uses: peaceiris/actions-gh-pages@v4"
    }
  ]
}
```

## Dependencies

### Prerequisites
- **Spec 126**: Workflow template system must be defined first
- Established naming conventions and best practices

### Affected Components
- Prodigy CLI (`src/cli/commands/validate_workflows.rs`)
- New validation module (`src/validation/`)
- CI/CD workflows in prodigy repository
- Documentation (add section on workflow validation)

### External Dependencies
- `serde_yaml` - YAML parsing
- `anyhow` - Error handling
- `clap` - CLI argument parsing
- `colored` - Terminal output formatting
- `serde_json` - JSON output format

## Testing Strategy

### Unit Tests
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_documentation_workflow_naming() {
        let rule = DocumentationWorkflowNaming::new();
        let workflow = load_test_workflow("tests/fixtures/docs.yml");
        let violations = rule.validate(&workflow);

        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].rule, "documentation-workflow-naming");
    }

    #[test]
    fn test_valid_workflow_no_violations() {
        let validator = WorkflowValidator::default();
        let workflow = load_test_workflow("tests/fixtures/deploy-docs.yml");
        let violations = validator.validate(&workflow);

        assert_eq!(violations.len(), 0);
    }

    #[test]
    fn test_path_filters_detection() {
        let rule = PathFiltersRequired::new();
        let workflow = load_test_workflow("tests/fixtures/no-filters.yml");
        let violations = rule.validate(&workflow);

        assert!(violations.iter().any(|v| v.rule == "path-filters-required"));
    }
}
```

### Integration Tests
1. Create test repository with various workflow files
2. Run validation on known-good workflows (expect zero violations)
3. Run validation on workflows with known issues (expect specific violations)
4. Test CLI with different output formats
5. Verify GitHub Actions integration produces proper annotations

### Regression Tests
- Use WORKFLOW_SETUP_ISSUES.md examples as test cases
- Ensure all documented issues are detected
- Verify suggestions lead to valid workflows

## Documentation Requirements

### Code Documentation
- Document each validation rule with examples
- Explain rule severity levels and when to use each
- Document configuration file format
- Provide examples of custom rule creation

### User Documentation

Add new section: **"Validating GitHub Workflows"**
- Overview of workflow validation
- Running validation locally
- Interpreting validation results
- Configuring validation rules
- Adding custom rules
- CI/CD integration guide
- Troubleshooting common validation failures

### Architecture Updates
Update `ARCHITECTURE.md` to document:
- Validation engine architecture
- Rule plugin system
- Configuration loading mechanism
- Integration points with prodigy CLI

## Implementation Notes

### Line Number Detection

Accurately reporting line numbers requires preserving YAML structure:

```rust
use serde_yaml::Value;

fn find_line_number(raw_yaml: &str, path: &[&str]) -> Option<usize> {
    // Parse YAML preserving line numbers
    let lines: Vec<&str> = raw_yaml.lines().collect();

    // Search for path in YAML structure
    let mut current_indent = 0;
    for (i, line) in lines.iter().enumerate() {
        if matches_yaml_path(line, path, current_indent) {
            return Some(i + 1);
        }
    }
    None
}
```

### False Positive Prevention

Avoid flagging intentional deviations:
- Support `# prodigy-validation: disable-next-line` comments
- Allow rule-specific disables: `# prodigy-validation: disable documentation-workflow-naming`
- Support file-level disables in frontmatter

### Performance Optimization

- Cache parsed YAML to avoid re-parsing for each rule
- Run rules in parallel when possible
- Support incremental validation (only changed files)
- Pre-compile regex patterns used in rules

## Migration and Compatibility

### Breaking Changes
None - this is a new feature

### Gradual Adoption Path

**Phase 1: Warning Mode**
- Deploy validation tool in warning-only mode
- Allow all workflows to pass CI
- Developers see warnings but can proceed
- Collect feedback and refine rules

**Phase 2: Error Mode for New Workflows**
- Enforce validation for new workflow files
- Existing workflows grandfathered with warnings
- Gradually migrate existing workflows

**Phase 3: Full Enforcement**
- All workflows must pass validation
- CI fails on validation errors
- Warnings converted to errors

### Compatibility Considerations
- Support both GitHub.com and GitHub Enterprise Server
- Handle different YAML parsers' quirks
- Support workflows using reusable workflows from Spec 126
- Don't break workflows using composite actions

## Success Metrics

- **Error Detection**: 100% of issues from WORKFLOW_SETUP_ISSUES.md detected
- **Adoption**: Validation enabled in CI for all prodigy ecosystem repositories
- **Time Saved**: Reduce workflow debugging time by 80%
- **Consistency**: All new workflows follow standards from Spec 126
- **Developer Experience**: Positive feedback on error messages and suggestions

## Future Enhancements

- Auto-fix capability for common violations
- Integration with IDE/editor (LSP server for workflow YAML)
- Visualization of workflow structure
- Performance analysis of workflows (identify inefficiencies)
- Best practice suggestions beyond validation
- Machine learning to detect anti-patterns
- Integration with GitHub Code Scanning
- Support for validating composite actions and reusable workflows
