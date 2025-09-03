---
number: 59
title: Input Abstraction
category: architecture
priority: high
status: draft
dependencies: [58]
created: 2025-09-03
---

# Specification 59: Input Abstraction

**Category**: architecture
**Priority**: high
**Status**: draft
**Dependencies**: [58 - Unified Execution Model]

## Context

The current workflow execution system handles inputs in different ways depending on the invocation method, leading to inconsistent behavior and complex code paths. The Architecture Assessment identified that workflows can be invoked with:

1. **No inputs** - Standard sequential execution
2. **Arguments** - Via `--args` flag with comma-separated values
3. **File patterns** - Via `--map` flag with glob patterns
4. **MapReduce data** - Complex structured data for parallel processing

Each input type currently requires different parsing, variable substitution, and execution logic scattered throughout the codebase. This creates several problems:

- **Inconsistent Variable Substitution**: Different input types use different template variable names and formats
- **Code Duplication**: Input parsing and processing logic repeated across execution paths  
- **Limited Composability**: Difficult to combine different input types or create hybrid workflows
- **Testing Complexity**: Each input type requires separate test scenarios and mock setups
- **Feature Gaps**: Some workflow features only work with certain input types

The unified execution model (Specification 58) provides the foundation for consistent execution, but requires a standardized input abstraction layer to achieve true consistency across all invocation modes.

## Objective

Create a unified input abstraction system that standardizes how workflows receive and process inputs regardless of source. This system should provide consistent variable substitution, composable input sources, and seamless integration with the unified execution model while maintaining backward compatibility with existing workflow configurations.

## Requirements

### Functional Requirements

#### Input Source Abstraction
- Support for empty inputs (standard workflows)
- Command-line arguments parsing and variable mapping
- File pattern expansion with metadata extraction
- Structured data inputs for MapReduce and complex workflows
- Environment variable inputs with validation
- Configuration file inputs with schema validation
- Standard input (stdin) processing for pipeline integration

#### Variable Substitution System
- Consistent variable naming across all input types
- Nested variable substitution with dependency resolution
- Input-specific variable namespaces to prevent conflicts
- Built-in helper functions for common transformations
- Type-aware variable formatting (string, number, boolean, path)
- Conditional variable substitution based on input properties

#### Input Validation and Transformation
- Schema validation for structured inputs
- Type coercion and formatting for command-line arguments
- File existence and permission validation for path inputs
- Range validation for numeric inputs
- Pattern matching for string inputs
- Custom validation rules via configuration

#### Composable Input Sources
- Ability to combine multiple input sources
- Input chaining and dependency relationships
- Input filtering and transformation pipelines
- Dynamic input generation based on previous outputs
- Input caching and reuse across workflow invocations

### Non-Functional Requirements

#### Performance
- Lazy evaluation of input processing
- Efficient memory usage for large input sets
- Streaming support for continuous input sources
- Parallel input processing where applicable
- Minimal overhead for simple input cases

#### Usability
- Intuitive variable naming conventions
- Clear error messages for input validation failures
- Helpful suggestions for common input patterns
- Comprehensive documentation and examples
- IDE support through schema definitions

#### Reliability
- Robust error handling for malformed inputs
- Graceful degradation for missing optional inputs
- Input sanitization to prevent injection attacks
- Consistent behavior across different platforms
- Comprehensive logging of input processing

## Acceptance Criteria

- [ ] Single InputProvider interface handles all input types
- [ ] Consistent variable substitution across all input sources
- [ ] Backward compatibility with existing `--args` and `--map` usage
- [ ] Support for complex structured inputs (JSON, YAML, TOML)
- [ ] Input validation with clear error messages
- [ ] Composable input sources with proper dependency handling
- [ ] Standard variable names available in all workflows
- [ ] Performance benchmarks show no regression for existing usage
- [ ] Comprehensive test coverage for all input types and combinations
- [ ] Documentation includes migration guide and best practices
- [ ] Integration with unified execution model is seamless
- [ ] IDE support through JSON schemas for input configurations

## Technical Details

### Implementation Approach

#### Phase 1: Core Input Abstraction

