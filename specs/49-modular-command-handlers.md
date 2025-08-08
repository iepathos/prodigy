---
number: 49
title: Modular Command Handler Architecture
category: foundation
priority: high
status: draft
dependencies: []
created: 2025-01-08
---

# Specification 49: Modular Command Handler Architecture

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: None

## Context

MMM's workflow system currently has hardcoded support for `claude:` and `shell:` commands in the workflow executor. This approach, while functional, lacks the extensibility and modularity that made Ansible successful. As we expand MMM's capabilities, we need a plugin-based architecture that allows easy addition of new command types without modifying core execution logic.

The current implementation has command-specific logic scattered across multiple files:
- Command parsing in `src/config/command.rs`
- Execution logic in `src/cook/workflow/executor.rs` and `src/cook/orchestrator.rs`
- Special handling for test commands, on_failure, and other attributes mixed into execution flow

This specification defines a modular command handler system that:
1. Separates command definition from execution
2. Provides a plugin-like interface for new command types
3. Standardizes command attributes and behaviors
4. Maintains backward compatibility with existing workflows

## Objective

Create a modular, extensible command handler architecture that allows MMM to support new command types through a plugin-like system, inspired by Ansible's module architecture. This system should make it trivial to add new command types (e.g., `docker:`, `git:`, `npm:`, `python:`, `rust:`) without modifying core workflow execution logic.

## Requirements

### Functional Requirements

1. **Command Handler Trait**
   - Define a trait that all command handlers must implement
   - Support validation, execution, and result handling
   - Provide hooks for pre/post execution logic
   - Enable async execution patterns

2. **Command Registry**
   - Central registry for command handlers
   - Dynamic registration of handlers at runtime
   - Discovery mechanism for built-in and custom handlers
   - Namespace support to avoid conflicts

3. **Standardized Attributes**
   - Common attributes available to all command types (timeout, env, working_dir)
   - Command-specific attributes through flexible schema
   - Validation of attributes at parse time
   - Type-safe attribute access

4. **Execution Context**
   - Shared context passed to all handlers
   - Variable interpolation before handler execution
   - Output capture and result propagation
   - Error handling and retry logic

5. **Backward Compatibility**
   - Existing `claude:` and `shell:` commands continue to work
   - Smooth migration path for current workflows
   - Preserve all current functionality

### Non-Functional Requirements

1. **Performance**
   - Minimal overhead for command dispatch
   - Lazy loading of handlers
   - Efficient attribute parsing and validation

2. **Extensibility**
   - New handlers can be added without core changes
   - Support for external handler plugins (future)
   - Clear documentation for handler development

3. **Maintainability**
   - Clear separation of concerns
   - Testable handler implementations
   - Consistent error messages and debugging

4. **Type Safety**
   - Compile-time verification where possible
   - Runtime validation with clear error messages
   - Proper error propagation

## Acceptance Criteria

- [ ] Command handler trait defined with all necessary methods
- [ ] Registry system implemented with dynamic registration
- [ ] Built-in handlers for `claude:` and `shell:` migrated to new system
- [ ] At least 3 new example handlers implemented (e.g., `git:`, `cargo:`, `file:`)
- [ ] All existing workflows continue to work without modification
- [ ] Handler development guide documented
- [ ] Unit tests for registry and each handler
- [ ] Integration tests for complex workflows
- [ ] Performance benchmarks show < 1ms overhead per command dispatch
- [ ] Error messages clearly indicate which handler failed and why

## Technical Details

### Implementation Approach

#### 1. Command Handler Trait

