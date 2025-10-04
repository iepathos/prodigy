---
number: 120
title: Add Environment Variable Support to MapReduce Workflows
category: parallel
priority: high
status: draft
dependencies: []
created: 2025-10-04
---

# Specification 120: Add Environment Variable Support to MapReduce Workflows

**Category**: parallel
**Priority**: high
**Status**: draft
**Dependencies**: None

## Context

Standard Prodigy workflows (`WorkflowConfig`) support environment variables through `env`, `secrets`, `env_files`, and `profiles` fields. This enables workflows to define reusable variables that can be referenced in commands.

MapReduce workflows (`MapReduceWorkflowConfig`) currently do NOT support environment variables. This is a gap that limits workflow reusability and makes it harder to parameterize MapReduce workflows for different contexts (e.g., different projects, environments, configurations).

Specs 118 and 119 (generalized book documentation) rely on environment variables to make the same workflow commands work across different projects (Prodigy, Debtmap, etc.). This spec enables that functionality.

## Objective

Add environment variable support to MapReduce workflows to enable:
1. Defining global environment variables at the workflow level
2. Using environment variables in setup, map, reduce, and merge phases
3. Passing project-specific parameters via environment variables
4. Maintaining consistency with standard workflow environment features
5. Supporting the book documentation workflows (Specs 118-119)

## Requirements

### Functional Requirements

**FR1**: Add environment variable fields to MapReduceWorkflowConfig:
- `env: Option<HashMap<String, String>>` - Global environment variables
- `secrets: Option<HashMap<String, SecretValue>>` - Secret variables (masked in logs)
- `env_files: Option<Vec<PathBuf>>` - Load environment from .env files
- `profiles: Option<HashMap<String, EnvProfile>>` - Environment profiles

**FR2**: Environment variable interpolation in all phases:
- Setup phase commands can reference `$VAR_NAME` or `${VAR_NAME}`
- Map phase agent_template commands can reference environment variables
- Reduce phase commands can reference environment variables
- Merge phase commands can reference environment variables

**FR3**: Maintain backward compatibility:
- Existing MapReduce workflows without `env` field continue to work
- Environment variables are optional (default to None)
- No breaking changes to existing workflow syntax

**FR4**: Consistent behavior with standard workflows:
- Same environment variable precedence rules
- Same variable interpolation syntax
- Same secret masking behavior
- Same profile activation mechanism

### Non-Functional Requirements

**NFR1**: **Consistency**: Environment behavior identical to standard workflows

**NFR2**: **Performance**: No performance impact for workflows not using environment variables

**NFR3**: **Security**: Secrets properly masked in all log output

**NFR4**: **Usability**: Clear error messages for undefined variables or syntax errors

## Acceptance Criteria

- [ ] `MapReduceWorkflowConfig` has `env`, `secrets`, `env_files`, `profiles` fields
- [ ] Environment variables work in setup phase commands
- [ ] Environment variables work in map phase agent_template
- [ ] Environment variables work in reduce phase commands
- [ ] Environment variables work in merge phase commands
- [ ] Variable interpolation supports `$VAR` and `${VAR}` syntax
- [ ] Secrets are masked in logs and output
- [ ] Existing MapReduce workflows continue to work without changes
- [ ] Book documentation workflows (Specs 118-119) can use environment variables
- [ ] Documentation updated with MapReduce environment variable examples

## Technical Details

### Implementation Approach

#### 1. Update MapReduceWorkflowConfig Structure

**File**: `src/config/mapreduce.rs`

Add environment fields to the struct:

```rust
pub struct MapReduceWorkflowConfig {
    /// Workflow name
    pub name: String,

    /// Workflow mode (should be "mapreduce")
    #[serde(default = "default_mode")]
    pub mode: String,

    /// Global environment variables for all commands
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,

    /// Secret environment variables (masked in logs)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secrets: Option<HashMap<String, SecretValue>>,

    /// Environment files to load (.env format)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env_files: Option<Vec<PathBuf>>,

    /// Environment profiles for different contexts
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profiles: Option<HashMap<String, EnvProfile>>,

    /// Optional setup phase
    #[serde(skip_serializing_if = "Option::is_none")]
    pub setup: Option<SetupPhaseConfig>,

    /// Map phase configuration
    pub map: MapPhaseYaml,

    /// Optional reduce phase
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reduce: Option<ReducePhaseYaml>,

    /// Workflow-level error handling policy
    #[serde(default, skip_serializing_if = "is_default_error_policy")]
    pub error_policy: WorkflowErrorPolicy,

    // ... other existing fields
}
```

#### 2. Environment Variable Propagation

**Approach**: Environment variables defined at the workflow level should be available to all phases (setup, map, reduce, merge).

**Implementation**:
- When executing setup commands, merge workflow env with shell environment
- When creating map agents, pass environment variables to each agent's execution context
- When executing reduce commands, apply environment variables
- When executing merge commands, apply environment variables

#### 3. Variable Interpolation