```rust
// src/cook/input/provider.rs
#[async_trait]
pub trait InputProvider: Send + Sync {
    /// Get the type of input this provider handles
    fn input_type(&self) -> InputType;
    
    /// Validate input configuration before processing
    async fn validate(&self, config: &InputConfig) -> Result<Vec<ValidationIssue>>;
    
    /// Generate execution inputs from the configuration
    async fn generate_inputs(&self, config: &InputConfig) -> Result<Vec<ExecutionInput>>;
    
    /// Get available variable names for this input type
    fn available_variables(&self, config: &InputConfig) -> Result<Vec<VariableDefinition>>;
    
    /// Check if this provider can handle the given configuration
    fn supports(&self, config: &InputConfig) -> bool;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionInput {
    pub id: String,
    pub input_type: InputType,
    pub variables: HashMap<String, VariableValue>,
    pub metadata: InputMetadata,
    pub dependencies: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InputType {
    Empty,
    Arguments { separator: Option<String> },
    FilePattern { patterns: Vec<String>, recursive: bool },
    StructuredData { format: DataFormat, schema: Option<String> },
    Environment { prefix: Option<String> },
    StandardInput { format: DataFormat },
    Generated { generator: String, config: serde_json::Value },
    Composite { sources: Vec<InputSource> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputMetadata {
    pub source: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub size_bytes: Option<u64>,
    pub checksum: Option<String>,
    pub content_type: Option<String>,
    pub custom_fields: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableDefinition {
    pub name: String,
    pub var_type: VariableType,
    pub description: String,
    pub required: bool,
    pub default_value: Option<String>,
    pub validation_rules: Vec<ValidationRule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VariableType {
    String,
    Number,
    Boolean,
    Path,
    Url,
    Array,
    Object,
}

impl ExecutionInput {
    pub fn new(id: String, input_type: InputType) -> Self {
        Self {
            id,
            input_type,
            variables: HashMap::new(),
            metadata: InputMetadata::default(),
            dependencies: Vec::new(),
        }
    }
    
    pub fn add_variable(&mut self, name: String, value: VariableValue) -> &mut Self {
        self.variables.insert(name, value);
        self
    }
    
    pub fn with_metadata(&mut self, metadata: InputMetadata) -> &mut Self {
        self.metadata = metadata;
        self
    }
    
    pub fn substitute_in_template(&self, template: &str) -> Result<String> {
        let mut result = template.to_string();
        
        // Standard variables available to all inputs
        result = result.replace("{input_id}", &self.id);
        result = result.replace("{input_type}", &format!("{:?}", self.input_type));
        
        // Input-specific variables
        for (key, value) in &self.variables {
            let placeholder = format!("{{{}}}", key);
            result = result.replace(&placeholder, &value.to_string());
        }
        
        // Apply helper functions
        result = self.apply_helper_functions(&result)?;
        
        Ok(result)
    }
    
    fn apply_helper_functions(&self, template: &str) -> Result<String> {
        let mut result = template.to_string();
        
        // Example: {file_path|basename} -> filename without path
        let re = regex::Regex::new(r"\{([^|}]+)\|([^}]+)\}")?;
        
        for capture in re.captures_iter(template) {
            let var_name = &capture[1];
            let function = &capture[2];
            let full_match = &capture[0];
            
            if let Some(value) = self.variables.get(var_name) {
                let transformed = self.apply_transformation(value, function)?;
                result = result.replace(full_match, &transformed);
            }
        }
        
        Ok(result)
    }
    
    fn apply_transformation(&self, value: &VariableValue, function: &str) -> Result<String> {
        match function {
            "basename" => Ok(PathBuf::from(value.to_string())
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()),
            "dirname" => Ok(PathBuf::from(value.to_string())
                .parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| ".".to_string())),
            "uppercase" => Ok(value.to_string().to_uppercase()),
            "lowercase" => Ok(value.to_string().to_lowercase()),
            "trim" => Ok(value.to_string().trim().to_string()),
            _ => Err(anyhow::anyhow!("Unknown transformation function: {}", function)),
        }
    }
}
```

#### Phase 2: Specific Input Providers