```rust
// src/commands/mod.rs
#[async_trait]
pub trait CommandHandler: Send + Sync {
    /// Unique identifier for this handler (e.g., "shell", "claude", "git")
    fn name(&self) -> &str;
    
    /// Description of what this handler does
    fn description(&self) -> &str;
    
    /// Schema for command-specific attributes
    fn attribute_schema(&self) -> AttributeSchema;
    
    /// Validate command attributes against schema
    fn validate(&self, attrs: &CommandAttributes) -> Result<()>;
    
    /// Execute the command with given context
    async fn execute(
        &self,
        command: &str,
        attrs: &CommandAttributes,
        context: &mut ExecutionContext,
    ) -> Result<CommandResult>;
    
    /// Optional pre-execution hook
    async fn pre_execute(&self, context: &mut ExecutionContext) -> Result<()> {
        Ok(())
    }
    
    /// Optional post-execution hook  
    async fn post_execute(&self, result: &CommandResult, context: &mut ExecutionContext) -> Result<()> {
        Ok(())
    }
    
    /// Whether this handler supports retry logic
    fn supports_retry(&self) -> bool {
        false
    }
    
    /// Custom retry logic if supported
    async fn retry_execute(
        &self,
        command: &str,
        attrs: &CommandAttributes,
        context: &mut ExecutionContext,
        attempt: u32,
    ) -> Result<CommandResult> {
        self.execute(command, attrs, context).await
    }
}
```

#### 2. Command Registry

```rust
// src/commands/registry.rs
pub struct CommandRegistry {
    handlers: HashMap<String, Arc<dyn CommandHandler>>,
    aliases: HashMap<String, String>,
}

impl CommandRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            handlers: HashMap::new(),
            aliases: HashMap::new(),
        };
        
        // Register built-in handlers
        registry.register_builtin_handlers();
        registry
    }
    
    pub fn register(&mut self, handler: Arc<dyn CommandHandler>) -> Result<()> {
        let name = handler.name().to_string();
        if self.handlers.contains_key(&name) {
            return Err(anyhow!("Handler '{}' already registered", name));
        }
        self.handlers.insert(name, handler);
        Ok(())
    }
    
    pub fn register_alias(&mut self, alias: &str, handler_name: &str) -> Result<()> {
        if !self.handlers.contains_key(handler_name) {
            return Err(anyhow!("Handler '{}' not found", handler_name));
        }
        self.aliases.insert(alias.to_string(), handler_name.to_string());
        Ok(())
    }
    
    pub fn get(&self, name: &str) -> Option<Arc<dyn CommandHandler>> {
        // Check direct name first, then aliases
        self.handlers.get(name)
            .or_else(|| {
                self.aliases.get(name)
                    .and_then(|real_name| self.handlers.get(real_name))
            })
            .cloned()
    }
    
    pub fn list_handlers(&self) -> Vec<HandlerInfo> {
        self.handlers.values()
            .map(|h| HandlerInfo {
                name: h.name().to_string(),
                description: h.description().to_string(),
                supports_retry: h.supports_retry(),
            })
            .collect()
    }
    
    fn register_builtin_handlers(&mut self) {
        // Register core handlers
        self.register(Arc::new(ShellHandler::new())).unwrap();
        self.register(Arc::new(ClaudeHandler::new())).unwrap();
        self.register(Arc::new(GitHandler::new())).unwrap();
        self.register(Arc::new(CargoHandler::new())).unwrap();
        self.register(Arc::new(FileHandler::new())).unwrap();
        
        // Register aliases for backward compatibility
        self.register_alias("test", "shell").unwrap(); // test: -> shell: with retry
    }
}
```

#### 3. Attribute Schema System

