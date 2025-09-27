---
number: 112
title: Setup Phase Variable Capture Implementation
category: functionality
priority: important
status: draft
dependencies: []
created: 2025-09-27
---

# Specification 112: Setup Phase Variable Capture Implementation

## Context

The current MapReduce implementation includes a `capture_outputs` field in the setup phase configuration but this functionality is not implemented. This prevents users from capturing variables from setup commands for use in subsequent map and reduce phases, limiting the flexibility of workflow orchestration.

Current gaps:
- `capture_outputs` field exists in configuration but is ignored during execution
- No mechanism to capture command outputs as variables
- Missing variable interpolation for captured setup variables
- No validation of capture configuration
- Setup phase results are not preserved for later phases

This functionality is important for workflows that need to:
- Generate dynamic input data during setup
- Configure map phase parameters based on setup results
- Pass setup-time calculations to agent templates
- Create workflow-specific variables for reduce phase

## Objective

Implement comprehensive setup phase variable capture functionality that allows users to capture command outputs as variables and use them throughout the MapReduce workflow execution.

## Requirements

### Functional Requirements

#### Variable Capture Configuration
- Support capture_outputs mapping in setup phase YAML configuration
- Allow mapping command indices to variable names
- Support both stdout and stderr capture options
- Enable selective line/pattern capture from command output
- Validate capture configuration during workflow parsing

#### Output Capture Mechanisms
- Capture complete stdout from specified setup commands
- Optionally capture stderr as separate variables
- Support JSON parsing of captured output
- Enable regex-based pattern extraction from output
- Handle multi-line output with formatting options

#### Variable Availability
- Make captured variables available in map phase agent templates
- Support variable interpolation in reduce phase commands
- Enable use of setup variables in conditional logic
- Preserve captured variables in checkpoints and resume
- Support variable scoping and namespacing

#### Integration with Existing Variables
- Integrate with existing variable interpolation system
- Support setup variables alongside item, map, and shell variables
- Enable variable transformation and formatting
- Support default values for failed captures
- Handle variable conflicts and precedence

### Non-Functional Requirements
- Variable capture should add minimal overhead to setup execution
- Captured output should be size-limited to prevent memory issues
- Variable interpolation should be performed efficiently
- Support for large output captures with streaming/chunking
- Clear error messages for capture failures

## Acceptance Criteria

- [ ] Setup phase `capture_outputs` configuration is parsed and validated
- [ ] Command outputs are captured according to configuration
- [ ] Captured variables are available in map phase agent templates
- [ ] Setup variables work with existing variable interpolation syntax
- [ ] Error handling provides clear feedback for capture failures
- [ ] Variable capture preserves values in checkpoints and resume
- [ ] Documentation includes examples of variable capture usage
- [ ] Performance impact is measurable and acceptable

## Technical Details

### Implementation Approach

#### 1. Enhanced Setup Phase Configuration