```rust
// src/cook/input/arguments.rs
pub struct ArgumentsInputProvider;

#[async_trait]
impl InputProvider for ArgumentsInputProvider {
    fn input_type(&self) -> InputType {
        InputType::Arguments { separator: Some(",".to_string()) }
    }
    
    async fn generate_inputs(&self, config: &InputConfig) -> Result<Vec<ExecutionInput>> {
        let args_str = config.get_string("args")?;
        let separator = config.get_string("separator").unwrap_or_else(|_| ",".to_string());
        
        let arguments: Vec<String> = args_str
            .split(&separator)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        
        let mut inputs = Vec::new();
        
        for (index, arg) in arguments.iter().enumerate() {
            let mut input = ExecutionInput::new(
                format!("arg_{}", index),
                InputType::Arguments { separator: Some(separator.clone()) }
            );
            
            // Standard argument variables
            input.add_variable("arg".to_string(), VariableValue::String(arg.clone()));
            input.add_variable("arg_index".to_string(), VariableValue::Number(index as i64));
            input.add_variable("arg_count".to_string(), VariableValue::Number(arguments.len() as i64));
            
            // Try to parse as key=value pair
            if let Some((key, value)) = arg.split_once('=') {
                input.add_variable("arg_key".to_string(), VariableValue::String(key.to_string()));
                input.add_variable("arg_value".to_string(), VariableValue::String(value.to_string()));
            }
            
            inputs.push(input);
        }
        
        Ok(inputs)
    }
    
    fn available_variables(&self, _config: &InputConfig) -> Result<Vec<VariableDefinition>> {
        Ok(vec![
            VariableDefinition {
                name: "arg".to_string(),
                var_type: VariableType::String,
                description: "The current argument value".to_string(),
                required: true,
                default_value: None,
                validation_rules: vec![],
            },
            VariableDefinition {
                name: "arg_index".to_string(),
                var_type: VariableType::Number,
                description: "Zero-based index of the current argument".to_string(),
                required: true,
                default_value: None,
                validation_rules: vec![],
            },
            VariableDefinition {
                name: "arg_count".to_string(),
                var_type: VariableType::Number,
                description: "Total number of arguments".to_string(),
                required: true,
                default_value: None,
                validation_rules: vec![],
            },
            VariableDefinition {
                name: "arg_key".to_string(),
                var_type: VariableType::String,
                description: "Key part of key=value argument (if applicable)".to_string(),
                required: false,
                default_value: None,
                validation_rules: vec![],
            },
            VariableDefinition {
                name: "arg_value".to_string(),
                var_type: VariableType::String,
                description: "Value part of key=value argument (if applicable)".to_string(),
                required: false,
                default_value: None,
                validation_rules: vec![],
            },
        ])
    }
}

// src/cook/input/file_pattern.rs
pub struct FilePatternInputProvider {
    file_system: Arc<dyn FileSystemOperations>,
}

#[async_trait]
impl InputProvider for FilePatternInputProvider {
    fn input_type(&self) -> InputType {
        InputType::FilePattern { patterns: vec![], recursive: false }
    }
    
    async fn generate_inputs(&self, config: &InputConfig) -> Result<Vec<ExecutionInput>> {
        let patterns = config.get_array("patterns")?;
        let recursive = config.get_bool("recursive").unwrap_or(false);
        
        let mut all_files = HashSet::new();
        
        for pattern in patterns {
            let pattern_str = pattern.as_str().ok_or_else(|| 
                anyhow::anyhow!("Pattern must be a string"))?;
            
            let files = if recursive {
                self.file_system.glob_recursive(pattern_str).await?
            } else {
                self.file_system.glob(pattern_str).await?
            };
            
            all_files.extend(files);
        }
        
        let mut inputs = Vec::new();
        
        for (index, file_path) in all_files.iter().enumerate() {
            let metadata = self.file_system.metadata(file_path).await?;
            
            let mut input = ExecutionInput::new(
                format!("file_{}", index),
                InputType::FilePattern { patterns: patterns.clone(), recursive }
            );
            
            // File path variables
            input.add_variable("file_path".to_string(), 
                VariableValue::Path(file_path.clone()));
            input.add_variable("file_name".to_string(), 
                VariableValue::String(
                    file_path.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string()
                ));
            input.add_variable("file_dir".to_string(),
                VariableValue::Path(
                    file_path.parent()
                        .unwrap_or_else(|| Path::new("."))
                        .to_path_buf()
                ));
            input.add_variable("file_stem".to_string(),
                VariableValue::String(
                    file_path.file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string()
                ));
            input.add_variable("file_extension".to_string(),
                VariableValue::String(
                    file_path.extension()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string()
                ));
            
            // File metadata variables
            input.add_variable("file_size".to_string(),
                VariableValue::Number(metadata.size as i64));
            input.add_variable("file_modified".to_string(),
                VariableValue::String(metadata.modified.to_rfc3339()));
            
            // Content-based variables (for small files)
            if metadata.size < 1024 * 1024 { // 1MB limit
                if let Ok(content) = self.file_system.read_to_string(file_path).await {
                    input.add_variable("file_content".to_string(),
                        VariableValue::String(content.clone()));
                    input.add_variable("file_lines".to_string(),
                        VariableValue::Number(content.lines().count() as i64));
                }
            }
            
            let input_metadata = InputMetadata {
                source: file_path.to_string_lossy().to_string(),
                created_at: chrono::Utc::now(),
                size_bytes: Some(metadata.size),
                checksum: None, // TODO: Calculate if needed
                content_type: Some(mime_guess::from_path(file_path).first_or_octet_stream().to_string()),
                custom_fields: HashMap::new(),
            };
            
            input.with_metadata(input_metadata);
            inputs.push(input);
        }
        
        Ok(inputs)
    }
    
    fn available_variables(&self, _config: &InputConfig) -> Result<Vec<VariableDefinition>> {
        Ok(vec![
            VariableDefinition {
                name: "file_path".to_string(),
                var_type: VariableType::Path,
                description: "Full path to the matched file".to_string(),
                required: true,
                default_value: None,
                validation_rules: vec![ValidationRule::FileExists],
            },
            VariableDefinition {
                name: "file_name".to_string(),
                var_type: VariableType::String,
                description: "File name with extension".to_string(),
                required: true,
                default_value: None,
                validation_rules: vec![],
            },
            VariableDefinition {
                name: "file_dir".to_string(),
                var_type: VariableType::Path,
                description: "Directory containing the file".to_string(),
                required: true,
                default_value: None,
                validation_rules: vec![],
            },
            VariableDefinition {
                name: "file_stem".to_string(),
                var_type: VariableType::String,
                description: "File name without extension".to_string(),
                required: true,
                default_value: None,
                validation_rules: vec![],
            },
            VariableDefinition {
                name: "file_extension".to_string(),
                var_type: VariableType::String,
                description: "File extension (without dot)".to_string(),
                required: false,
                default_value: Some("".to_string()),
                validation_rules: vec![],
            },
            VariableDefinition {
                name: "file_size".to_string(),
                var_type: VariableType::Number,
                description: "File size in bytes".to_string(),
                required: true,
                default_value: None,
                validation_rules: vec![ValidationRule::Range { min: Some(0), max: None }],
            },
        ])
    }
}
```

