## Troubleshooting

Common issues with workflow composition and their solutions.

### Circular Dependency Errors

**Error:**
```
Error: Circular dependency detected
  workflow-a.yml -> workflow-b.yml -> workflow-a.yml
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
```bash
# View full dependency chain
prodigy run workflow.yml --dry-run --show-composition
```

### Template Not Found in Registry

**Error:**
```
Error: Template 'ci-pipeline' not found in registry
```

**Cause:** Template doesn't exist in `~/.prodigy/templates/`

**Solutions:**

1. **Verify template location:**
```bash
ls ~/.prodigy/templates/ci-pipeline.yml
```

2. **Check template name:**
```yaml
# Ensure template file name matches reference
template:
  source:
    registry: "ci-pipeline"  # Looks for ci-pipeline.yml
```

3. **Add template to registry:**
```bash
cp my-template.yml ~/.prodigy/templates/
```

4. **Use file-based template instead:**
```yaml
template:
  source:
    file: "path/to/template.yml"
```

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

### Template Parameter Substitution Issues

**Error:**
```
Workflow runs but ${param} appears literally in output
```

**Cause:** Template parameter substitution in command fields is partially implemented.

**Current Status:**
- ✅ Parameter validation works
- ⏳ Parameter substitution in commands (TODO)

**Workaround:**
```yaml
# Use environment variables for now
env:
  ENVIRONMENT: "${environment}"

commands:
  - shell: "echo Deploying to $ENVIRONMENT"
```

### Sub-Workflow Execution Not Running

**Error:**
```
Sub-workflows defined but not executing
```

**Cause:** Sub-workflow execution integration is in progress.

**Current Status:**
- ✅ Sub-workflow configuration parsing
- ✅ Sub-workflow validation
- ⏳ Executor runtime integration (in development)

**Workaround:**
```yaml
# Use shell commands to invoke workflows manually
commands:
  - shell: "prodigy run workflows/test.yml"
  - shell: "prodigy run workflows/build.yml"
```

### Default Values Not Applied

**Error:**
```
Parameters require values even though defaults are set
```

**Cause:** Default value merge logic is pending implementation.

**Current Status:**
- ✅ Defaults parsing and storage
- ⏳ Merge logic (TODO in apply_defaults)

**Workaround:**
```yaml
# Use parameter defaults instead of workflow defaults
parameters:
  definitions:
    timeout:
      type: Number
      default: 300  # Works now

# Instead of:
# defaults:
#   timeout: 300  # Not yet applied
```

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

### Composition Metadata Not Showing

**Error:**
```
--show-composition flag doesn't display metadata
```

**Cause:** Flag may not be implemented in current CLI.

**Workaround:**
```bash
# Use verbose mode to see composition details
prodigy run workflow.yml --dry-run -vv
```

### Debugging Strategies

**Enable Verbose Logging:**
```bash
# Show composition steps
prodigy run workflow.yml -v

# Show detailed debug output
prodigy run workflow.yml -vv

# Show trace-level output
prodigy run workflow.yml -vvv
```

**Dry-Run Validation:**
```bash
# Validate composition without execution
prodigy run workflow.yml --dry-run
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
```

**Verify JSON Syntax:**
```bash
# Validate param file
jq . params.json

# Check for syntax errors
cat params.json | jq empty
```

### Getting Help

If issues persist:

1. **Check implementation status** in relevant subsection docs
2. **Review error context** in error messages
3. **File issue** with minimal reproduction case
4. **Include:**
   - Workflow files
   - Command used
   - Full error output
   - Prodigy version
