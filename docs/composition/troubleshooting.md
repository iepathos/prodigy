## Troubleshooting

Common issues with workflow composition and their solutions.

### Circular Dependency Errors

!!! danger "Error"
    ```
    Error: Circular dependency detected in workflow composition
    ```

**Cause:** Workflow inheritance or imports form a cycle.

!!! tip "Solution"
    Use a common base workflow instead of circular extends:

    === "Bad: Circular Dependency"
        ```yaml
        # workflow-a.yml extends workflow-b.yml
        # workflow-b.yml extends workflow-a.yml
        ```

    === "Good: Common Base"
        ```yaml
        # base.yml (no extends)
        # workflow-a.yml extends base.yml
        # workflow-b.yml extends base.yml
        ```

!!! note "Debugging"
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

**Source:** Error generated in `src/cook/workflow/composition/composer.rs:712`

### Template Not Found in Registry

!!! danger "Error"
    ```
    Error: Template 'ci-pipeline' not found in registry
    ```

**Cause:** Template doesn't exist in the template search path.

!!! tip "Solutions"

    **1. Verify template locations (searched in order):**

    Prodigy searches for templates in this priority order:

    ```bash title="Template Search Order"
    # 1. Global templates (highest priority, shared across repos)
    ls ~/.prodigy/templates/ci-pipeline.yml

    # 2. Project-local templates
    ls .prodigy/templates/ci-pipeline.yml

    # 3. Legacy project-local templates
    ls templates/ci-pipeline.yml
    ```

    **Source:** Template search path defined in `src/cook/workflow/composer_integration.rs:94-110`

    **2. Check template name:**

    ```yaml title="workflow.yml"
    # Ensure template file name matches reference
    template:
      source:
        registry: "ci-pipeline"  # Looks for ci-pipeline.yml
    ```

    **3. Add template to registry:**

    === "Global Registry"
        ```bash
        # Available to all projects
        cp my-template.yml ~/.prodigy/templates/
        ```

    === "Project-Local Registry"
        ```bash
        # Only available in this project
        cp my-template.yml .prodigy/templates/
        ```

    **4. Use file-based template instead:**

    ```yaml title="workflow.yml"
    template:
      source:
        file: "path/to/template.yml"
    ```

**See also:** [Template System](template-system.md) for more details on template sources.

### Parameter Validation Failures

!!! danger "Error"
    ```
    Error: Parameter validation failed
      - 'environment': Expected String, got Number
    ```

**Cause:** Parameter type mismatch.

!!! tip "Solutions"

    **1. Check parameter type:**

    ```yaml title="workflow.yml"
    parameters:
      definitions:
        environment:
          type: String  # Must pass string value
    ```

    ```bash title="Command Line"
    # Pass correct type
    prodigy run workflow.yml --param environment="production"  # String
    # Not: --param environment=123  # Number
    ```

    **2. Verify validation expression:**

    ```yaml title="workflow.yml"
    parameters:
      definitions:
        environment:
          type: String
          validation: "matches('^(dev|staging|prod)$')"
    ```

    ```bash title="Command Line"
    # Must match regex pattern
    prodigy run workflow.yml --param environment="staging"  # OK
    # Not: --param environment="test"  # Fails validation
    ```

    **3. Check required vs optional:**

    ```yaml title="workflow.yml"
    parameters:
      required:
        - environment  # Must provide
    ```

    ```bash title="Command Line"
    # Error if missing:
    prodigy run workflow.yml  # Fails

    # Solution:
    prodigy run workflow.yml --param environment="dev"
    ```

**See also:** [Parameter Definitions](parameter-definitions.md) for complete parameter validation reference.

### Import Path Resolution Errors

!!! danger "Error"
    ```
    Error: Failed to load import: shared/utilities.yml
      No such file or directory
    ```

**Cause:** Import path doesn't exist or is incorrect.

!!! tip "Solutions"

    **1. Use absolute path:**

    ```yaml title="workflow.yml"
    imports:
      - path: "/full/path/to/shared/utilities.yml"
    ```

    **2. Verify relative path:**

    ```bash title="Check file exists"
    # From workflow file directory
    ls shared/utilities.yml
    ```

    ```yaml title="workflow.yml"
    # If in different location:
    imports:
      - path: "../shared/utilities.yml"  # Go up one level
    ```

    **3. Check current directory:**

    ```bash
    # Run from correct directory
    cd /path/to/workflows
    prodigy run my-workflow.yml
    ```

### Type Mismatch Errors

!!! danger "Error"
    ```
    Error: Type mismatch for parameter 'timeout'
      Expected Number, got String "300"
    ```

**Cause:** Parameter value type doesn't match definition.