#### Phase 3: Input Configuration System

```rust
// src/cook/input/config.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputConfig {
    pub sources: Vec<InputSource>,
    pub validation: ValidationConfig,
    pub transformation: TransformationConfig,
    pub caching: CachingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InputSource {
    Empty,
    Arguments {
        value: String,
        separator: Option<String>,
        validation: Option<ArgumentValidation>,
    },
    FilePattern {
        patterns: Vec<String>,
        recursive: bool,
        filters: Option<FileFilters>,
    },
    StructuredData {
        source: DataSource,
        format: DataFormat,
        schema: Option<String>,
        path: Option<String>, // JSONPath or similar for extracting subset
    },
    Environment {
        prefix: Option<String>,
        required_vars: Vec<String>,
        optional_vars: Vec<String>,
    },
    StandardInput {
        format: DataFormat,
        buffer_size: Option<usize>,
    },
    Generated {
        generator: GeneratorType,
        count: usize,
        config: serde_json::Value,
    },
    Composite {
        sources: Vec<InputSource>,
        merge_strategy: MergeStrategy,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DataSource {
    File(PathBuf),
    Url(String),
    Inline(String),
    StandardInput,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DataFormat {
    Json,
    Yaml,
    Toml,
    Csv,
    Xml,
    PlainText,
    Auto, // Detect from extension or content
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileFilters {
    pub min_size: Option<u64>,
    pub max_size: Option<u64>,
    pub extensions: Option<Vec<String>>,
    pub exclude_patterns: Option<Vec<String>>,
    pub modified_since: Option<chrono::DateTime<chrono::Utc>>,
    pub modified_before: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationConfig {
    pub strict: bool,
    pub custom_rules: Vec<CustomValidationRule>,
    pub stop_on_first_error: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransformationConfig {
    pub variable_transformations: HashMap<String, String>,
    pub input_filters: Vec<InputFilter>,
    pub sorting: Option<SortConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachingConfig {
    pub enabled: bool,
    pub ttl_seconds: Option<u64>,
    pub cache_key_template: Option<String>,
    pub invalidate_on_change: bool,
}

impl InputConfig {
    pub fn from_command_args(args: &str) -> Self {
        Self {
            sources: vec![InputSource::Arguments {
                value: args.to_string(),
                separator: Some(",".to_string()),
                validation: None,
            }],
            validation: ValidationConfig::default(),
            transformation: TransformationConfig::default(),
            caching: CachingConfig::default(),
        }
    }
    
    pub fn from_file_patterns(patterns: Vec<String>) -> Self {
        Self {
            sources: vec![InputSource::FilePattern {
                patterns,
                recursive: false,
                filters: None,
            }],
            validation: ValidationConfig::default(),
            transformation: TransformationConfig::default(),
            caching: CachingConfig::default(),
        }
    }
    
    pub fn from_mapreduce_config(config: &crate::config::MapReduceWorkflowConfig) -> Result<Self> {
        // Convert MapReduce configuration to input configuration
        let source = match &config.input_source {
            crate::config::MapReduceInputSource::FilePattern(patterns) => {
                InputSource::FilePattern {
                    patterns: patterns.clone(),
                    recursive: config.recursive.unwrap_or(false),
                    filters: None,
                }
            }
            crate::config::MapReduceInputSource::StructuredData { source, format } => {
                InputSource::StructuredData {
                    source: match source.as_str() {
                        path if PathBuf::from(path).exists() => DataSource::File(PathBuf::from(path)),
                        url if url.starts_with("http") => DataSource::Url(url.to_string()),
                        _ => DataSource::Inline(source.clone()),
                    },
                    format: match format.as_str() {
                        "json" => DataFormat::Json,
                        "yaml" => DataFormat::Yaml,
                        "csv" => DataFormat::Csv,
                        _ => DataFormat::Auto,
                    },
                    schema: None,
                    path: None,
                }
            }
        };
        
        Ok(Self {
            sources: vec![source],
            validation: ValidationConfig::default(),
            transformation: TransformationConfig::default(),
            caching: CachingConfig::default(),
        })
    }
}
```

