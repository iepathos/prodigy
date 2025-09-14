---
number: 66
title: Variable Capture and Output Management
category: foundation
priority: critical
status: draft
dependencies: []
created: 2025-01-14
---

# Specification 66: Variable Capture and Output Management

**Category**: foundation
**Priority**: critical
**Status**: draft
**Dependencies**: None

## Context

The whitepaper shows variable capture as a fundamental feature:
```yaml
tasks:
  - name: "Build"
    shell: "npm build"
    capture: build_output

  - name: "Deploy"
    when: "${build_output.success}"
    shell: "npm deploy"
```

Currently, variable capture and output management is incomplete. Commands can execute but their outputs aren't properly captured into variables for use in subsequent steps, limiting workflow flexibility and decision-making capabilities.

## Objective

Implement comprehensive variable capture and output management system that enables workflows to capture command outputs, parse structured data, and use these variables throughout the workflow execution.

## Requirements

### Functional Requirements
- Capture stdout/stderr from shell commands into variables
- Parse JSON output automatically when detected
- Support structured field access: `${var.field.subfield}`
- Capture exit codes as `${command.exit_code}`
- Capture execution time as `${command.duration}`
- Enable output redirection to files
- Support multiple capture formats (string, json, lines)
- Variable persistence across workflow steps
- Variable interpolation in all command types
- MapReduce result aggregation into variables

### Non-Functional Requirements
- Efficient memory usage for large outputs
- Thread-safe variable access
- Clear error messages for undefined variables
- Minimal performance overhead

## Acceptance Criteria

- [ ] `capture: var_name` stores command output in variable
- [ ] `${var_name}` interpolates captured value in subsequent steps
- [ ] `${var.field}` accesses JSON fields when output is JSON
- [ ] `${var.exit_code}` provides command exit code
- [ ] `${var.success}` provides boolean success status
- [ ] `${var.duration}` provides execution time
- [ ] `capture_format: json` parses output as JSON
- [ ] `capture_format: lines` splits output into array
- [ ] Variables persist across workflow steps
- [ ] MapReduce results accessible as `${map.results}`
- [ ] Clear errors for undefined variable access

## Technical Details

### Implementation Approach

1. **Enhanced Workflow Step with Capture**:
   ```rust
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct WorkflowStep {
       #[serde(flatten)]
       pub command: CommandType,

       /// Variable name to capture output
       #[serde(skip_serializing_if = "Option::is_none")]
       pub capture: Option<String>,

       /// Format for captured output
       #[serde(skip_serializing_if = "Option::is_none")]
       pub capture_format: Option<CaptureFormat>,

       /// Fields to capture (stdout, stderr, both)
       #[serde(default)]
       pub capture_streams: CaptureStreams,

       /// Output file for command results
       #[serde(skip_serializing_if = "Option::is_none")]
       pub output_file: Option<PathBuf>,
   }

   #[derive(Debug, Clone, Serialize, Deserialize)]
   #[serde(rename_all = "snake_case")]
   pub enum CaptureFormat {
       String,     // Raw string output
       Json,       // Parse as JSON
       Lines,      // Split into array of lines
       Number,     // Parse as number
       Boolean,    // Parse as boolean
   }

   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct CaptureStreams {
       #[serde(default = "default_true")]
       pub stdout: bool,
       #[serde(default)]
       pub stderr: bool,
       #[serde(default = "default_true")]
       pub exit_code: bool,
       #[serde(default = "default_true")]
       pub duration: bool,
   }
   ```