!!! tip "Solutions"

    **1. Pass correct type:**

    ```bash title="Command Line Types"
    # Number type - no quotes
    prodigy run workflow.yml --param timeout=300

    # String type - use quotes
    prodigy run workflow.yml --param environment="production"

    # Boolean type - no quotes
    prodigy run workflow.yml --param enable_debug=true
    ```

    **2. Check parameter file format:**

    ```json title="params.json"
    {
      "timeout": 300,        // Number (no quotes)
      "environment": "prod", // String (quotes)
      "debug": true          // Boolean (no quotes)
    }
    ```

### Base Workflow Resolution Failures

!!! danger "Error"
    ```
    Error: Failed to resolve base workflow: base-config.yml
    ```

**Cause:** Extended workflow file not found.

!!! tip "Solutions"

    **1. Verify extends path:**

    ```yaml title="workflow.yml"
    # Relative to current workflow file
    extends: "base-config.yml"         # Same directory
    extends: "../base/config.yml"      # Parent directory
    extends: "shared/base-config.yml"  # Subdirectory
    ```

    **2. Use absolute path:**

    ```yaml title="workflow.yml"
    extends: "/full/path/to/base-config.yml"
    ```

    **3. Check file exists:**

    ```bash
    ls -la base-config.yml
    ```

### Parameter Substitution Issues

!!! warning "Issue"
    ```
    Workflow runs but ${param} appears literally in output
    ```

**Cause:** Parameter not found in the parameters map, or incorrect syntax.

!!! info "Status"
    Parameter substitution is fully implemented and works in all command types.

**Source:** Implemented in `src/cook/workflow/composition/composer.rs:745-871` (supports Simple, Structured, WorkflowStep, and SimpleObject command types)