#### Phase 4: Input Processing Pipeline

```rust
// src/cook/input/processor.rs
pub struct InputProcessor {
    providers: HashMap<String, Box<dyn InputProvider>>,
    cache: Arc<RwLock<HashMap<String, CachedInput>>>,
    file_system: Arc<dyn FileSystemOperations>,
}

impl InputProcessor {
    pub fn new(file_system: Arc<dyn FileSystemOperations>) -> Self {
        let mut providers: HashMap<String, Box<dyn InputProvider>> = HashMap::new();
        
        providers.insert("arguments".to_string(), Box::new(ArgumentsInputProvider));
        providers.insert("file_pattern".to_string(), 
            Box::new(FilePatternInputProvider::new(file_system.clone())));
        providers.insert("structured_data".to_string(),
            Box::new(StructuredDataInputProvider::new()));
        providers.insert("environment".to_string(),
            Box::new(EnvironmentInputProvider));
        providers.insert("stdin".to_string(),
            Box::new(StandardInputProvider));
        
        Self {
            providers,
            cache: Arc::new(RwLock::new(HashMap::new())),
            file_system,
        }
    }
    
    pub async fn process_inputs(&self, config: &InputConfig) -> Result<Vec<ExecutionInput>> {
        let mut all_inputs = Vec::new();
        
        for source in &config.sources {
            let inputs = self.process_input_source(source, config).await?;
            all_inputs.extend(inputs);
        }
        
        // Apply transformations
        let transformed_inputs = self.apply_transformations(&all_inputs, &config.transformation).await?;
        
        // Apply validation
        self.validate_inputs(&transformed_inputs, &config.validation).await?;
        
        Ok(transformed_inputs)
    }
    
    async fn process_input_source(
        &self,
        source: &InputSource,
        config: &InputConfig,
    ) -> Result<Vec<ExecutionInput>> {
        match source {
            InputSource::Empty => {
                Ok(vec![ExecutionInput::new("empty".to_string(), InputType::Empty)])
            }
            InputSource::Composite { sources, merge_strategy } => {
                self.process_composite_source(sources, merge_strategy, config).await
            }
            _ => {
                let provider_name = self.get_provider_name(source);
                let provider = self.providers.get(&provider_name)
                    .ok_or_else(|| anyhow::anyhow!("No provider for input type: {}", provider_name))?;
                
                let source_config = self.create_source_config(source)?;
                
                // Check cache if enabled
                if config.caching.enabled {
                    if let Some(cached) = self.check_cache(&source_config, &config.caching).await? {
                        return Ok(cached.inputs);
                    }
                }
                
                let inputs = provider.generate_inputs(&source_config).await?;
                
                // Store in cache if enabled
                if config.caching.enabled {
                    self.store_in_cache(&source_config, &inputs, &config.caching).await?;
                }
                
                Ok(inputs)
            }
        }
    }
    
    async fn apply_transformations(
        &self,
        inputs: &[ExecutionInput],
        config: &TransformationConfig,
    ) -> Result<Vec<ExecutionInput>> {
        let mut transformed = inputs.to_vec();
        
        // Apply variable transformations
        for input in &mut transformed {
            for (var_name, transformation) in &config.variable_transformations {
                if let Some(value) = input.variables.get(var_name) {
                    let transformed_value = self.apply_transformation(value, transformation)?;
                    input.variables.insert(var_name.clone(), transformed_value);
                }
            }
        }
        
        // Apply input filters
        for filter in &config.input_filters {
            transformed = self.apply_input_filter(transformed, filter)?;
        }
        
        // Apply sorting if configured
        if let Some(sort_config) = &config.sorting {
            transformed = self.sort_inputs(transformed, sort_config)?;
        }
        
        Ok(transformed)
    }
}

#[derive(Debug, Clone)]
struct CachedInput {
    inputs: Vec<ExecutionInput>,
    created_at: chrono::DateTime<chrono::Utc>,
    expires_at: Option<chrono::DateTime<chrono::Utc>>,
}
```

