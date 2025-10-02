# Workflow Syntax Documentation Updates

## Summary
- Analyzed: 6 sections
- Found drift: 6 sections
- Total issues fixed: 56
- Severity: 6 high, 25 medium, 25 low

## Sections Updated

### 1. Variable Interpolation (High severity - 9 issues fixed)

**CRITICAL REMOVALS (Outdated Syntax):**
- ✓ Removed non-existent git variables: `step.commit_count`, `step.insertions`, `step.deletions`, `workflow.commit_count`
- ✓ Kept only actual git variables: `step.commits`, `workflow.commits`

**Additions:**
- ✓ Added standard variables: `workflow.name`, `workflow.id`, `workflow.iteration`, `step.name`, `step.index`
- ✓ Added output variables: `last.output`, `last.exit_code`, `handler.output`, `test.output`, `goal_seek.output`
- ✓ Added item variables: `item.value`, `item.path`, `item.name`, `item_index`, `item_total`
- ✓ Added MapReduce variables: `map.key`, `worker.id`, `item.*` wildcard pattern
- ✓ Added validation variables: `validation.completion_percentage`, `validation.implemented`
- ✓ Added new "Legacy Variable Aliases" section documenting: `$ARG`, `$ARGUMENT`, `$FILE`, `$FILE_PATH`
- ✓ Added `duration` field to `capture_streams` example

### 2. Command Types (High severity - 16 issues fixed)

**New Section Added:**
- ✓ Created "Advanced Command Features" section with examples for:
  - Enhanced retry configuration with exponential backoff
  - Working directory support
  - Auto-commit functionality
  - Command-level environment variables
  - Output file redirection
  - Modular handlers
  - Step validation
  - Advanced exit code handling

**Command Reference Table Updated:**
- ✓ Added `capture` field (replaces deprecated `capture_output`)
- ✓ Clarified `capture_streams` as CaptureStreams object with fields: stdout, stderr, exit_code, success, duration
- ✓ Clarified `on_exit_code` maps to full WorkflowStep objects
- ✓ Added `handler` field for modular command handlers
- ✓ Added `retry` field for enhanced retry with exponential backoff
- ✓ Added `working_dir` field for custom working directories
- ✓ Added `env` field for command-level environment variables
- ✓ Added `output_file` field for output redirection
- ✓ Added `auto_commit` field for automatic commits
- ✓ Added `commit_config` field for advanced commit control
- ✓ Added `step_validate` field for post-execution validation
- ✓ Added `skip_validation`, `validation_timeout`, `ignore_validation_failure` fields
- ✓ Clarified `on_failure` accepts full OnFailureConfig object
- ✓ Clarified `capture_format` is an enum type

### 3. Environment Configuration (High severity - 11 issues fixed)

**Global Configuration:**
- ✓ Added `inherit` field for controlling parent environment inheritance
- ✓ Added `active_profile` field for global profile activation
- ✓ Fixed incorrect profile activation syntax (was command-level `env.profile`, now root-level `active_profile`)

**Secrets Management:**
- ✓ Added structured secret format with Provider variants
- ✓ Documented secret providers: env, file, vault, aws, custom

**Environment Profiles:**
- ✓ Added profile `description` field

**Step-Level Environment:**
- ✓ Added new "Step-Level Environment Configuration" section
- ✓ Documented `working_dir`, `clear_env`, `temporary` fields
- ✓ Documented step-specific environment variables

**EnvValue Types:**
- ✓ Clarified EnvValue variants: Static, Dynamic, Conditional

**Example Fixes:**
- ✓ Updated Example 6 to show correct profile activation with `active_profile`

### 4. MapReduce Workflows (Medium severity - 8 issues fixed)

**Error Handling Configuration:**
- ✓ Added `circuit_breaker` configuration with all fields: failure_threshold, success_threshold, timeout, half_open_requests
- ✓ Added `retry_config` with backoff strategies: fixed, linear, exponential, fibonacci

**Error Policy:**
- ✓ Added `custom` option to `on_item_failure` for custom handlers
- ✓ Documented convenience fields that map to error_policy

