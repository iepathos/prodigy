# Analyze Documentation Section Drift

Analyze a specific section of the workflow syntax documentation for drift against the actual codebase implementation.

## Context

You are comparing a section of `docs/workflow-syntax.md` against the actual Prodigy codebase to identify drift (discrepancies between documentation and implementation).

## Input Variables

The workflow provides these variables:
- `${item.id}` - Section identifier (e.g., "command-types", "variable-interpolation")
- `${item.title}` - Section title (e.g., "Command Types")
- `${item.file}` - Documentation file path
- `${item.start_marker}` - Start marker for section extraction
- `${item.end_marker}` - End marker for section extraction
- `${item.validation}` - Validation focus for this section

## Analysis Steps

### 1. Extract Current Documentation

Read the documentation section from `${item.file}`:
- Find content between `${item.start_marker}` and `${item.end_marker}`
- Parse YAML examples and field descriptions
- Note any version compatibility statements

### 2. Load Feature Analysis

Read the comprehensive feature analysis:
- File: `.prodigy/syntax-analysis/features.json`
- This contains the ground truth from the codebase

### 3. Compare and Identify Drift

Based on the section ID, perform specific drift checks:

#### For "command-types":
- Compare documented command types against `features.json.command_types`
- Check each command type has all fields from the struct definition
- Verify YAML examples use correct field names and types
- Check deprecated fields are marked as deprecated
- Ensure no new fields are missing

#### For "variable-interpolation":
- Compare documented variables against `features.json.variable_types`
- Check all standard, mapreduce, git_context, validation, merge variables
- Verify variable syntax examples (${} format)
- Ensure all variable contexts are explained

#### For "mapreduce-workflows":
- Compare structure against `features.json.mapreduce_config`
- Check setup phase fields match SetupPhaseConfig
- Check map phase fields match MapPhaseYaml
- Check reduce phase formats (simple_array and nested_commands)
- Verify agent_template syntax (both formats)
- Check merge workflow configuration

#### For "error-handling":
- Compare against `features.json.error_handling`
- Check workflow-level error_policy fields
- Check command-level on_failure configuration
- Verify all on_item_failure options documented
- Check error_collection strategies

#### For "validation-commands":
- Compare against `features.json.validation_features`
- Check ValidationConfig fields (shell, claude, commands, threshold, etc.)
- Check OnIncompleteConfig fields
- Verify array format support is documented
- Check result_file usage

#### For "environment-config":
- Check environment variable syntax
- Check secrets management
- Check profiles configuration
- Verify dynamic and conditional variables

### 4. Categorize Issues

For each drift issue found, categorize by type:

**Missing Feature** (High severity):
- Feature exists in code but not documented
- New field added to struct but not in docs
- New command type not documented

**Outdated Syntax** (High severity):
- Documentation shows syntax that no longer works
- Field names changed but docs show old names
- Required fields now optional or vice versa

**Incorrect Example** (Medium severity):
- YAML example won't parse with current code
- Example uses wrong field types
- Example missing required fields

**Missing Field** (Medium severity):
- Documented command type missing some fields
- Field exists in struct but not in field list
- Optional field not mentioned

**Deprecated Not Marked** (Low severity):
- Deprecated feature shown without warning
- No replacement suggested
- Missing migration guide

**Incomplete Description** (Low severity):
- Field described but missing type information
- Missing default value
- Missing "required" vs "optional" indication

### 5. Determine Severity

Overall section severity:
- **Critical**: Multiple high severity issues, docs will cause errors
- **High**: Several missing features or outdated syntax
- **Medium**: Incorrect examples or missing fields
- **Low**: Minor issues, incomplete descriptions
- **None**: No drift detected

## Output Format

Create drift report at `.prodigy/syntax-analysis/drift-${item.id}.json`:

```json
{
  "section_id": "${item.id}",
  "section_title": "${item.title}",
  "drift_detected": true,
  "severity": "high",
  "issues": [
    {
      "type": "missing_feature",
      "severity": "high",
      "description": "goal_seek command type is implemented but not documented",
      "current_doc": "Command types: shell, claude, foreach",
      "should_be": "Command types: shell, claude, goal_seek, foreach",
      "fix_suggestion": "Add section documenting goal_seek command with all fields from GoalSeekConfig struct",
      "source_reference": "src/cook/goal_seek/mod.rs:GoalSeekConfig"
    },
    {
      "type": "missing_field",
      "severity": "medium",
      "description": "WorkflowStep.capture_streams field not documented",
      "current_doc": "Shell command fields: shell, timeout, capture",
      "should_be": "Shell command fields: shell, timeout, capture, capture_streams",
      "fix_suggestion": "Add capture_streams field to shell command documentation with CaptureStreams struct fields",
      "source_reference": "src/cook/workflow/executor.rs:WorkflowStep.capture_streams"
    },
    {
      "type": "outdated_syntax",
      "severity": "high",
      "description": "Documentation shows deprecated 'test:' command syntax",
      "current_doc": "test:\n  command: cargo test\n  on_failure: ...",
      "should_be": "shell: cargo test\non_failure: ...",
      "fix_suggestion": "Replace test: syntax with shell: and add deprecation notice",
      "source_reference": "src/config/command.rs:WorkflowStepCommand (deprecated test field)"
    }
  ],
  "analysis_summary": "Found 3 drift issues: 1 missing feature, 1 missing field, 1 outdated syntax. Documentation needs updates to reflect current struct definitions and remove deprecated syntax.",
  "metadata": {
    "analyzed_at": "2025-01-XX",
    "feature_analysis_file": ".prodigy/syntax-analysis/features.json",
    "validation_focus": "${item.validation}"
  }
}
```

## Analysis Guidelines

### Be Thorough
- Read actual Rust struct definitions, don't guess
- Compare field by field
- Check serde attributes for serialization behavior
- Look for `#[serde(skip_serializing_if = "Option::is_none")]` - these are optional
- Look for `#[serde(default)]` - these have defaults

### Check Examples
- Parse YAML examples in documentation
- Verify they match current struct definitions
- Ensure required fields are present
- Check field types match (string vs number vs boolean)

### Identify Patterns
- Look for untagged enum support (multiple formats)
- Check for array format vs object format support
- Identify backward compatibility patterns
- Note deprecated features with warnings in code

### Source References
- Always include source file path and struct/field name
- Help the fix command find the exact implementation
- Reference line numbers if helpful

### Accuracy
- Only report actual drift, not style preferences
- Compare against features.json as ground truth
- Don't suggest changes that aren't in the code
- Flag truly missing features vs. just poorly documented

## Success Criteria

The drift report should:
1. Accurately identify ALL drift between docs and code
2. Categorize each issue by type and severity
3. Provide clear fix suggestions with source references
4. Include enough detail for automated fixes
5. Set appropriate section severity level
6. Be actionable - fixable based on the information provided
