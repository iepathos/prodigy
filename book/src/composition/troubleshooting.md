## Troubleshooting

Common issues with workflow composition and their solutions.

### Circular Dependency Errors

**Error:**
```
Error: Circular dependency detected in workflow composition
```

**Cause:** Workflow inheritance or imports form a cycle.

**Solution:**
```yaml
# Bad: Circular dependency
# workflow-a.yml extends workflow-b.yml
# workflow-b.yml extends workflow-a.yml

# Good: Use common base
# base.yml (no extends)
# workflow-a.yml extends base.yml
# workflow-b.yml extends base.yml
```

**Debugging:**

The error message doesn't include the dependency chain. To diagnose which workflows are involved:

```bash
# Use verbose mode to see composition steps
prodigy run workflow.yml --dry-run -vv

# Manually trace the chain
# 1. Check what the workflow extends
grep "^extends:" workflow-a.yml

# 2. Check what that workflow extends
grep "^extends:" workflow-b.yml

# 3. Continue until you find the cycle
```

**Source:** Error generated in src/cook/workflow/composition/composer.rs:727

### Template Not Found in Registry

**Error:**
```
Error: Template 'ci-pipeline' not found in registry
```

**Cause:** Template doesn't exist in the template search path.

**Solutions:**

1. **Verify template locations (searched in order):**

Prodigy searches for templates in this priority order:

```bash
# 1. Global templates (highest priority, shared across repos)
ls ~/.prodigy/templates/ci-pipeline.yml

# 2. Project-local templates
ls .prodigy/templates/ci-pipeline.yml

# 3. Legacy project-local templates
ls templates/ci-pipeline.yml
```

**Source:** Template search path defined in src/cook/workflow/composer_integration.rs:94-110

2. **Check template name:**
```yaml
# Ensure template file name matches reference
template:
  source:
    registry: "ci-pipeline"  # Looks for ci-pipeline.yml
```

3. **Add template to registry:**
```bash
# Add to global registry (available to all projects)
cp my-template.yml ~/.prodigy/templates/

# Or add to project-local registry
cp my-template.yml .prodigy/templates/
```

4. **Use file-based template instead:**
```yaml
template:
  source:
    file: "path/to/template.yml"
```

**See also:** [Template System](template-system.md) for more details on template sources.

### Parameter Validation Failures

**Error:**
```
Error: Parameter validation failed
  - 'environment': Expected String, got Number
```

**Cause:** Parameter type mismatch.

**Solutions:**

1. **Check parameter type:**
```yaml
parameters:
  definitions:
    environment:
      type: String  # Must pass string value

# Pass correct type
prodigy run workflow.yml --param environment="production"  # String
# Not: --param environment=123  # Number
```

2. **Verify validation expression:**
```yaml
parameters:
  definitions:
    environment:
      type: String
      validation: "matches('^(dev|staging|prod)$')"

# Must match regex pattern
prodigy run workflow.yml --param environment="staging"  # OK
# Not: --param environment="test"  # Fails validation
```

3. **Check required vs optional:**
```yaml
parameters:
  required:
    - environment  # Must provide

# Error if missing:
prodigy run workflow.yml  # Fails

# Solution:
prodigy run workflow.yml --param environment="dev"
```

**See also:** [Parameter Definitions](parameter-definitions.md) for complete parameter validation reference.

### Import Path Resolution Errors

**Error:**
```
Error: Failed to load import: shared/utilities.yml
  No such file or directory
```

**Cause:** Import path doesn't exist or is incorrect.

**Solutions:**

1. **Use absolute path:**
```yaml
imports:
  - path: "/full/path/to/shared/utilities.yml"
```

2. **Verify relative path:**
```bash
# From workflow file directory
ls shared/utilities.yml

# If in different location:
imports:
  - path: "../shared/utilities.yml"  # Go up one level
```

3. **Check current directory:**
```bash
# Run from correct directory
cd /path/to/workflows
prodigy run my-workflow.yml
```

### Type Mismatch Errors

**Error:**
```
Error: Type mismatch for parameter 'timeout'
  Expected Number, got String "300"
```

**Cause:** Parameter value type doesn't match definition.

**Solutions:**