### Architecture Changes

#### Input Processing Pipeline
```
┌─────────────────────────────────────────────────────┐
│                Command Line / Config                 │
│  ┌─────────────────────────────────────────────┐    │
│  │    --args "a,b,c"    --map "*.rs"           │    │
│  │    --config input.yml                       │    │
│  └─────────────────────────────────────────────┘    │
└─────────────────────┬───────────────────────────────┘
                     │
┌─────────────────────▼───────────────────────────────┐
│              Input Configuration                     │
│  ┌─────────────────────────────────────────────┐    │
│  │         InputConfig Parser                   │    │
│  │                                             │    │
│  │  • Normalize different input formats        │    │
│  │  • Apply configuration defaults            │    │
│  │  • Validate configuration schema           │    │
│  └─────────────────────────────────────────────┘    │
└─────────────────────┬───────────────────────────────┘
                     │
┌─────────────────────▼───────────────────────────────┐
│              Input Processor                         │
│  ┌─────────────────────────────────────────────┐    │
│  │          Input Providers                     │    │
│  │                                             │    │
│  │  • ArgumentsInputProvider                   │    │
│  │  • FilePatternInputProvider                 │    │
│  │  • StructuredDataInputProvider              │    │
│  │  • EnvironmentInputProvider                 │    │
│  │  • StandardInputProvider                    │    │
│  │  • CompositeInputProvider                   │    │
│  └─────────────────────────────────────────────┘    │
│  ┌─────────────────────────────────────────────┐    │
│  │       Transformation Pipeline                │    │
│  │                                             │    │
│  │  • Variable transformation                  │    │
│  │  • Input filtering                          │    │
│  │  • Sorting and grouping                     │    │
│  │  • Validation and sanitization              │    │
│  └─────────────────────────────────────────────┘    │
└─────────────────────┬───────────────────────────────┘
                     │
┌─────────────────────▼───────────────────────────────┐
│            Standardized Execution Inputs             │
│  ┌─────────────────────────────────────────────┐    │
│  │         ExecutionInput[]                     │    │
│  │                                             │    │
│  │  • Consistent variable naming               │    │
│  │  • Rich metadata                            │    │
│  │  • Type-safe values                         │    │
│  │  • Dependency information                   │    │
│  └─────────────────────────────────────────────┘    │
└─────────────────────┬───────────────────────────────┘
                     │
┌─────────────────────▼───────────────────────────────┐
│             Unified Workflow Executor                │
│           (from Specification 58)                   │
└─────────────────────────────────────────────────────┘
```