```rust
// src/commands/attributes.rs
#[derive(Debug, Clone)]
pub struct AttributeSchema {
    /// Required attributes
    required: Vec<AttributeDef>,
    /// Optional attributes
    optional: Vec<AttributeDef>,
    /// Whether to allow unknown attributes
    allow_unknown: bool,
}

#[derive(Debug, Clone)]
pub struct AttributeDef {
    name: String,
    attr_type: AttributeType,
    description: String,
    default: Option<serde_json::Value>,
    validator: Option<Box<dyn Fn(&serde_json::Value) -> Result<()>>>,
}

#[derive(Debug, Clone)]
pub enum AttributeType {
    String,
    Integer,
    Boolean,
    List(Box<AttributeType>),
    Map(Box<AttributeType>),
    Duration,  // Parses to Duration
    Path,      // Validates as path
    Command,   // Another command reference
    Any,       // Any JSON value
}

// Common attributes shared by all handlers
pub struct CommonAttributes {
    pub id: Option<String>,
    pub timeout: Option<Duration>,
    pub env: HashMap<String, String>,
    pub working_dir: Option<PathBuf>,
    pub capture_output: bool,
    pub commit_required: bool,
    pub analysis: Option<AnalysisConfig>,
    pub on_failure: Option<FailureHandler>,
    pub on_success: Option<SuccessHandler>,
    pub on_exit_code: HashMap<i32, ConditionalHandler>,
}
```

#### 4. Built-in Command Handlers

```rust
// src/commands/handlers/shell.rs
pub struct ShellHandler {
    subprocess: SubprocessManager,
}

impl ShellHandler {
    pub fn new() -> Self {
        Self {
            subprocess: SubprocessManager::new(),
        }
    }
}

#[async_trait]
impl CommandHandler for ShellHandler {
    fn name(&self) -> &str { "shell" }
    
    fn description(&self) -> &str {
        "Execute shell commands with full bash capabilities"
    }
    
    fn attribute_schema(&self) -> AttributeSchema {
        AttributeSchema {
            required: vec![],
            optional: vec![
                AttributeDef::new("shell", AttributeType::String)
                    .with_default("/bin/bash")
                    .with_description("Shell to use for execution"),
                AttributeDef::new("check", AttributeType::Boolean)
                    .with_default(true)
                    .with_description("Whether to check exit code"),
                AttributeDef::new("stdin", AttributeType::String)
                    .with_description("Input to provide on stdin"),
            ],
            allow_unknown: false,
        }
    }
    
    async fn execute(
        &self,
        command: &str,
        attrs: &CommandAttributes,
        context: &mut ExecutionContext,
    ) -> Result<CommandResult> {
        let shell = attrs.get_string("shell").unwrap_or("/bin/bash");
        let check = attrs.get_bool("check").unwrap_or(true);
        
        // Variable interpolation
        let interpolated = context.interpolate(command);
        
        // Execute command
        let result = self.subprocess.run_command(
            &interpolated,
            shell,
            &context.env_vars,
            attrs.common.timeout,
        ).await?;
        
        // Check exit code if required
        if check && !result.success() {
            return Err(anyhow!("Command failed with exit code: {}", 
                result.exit_code.unwrap_or(-1)));
        }
        
        Ok(CommandResult {
            stdout: result.stdout,
            stderr: result.stderr,
            exit_code: result.exit_code,
            success: result.success(),
            duration: result.duration,
        })
    }
    
    fn supports_retry(&self) -> bool { true }
}

// src/commands/handlers/git.rs
pub struct GitHandler {
    git: GitOperations,
}

#[async_trait]
impl CommandHandler for GitHandler {
    fn name(&self) -> &str { "git" }
    
    fn description(&self) -> &str {
        "Git operations with enhanced error handling and retry logic"
    }
    
    fn attribute_schema(&self) -> AttributeSchema {
        AttributeSchema {
            required: vec![
                AttributeDef::new("operation", AttributeType::String)
                    .with_description("Git operation to perform")
                    .with_validator(|v| {
                        let op = v.as_str().unwrap();
                        if !["commit", "push", "pull", "status", "diff", "add"].contains(&op) {
                            return Err(anyhow!("Unknown git operation: {}", op));
                        }
                        Ok(())
                    }),
            ],
            optional: vec![
                AttributeDef::new("message", AttributeType::String)
                    .with_description("Commit message"),
                AttributeDef::new("files", AttributeType::List(Box::new(AttributeType::String)))
                    .with_description("Files to operate on"),
                AttributeDef::new("branch", AttributeType::String)
                    .with_description("Branch name"),
            ],
            allow_unknown: false,
        }
    }
    
    async fn execute(
        &self,
        _command: &str,  // Git handler uses attributes instead
        attrs: &CommandAttributes,
        context: &mut ExecutionContext,
    ) -> Result<CommandResult> {
        let operation = attrs.get_string("operation").unwrap();
        
        match operation.as_str() {
            "commit" => {
                let message = attrs.get_string("message")
                    .ok_or_else(|| anyhow!("Commit message required"))?;
                self.git.create_commit(&message).await?;
                Ok(CommandResult::success("Commit created"))
            }
            "add" => {
                let files = attrs.get_string_list("files")
                    .unwrap_or_else(|| vec![".".to_string()]);
                for file in files {
                    self.git.add_file(&file).await?;
                }
                Ok(CommandResult::success("Files added"))
            }
            // ... other operations
            _ => unreachable!()
        }
    }
}
```