1. **Pass correct type:**
```bash
# Number type - no quotes
prodigy run workflow.yml --param timeout=300

# String type - use quotes
prodigy run workflow.yml --param environment="production"

# Boolean type - no quotes
prodigy run workflow.yml --param enable_debug=true
```

2. **Check parameter file format:**
```json
{
  "timeout": 300,        // Number (no quotes)
  "environment": "prod", // String (quotes)
  "debug": true          // Boolean (no quotes)
}
```

### Base Workflow Resolution Failures

**Error:**
```
Error: Failed to resolve base workflow: base-config.yml
```

**Cause:** Extended workflow file not found.

**Solutions:**

1. **Verify extends path:**
```yaml
# Relative to current workflow file
extends: "base-config.yml"         # Same directory
extends: "../base/config.yml"      # Parent directory
extends: "shared/base-config.yml"  # Subdirectory
```

2. **Use absolute path:**
```yaml
extends: "/full/path/to/base-config.yml"
```

3. **Check file exists:**
```bash
ls -la base-config.yml
```

### Parameter Substitution Issues

**Error:**
```
Workflow runs but ${param} appears literally in output
```

**Cause:** Parameter not found in the parameters map, or incorrect syntax.

**Status:** Parameter substitution is fully implemented and works in all command types.

**Source:** Implemented in src/cook/workflow/composition/composer.rs:760-877 (supports Simple, Structured, WorkflowStep, and SimpleObject command types)

**Solutions:**

1. **Verify parameter is defined:**
```yaml
parameters:
  definitions:
    environment:
      type: String

# Then use in commands
commands:
  - shell: "echo Deploying to ${environment}"
  - claude: "/deploy ${environment}"
```

2. **Check parameter is provided:**
```bash
# Parameter must be passed at runtime
prodigy run workflow.yml --param environment="production"
```

3. **Use correct syntax:**
```yaml
# Correct: ${param_name}
- shell: "process ${target_file}"

# Incorrect: $param_name (shell variable, not Prodigy parameter)
- shell: "process $target_file"
```

4. **Parameter substitution works in all command types:**
```yaml
commands:
  # Simple string commands
  - "shell: process ${file}"

  # Structured commands
  - name: "process"
    args: ["${file}", "${output}"]

  # WorkflowStep format
  - shell: "test ${file}"
    id: "test-${file}"

  # SimpleObject format
  - name: "build"
    args: ["${target}"]
```

**Supported value types:**
- Strings: Used as-is
- Numbers: Converted to string representation
- Booleans: Converted to "true" or "false"
- Arrays/Objects: Serialized as JSON
- Null: Becomes empty string

**See also:** [Parameter Definitions](parameter-definitions.md) for parameter syntax reference.

### Sub-Workflow Execution Issues

**Error:**
```
Sub-workflows defined but not executing as expected
```

**Status:** Sub-workflow execution is fully implemented via SubWorkflowExecutor.

**Source:** Implemented in src/cook/workflow/composition/sub_workflow.rs:67-176

**Common Issues:**

1. **Sub-workflow file path incorrect:**
```yaml
# Verify the source path exists
workflows:
  build:
    source: "workflows/build.yml"  # Must exist relative to current file
```

```bash
# Check file exists
ls workflows/build.yml
```

2. **Parameter type mismatch:**
```yaml
# Sub-workflow parameters must match defined types
workflows:
  deploy:
    source: "deploy.yml"
    parameters:
      timeout: 300        # Number, not "300"
      environment: "prod" # String with quotes
```

3. **Input/output mapping errors:**
```yaml
# Input variables must exist in parent context
workflows:
  test:
    source: "test.yml"
    inputs:
      target_file: "build_output"  # Parent var 'build_output' must exist
    outputs:
      - "test_result"  # Will be available in parent after execution
```

4. **Timeout too short:**
```yaml
workflows:
  long_running:
    source: "build.yml"
    timeout: 60  # Seconds - may be too short

# Increase if sub-workflow times out
workflows:
  long_running:
    source: "build.yml"
    timeout: 600  # 10 minutes
```

**Supported features:**
- Parameter passing (JSON values)
- Input/output variable mapping
- Context isolation (sub-workflow has clean context)
- Error handling with `continue_on_error` flag
- Timeout support
- Parallel execution with `parallel: true`