**Syntax Support**:
- `$VAR_NAME` - Simple variable reference
- `${VAR_NAME}` - Braced variable reference (preferred for clarity)
- `${VAR_NAME:-default}` - Variable with default value (future enhancement)

**Example Usage**:
```yaml
env:
  PROJECT_NAME: "Prodigy"
  PROJECT_CONFIG: ".prodigy/book-config.json"
  FEATURES_PATH: ".prodigy/book-analysis/features.json"

setup:
  - claude: "/prodigy-analyze-features-for-book --project $PROJECT_NAME --config $PROJECT_CONFIG"

map:
  agent_template:
    - claude: "/prodigy-analyze-book-chapter-drift --project $PROJECT_NAME --json '${item}' --features $FEATURES_PATH"
```

#### 4. Secret Masking

**Implementation**: Reuse existing secret masking infrastructure from standard workflows

**Behavior**:
- Secret values defined in `secrets:` field are masked in all output
- Secrets are available as environment variables to commands
- Secrets are not shown in logs or debug output

**Example**:
```yaml
env:
  API_URL: "https://api.example.com"

secrets:
  API_KEY: "${env:SECRET_API_KEY}"  # Read from system environment

setup:
  - shell: "curl -H 'Authorization: Bearer $API_KEY' $API_URL/data"
    # Output shows: curl -H 'Authorization: Bearer ***MASKED***' https://api.example.com/data
```

### Architecture Changes

**Modified Files**:
- `src/config/mapreduce.rs` - Add env fields to MapReduceWorkflowConfig
- `src/cook/execution/mapreduce/mod.rs` - Pass environment to phases
- `src/cook/execution/mapreduce/setup.rs` - Apply env in setup phase
- `src/cook/execution/mapreduce/map.rs` - Pass env to map agents
- `src/cook/execution/mapreduce/reduce.rs` - Apply env in reduce phase
- `src/cook/execution/mapreduce/merge.rs` - Apply env in merge phase

**New Files**:
None - reuses existing environment infrastructure from standard workflows

### Data Structures

**No new types needed** - reuse existing:
- `HashMap<String, String>` for env
- `SecretValue` for secrets (already defined in `src/cook/environment.rs`)
- `EnvProfile` for profiles (already defined in `src/cook/environment.rs`)

### Integration Points

**Environment Module** (`src/cook/environment.rs`):
- Already handles variable interpolation
- Already handles secret masking
- Already handles profile management
- MapReduce executor needs to call these utilities

**Execution Flow**:
1. Parse MapReduce workflow YAML
2. Load environment variables (env, secrets, env_files, profiles)
3. Merge with system environment
4. Pass environment context to setup executor
5. Pass environment context to each map agent
6. Pass environment context to reduce executor
7. Pass environment context to merge executor

## Dependencies

**Prerequisites**: None - environment infrastructure already exists

**Affected Components**:
- MapReduce workflow parser (add new fields)
- MapReduce executors (use environment variables)
- All MapReduce phases (setup, map, reduce, merge)

**External Dependencies**: None

## Testing Strategy

### Unit Tests

**Test 1: Config Parsing**
```rust
#[test]
fn test_mapreduce_with_env() {
    let yaml = r#"
name: test-workflow
mode: mapreduce
env:
  PROJECT_NAME: "Test"
  CONFIG_PATH: ".test/config.json"
map:
  input: "items.json"
  json_path: "$.items[*]"
  agent_template:
    - shell: "echo $PROJECT_NAME"
"#;
    let config: MapReduceWorkflowConfig = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(config.env.unwrap().get("PROJECT_NAME"), Some(&"Test".to_string()));
}
```

**Test 2: Variable Interpolation**
- Test `$VAR` syntax
- Test `${VAR}` syntax
- Test undefined variable handling
- Test variable in different phases

**Test 3: Secret Masking**
- Test secret values are masked in logs
- Test secret values are available to commands
- Test secret values work in all phases

### Integration Tests

**Test 1: Environment in Setup Phase**
- Define env vars in workflow
- Reference in setup commands
- Verify variables are available

**Test 2: Environment in Map Phase**
- Define env vars in workflow
- Reference in agent_template commands
- Verify each agent has access to env vars
- Verify variables work alongside ${item}

**Test 3: Environment in Reduce Phase**
- Define env vars in workflow
- Reference in reduce commands
- Verify variables are available

**Test 4: Environment in Merge Phase**
- Define env vars in workflow
- Reference in merge commands
- Verify variables are available

**Test 5: Book Documentation Workflow**
- Run book-docs-drift.yml with env vars
- Verify PROJECT_NAME, PROJECT_CONFIG, FEATURES_PATH work
- Verify commands receive correct parameters

**Test 6: Backward Compatibility**
- Run existing MapReduce workflows without env field
- Verify they work identically
- No errors or warnings

### Performance Tests

**Test 1: Workflow Without Env Vars**
- Baseline: existing workflow without env
- Modified: same workflow with env parsing (empty)
- Verify no performance degradation