!!! tip "Solutions"

    **1. Verify parameter is defined:**

    ```yaml title="workflow.yml"
    parameters:
      definitions:
        environment:
          type: String

    # Then use in commands
    commands:
      - shell: "echo Deploying to ${environment}"
      - claude: "/deploy ${environment}"
    ```

    **2. Check parameter is provided:**

    ```bash title="Command Line"
    # Parameter must be passed at runtime
    prodigy run workflow.yml --param environment="production"
    ```

    **3. Use correct syntax:**

    === "Correct"
        ```yaml title="workflow.yml"
        # ${param_name} is Prodigy parameter substitution
        - shell: "process ${target_file}"
        ```

    === "Incorrect"
        ```yaml title="workflow.yml"
        # $param_name is shell variable, not Prodigy parameter
        - shell: "process $target_file"
        ```

    **4. Parameter substitution works in all command types:**

    ```yaml title="workflow.yml"
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

!!! note "Supported Value Types"
    | Type | Behavior |
    |------|----------|
    | Strings | Used as-is |
    | Numbers | Converted to string representation |
    | Booleans | Converted to "true" or "false" |
    | Arrays/Objects | Serialized as JSON |
    | Null | Becomes empty string |

**See also:** [Parameter Definitions](parameter-definitions.md) for parameter syntax reference.

### Sub-Workflow Execution Issues

!!! warning "Issue"
    ```
    Sub-workflows defined but not executing as expected
    ```

!!! info "Status"
    Sub-workflow execution is fully implemented via SubWorkflowExecutor.

**Source:** Implemented in `src/cook/workflow/composition/sub_workflow.rs:67-176`

!!! tip "Common Issues"

    **1. Sub-workflow file path incorrect:**

    ```yaml title="workflow.yml"
    # Verify the source path exists
    workflows:
      build:
        source: "workflows/build.yml"  # Must exist relative to current file
    ```

    ```bash title="Verify file exists"
    ls workflows/build.yml
    ```

    **2. Parameter type mismatch:**

    ```yaml title="workflow.yml"
    # Sub-workflow parameters must match defined types
    workflows:
      deploy:
        source: "deploy.yml"
        parameters:
          timeout: 300        # Number, not "300"
          environment: "prod" # String with quotes
    ```

    **3. Input/output mapping errors:**

    ```yaml title="workflow.yml"
    # Input variables must exist in parent context
    workflows:
      test:
        source: "test.yml"
        inputs:
          target_file: "build_output"  # Parent var 'build_output' must exist
        outputs:
          - "test_result"  # Will be available in parent after execution
    ```

    **4. Timeout too short:**

    === "Problem"
        ```yaml title="workflow.yml"
        workflows:
          long_running:
            source: "build.yml"
            timeout: 60  # Seconds - may be too short
        ```

    === "Solution"
        ```yaml title="workflow.yml"
        workflows:
          long_running:
            source: "build.yml"
            timeout: 600  # 10 minutes
        ```

!!! note "Supported Features"
    - Parameter passing (JSON values)
    - Input/output variable mapping
    - Context isolation (sub-workflow has clean context)
    - Error handling with `continue_on_error` flag
    - Timeout support
    - Parallel execution with `parallel: true`

**See also:** [Sub-Workflows](sub-workflows.md) for complete usage guide.

### Default Values Not Applied

!!! warning "Issue"
    ```
    Parameters require values even though defaults are set
    ```

!!! info "Status"
    Default values are fully applied through the apply_defaults method.

**Source:** Implemented in `src/cook/workflow/composition/composer.rs:217-257`

!!! example "How Defaults Work"

    **1. Workflow-level defaults are applied to environment variables:**

    ```yaml title="workflow.yml"
    defaults:
      TIMEOUT: "300"
      ENVIRONMENT: "dev"

    # These become available as env vars in all commands
    commands:
      - shell: "echo Timeout: $TIMEOUT"  # Uses default
    ```

    **2. Parameter-level defaults work differently:**

    ```yaml title="workflow.yml"
    parameters:
      definitions:
        timeout:
          type: Number
          default: 300  # Used if not provided at runtime
    ```

    ```bash
    # Run without providing timeout
    prodigy run workflow.yml  # Uses default 300
    ```

    **3. Precedence order (highest to lowest):**

    1. Explicitly provided parameter values (`--param`)
    2. Parameter definition defaults
    3. Workflow-level defaults
    4. No value (error if required parameter)

!!! danger "Common Mistakes"

    **1. Workflow defaults don't set parameter values:**

    === "Problem"
        ```yaml title="workflow.yml"
        defaults:
          timeout: 300  # Sets env var TIMEOUT, not parameter 'timeout'

        parameters:
          required:
            - timeout  # Still required!
        ```

    === "Solution"
        ```yaml title="workflow.yml"
        parameters:
          definitions:
            timeout:
              type: Number
              default: 300  # Now parameter has a default
        ```

    **2. Existing values are not overwritten:**

    ```yaml title="workflow.yml"
    # If a value is already set, defaults don't override
    parameters:
      definitions:
        timeout:
          type: Number
          default: 300  # Only used if not already set
    ```

    ```bash
    # This overrides the default
    prodigy run workflow.yml --param timeout=600
    ```

**See also:** [Default Values](default-values.md) for complete default value semantics.

### URL Template Source Errors

!!! danger "Error"
    ```
    Error: URL template sources are not yet implemented
    ```

**Cause:** URL-based template loading is planned but not implemented.

!!! tip "Workarounds"

    **1. Download template to file:**

    ```bash
    curl https://example.com/template.yml > /tmp/template.yml
    ```

    **2. Use file-based template:**

    ```yaml title="workflow.yml"
    template:
      source:
        file: "/tmp/template.yml"
    ```

    **3. Add to registry:**

    ```bash
    curl https://example.com/template.yml > ~/.prodigy/templates/template.yml
    ```

    ```yaml title="workflow.yml"
    template:
      source:
        registry: "template"
    ```

### Debugging Strategies

!!! tip "Enable Verbose Logging"

    ```bash title="Verbosity Levels"
    # Show composition steps
    prodigy run workflow.yml -v

    # Show detailed debug output
    prodigy run workflow.yml -vv

    # Show trace-level output (includes full composition details)
    prodigy run workflow.yml -vvv
    ```

!!! tip "Dry-Run Validation"

    ```bash
    # Validate composition without execution
    prodigy run workflow.yml --dry-run

    # Combine with verbose mode to see composition steps
    prodigy run workflow.yml --dry-run -vv
    ```

!!! tip "Isolate Composition Layers"

    ```bash
    # Test base workflow alone
    prodigy run base-config.yml --dry-run

    # Add one composition feature at a time
    # 1. Test with imports only
    # 2. Add extends
    # 3. Add template
    # 4. Add parameters
    ```

!!! tip "Check File Permissions"

    ```bash
    # Verify read access
    ls -la workflow.yml base-config.yml

    # Check registry permissions
    ls -la ~/.prodigy/templates/
    ls -la .prodigy/templates/
    ```

!!! tip "Verify JSON Syntax"

    ```bash
    # Validate param file
    jq . params.json

    # Check for syntax errors
    cat params.json | jq empty
    ```

!!! tip "Trace Parameter Substitution"

    ```bash
    # Use verbose mode to see parameter values
    prodigy run workflow.yml --param environment="prod" -vv

    # Check which parameters are being substituted
    ```

!!! tip "Debug Sub-Workflow Execution"

    ```bash
    # Test sub-workflow independently first
    prodigy run workflows/build.yml --dry-run

    # Then test from parent workflow
    prodigy run main.yml -vv  # See sub-workflow execution logs
    ```

### Getting Help

!!! note "If Issues Persist"
    1. **Check implementation status** in relevant subsection docs
    2. **Review error context** in error messages
    3. **Use verbose mode** (`-vv` or `-vvv`) to understand what's happening
    4. **Test components independently** (base workflows, sub-workflows, templates)
    5. **File issue** with minimal reproduction case

!!! info "Include in Bug Reports"
    - Workflow files
    - Command used
    - Full error output
    - Prodigy version
    - Output from verbose mode (`-vv`)