**See also:** [Sub-Workflows](sub-workflows.md) for complete usage guide.

### Default Values Not Applied

**Error:**
```
Parameters require values even though defaults are set
```

**Status:** Default values are fully applied through the apply_defaults method.

**Source:** Implemented in src/cook/workflow/composition/composer.rs:217-257

**How defaults work:**

1. **Workflow-level defaults are applied to environment variables:**
```yaml
defaults:
  TIMEOUT: "300"
  ENVIRONMENT: "dev"

# These become available as env vars in all commands
commands:
  - shell: "echo Timeout: $TIMEOUT"  # Uses default
```

2. **Parameter-level defaults work differently:**
```yaml
parameters:
  definitions:
    timeout:
      type: Number
      default: 300  # Used if not provided at runtime

# Run without providing timeout
prodigy run workflow.yml  # Uses default 300
```

3. **Precedence order (highest to lowest):**
   - Explicitly provided parameter values (--param)
   - Parameter definition defaults
   - Workflow-level defaults
   - No value (error if required parameter)

**Common mistakes:**

1. **Workflow defaults don't set parameter values:**
```yaml
# This does NOT work as expected
defaults:
  timeout: 300  # Sets env var TIMEOUT, not parameter 'timeout'

parameters:
  required:
    - timeout  # Still required!

# Solution: Use parameter default instead
parameters:
  definitions:
    timeout:
      type: Number
      default: 300  # Now parameter has a default
```

2. **Existing values are not overwritten:**
```yaml
# If a value is already set, defaults don't override
parameters:
  definitions:
    timeout:
      type: Number
      default: 300  # Only used if not already set

# This overrides the default
prodigy run workflow.yml --param timeout=600
```

**See also:** [Default Values](default-values.md) for complete default value semantics.

### URL Template Source Errors

**Error:**
```
Error: URL template sources are not yet implemented
```

**Cause:** URL-based template loading is planned but not implemented.

**Solutions:**

1. **Download template to file:**
```bash
curl https://example.com/template.yml > /tmp/template.yml
```

2. **Use file-based template:**
```yaml
template:
  source:
    file: "/tmp/template.yml"
```

3. **Add to registry:**
```bash
curl https://example.com/template.yml > ~/.prodigy/templates/template.yml
```

```yaml
template:
  source:
    registry: "template"
```

### Debugging Strategies

**Enable Verbose Logging:**
```bash
# Show composition steps
prodigy run workflow.yml -v

# Show detailed debug output
prodigy run workflow.yml -vv

# Show trace-level output (includes full composition details)
prodigy run workflow.yml -vvv
```

**Dry-Run Validation:**
```bash
# Validate composition without execution
prodigy run workflow.yml --dry-run

# Combine with verbose mode to see composition steps
prodigy run workflow.yml --dry-run -vv
```

**Isolate Composition Layers:**
```bash
# Test base workflow alone
prodigy run base-config.yml --dry-run

# Add one composition feature at a time
# 1. Test with imports only
# 2. Add extends
# 3. Add template
# 4. Add parameters
```

**Check File Permissions:**
```bash
# Verify read access
ls -la workflow.yml base-config.yml

# Check registry permissions
ls -la ~/.prodigy/templates/
ls -la .prodigy/templates/
```

**Verify JSON Syntax:**
```bash
# Validate param file
jq . params.json

# Check for syntax errors
cat params.json | jq empty
```

**Trace Parameter Substitution:**
```bash
# Use verbose mode to see parameter values
prodigy run workflow.yml --param environment="prod" -vv

# Check which parameters are being substituted
```

**Debug Sub-Workflow Execution:**
```bash
# Test sub-workflow independently first
prodigy run workflows/build.yml --dry-run

# Then test from parent workflow
prodigy run main.yml -vv  # See sub-workflow execution logs
```

### Getting Help

If issues persist:

1. **Check implementation status** in relevant subsection docs
2. **Review error context** in error messages
3. **Use verbose mode** (-vv or -vvv) to understand what's happening
4. **Test components independently** (base workflows, sub-workflows, templates)
5. **File issue** with minimal reproduction case
6. **Include:**
   - Workflow files
   - Command used
   - Full error output
   - Prodigy version
   - Output from verbose mode (-vv)