**Test 2: Large Number of Env Vars**
- Define 100+ environment variables
- Verify interpolation remains fast
- Check memory usage is reasonable

## Documentation Requirements

### Code Documentation
- Document env fields in MapReduceWorkflowConfig
- Add examples to docstrings
- Explain variable interpolation syntax

### User Documentation

**Update**: `CLAUDE.md` section on MapReduce workflows
```markdown
## MapReduce Environment Variables

MapReduce workflows support environment variables just like standard workflows:

```yaml
name: example-mapreduce
mode: mapreduce

env:
  PROJECT_NAME: "MyProject"
  CONFIG_PATH: ".myproject/config.json"
  OUTPUT_DIR: ".myproject/output"

setup:
  - claude: "/analyze --project $PROJECT_NAME --config $CONFIG_PATH"

map:
  input: "items.json"
  json_path: "$.items[*]"
  agent_template:
    - claude: "/process --project $PROJECT_NAME --item '${item}'"

reduce:
  - claude: "/aggregate --project $PROJECT_NAME --output $OUTPUT_DIR"
```

Environment variables are available in all phases: setup, map, reduce, and merge.
```

**Update**: `workflows/README.md` with MapReduce environment examples

**New Example**: `workflows/mapreduce-with-env-example.yml`
```yaml
name: mapreduce-environment-example
mode: mapreduce

env:
  PROJECT: "ExampleProject"
  PARALLELISM: "5"
  OUTPUT_FORMAT: "json"

setup:
  - shell: "echo 'Processing $PROJECT with parallelism=$PARALLELISM'"

map:
  input: "data/items.json"
  json_path: "$.items[*]"
  max_parallel: 5

  agent_template:
    - shell: "echo 'Processing ${item.name} for $PROJECT'"
    - claude: "/transform --project $PROJECT --format $OUTPUT_FORMAT"

reduce:
  - shell: "echo 'Aggregating results for $PROJECT'"
```

## Implementation Notes

### Reuse Existing Infrastructure

**Don't reinvent the wheel**: Standard workflows already have:
- Variable interpolation logic (`src/cook/environment/variables.rs`)
- Secret masking (`src/cook/environment/secrets.rs`)
- Profile management (`src/cook/environment/profiles.rs`)
- Environment merging and precedence

**MapReduce should reuse these** rather than duplicating logic.

### Execution Context

Each phase executor needs access to the environment context:

```rust
pub struct ExecutionContext {
    pub env: HashMap<String, String>,
    pub secrets: HashMap<String, SecretValue>,
    // ... other context
}
```

This context is created once at workflow start and passed to all executors.

### Variable Scope

**Global scope**: Variables defined in workflow `env:` field
**Command scope**: Variables defined in individual command `env:` field (if supported)
**System scope**: Variables from system environment

**Precedence** (highest to lowest):
1. Command-level env
2. Workflow-level env
3. System environment

### Common Pitfalls

**Pitfall 1**: Forgetting to pass env to map agents
- **Risk**: Map agents don't have access to workflow env vars
- **Mitigation**: Pass env context when creating each agent's execution environment

**Pitfall 2**: Variable interpolation in agent_template
- **Risk**: `${item}` interpolation conflicts with `${VAR}` interpolation
- **Mitigation**: Interpolate workflow vars first, then map-specific vars

**Pitfall 3**: Secret masking in map agent logs
- **Risk**: Secrets leaked in distributed agent logs
- **Mitigation**: Ensure secret masking is applied in each agent's log output

## Migration and Compatibility

### Breaking Changes
None - all changes are additive

### Migration Path

**Existing workflows**: Continue to work without changes
**New workflows**: Can opt-in to environment variables

**Example migration**:

**Before** (hardcoded values):
```yaml
name: my-workflow
mode: mapreduce
setup:
  - claude: "/analyze --project Prodigy --config .prodigy/config.json"
```

**After** (parameterized):
```yaml
name: my-workflow
mode: mapreduce

env:
  PROJECT_NAME: "Prodigy"
  PROJECT_CONFIG: ".prodigy/config.json"

setup:
  - claude: "/analyze --project $PROJECT_NAME --config $PROJECT_CONFIG"
```

### Backward Compatibility

**All fields optional**: Environment fields default to None
**No behavior change**: Workflows without env field work identically
**Opt-in feature**: Only workflows that define env fields get env var support

## Success Metrics

**Functionality**:
- Environment variables work in all MapReduce phases
- Book documentation workflows (Specs 118-119) work with env vars
- Secret masking works correctly
- All existing MapReduce workflows continue to work

**Quality**:
- Environment behavior matches standard workflows
- Error messages are clear and helpful
- Performance impact is negligible
- Code reuses existing infrastructure (minimal duplication)

**Usability**:
- Developers can easily parameterize MapReduce workflows
- Multiple projects can use same workflow with different env vars
- Environment variable syntax is intuitive
