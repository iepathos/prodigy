## Parameter Definitions

Define parameters with type validation to create flexible, reusable workflows. Parameters enable workflows and templates to accept inputs with enforced types, default values, and validation rules.

### Basic Parameter Definition

```yaml
name: deployment-workflow

parameters:
  # Required parameters (must be provided)
  required:
    - environment
    - version

  # Optional parameters (have defaults or can be omitted)
  optional:
    - debug_mode
    - timeout
```

### Parameter Structure

Parameters are organized into `required` and `optional` arrays. Each parameter specifies its type, description, and validation rules:

```yaml
parameters:
  required:
    - name: environment
      type: String
      description: "Target environment for deployment"
      validation: "matches('^(dev|staging|prod)$')"

    - name: version
      type: String
      description: "Application version to deploy"

  optional:
    - name: port
      type: Number
      description: "Server port number"
      default: 8080

    - name: enable_ssl
      type: Boolean
      description: "Enable SSL/TLS"
      default: true

    - name: allowed_hosts
      type: Array
      description: "List of allowed hostnames"
      default: ["localhost"]

    - name: config
      type: Object
      description: "Configuration object"
      default: {"timeout": 30}

    - name: data
      type: Any
      description: "Free-form data of any type"
```

**Source**: `ParameterDefinitions` structure in src/cook/workflow/composition/mod.rs:97-107

### Parameter Types

Prodigy supports six parameter types with validation (defined in src/cook/workflow/composition/mod.rs:131-141):

| Type | Description | Example Values |
|------|-------------|----------------|
| `String` | Text values | `"production"`, `"v1.2.3"` |
| `Number` | Integer or float | `42`, `3.14`, `-100` |
| `Boolean` | True or false | `true`, `false` |
| `Array` | List of values | `[1, 2, 3]`, `["a", "b"]` |
| `Object` | Key-value map | `{"key": "value"}` |
| `Any` | Any JSON value | Any valid JSON |

**Source**: `ParameterType` enum in src/cook/workflow/composition/mod.rs:131-141

**Type Validation:**
- Type checking is enforced when parameters are provided (src/cook/workflow/composition/mod.rs:226-280)
- Mismatched types cause workflow validation errors
- `Any` type accepts any value without validation
- Validation logic in `validate_parameters` function

**Test example**: tests/workflow_composition_test.rs:49-79 demonstrates parameter validation with String type

### Default Values

Parameters can specify default values used when no value is provided. Defaults can be set at two levels:

**Parameter-Level Defaults** (in parameter definition):

```yaml
parameters:
  optional:
    - name: timeout
      type: Number
      description: "Operation timeout in seconds"
      default: 300

    - name: log_level
      type: String
      description: "Logging verbosity"
      default: "info"

    - name: retry_enabled
      type: Boolean
      description: "Enable retry logic"
      default: true
```

**Workflow-Level Defaults** (applies to all sub-workflows):

```yaml
name: parent-workflow

defaults:
  environment: "development"
  debug_mode: true
  timeout: 600

parameters:
  optional:
    - name: environment
      type: String
      default: "production"  # Overridden by workflow defaults

    - name: timeout
      type: Number
      default: 300  # Overridden by workflow defaults
```

**Source**: `defaults` field in src/cook/workflow/composition/mod.rs:204, `default` field in src/cook/workflow/composition/mod.rs:123-124

### Validation Expressions

The `validation` field allows custom validation logic:

```yaml
parameters:
  definitions:
    email:
      type: String
      validation: "matches('^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\\.[a-zA-Z]{2,}$')"

    priority:
      type: Number
      validation: "value >= 1 && value <= 10"

    status:
      type: String
      validation: "in(['pending', 'active', 'completed'])"
```

*Note: Validation expressions are currently stored and validated for syntax, but custom expression evaluation is not yet implemented. Type validation is fully functional.*

### Providing Parameter Values

**Via Command Line:**
```bash
# Individual parameters (with automatic type inference)
prodigy run workflow.yml --param environment=production --param timeout=600

# From JSON file
prodigy run workflow.yml --param-file params.json
```

**Automatic Type Inference:**

When using `--param` flags, Prodigy automatically infers parameter types:
- **Numbers**: `--param port=8080` → parsed as Number (i64 or f64)
- **Booleans**: `--param debug=true` → parsed as Boolean
- **Strings**: `--param name=app` → parsed as String (default if no other type matches)

```bash
# These are automatically typed correctly:
prodigy run workflow.yml \
  --param port=8080 \           # Number
  --param timeout=30.5 \        # Number (float)
  --param debug=true \          # Boolean
  --param environment=prod      # String
```

**Source**: `parse_param_value` function in src/cli/params.rs:51-72

**params.json:**
```json
{
  "environment": "staging",
  "version": "2.1.0",
  "debug_mode": false,
  "timeout": 300
}
```

**Parameter Precedence:**
1. CLI `--param` flags (highest priority)
2. `--param-file` values
3. Workflow `defaults` values
4. Parameter `default` values (lowest priority)

### Using Parameters in Workflows

Parameters are interpolated into commands using the standard variable interpolation system with `${param_name}` syntax. This is the same syntax used for all workflow variables (captured outputs, environment variables, etc.).

```yaml
parameters:
  required:
    - name: app_name
      type: String
    - name: deploy_env
      type: String

commands:
  - shell: "echo Deploying ${app_name} to ${deploy_env}"
  - shell: "kubectl apply -f k8s/${deploy_env}/deployment.yml"
  - claude: "/deploy ${app_name} --environment ${deploy_env}"
```

Parameters are resolved during variable interpolation before command execution, making them available everywhere workflow variables are supported.

**Source**: Variable interpolation system in src/cook/workflow/variables.rs

### Complete Example

```yaml
name: database-migration
mode: standard

parameters:
  required:
    - name: database_url
      type: String
      description: "Database connection string"
      validation: "matches('^postgres://')"

    - name: migration_version
      type: String
      description: "Target migration version"

  optional:
    - name: dry_run
      type: Boolean
      description: "Run in dry-run mode"
      default: false

    - name: timeout
      type: Number
      description: "Migration timeout in seconds"
      default: 300

commands:
  - shell: "echo Running migration to ${migration_version}"
  - shell: |
      migrate --database-url ${database_url} \
              --target ${migration_version} \
              --timeout ${timeout} \
              $( [ "${dry_run}" = "true" ] && echo "--dry-run" )
```

**Run with parameters:**
```bash
prodigy run migration.yml \
  --param database_url="postgres://localhost/mydb" \
  --param migration_version="20250109_001" \
  --param dry_run=true
```

### Parameter Validation Errors

When validation fails, Prodigy provides clear error messages:

```
Error: Parameter validation failed
  - 'environment': Expected String, got Number
  - 'port': Value 99999 exceeds valid range
  - 'config': Required parameter not provided
```

### Implementation Status

- ✅ Parameter type definitions (all 6 types)
- ✅ Type validation enforcement
- ✅ Default value support
- ✅ Required/optional parameter tracking
- ✅ CLI parameter passing (--param, --param-file)
- ✅ Parameter precedence handling
- ⏳ Custom validation expression evaluation (field stored, not evaluated)

### Related Topics

- [Template System](template-system.md) - Use parameters in templates
- [Default Values](default-values.md) - Set workflow-level defaults
- [Providing Parameter Values](#providing-parameter-values) - Command-line parameter usage

