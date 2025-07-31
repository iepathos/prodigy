# Spec 49: Dynamic Command Discovery System

## Overview

Replace the current hardcoded command registry system with a dynamic discovery mechanism that automatically detects and loads commands from the `.claude/commands` directory. This will enable seamless addition of new commands without modifying the core MMM codebase while maintaining full backward compatibility and validation.

## Background

### Current State Analysis

Based on the codebase analysis:

1. **Hardcoded Registry**: `src/config/command_validator.rs` contains a hardcoded registry with 5 built-in commands (mmm-code-review, mmm-implement-spec, mmm-lint, mmm-product-enhance, mmm-cleanup-tech-debt)

2. **Static Command Definitions**: Each command is manually registered with argument definitions, option specifications, and metadata defaults

3. **Rich Validation System**: The current system provides comprehensive validation including argument types, option validation, and metadata application

4. **Command Structure**: Commands are well-structured with CommandDefinition containing name, description, arguments, options, and defaults

5. **Available Commands**: The `.claude/commands` directory contains 15+ command files in markdown format with rich metadata and documentation

### Problems with Current Approach

1. **Maintenance Overhead**: Adding new commands requires code changes and recompilation
2. **Limited Extensibility**: Users cannot add custom commands without modifying source code
3. **Inconsistency**: Commands exist in `.claude/commands` but aren't automatically recognized by the registry
4. **Development Friction**: Command development requires both markdown documentation and code registration

## Requirements

### Core Requirements

1. **Dynamic Discovery**: Automatically detect and load all `.md` files from `.claude/commands` directory
2. **Metadata Extraction**: Parse command metadata from markdown frontmatter or structured sections
3. **Backward Compatibility**: Maintain existing workflow execution and validation behavior
4. **Validation Preservation**: Keep current argument/option validation with fallback to permissive validation
5. **Error Handling**: Graceful handling of malformed command files with clear error messages
6. **Performance**: Efficient loading with caching to avoid repeated filesystem access

### Advanced Requirements

1. **Hot Reloading**: Detect changes to command files and reload registry (optional enhancement)
2. **Command Versioning**: Support versioned command definitions (future enhancement)
3. **Dependency Resolution**: Handle command dependencies and prerequisites (future enhancement)
4. **Custom Validation**: Allow commands to define custom validation rules (future enhancement)

## Design

### Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                          MMM Core                                   │
├─────────────────────────────────────────────────────────────────────┤
│  WorkflowExecutor                                                   │
│    │                                                               │
│    └── CommandRegistry (Dynamic)                                   │
│          │                                                         │
│          ├── CommandDiscovery ←── .claude/commands/*.md             │
│          ├── MetadataParser                                        │
│          ├── ValidationEngine                                      │
│          └── FallbackRegistry (built-in commands)                  │
└─────────────────────────────────────────────────────────────────────┘
```

### Component Design

#### 1. CommandDiscovery

**Responsibility**: Filesystem scanning and command file detection

```rust
pub struct CommandDiscovery {
    commands_dir: PathBuf,
    cache: Option<HashMap<String, CommandFile>>,
    last_scan: Option<SystemTime>,
}

impl CommandDiscovery {
    pub async fn scan_commands(&mut self) -> Result<Vec<CommandFile>>;
    pub async fn watch_for_changes(&self) -> Result<()>; // Future enhancement
}

pub struct CommandFile {
    pub path: PathBuf,
    pub name: String,
    pub content: String,
    pub modified: SystemTime,
}
```

#### 2. MetadataParser

**Responsibility**: Extract command metadata from markdown files

```rust
pub struct MetadataParser;

impl MetadataParser {
    pub fn parse_command_file(&self, file: &CommandFile) -> Result<CommandDefinition>;
    pub fn extract_frontmatter(&self, content: &str) -> Result<CommandMetadata>;
    pub fn parse_structured_sections(&self, content: &str) -> Result<CommandStructure>;
}

pub struct CommandStructure {
    pub variables: Vec<VariableDefinition>,
    pub usage_examples: Vec<String>,
    pub validation_rules: Option<ValidationRules>,
}
```

#### 3. DynamicCommandRegistry

**Responsibility**: Unified registry combining discovered and built-in commands

```rust
pub struct DynamicCommandRegistry {
    discovered_commands: HashMap<String, CommandDefinition>,
    fallback_registry: StaticCommandRegistry,
    discovery: CommandDiscovery,
    parser: MetadataParser,
}

impl DynamicCommandRegistry {
    pub async fn new(commands_dir: Option<PathBuf>) -> Result<Self>;
    pub async fn refresh(&mut self) -> Result<()>;
    pub fn get_command(&self, name: &str) -> Option<&CommandDefinition>;
    pub fn validate_command(&self, command: &Command) -> Result<()>;
    pub fn apply_defaults(&self, command: &mut Command);
}
```

### Metadata Extraction Strategy

#### Frontmatter-Based Approach (Primary)

Commands can optionally include YAML frontmatter for structured metadata:

```markdown
---
name: mmm-custom-command
description: "Custom command for specific workflow"
arguments:
  - name: target
    type: string
    required: true
    description: "Target file or directory"
options:
  - name: verbose
    type: boolean
    default: false
    description: "Enable verbose output"
metadata:
  retries: 3
  timeout: 300
  continue_on_error: false
---

# /mmm-custom-command

Command documentation content...
```

#### Section-Based Approach (Fallback)

Parse structured sections from existing command format:

```markdown
# /mmm-code-review

## Variables
SCOPE: $ARGUMENTS (optional - specify scope)
FOCUS: $MMM_FOCUS (optional - focus directive)

## Options
- `--max-issues`: Maximum number of issues (default: 10)
- `--severity`: Minimum severity level (critical|high|medium|low)
```

#### Parsing Logic

```rust
impl MetadataParser {
    pub fn parse_command_file(&self, file: &CommandFile) -> Result<CommandDefinition> {
        // Try frontmatter first
        if let Ok(definition) = self.parse_frontmatter(&file.content) {
            return Ok(definition);
        }
        
        // Fall back to section parsing
        if let Ok(definition) = self.parse_sections(&file.content) {
            return Ok(definition);
        }
        
        // Minimal definition from filename and content
        Ok(self.create_minimal_definition(file))
    }
    
    fn create_minimal_definition(&self, file: &CommandFile) -> CommandDefinition {
        CommandDefinition {
            name: file.name.clone(),
            description: self.extract_description(&file.content),
            required_args: vec![], // Permissive - accept any args
            optional_args: vec![],
            options: vec![], // Accept any options
            defaults: CommandMetadata::default(),
        }
    }
}
```

### Validation Strategy

#### Tiered Validation Approach

1. **Strict Validation**: For commands with explicit metadata (frontmatter or structured sections)
2. **Permissive Validation**: For commands without metadata - accept any arguments/options
3. **Fallback Validation**: Use static registry for built-in commands

```rust
impl DynamicCommandRegistry {
    pub fn validate_command(&self, command: &Command) -> Result<()> {
        // Check discovered commands first
        if let Some(definition) = self.discovered_commands.get(&command.name) {
            return self.validate_against_definition(command, definition);
        }
        
        // Fall back to static registry
        if let Some(definition) = self.fallback_registry.get(&command.name) {
            return self.fallback_registry.validate_command(command);
        }
        
        // Command not found - this is an error
        Err(anyhow!("Unknown command: {}", command.name))
    }
    
    fn validate_against_definition(&self, command: &Command, definition: &CommandDefinition) -> Result<()> {
        if definition.required_args.is_empty() && definition.options.is_empty() {
            // Permissive validation - minimal command definition
            return Ok(());
        }
        
        // Strict validation - use existing validation logic
        self.validate_strict(command, definition)
    }
}
```

### Integration Points

#### 1. WorkflowExecutor Integration

Minimal changes to existing workflow execution:

```rust
impl WorkflowExecutor {
    pub async fn new(config: WorkflowConfig, verbose: bool, max_iterations: u32) -> Result<Self> {
        // Replace static registry with dynamic registry
        let registry = DynamicCommandRegistry::new(
            Some(PathBuf::from(".claude/commands"))
        ).await?;
        
        Self {
            config,
            verbose,
            max_iterations,
            registry, // Use dynamic registry instead of static COMMAND_REGISTRY
            // ... other fields
        }
    }
}
```

#### 2. ConfigLoader Integration

Support command directory configuration:

```yaml
# .mmm/config.yml
global:
  commands_dir: ".claude/commands"  # Default
  enable_command_discovery: true    # Default
  
workflow:
  commands:
    - mmm-code-review
    - custom-command  # Now automatically discovered
```

## Implementation Plan

### Phase 1: Core Discovery Infrastructure

**Files to Create:**
1. `src/config/command_discovery.rs` - CommandDiscovery implementation
2. `src/config/metadata_parser.rs` - MetadataParser implementation  
3. `src/config/dynamic_registry.rs` - DynamicCommandRegistry implementation

**Files to Modify:**
1. `src/config/mod.rs` - Add new modules and exports
2. `src/config/command_validator.rs` - Refactor as StaticCommandRegistry
3. `src/cook/workflow.rs` - Integrate DynamicCommandRegistry

### Phase 2: Metadata Parsing

**Implementation Steps:**
1. Add frontmatter parsing with `serde_yaml`
2. Implement section-based parsing for existing format
3. Create fallback minimal definition generation
4. Add comprehensive error handling

### Phase 3: Integration and Testing

**Implementation Steps:**
1. Update WorkflowExecutor to use DynamicCommandRegistry
2. Modify config loading to support command directory configuration
3. Add comprehensive unit tests
4. Add integration tests with real command files

### Phase 4: Documentation and Migration

**Implementation Steps:**
1. Update documentation for dynamic command system
2. Create migration guide for adding custom commands
3. Add examples of frontmatter-based command definitions
4. Update existing commands with optional frontmatter

## Detailed Implementation

### Command Discovery Implementation

```rust
// src/config/command_discovery.rs
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tokio::fs;

pub struct CommandDiscovery {
    commands_dir: PathBuf,
    cache: HashMap<String, CommandFile>,
    last_scan: Option<SystemTime>,
}

impl CommandDiscovery {
    pub fn new(commands_dir: PathBuf) -> Self {
        Self {
            commands_dir,
            cache: HashMap::new(),
            last_scan: None,
        }
    }
    
    pub async fn scan_commands(&mut self) -> Result<Vec<CommandFile>> {
        if !self.commands_dir.exists() {
            return Ok(vec![]);
        }
        
        let mut commands = Vec::new();
        let mut entries = fs::read_dir(&self.commands_dir).await
            .with_context(|| format!("Failed to read commands directory: {}", self.commands_dir.display()))?;
        
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            
            // Only process .md files
            if !path.extension().map_or(false, |ext| ext == "md") {
                continue;
            }
            
            // Skip non-command files
            let file_name = path.file_stem()
                .and_then(|name| name.to_str())
                .unwrap_or("");
            
            if !file_name.starts_with("mmm-") {
                continue;
            }
            
            let metadata = entry.metadata().await?;
            let modified = metadata.modified()?;
            
            // Check cache first
            if let Some(cached) = self.cache.get(file_name) {
                if cached.modified >= modified {
                    commands.push(cached.clone());
                    continue;
                }
            }
            
            // Read and cache the file
            let content = fs::read_to_string(&path).await
                .with_context(|| format!("Failed to read command file: {}", path.display()))?;
            
            let command_file = CommandFile {
                path: path.clone(),
                name: file_name.to_string(),
                content,
                modified,
            };
            
            self.cache.insert(file_name.to_string(), command_file.clone());
            commands.push(command_file);
        }
        
        self.last_scan = Some(SystemTime::now());
        Ok(commands)
    }
    
    pub fn needs_refresh(&self) -> bool {
        self.last_scan.is_none()
    }
}

#[derive(Clone, Debug)]
pub struct CommandFile {
    pub path: PathBuf,
    pub name: String,
    pub content: String,
    pub modified: SystemTime,
}
```

### Metadata Parser Implementation

```rust
// src/config/metadata_parser.rs
use super::command_validator::{ArgumentDef, ArgumentType, CommandDefinition, OptionDef};
use super::command::{CommandMetadata};
use crate::config::command_discovery::CommandFile;
use anyhow::{anyhow, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub struct MetadataParser {
    frontmatter_regex: Regex,
    variable_regex: Regex,
}

impl MetadataParser {
    pub fn new() -> Self {
        Self {
            frontmatter_regex: Regex::new(r"^---\n(.*?)\n---").unwrap(),
            variable_regex: Regex::new(r"^(\w+):\s*(.+)$").unwrap(),
        }
    }
    
    pub fn parse_command_file(&self, file: &CommandFile) -> Result<CommandDefinition> {
        // Try frontmatter first
        if let Ok(definition) = self.parse_frontmatter(file) {
            return Ok(definition);
        }
        
        // Fall back to section parsing
        if let Ok(definition) = self.parse_sections(file) {
            return Ok(definition);
        }
        
        // Create minimal definition
        Ok(self.create_minimal_definition(file))
    }
    
    fn parse_frontmatter(&self, file: &CommandFile) -> Result<CommandDefinition> {
        let captures = self.frontmatter_regex.captures(&file.content)
            .ok_or_else(|| anyhow!("No frontmatter found"))?;
        
        let yaml_content = captures.get(1)
            .ok_or_else(|| anyhow!("Invalid frontmatter"))?
            .as_str();
        
        let metadata: FrontmatterMetadata = serde_yaml::from_str(yaml_content)
            .map_err(|e| anyhow!("Failed to parse frontmatter: {}", e))?;
        
        Ok(self.convert_frontmatter_to_definition(file, metadata))
    }
    
    fn parse_sections(&self, file: &CommandFile) -> Result<CommandDefinition> {
        let variables = self.extract_variables_section(&file.content)?;
        let options = self.extract_options_section(&file.content)?;
        
        Ok(CommandDefinition {
            name: file.name.clone(),
            description: self.extract_description(&file.content),
            required_args: self.parse_variables_to_args(&variables)?,
            optional_args: vec![],
            options: self.parse_options(&options)?,
            defaults: CommandMetadata::default(),
        })
    }
    
    fn create_minimal_definition(&self, file: &CommandFile) -> CommandDefinition {
        CommandDefinition {
            name: file.name.clone(),
            description: self.extract_description(&file.content),
            required_args: vec![],
            optional_args: vec![],
            options: vec![],
            defaults: CommandMetadata::default(),
        }
    }
    
    fn extract_description(&self, content: &str) -> String {
        // Extract first paragraph after the title
        let lines: Vec<&str> = content.lines().collect();
        let mut found_title = false;
        let mut description_lines = Vec::new();
        
        for line in lines {
            let trimmed = line.trim();
            
            if trimmed.starts_with("# /") {
                found_title = true;
                continue;
            }
            
            if found_title {
                if trimmed.is_empty() {
                    if !description_lines.is_empty() {
                        break;
                    }
                    continue;
                }
                
                if trimmed.starts_with("#") {
                    break;
                }
                
                description_lines.push(trimmed);
            }
        }
        
        description_lines.join(" ")
    }
    
    fn extract_variables_section(&self, content: &str) -> Result<Vec<String>> {
        self.extract_section(content, "## Variables")
    }
    
    fn extract_options_section(&self, content: &str) -> Result<Vec<String>> {
        self.extract_section(content, "## Options")
    }
    
    fn extract_section(&self, content: &str, section_header: &str) -> Result<Vec<String>> {
        let lines: Vec<&str> = content.lines().collect();
        let mut in_section = false;
        let mut section_lines = Vec::new();
        
        for line in lines {
            let trimmed = line.trim();
            
            if trimmed == section_header {
                in_section = true;
                continue;
            }
            
            if in_section {
                if trimmed.starts_with("##") {
                    break;
                }
                
                if !trimmed.is_empty() {
                    section_lines.push(trimmed.to_string());
                }
            }
        }
        
        Ok(section_lines)
    }
    
    fn parse_variables_to_args(&self, variables: &[String]) -> Result<Vec<ArgumentDef>> {
        let mut args = Vec::new();
        
        for var_line in variables {
            if let Some(captures) = self.variable_regex.captures(var_line) {
                let name = captures.get(1).unwrap().as_str();
                let spec = captures.get(2).unwrap().as_str();
                
                // Parse whether it's required
                let required = !spec.contains("optional");
                
                if required {
                    args.push(ArgumentDef {
                        name: name.to_lowercase(),
                        description: spec.to_string(),
                        arg_type: ArgumentType::String,
                    });
                }
            }
        }
        
        Ok(args)
    }
    
    fn parse_options(&self, options: &[String]) -> Result<Vec<OptionDef>> {
        let mut opts = Vec::new();
        
        for opt_line in options {
            if opt_line.starts_with("- `--") {
                let parts: Vec<&str> = opt_line.split(':').collect();
                if parts.len() >= 2 {
                    let name_part = parts[0].trim_start_matches("- `--").trim_end_matches("`");
                    let desc = parts[1].trim();
                    
                    opts.push(OptionDef {
                        name: name_part.to_string(),
                        description: desc.to_string(),
                        option_type: ArgumentType::String,
                        default: None,
                    });
                }
            }
        }
        
        Ok(opts)
    }
    
    fn convert_frontmatter_to_definition(&self, file: &CommandFile, metadata: FrontmatterMetadata) -> CommandDefinition {
        CommandDefinition {
            name: metadata.name.unwrap_or_else(|| file.name.clone()),
            description: metadata.description.unwrap_or_else(|| self.extract_description(&file.content)),
            required_args: metadata.arguments.unwrap_or_default().into_iter()
                .filter(|arg| arg.required.unwrap_or(false))
                .map(|arg| arg.into())
                .collect(),
            optional_args: metadata.arguments.unwrap_or_default().into_iter()
                .filter(|arg| !arg.required.unwrap_or(false))
                .map(|arg| arg.into())
                .collect(),
            options: metadata.options.unwrap_or_default().into_iter()
                .map(|opt| opt.into())
                .collect(),
            defaults: metadata.metadata.unwrap_or_default(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct FrontmatterMetadata {
    name: Option<String>,
    description: Option<String>,
    arguments: Option<Vec<FrontmatterArgument>>,
    options: Option<Vec<FrontmatterOption>>,
    metadata: Option<CommandMetadata>,
}

#[derive(Debug, Deserialize, Serialize)]
struct FrontmatterArgument {
    name: String,
    #[serde(rename = "type")]
    arg_type: Option<String>,
    required: Option<bool>,
    description: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct FrontmatterOption {
    name: String,
    #[serde(rename = "type")]
    option_type: Option<String>,
    default: Option<serde_json::Value>,
    description: Option<String>,
}

impl From<FrontmatterArgument> for ArgumentDef {
    fn from(arg: FrontmatterArgument) -> Self {
        Self {
            name: arg.name,
            description: arg.description.unwrap_or_default(),
            arg_type: match arg.arg_type.as_deref() {
                Some("integer") => ArgumentType::Integer,
                Some("boolean") => ArgumentType::Boolean,
                Some("path") => ArgumentType::Path,
                _ => ArgumentType::String,
            },
        }
    }
}

impl From<FrontmatterOption> for OptionDef {
    fn from(opt: FrontmatterOption) -> Self {
        Self {
            name: opt.name,
            description: opt.description.unwrap_or_default(),
            option_type: match opt.option_type.as_deref() {
                Some("integer") => ArgumentType::Integer,
                Some("boolean") => ArgumentType::Boolean,
                Some("path") => ArgumentType::Path,
                _ => ArgumentType::String,
            },
            default: opt.default,
        }
    }
}
```

### Dynamic Registry Implementation

```rust
// src/config/dynamic_registry.rs
use super::command_discovery::{CommandDiscovery, CommandFile};
use super::command_validator::{CommandDefinition, CommandRegistry as StaticCommandRegistry};
use super::metadata_parser::MetadataParser;
use super::command::Command;
use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::path::PathBuf;

pub struct DynamicCommandRegistry {
    discovered_commands: HashMap<String, CommandDefinition>,
    fallback_registry: StaticCommandRegistry,
    discovery: CommandDiscovery,
    parser: MetadataParser,
}

impl DynamicCommandRegistry {
    pub async fn new(commands_dir: Option<PathBuf>) -> Result<Self> {
        let commands_dir = commands_dir.unwrap_or_else(|| PathBuf::from(".claude/commands"));
        let mut discovery = CommandDiscovery::new(commands_dir);
        let parser = MetadataParser::new();
        let fallback_registry = StaticCommandRegistry::new();
        
        let mut registry = Self {
            discovered_commands: HashMap::new(),
            fallback_registry,
            discovery,
            parser,
        };
        
        registry.refresh().await?;
        Ok(registry)
    }
    
    pub async fn refresh(&mut self) -> Result<()> {
        let command_files = self.discovery.scan_commands().await?;
        let mut new_commands = HashMap::new();
        
        for file in command_files {
            match self.parser.parse_command_file(&file) {
                Ok(definition) => {
                    new_commands.insert(definition.name.clone(), definition);
                }
                Err(e) => {
                    eprintln!("Warning: Failed to parse command file {}: {}", file.path.display(), e);
                    // Create minimal definition for unparseable commands
                    let minimal = self.parser.create_minimal_definition(&file);
                    new_commands.insert(minimal.name.clone(), minimal);
                }
            }
        }
        
        self.discovered_commands = new_commands;
        Ok(())
    }
    
    pub fn get(&self, name: &str) -> Option<&CommandDefinition> {
        self.discovered_commands.get(name)
            .or_else(|| self.fallback_registry.get(name))
    }
    
    pub fn validate_command(&self, command: &Command) -> Result<()> {
        // Check discovered commands first
        if let Some(definition) = self.discovered_commands.get(&command.name) {
            return self.validate_against_definition(command, definition);
        }
        
        // Fall back to static registry
        if let Some(_definition) = self.fallback_registry.get(&command.name) {
            return self.fallback_registry.validate_command(command);
        }
        
        // Command not found
        Err(anyhow!("Unknown command: {}", command.name))
    }
    
    pub fn apply_defaults(&self, command: &mut Command) {
        if let Some(definition) = self.discovered_commands.get(&command.name) {
            self.apply_definition_defaults(command, definition);
        } else {
            self.fallback_registry.apply_defaults(command);
        }
    }
    
    fn validate_against_definition(&self, command: &Command, definition: &CommandDefinition) -> Result<()> {
        if definition.required_args.is_empty() && definition.options.is_empty() {
            // Permissive validation for minimal definitions
            return Ok(());
        }
        
        // Use existing validation logic for detailed definitions
        self.validate_strict(command, definition)
    }
    
    fn validate_strict(&self, command: &Command, definition: &CommandDefinition) -> Result<()> {
        // Reuse existing validation logic from StaticCommandRegistry
        // This ensures consistency between static and dynamic validation
        
        // Validate required arguments
        if command.args.len() < definition.required_args.len() {
            return Err(anyhow!(
                "Command '{}' requires {} arguments, but {} provided",
                command.name,
                definition.required_args.len(),
                command.args.len()
            ));
        }
        
        // Additional validation logic would go here...
        Ok(())
    }
    
    fn apply_definition_defaults(&self, command: &mut Command, definition: &CommandDefinition) {
        // Apply default metadata values if not set
        if command.metadata.retries.is_none() {
            command.metadata.retries = definition.defaults.retries;
        }
        if command.metadata.timeout.is_none() {
            command.metadata.timeout = definition.defaults.timeout;
        }
        if command.metadata.continue_on_error.is_none() {
            command.metadata.continue_on_error = definition.defaults.continue_on_error;
        }
        
        // Apply default option values if not set
        for opt_def in &definition.options {
            if !command.options.contains_key(&opt_def.name) {
                if let Some(default_value) = &opt_def.default {
                    command.options.insert(opt_def.name.clone(), default_value.clone());
                }
            }
        }
    }
    
    pub fn list_commands(&self) -> Vec<String> {
        let mut commands: Vec<String> = self.discovered_commands.keys().cloned().collect();
        commands.extend(self.fallback_registry.list_commands());
        commands.sort();
        commands.dedup();
        commands
    }
}

impl Default for DynamicCommandRegistry {
    fn default() -> Self {
        // For tests and cases where async construction isn't possible
        Self {
            discovered_commands: HashMap::new(),
            fallback_registry: StaticCommandRegistry::new(),
            discovery: CommandDiscovery::new(PathBuf::from(".claude/commands")),
            parser: MetadataParser::new(),
        }
    }
}
```

## Testing Strategy

### Unit Tests

1. **CommandDiscovery Tests**
   - Scan empty directory
   - Scan directory with various file types
   - Cache behavior and invalidation
   - Error handling for inaccessible directories

2. **MetadataParser Tests**
   - Frontmatter parsing with valid/invalid YAML
   - Section-based parsing with various formats
   - Minimal definition creation
   - Error handling for malformed files

3. **DynamicCommandRegistry Tests**
   - Command registration and lookup
   - Validation with discovered vs built-in commands
   - Default application
   - Fallback behavior

### Integration Tests

1. **End-to-End Command Discovery**
   - Create test command files
   - Verify they're discovered and executable
   - Test with WorkflowExecutor

2. **Backward Compatibility**
   - Ensure existing workflows continue to work
   - Verify built-in commands still function
   - Test mixed workflows (built-in + discovered)

## Migration Path

### Phase 1: Implementation (Week 1-2)
1. Implement core discovery infrastructure
2. Add metadata parsing capabilities
3. Create dynamic registry
4. Add comprehensive unit tests

### Phase 2: Integration (Week 3)
1. Integrate with WorkflowExecutor
2. Update configuration loading
3. Add integration tests
4. Performance testing and optimization

### Phase 3: Documentation (Week 4)
1. Update documentation
2. Create migration guides
3. Add command development examples
4. Update existing commands with metadata

### Phase 4: Rollout (Week 5)
1. Beta testing with existing workflows
2. Address any compatibility issues
3. Final testing and validation
4. Release and monitoring

## Success Criteria

### Functional Requirements
- [ ] All existing workflows continue to work unchanged
- [ ] New commands can be added by placing `.md` files in `.claude/commands`
- [ ] Commands with frontmatter metadata are validated strictly
- [ ] Commands without metadata are validated permissively
- [ ] Error messages are clear and actionable

### Performance Requirements
- [ ] Command discovery completes in <100ms for typical command sets
- [ ] Memory usage increases by <10MB for large command sets
- [ ] No performance regression in workflow execution

### Quality Requirements
- [ ] 95%+ test coverage for new components
- [ ] Zero regressions in existing functionality
- [ ] Clear documentation and examples
- [ ] Comprehensive error handling

## Future Enhancements

### Hot Reloading
- Watch filesystem for changes to command files
- Automatically refresh registry when commands are modified
- Notify running workflows of command updates

### Command Versioning
- Support versioned command definitions
- Allow workflows to specify required command versions
- Backward compatibility management

### Custom Validation
- Allow commands to define custom validation scripts
- Support for complex argument interdependencies
- Runtime validation of command prerequisites

### Command Dependencies
- Declare command dependencies and prerequisites
- Automatic dependency resolution
- Circular dependency detection

This specification provides a comprehensive plan for implementing dynamic command discovery while maintaining backward compatibility and system reliability. The phased approach ensures a smooth transition from the current hardcoded system to a flexible, extensible architecture.