Extend the existing setup configuration to support detailed capture options:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupPhaseConfig {
    /// Commands to execute during setup
    pub commands: Vec<WorkflowStep>,

    /// Timeout for the entire setup phase (in seconds)
    pub timeout: u64,

    /// Variables to capture from setup commands
    #[serde(default)]
    pub capture_outputs: HashMap<String, CaptureConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CaptureConfig {
    /// Simple capture: command index only
    Simple(usize),
    /// Detailed capture configuration
    Detailed {
        /// Command index to capture from
        command_index: usize,
        /// What to capture (stdout, stderr, or both)
        #[serde(default = "default_capture_source")]
        source: CaptureSource,
        /// Optional regex pattern to extract
        pattern: Option<String>,
        /// Optional JSON path for JSON output
        json_path: Option<String>,
        /// Maximum output size to capture (bytes)
        #[serde(default = "default_max_capture_size")]
        max_size: usize,
        /// Default value if capture fails
        default: Option<String>,
        /// How to handle multi-line output
        #[serde(default)]
        multiline: MultilineHandling,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CaptureSource {
    Stdout,
    Stderr,
    Both,
    Combined,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MultilineHandling {
    /// Keep all lines as single string with newlines
    Preserve,
    /// Join lines with spaces
    Join,
    /// Take only first line
    FirstLine,
    /// Take only last line
    LastLine,
    /// Return as array of lines
    Array,
}

fn default_capture_source() -> CaptureSource {
    CaptureSource::Stdout
}

fn default_max_capture_size() -> usize {
    1024 * 1024 // 1MB default limit
}

impl Default for MultilineHandling {
    fn default() -> Self {
        MultilineHandling::Preserve
    }
}
```

#### 2. Variable Capture Engine

Create a dedicated service for capturing and processing command outputs:

```rust
pub struct VariableCaptureEngine {
    config: HashMap<String, CaptureConfig>,
    captured_variables: HashMap<String, CapturedVariable>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapturedVariable {
    pub name: String,
    pub value: serde_json::Value,
    pub source_command: usize,
    pub captured_at: DateTime<Utc>,
    pub metadata: CaptureMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaptureMetadata {
    pub source: CaptureSource,
    pub original_size: usize,
    pub truncated: bool,
    pub pattern_matched: bool,
    pub parsing_successful: bool,
}

impl VariableCaptureEngine {
    pub fn new(capture_config: HashMap<String, CaptureConfig>) -> Self {
        Self {
            config: capture_config,
            captured_variables: HashMap::new(),
        }
    }

    pub async fn capture_from_command(
        &mut self,
        command_index: usize,
        command_result: &CommandResult,
    ) -> Result<(), CaptureError> {
        for (var_name, capture_config) in &self.config {
            if self.should_capture_from_command(capture_config, command_index) {
                let captured = self.perform_capture(var_name, capture_config, command_result).await?;
                self.captured_variables.insert(var_name.clone(), captured);
            }
        }
        Ok(())
    }

    async fn perform_capture(
        &self,
        var_name: &str,
        config: &CaptureConfig,
        command_result: &CommandResult,
    ) -> Result<CapturedVariable, CaptureError> {
        let (command_index, source, pattern, json_path, max_size, default, multiline) =
            self.extract_capture_params(config);

        // Get raw output based on source
        let raw_output = self.get_output_by_source(&source, command_result)?;

        // Apply size limit
        let limited_output = self.apply_size_limit(&raw_output, max_size);

        // Apply pattern extraction if specified
        let pattern_output = if let Some(pattern) = pattern {
            self.apply_pattern_extraction(&limited_output, pattern)?
        } else {
            limited_output
        };

        // Handle multiline processing
        let processed_output = self.handle_multiline(&pattern_output, &multiline);

        // Parse JSON if json_path is specified
        let final_value = if let Some(json_path) = json_path {
            self.extract_json_value(&processed_output, json_path)?
        } else {
            serde_json::Value::String(processed_output)
        };

        Ok(CapturedVariable {
            name: var_name.to_string(),
            value: final_value,
            source_command: command_index,
            captured_at: Utc::now(),
            metadata: CaptureMetadata {
                source: source.clone(),
                original_size: raw_output.len(),
                truncated: raw_output.len() > max_size,
                pattern_matched: pattern.is_some(),
                parsing_successful: true,
            },
        })
    }

    fn get_output_by_source(
        &self,
        source: &CaptureSource,
        result: &CommandResult,
    ) -> Result<String, CaptureError> {
        match source {
            CaptureSource::Stdout => Ok(result.stdout.clone()),
            CaptureSource::Stderr => Ok(result.stderr.clone()),
            CaptureSource::Both => Ok(format!("stdout: {}\nstderr: {}", result.stdout, result.stderr)),
            CaptureSource::Combined => {
                // Interleave stdout and stderr based on timestamps if available
                Ok(format!("{}{}", result.stdout, result.stderr))
            }
        }
    }

    fn apply_pattern_extraction(&self, input: &str, pattern: &str) -> Result<String, CaptureError> {
        let regex = regex::Regex::new(pattern)
            .map_err(|e| CaptureError::InvalidPattern(pattern.to_string(), e))?;

        if let Some(captures) = regex.captures(input) {
            // If there are capture groups, use the first one, otherwise use the whole match
            if captures.len() > 1 {
                Ok(captures.get(1).unwrap().as_str().to_string())
            } else {
                Ok(captures.get(0).unwrap().as_str().to_string())
            }
        } else {
            Err(CaptureError::PatternNotMatched(pattern.to_string()))
        }
    }

    fn handle_multiline(&self, input: &str, handling: &MultilineHandling) -> String {
        match handling {
            MultilineHandling::Preserve => input.to_string(),
            MultilineHandling::Join => input.lines().collect::<Vec<_>>().join(" "),
            MultilineHandling::FirstLine => input.lines().next().unwrap_or("").to_string(),
            MultilineHandling::LastLine => input.lines().last().unwrap_or("").to_string(),
            MultilineHandling::Array => {
                // For array handling, we'll return a JSON array as string
                let lines: Vec<&str> = input.lines().collect();
                serde_json::to_string(&lines).unwrap_or_else(|_| input.to_string())
            }
        }
    }

    fn extract_json_value(&self, input: &str, json_path: &str) -> Result<serde_json::Value, CaptureError> {
        let data: serde_json::Value = serde_json::from_str(input)
            .map_err(|e| CaptureError::JsonParseError(e))?;

        let selector = jsonpath_rust::JsonPathFinder::from_str(input, json_path)
            .map_err(|e| CaptureError::JsonPathError(json_path.to_string(), e.to_string()))?;

        let result = selector.find();
        match result.len() {
            0 => Err(CaptureError::JsonPathNoMatch(json_path.to_string())),
            1 => Ok(result[0].clone()),
            _ => Ok(serde_json::Value::Array(result)),
        }
    }

    pub fn get_captured_variables(&self) -> &HashMap<String, CapturedVariable> {
        &self.captured_variables
    }

    pub fn get_variable_value(&self, name: &str) -> Option<&serde_json::Value> {
        self.captured_variables.get(name).map(|v| &v.value)
    }
}
```

#### 3. Setup Phase Executor Integration

Integrate variable capture with the existing setup phase executor:

```rust
impl SetupPhaseExecutor {
    pub async fn execute(&self, context: &mut PhaseContext) -> Result<PhaseResult, PhaseError> {
        let mut variable_capture = VariableCaptureEngine::new(
            self.setup_phase.capture_outputs.clone()
        );

        let mut command_results = Vec::new();
        let mut captured_variables = HashMap::new();

        // Execute commands and capture outputs
        for (index, command) in self.setup_phase.commands.iter().enumerate() {
            let result = self.execute_command(command, context).await?;

            // Attempt variable capture for this command
            if let Err(capture_error) = variable_capture.capture_from_command(index, &result).await {
                tracing::warn!("Failed to capture variables from command {}: {}", index, capture_error);
                // Continue execution unless capture is critical
            }

            command_results.push(result);
        }

        // Store captured variables in context for use in subsequent phases
        context.setup_variables = variable_capture.get_captured_variables().clone();

        // Create setup phase result with captured variables
        Ok(PhaseResult {
            phase_type: PhaseType::Setup,
            success: true,
            data: Some(serde_json::to_value(&command_results)?),
            error_message: None,
            metrics: PhaseMetrics {
                duration: self.start_time.elapsed(),
                items_processed: self.setup_phase.commands.len(),
                memory_used: 0, // TODO: Implement memory tracking
                variables_captured: variable_capture.get_captured_variables().len(),
            },
        })
    }
}
```

#### 4. Variable Interpolation Integration

Extend the existing variable interpolation system to support setup variables:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableContext {
    pub item_variables: HashMap<String, serde_json::Value>,
    pub setup_variables: HashMap<String, CapturedVariable>,
    pub map_variables: HashMap<String, serde_json::Value>,
    pub shell_variables: HashMap<String, serde_json::Value>,
    pub merge_variables: HashMap<String, serde_json::Value>,
}

impl VariableContext {
    pub fn resolve_variable(&self, name: &str) -> Option<serde_json::Value> {
        // Variable resolution precedence:
        // 1. Item variables (highest precedence for map phase)
        // 2. Shell variables (command-specific)
        // 3. Setup variables
        // 4. Map variables
        // 5. Merge variables

        if let Some(value) = self.item_variables.get(name) {
            return Some(value.clone());
        }

        if let Some(value) = self.shell_variables.get(name) {
            return Some(value.clone());
        }

        if let Some(captured_var) = self.setup_variables.get(name) {
            return Some(captured_var.value.clone());
        }

        if let Some(value) = self.map_variables.get(name) {
            return Some(value.clone());
        }

        if let Some(value) = self.merge_variables.get(name) {
            return Some(value.clone());
        }

        None
    }

    pub fn interpolate_string(&self, template: &str) -> Result<String, InterpolationError> {
        let mut result = template.to_string();

        // Find all variable references: ${variable_name}
        let re = regex::Regex::new(r"\$\{([^}]+)\}").unwrap();

        for captures in re.captures_iter(template) {
            let full_match = &captures[0];
            let var_name = &captures[1];

            if let Some(value) = self.resolve_variable(var_name) {
                let replacement = self.value_to_string(&value)?;
                result = result.replace(full_match, &replacement);
            } else {
                return Err(InterpolationError::VariableNotFound(var_name.to_string()));
            }
        }

        Ok(result)
    }

    fn value_to_string(&self, value: &serde_json::Value) -> Result<String, InterpolationError> {
        match value {
            serde_json::Value::String(s) => Ok(s.clone()),
            serde_json::Value::Number(n) => Ok(n.to_string()),
            serde_json::Value::Bool(b) => Ok(b.to_string()),
            serde_json::Value::Null => Ok("null".to_string()),
            serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
                // For complex types, serialize to JSON
                serde_json::to_string(value)
                    .map_err(|e| InterpolationError::SerializationError(e))
            }
        }
    }
}
```

#### 5. YAML Configuration Examples

Support various capture configuration patterns:

```yaml
name: example-with-variable-capture
mode: mapreduce

setup:
  commands:
    - shell: "echo 'Processing started at $(date)'"
    - shell: "find . -name '*.rs' | wc -l"
    - shell: "curl -s https://api.github.com/repos/owner/repo | jq '.stargazers_count'"
    - claude: "/analyze-codebase --output-json"

  capture_outputs:
    # Simple capture - just command index
    timestamp: 0

    # Capture with pattern extraction
    file_count:
      command_index: 1
      pattern: "^(\\d+)\\s"
      multiline: first_line

    # JSON capture
    star_count:
      command_index: 2
      source: stdout
      max_size: 1024

    # Complex capture with JSON path
    analysis_result:
      command_index: 3
      source: stdout
      json_path: "$.recommendations[*].priority"
      default: "[]"

map:
  input: "items.json"
  json_path: "$.items[*]"

  agent_template:
    - shell: "echo 'Processing ${item.name} at ${timestamp}'"
    - shell: "echo 'Total files in project: ${file_count}'"
    - claude: "/process-item '${item}' --priority ${analysis_result}"

reduce:
  - claude: "/generate-summary --file-count ${file_count} --results ${map.results}"
```

### Error Handling

```rust
#[derive(Debug, thiserror::Error)]
pub enum CaptureError {
    #[error("Invalid regex pattern '{0}': {1}")]
    InvalidPattern(String, regex::Error),

    #[error("Pattern '{0}' did not match any text")]
    PatternNotMatched(String),

    #[error("JSON parsing failed: {0}")]
    JsonParseError(#[from] serde_json::Error),

    #[error("JSONPath '{0}' error: {1}")]
    JsonPathError(String, String),

    #[error("JSONPath '{0}' matched no values")]
    JsonPathNoMatch(String),

    #[error("Command index {0} out of range")]
    CommandIndexOutOfRange(usize),

    #[error("Output size {0} exceeds limit {1}")]
    OutputSizeExceeded(usize, usize),

    #[error("Variable '{0}' already captured")]
    VariableAlreadyCaptured(String),
}

#[derive(Debug, thiserror::Error)]
pub enum InterpolationError {
    #[error("Variable '{0}' not found")]
    VariableNotFound(String),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Circular variable reference detected")]
    CircularReference,
}
```

## Testing Strategy

### Unit Tests
- Test variable capture configuration parsing
- Test output capture with various sources and patterns
- Test JSON parsing and JSONPath extraction
- Test variable interpolation with setup variables
- Test error handling for capture failures

### Integration Tests
- Test end-to-end setup phase with variable capture
- Test setup variables in map phase agent templates
- Test setup variables in reduce phase commands
- Test checkpoint preservation of captured variables
- Test variable precedence and conflict resolution

### Performance Tests
- Benchmark variable capture overhead vs. setup execution time
- Test memory usage with large captured outputs
- Test interpolation performance with many variables
- Test concurrent variable access during map phase

### Validation Tests
- Test capture configuration validation
- Test variable name conflicts and resolution
- Test pattern extraction accuracy
- Test JSON parsing edge cases

## Migration Strategy

### Phase 1: Core Capture Infrastructure
1. Implement `VariableCaptureEngine` and capture configuration parsing
2. Add output capture mechanisms for different sources
3. Implement pattern extraction and JSON processing

### Phase 2: Integration with Setup Phase
1. Integrate capture engine with setup phase executor
2. Add captured variables to phase context
3. Implement error handling and validation

### Phase 3: Variable Interpolation
1. Extend variable interpolation system for setup variables
2. Add variable precedence and conflict resolution
3. Update map and reduce phases to use setup variables

### Phase 4: Advanced Features
1. Add variable transformation and formatting options
2. Implement variable validation and type checking
3. Add monitoring and debugging for variable capture

## Documentation Requirements

- Update setup phase documentation with capture_outputs examples
- Document variable interpolation syntax including setup variables
- Create troubleshooting guide for capture failures
- Add best practices for variable capture and usage
- Document variable precedence and scoping rules

## Risk Assessment

### High Risk
- **Capture Failures**: Failed variable captures might break subsequent phases
- **Memory Usage**: Large captured outputs might cause memory issues
- **Variable Conflicts**: Name conflicts might cause unexpected behavior

### Medium Risk
- **Performance Impact**: Variable capture might slow down setup phase
- **Configuration Complexity**: Complex capture configs might be error-prone
- **Security Concerns**: Captured outputs might contain sensitive information

### Mitigation Strategies
- Implement size limits and validation for captured outputs
- Provide clear error messages and default value fallbacks
- Add configuration validation during workflow parsing
- Implement variable namespacing to avoid conflicts
- Add logging and monitoring for capture operations