#### 5. Workflow YAML Structure

```yaml
# New modular command syntax
- git:
    operation: add
    files: ["."]
  id: stage_files

- git:
    operation: commit
    message: "feat: add new feature"
  commit_required: true  # Common attribute

- cargo:
    command: test
    args: ["--verbose"]
    env:
      RUST_BACKTRACE: "1"
  on_failure:
    claude: "/mmm-debug-test --output ${cargo.output}"

- docker:
    operation: build
    dockerfile: Dockerfile
    tag: "myapp:latest"
    args:
      build_arg: value

- npm:
    command: install
    save_dev: true
    packages: ["eslint", "prettier"]

- python:
    script: |
      import analysis
      analysis.run()
    capture_output: true
  id: analysis_result

- file:
    operation: copy
    source: "template.txt"
    destination: "${analysis_result.output}/report.txt"
    
# Backward compatible
- claude: "/mmm-review"
- shell: "echo 'Hello World'"
```

### Architecture Changes

1. **New Module Structure**
   ```
   src/commands/
   ├── mod.rs              # CommandHandler trait and types
   ├── registry.rs         # Command registry implementation
   ├── attributes.rs       # Attribute schema and validation
   ├── context.rs          # ExecutionContext implementation
   ├── result.rs           # CommandResult types
   └── handlers/
       ├── mod.rs          # Handler exports
       ├── shell.rs        # Shell command handler
       ├── claude.rs       # Claude command handler  
       ├── git.rs          # Git operations handler
       ├── cargo.rs        # Cargo/Rust handler
       ├── docker.rs       # Docker handler
       ├── npm.rs          # NPM handler
       ├── python.rs       # Python script handler
       └── file.rs         # File operations handler
   ```

2. **Modified Workflow Executor**
   - Remove hardcoded command type handling
   - Use registry to dispatch commands
   - Standardize execution flow

3. **Configuration Updates**
   - Update `WorkflowCommand` enum to support new format
   - Add handler configuration in `.mmm/config.yml`
   - Support custom handler directories

### Data Structures

```rust
// Unified command representation
pub struct UnifiedCommand {
    /// Handler name (e.g., "shell", "git", "claude")
    pub handler: String,
    /// Command string (if applicable)
    pub command: Option<String>,
    /// Handler-specific attributes
    pub attributes: serde_json::Map<String, serde_json::Value>,
    /// Common attributes
    pub common: CommonAttributes,
}

// Execution context passed to handlers
pub struct ExecutionContext {
    /// Current working directory
    pub working_dir: PathBuf,
    /// Environment variables
    pub env_vars: HashMap<String, String>,
    /// Workflow variables for interpolation
    pub variables: HashMap<String, String>,
    /// Command outputs from previous steps
    pub outputs: HashMap<String, CommandResult>,
    /// User interaction handle
    pub interaction: Arc<dyn UserInteraction>,
    /// Subprocess manager
    pub subprocess: SubprocessManager,
    /// Session manager
    pub session: Arc<dyn SessionManager>,
}

// Standardized command result
pub struct CommandResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
    pub success: bool,
    pub duration: Duration,
    pub metadata: HashMap<String, serde_json::Value>,
}
```