2. **Variable Storage and Access**:
   ```rust
   pub struct VariableStore {
       variables: Arc<RwLock<HashMap<String, CapturedValue>>>,
       parent: Option<Arc<VariableStore>>, // For nested scopes
   }

   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub enum CapturedValue {
       String(String),
       Number(f64),
       Boolean(bool),
       Json(serde_json::Value),
       Array(Vec<CapturedValue>),
       Object(HashMap<String, CapturedValue>),
       CommandResult {
           stdout: Option<String>,
           stderr: Option<String>,
           exit_code: i32,
           success: bool,
           duration: Duration,
       },
   }

   impl VariableStore {
       pub async fn capture_command_result(
           &self,
           name: &str,
           result: CommandResult,
           format: CaptureFormat,
       ) -> Result<()> {
           let value = match format {
               CaptureFormat::String => {
                   CapturedValue::String(result.stdout.unwrap_or_default())
               }
               CaptureFormat::Json => {
                   let json: serde_json::Value =
                       serde_json::from_str(&result.stdout.unwrap_or("null"))?;
                   CapturedValue::Json(json)
               }
               CaptureFormat::Lines => {
                   let lines = result.stdout
                       .unwrap_or_default()
                       .lines()
                       .map(|s| CapturedValue::String(s.to_string()))
                       .collect();
                   CapturedValue::Array(lines)
               }
               CaptureFormat::Number => {
                   let num = result.stdout
                       .unwrap_or_default()
                       .trim()
                       .parse::<f64>()?;
                   CapturedValue::Number(num)
               }
               CaptureFormat::Boolean => {
                   let val = result.stdout
                       .unwrap_or_default()
                       .trim()
                       .parse::<bool>()
                       .or_else(|_| Ok(result.exit_code == 0))?;
                   CapturedValue::Boolean(val)
               }
           };

           // Store the formatted value
           self.set(name, value).await;

           // Also store metadata fields
           self.set(&format!("{}.exit_code", name),
                   CapturedValue::Number(result.exit_code as f64)).await;
           self.set(&format!("{}.success", name),
                   CapturedValue::Boolean(result.success)).await;
           self.set(&format!("{}.duration", name),
                   CapturedValue::Number(result.duration.as_secs_f64())).await;

           Ok(())
       }

       pub async fn resolve_variable(&self, path: &str) -> Result<CapturedValue> {
           let parts: Vec<&str> = path.split('.').collect();

           // Look up base variable
           let base_value = self.get(&parts[0]).await?;

           // Navigate nested path
           let mut current = base_value;
           for part in &parts[1..] {
               current = match current {
                   CapturedValue::Json(ref obj) => {
                       obj.get(*part)
                           .ok_or_else(|| anyhow!("Field {} not found", part))?
                           .clone()
                           .into()
                   }
                   CapturedValue::Object(ref map) => {
                       map.get(*part)
                           .ok_or_else(|| anyhow!("Field {} not found", part))?
                           .clone()
                   }
                   _ => return Err(anyhow!("Cannot access field {} on non-object", part)),
               };
           }

           Ok(current)
       }
   }
   ```

3. **Variable Interpolation Engine**:
   ```rust
   pub struct VariableInterpolator {
       store: Arc<VariableStore>,
       pattern: Regex,
   }

   impl VariableInterpolator {
       pub async fn interpolate(&self, template: &str) -> Result<String> {
           let mut result = template.to_string();

           // Find all variable references
           for cap in self.pattern.captures_iter(template) {
               let var_path = &cap[1];

               // Resolve variable value
               let value = self.store.resolve_variable(var_path).await?;

               // Convert to string for interpolation
               let str_value = match value {
                   CapturedValue::String(s) => s,
                   CapturedValue::Number(n) => n.to_string(),
                   CapturedValue::Boolean(b) => b.to_string(),
                   CapturedValue::Json(j) => serde_json::to_string(&j)?,
                   _ => format!("{:?}", value),
               };

               result = result.replace(&cap[0], &str_value);
           }

           Ok(result)
       }
   }
   ```

### Architecture Changes
- Add `VariableStore` to execution context
- Implement `VariableInterpolator` for all command types
- Enhance command executors to capture outputs
- Add variable resolution to conditional expressions
- Integrate with MapReduce result aggregation

### Data Structures
```yaml
# Example workflow with variable capture
tasks:
  - name: "Get version"
    shell: "cat package.json | jq -r .version"
    capture: version
    capture_format: string

  - name: "Build application"
    shell: "npm run build"
    capture: build
    capture_format: json
    capture_streams:
      stdout: true
      stderr: true

  - name: "Run tests"
    shell: "npm test"
    capture: test_results
    when: "${build.success}"

  - name: "Deploy"
    shell: "deploy.sh ${version}"
    when: "${test_results.exit_code} == 0"

  - name: "Process results"
    claude: "/analyze-metrics ${build.metrics} ${test_results.coverage}"
```

## Dependencies

- **Prerequisites**: None
- **Affected Components**:
  - `src/cook/workflow/variables.rs` - Variable management
  - `src/cook/execution/` - Output capture
  - `src/config/workflow.rs` - Capture configuration
- **External Dependencies**: `regex` for pattern matching

## Testing Strategy

- **Unit Tests**:
  - Variable storage and retrieval
  - Nested path resolution
  - Format conversions
  - Interpolation patterns
- **Integration Tests**:
  - End-to-end variable capture
  - Cross-step variable usage
  - MapReduce result aggregation
  - Complex JSON navigation
- **Edge Cases**:
  - Large output handling
  - Binary output handling
  - Concurrent variable access
  - Circular variable references

## Documentation Requirements

- **Code Documentation**: Document capture formats and variable paths
- **User Documentation**:
  - Variable capture guide
  - Interpolation syntax reference
  - Common patterns and examples
- **Architecture Updates**: Add variable flow to execution diagrams

## Implementation Notes

- Use streaming for large outputs to avoid memory issues
- Consider variable scoping for parallel execution
- Implement variable debugging/inspection commands
- Support environment variable passthrough
- Future: Variable persistence across workflow runs

## Migration and Compatibility

- Workflows without capture continue to work
- No breaking changes to existing workflows
- Gradual adoption possible
- Clear upgrade path from static to dynamic workflows