### Data Structures

#### Variable Value System
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum VariableValue {
    String(String),
    Number(i64),
    Float(f64),
    Boolean(bool),
    Path(PathBuf),
    Url(url::Url),
    Array(Vec<VariableValue>),
    Object(HashMap<String, VariableValue>),
    Null,
}

impl VariableValue {
    pub fn to_string(&self) -> String {
        match self {
            VariableValue::String(s) => s.clone(),
            VariableValue::Number(n) => n.to_string(),
            VariableValue::Float(f) => f.to_string(),
            VariableValue::Boolean(b) => b.to_string(),
            VariableValue::Path(p) => p.to_string_lossy().to_string(),
            VariableValue::Url(u) => u.to_string(),
            VariableValue::Array(arr) => format!("[{}]", 
                arr.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(", ")),
            VariableValue::Object(obj) => serde_json::to_string(obj).unwrap_or_default(),
            VariableValue::Null => "null".to_string(),
        }
    }
    
    pub fn as_path(&self) -> Result<PathBuf> {
        match self {
            VariableValue::Path(p) => Ok(p.clone()),
            VariableValue::String(s) => Ok(PathBuf::from(s)),
            _ => Err(anyhow::anyhow!("Cannot convert {:?} to path", self)),
        }
    }
    
    pub fn as_number(&self) -> Result<i64> {
        match self {
            VariableValue::Number(n) => Ok(*n),
            VariableValue::Float(f) => Ok(*f as i64),
            VariableValue::String(s) => s.parse().map_err(anyhow::Error::from),
            _ => Err(anyhow::anyhow!("Cannot convert {:?} to number", self)),
        }
    }
}
```

### APIs and Interfaces

#### Input Configuration API
```rust
pub trait InputConfigurationBuilder {
    fn new() -> Self;
    fn add_arguments(&mut self, args: &str) -> &mut Self;
    fn add_file_patterns(&mut self, patterns: Vec<String>) -> &mut Self;
    fn add_structured_data(&mut self, source: DataSource, format: DataFormat) -> &mut Self;
    fn add_environment_variables(&mut self, prefix: Option<String>) -> &mut Self;
    fn with_validation(&mut self, config: ValidationConfig) -> &mut Self;
    fn with_transformation(&mut self, config: TransformationConfig) -> &mut Self;
    fn with_caching(&mut self, config: CachingConfig) -> &mut Self;
    fn build(&self) -> Result<InputConfig>;
}

// Usage examples:
// let config = InputConfigurationBuilder::new()
//     .add_arguments("file1.txt,file2.txt,file3.txt")
//     .with_validation(ValidationConfig::strict())
//     .build()?;
//
// let config = InputConfigurationBuilder::new()
//     .add_file_patterns(vec!["src/**/*.rs".to_string()])
//     .with_transformation(TransformationConfig::new()
//         .add_variable_transform("file_path", "file_path|basename"))
//     .build()?;
```

#### Backward Compatibility Wrapper
```rust
// Maintains compatibility with existing command line arguments
pub struct LegacyInputAdapter;