**Setup Phase:**
- ✓ Added "Setup Phase (Advanced)" section
- ✓ Clarified setup supports both simple array and full config formats
- ✓ Documented `timeout` and `capture_outputs` fields

**Merge Configuration:**
- ✓ Clarified merge supports both array format and full format with timeout

**Deprecation Notices:**
- ✓ Added deprecation notice for nested `commands` syntax in `agent_template`
- ✓ Added deprecation notice for nested `commands` syntax in `reduce`

**Capture Outputs:**
- ✓ Clarified `capture_outputs` supports both Simple(index) and full CaptureConfig format

### 5. Error Handling (Medium severity - 6 issues fixed)

**Workflow-Level Error Policy:**
- ✓ Added `circuit_breaker` field documentation with all fields
- ✓ Added `retry_config` with backoff strategies section
- ✓ Added `custom` option to `on_item_failure`
- ✓ Added backoff strategy options: fixed, linear, exponential, fibonacci
- ✓ Added error metrics tracking note

**Command-Level Error Handling:**
- ✓ Fixed example to remove incorrect `continue_on_error` at command level
- ✓ Clarified `continue_on_error` only available in legacy CommandMetadata format

### 6. Validation Commands (Medium severity - 6 issues fixed)

**ValidationConfig Fields:**
- ✓ Added `expected_schema` field for JSON schema validation
- ✓ Added `commands` field for multi-step validation
- ✓ Added complete field documentation

**OnIncompleteConfig Fields:**
- ✓ Added `commands` field for multi-step gap filling
- ✓ Added `prompt` field for interactive guidance
- ✓ Added complete field documentation

**Deprecation:**
- ✓ Added deprecation note for `command` field (use `shell` instead)

**Clarifications:**
- ✓ Clarified array format parsing behavior with default threshold
- ✓ Added examples for multi-step validation and multi-step gap filling

## Examples Updated
- 8 YAML examples corrected
- 7 new comprehensive examples added
- 1 example updated for correct profile activation (Example 6)

## Deprecation Notices Added
- ✓ `test:` command (use `shell:` with `on_failure:`)
- ✓ `command:` in ValidationConfig (use `shell:`)
- ✓ `capture_output: true/false` (use `capture: "variable_name"`)
- ✓ Nested `commands:` in `agent_template` and `reduce` (use direct array format)
- ✓ Legacy variable aliases `$ARG`, `$ARGUMENT`, `$FILE`, `$FILE_PATH` (use modern `${item.*}` syntax)

## Source Files Referenced
- src/cook/workflow/executor.rs - WorkflowStep struct
- src/config/command.rs - WorkflowStepCommand struct
- src/cook/workflow/variables.rs - Variable types and CaptureStreams
- src/cook/environment/config.rs - Environment configuration
- src/config/mapreduce.rs - MapReduce configuration
- src/cook/workflow/error_policy.rs - Error policies and backoff
- src/cook/workflow/validation.rs - Validation configuration
- src/cook/goal_seek/mod.rs - Goal-seeking configuration
- src/cook/workflow/on_failure.rs - OnFailureConfig

## Documentation Quality Improvements

**Accuracy:**
- Removed 4 non-existent git variables that would cause user confusion
- Fixed profile activation syntax to match actual implementation
- Clarified 8+ fields that were documented as strings but are actually objects
- Added 30+ missing fields that exist in codebase but were undocumented

**Completeness:**
- All 56 identified drift issues addressed
- Every WorkflowStep field now documented
- All variable types comprehensively covered
- All error handling options explained

**Usability:**
- Added practical examples for all new features
- Clear deprecation notices with migration guidance
- Organized advanced features into dedicated section
- Maintained existing clear examples and structure

## Validation

✅ All 56 issues from drift reports addressed
✅ Documentation matches current codebase implementation
✅ All struct fields from source code documented
✅ YAML examples use valid syntax
✅ Deprecated features clearly marked
✅ Existing good content preserved
✅ Consistent formatting maintained (2-space YAML indentation)
✅ Version compatibility notes added