### APIs and Interfaces

1. **Handler Registration API**
   ```rust
   // Programmatic registration
   let registry = CommandRegistry::global();
   registry.register(Arc::new(CustomHandler::new()))?;
   
   // Discovery from directory
   registry.discover_handlers("~/.mmm/handlers")?;
   ```

2. **Handler Development API**
   ```rust
   // Simplified handler creation
   #[derive(CommandHandler)]
   #[handler(name = "myhandler")]
   struct MyHandler {
       #[attribute(required)]
       operation: String,
       
       #[attribute(optional, default = "default")]
       mode: String,
   }
   ```

3. **Workflow Execution API**
   ```rust
   // Execute with registry
   let executor = WorkflowExecutor::new(registry);
   let result = executor.execute_step(&step, &context).await?;
   ```

## Dependencies

- **Prerequisites**: None (foundational change)
- **Affected Components**: 
  - `src/cook/workflow/executor.rs` - Major refactor
  - `src/cook/orchestrator.rs` - Update command execution
  - `src/config/command.rs` - Extend command parsing
- **External Dependencies**: None required

## Testing Strategy

- **Unit Tests**: 
  - Test each handler in isolation
  - Validate attribute schema enforcement
  - Test registry operations
  - Verify variable interpolation

- **Integration Tests**: 
  - Full workflow execution with mixed handlers
  - Error propagation and retry logic
  - Handler interaction and data flow
  - Backward compatibility tests

- **Performance Tests**: 
  - Measure command dispatch overhead
  - Registry lookup performance
  - Large workflow execution times

- **User Acceptance**: 
  - All example workflows work
  - New handlers are discoverable
  - Clear error messages

## Documentation Requirements

- **Code Documentation**: 
  - Document CommandHandler trait thoroughly
  - Example handler implementations
  - Inline documentation for all public APIs

- **User Documentation**: 
  - Handler development guide
  - Available handlers reference
  - Migration guide for custom workflows

- **Architecture Updates**: 
  - Update ARCHITECTURE.md with new command system
  - Document handler discovery mechanism
  - Explain extensibility model

## Implementation Notes

1. **Phased Implementation**
   - Phase 1: Core trait and registry
   - Phase 2: Migrate existing handlers
   - Phase 3: Add new example handlers
   - Phase 4: External handler support

2. **Backward Compatibility**
   - Keep existing command parsing as fallback
   - Automatically convert old format to new
   - Deprecation warnings for old syntax

3. **Error Handling**
   - Handler failures should be clearly attributed
   - Schema violations caught at parse time
   - Helpful suggestions for common mistakes

4. **Performance Considerations**
   - Lazy handler initialization
   - Cache handler lookups
   - Minimize allocation in hot paths

5. **Security Considerations**
   - Validate handler sources
   - Sandbox external handlers (future)
   - Audit command execution

## Migration and Compatibility

1. **Existing Workflows**
   - All existing workflows continue to work
   - Automatic conversion to new internal format
   - No user action required

2. **Gradual Adoption**
   - New handlers can be used immediately
   - Old syntax remains supported
   - Migration tools provided

3. **Breaking Changes**
   - None for existing workflows
   - New handlers require new syntax
   - Some internal APIs will change

4. **Migration Path**
   ```yaml
   # Old syntax (still works)
   - shell: "cargo test"
   
   # New syntax (more features)
   - cargo:
       command: test
       args: ["--verbose"]
       env:
         RUST_BACKTRACE: "1"
   ```

## Future Enhancements

1. **External Handler Plugins**
   - Load handlers from external crates
   - Dynamic library loading
   - Handler marketplace

2. **Handler Composition**
   - Chain handlers together
   - Conditional handler execution
   - Parallel handler execution

3. **Advanced Features**
   - Handler middleware
   - Custom retry strategies
   - Handler versioning

4. **Development Tools**
   - Handler testing framework
   - Handler scaffolding CLI
   - Handler debugging tools