impl LegacyInputAdapter {
    pub fn from_cook_command(command: &CookCommand) -> Result<InputConfig> {
        let mut builder = InputConfigurationBuilder::new();
        
        if let Some(args) = &command.args {
            builder.add_arguments(args);
        }
        
        if let Some(patterns) = &command.map {
            builder.add_file_patterns(patterns.clone());
        }
        
        // Apply default transformations and validation
        builder
            .with_validation(ValidationConfig::default())
            .with_transformation(TransformationConfig::default())
            .build()
    }
    
    pub fn supports_mapreduce(config: &InputConfig) -> bool {
        config.sources.iter().any(|source| match source {
            InputSource::FilePattern { .. } => true,
            InputSource::StructuredData { .. } => true,
            InputSource::Composite { .. } => true,
            _ => false,
        })
    }
}
```

## Dependencies

- **Prerequisites**: 
  - Specification 58: Unified Execution Model (provides execution context)
- **Affected Components**:
  - `src/cook/orchestrator.rs`: Replace input handling logic
  - `src/cook/workflow/`: Integration with unified execution model
  - `src/config/`: Update configuration parsing
  - `src/cook/command.rs`: Modify CookCommand to use InputConfig
- **External Dependencies**:
  - `serde`: Serialization for configuration and data formats
  - `serde_json`: JSON processing for structured inputs
  - `serde_yaml`: YAML processing for configuration files
  - `glob`: File pattern matching
  - `regex`: Template variable processing
  - `url`: URL validation and parsing
  - `mime_guess`: Content type detection for files

## Testing Strategy

### Unit Tests
- Input provider implementations for all supported types
- Variable substitution with edge cases and transformations
- Input validation with various error conditions
- Configuration parsing and normalization
- Caching behavior and invalidation logic

### Integration Tests
- End-to-end input processing for all supported sources
- Backward compatibility with existing command line usage
- Performance benchmarks for large input sets
- Error handling and recovery scenarios
- Integration with unified execution model

### User Acceptance Tests
- Migration from existing `--args` and `--map` usage
- Complex workflow scenarios with multiple input types
- Error message quality and helpfulness
- Documentation accuracy and completeness
- IDE integration and schema validation

## Documentation Requirements

### Code Documentation
- Input provider interfaces and implementations
- Variable substitution system and helper functions
- Configuration schema and validation rules
- Caching strategies and performance considerations
- Error handling patterns and recovery mechanisms

### User Documentation
- Migration guide from current argument and mapping systems
- Input configuration reference with examples
- Variable substitution syntax and helper functions
- Best practices for input validation and transformation
- Troubleshooting guide for common input issues

### Architecture Documentation
- Input abstraction system architecture
- Integration with unified execution model
- Performance characteristics and optimization tips
- Extension points for custom input providers

## Implementation Notes

### Performance Considerations
- Lazy evaluation of input processing to avoid unnecessary work
- Streaming support for large input sets to manage memory usage
- Efficient caching strategies with configurable TTL and invalidation
- Parallel input processing where applicable (e.g., file processing)
- Smart buffering for network-based input sources

### Security Considerations
- Input sanitization to prevent injection attacks
- Path traversal protection for file-based inputs
- URL validation and restriction for remote sources
- Environment variable filtering for sensitive data
- Content validation and size limits to prevent DoS attacks

### Backward Compatibility Strategy
- LegacyInputAdapter provides seamless migration from existing CLI arguments
- All existing workflow configurations continue to work unchanged
- Gradual deprecation of old input handling code
- Feature flags to enable new input capabilities progressively
- Comprehensive migration testing with existing workflows

## Migration and Compatibility

### Breaking Changes
- None for standard command line usage (`--args`, `--map`)
- Internal APIs for custom input handling will need updates
- Advanced MapReduce configurations may require minor adjustments
- Some internal logging and error message formats will change

### Migration Path
1. **Phase 1**: Deploy input abstraction alongside existing system
2. **Phase 2**: Migrate internal components to use new input system
3. **Phase 3**: Add new input capabilities (structured data, environment vars, etc.)
4. **Phase 4**: Deprecate legacy input handling code
5. **Phase 5**: Remove legacy code after validation period

### Rollback Strategy
- Feature flag to disable new input abstraction
- Ability to fall back to legacy input processing
- Monitoring to detect processing errors or performance issues
- Quick rollback mechanism for production deployments