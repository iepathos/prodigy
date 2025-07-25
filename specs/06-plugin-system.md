# Feature: Plugin System

## Objective
Create an extensible plugin architecture that allows users to extend mmm's functionality with custom commands, integrations, and behaviors without modifying core code.

## Acceptance Criteria
- [ ] Plugin discovery and loading system
- [ ] Sandboxed plugin execution environment
- [ ] Plugin API with hooks and events
- [ ] Plugin marketplace/registry
- [ ] Version compatibility management
- [ ] Plugin configuration and state
- [ ] Hot-reload capability for development
- [ ] Security model for plugin permissions

## Technical Details

### Plugin Architecture

```rust
pub trait Plugin: Send + Sync {
    fn metadata(&self) -> PluginMetadata;
    fn init(&mut self, context: PluginContext) -> Result<()>;
    fn shutdown(&mut self) -> Result<()>;
}

pub struct PluginMetadata {
    pub name: String,
    pub version: Version,
    pub author: String,
    pub description: String,
    pub homepage: Option<String>,
    pub license: String,
    pub capabilities: Vec<Capability>,
    pub dependencies: Vec<Dependency>,
}

pub enum Capability {
    Command { name: String, aliases: Vec<String> },
    Hook { event: String },
    Integration { service: String },
    Reporter { format: String },
    Analyzer { name: String },
}

pub struct PluginContext {
    pub config: Config,
    pub api: Box<dyn PluginAPI>,
    pub event_bus: EventBus,
    pub logger: Logger,
}
```

### Plugin Types

1. **Command Plugins**
```rust
pub trait CommandPlugin: Plugin {
    fn execute(&self, args: CommandArgs) -> Result<CommandResult>;
    fn autocomplete(&self, partial: &str) -> Vec<String>;
}

// Example: Git integration plugin
pub struct GitPlugin;
impl CommandPlugin for GitPlugin {
    fn execute(&self, args: CommandArgs) -> Result<CommandResult> {
        match args.subcommand.as_str() {
            "commit" => self.create_commit_from_spec(),
            "pr" => self.create_pull_request(),
            _ => Err(anyhow!("Unknown subcommand")),
        }
    }
}
```

2. **Hook Plugins**
```rust
pub trait HookPlugin: Plugin {
    fn on_event(&mut self, event: Event) -> Result<Option<Action>>;
}

pub enum Event {
    BeforeSpecRun { spec: Spec },
    AfterSpecRun { spec: Spec, result: RunResult },
    WorkflowStageComplete { stage: String },
    ClaudeResponse { prompt: String, response: String },
}

// Example: Slack notification plugin
pub struct SlackPlugin {
    webhook_url: String,
}
impl HookPlugin for SlackPlugin {
    fn on_event(&mut self, event: Event) -> Result<Option<Action>> {
        match event {
            Event::WorkflowStageComplete { stage } => {
                self.send_notification(&format!("Stage {} completed", stage))?;
                Ok(None)
            }
            _ => Ok(None),
        }
    }
}
```

3. **Integration Plugins**
```rust
pub trait IntegrationPlugin: Plugin {
    fn authenticate(&mut self) -> Result<()>;
    fn sync(&self) -> Result<SyncResult>;
}

// Example: Jira integration
pub struct JiraPlugin {
    client: JiraClient,
}
impl IntegrationPlugin for JiraPlugin {
    fn sync(&self) -> Result<SyncResult> {
        // Sync specs with Jira issues
        // Update issue status based on spec completion
    }
}
```

### Plugin Manifest

```toml
# plugin.toml
[plugin]
name = "mmm-git-integration"
version = "0.1.0"
authors = ["Plugin Developer <dev@example.com>"]
description = "Git integration for mmm"
homepage = "https://github.com/user/mmm-git-integration"
license = "MIT"

[dependencies]
mmm = "^1.0"
git2 = "0.16"

[capabilities]
commands = ["git"]
hooks = ["after_spec_run", "before_commit"]

[permissions]
filesystem = ["read", "write"]
network = ["github.com", "gitlab.com"]
environment = ["GIT_*"]

[config]
default_branch = "main"
auto_commit = false
commit_template = "feat(mmm): {spec_name}"
```

### Plugin API

```rust
pub trait PluginAPI {
    // Project management
    fn get_current_project(&self) -> Result<Project>;
    fn get_spec(&self, name: &str) -> Result<Spec>;
    fn update_spec_status(&self, name: &str, status: SpecStatus) -> Result<()>;
    
    // Claude interaction
    fn claude_request(&self, prompt: &str, options: ClaudeOptions) -> Result<String>;
    fn get_claude_history(&self, spec: &str) -> Result<Vec<Exchange>>;
    
    // State management
    fn get_state(&self, key: &str) -> Result<Option<Value>>;
    fn set_state(&self, key: &str, value: Value) -> Result<()>;
    
    // Events
    fn emit_event(&self, event: Event) -> Result<()>;
    fn subscribe_to_event(&self, event_type: &str, callback: EventCallback) -> Result<()>;
    
    // UI/Output
    fn prompt_user(&self, message: &str, options: PromptOptions) -> Result<String>;
    fn display_progress(&self, message: &str, progress: f32) -> Result<()>;
    fn log(&self, level: LogLevel, message: &str) -> Result<()>;
}
```

### Plugin Loading and Discovery

```rust
pub struct PluginManager {
    registry: PluginRegistry,
    loader: PluginLoader,
    sandbox: PluginSandbox,
}

impl PluginManager {
    pub fn discover_plugins(&mut self) -> Result<Vec<PluginInfo>> {
        let mut plugins = vec![];
        
        // Search paths
        let search_paths = vec![
            dirs::home_dir().unwrap().join(".mmm/plugins"),
            PathBuf::from("/usr/local/lib/mmm/plugins"),
            env::current_dir()?.join(".mmm/plugins"),
        ];
        
        for path in search_paths {
            if path.exists() {
                plugins.extend(self.scan_directory(&path)?);
            }
        }
        
        Ok(plugins)
    }
    
    pub fn load_plugin(&mut self, name: &str) -> Result<Box<dyn Plugin>> {
        let plugin_info = self.registry.get(name)?;
        
        // Version compatibility check
        if !self.is_compatible(&plugin_info.required_version)? {
            return Err(anyhow!("Incompatible plugin version"));
        }
        
        // Load in sandbox
        let plugin = self.sandbox.load(&plugin_info.path)?;
        
        // Verify permissions
        self.verify_permissions(&plugin)?;
        
        Ok(plugin)
    }
}
```

### Plugin Sandbox

```rust
pub struct PluginSandbox {
    wasm_runtime: Option<WasmRuntime>,
    process_isolation: bool,
}

impl PluginSandbox {
    pub fn execute<F, R>(&self, plugin: &dyn Plugin, op: F) -> Result<R>
    where
        F: FnOnce() -> Result<R>,
    {
        if self.process_isolation {
            // Run in separate process
            self.execute_isolated(op)
        } else if let Some(runtime) = &self.wasm_runtime {
            // Run in WASM sandbox
            runtime.execute(op)
        } else {
            // Direct execution (development mode)
            op()
        }
    }
}
```

### Plugin Marketplace

```rust
pub struct PluginMarketplace {
    registry_url: String,
    cache_dir: PathBuf,
}

impl PluginMarketplace {
    pub async fn search(&self, query: &str) -> Result<Vec<PluginListing>> {
        let response = reqwest::get(&format!("{}/search?q={}", self.registry_url, query))
            .await?
            .json::<SearchResponse>()
            .await?;
            
        Ok(response.plugins)
    }
    
    pub async fn install(&self, plugin_name: &str) -> Result<()> {
        // Download plugin
        let plugin_data = self.download_plugin(plugin_name).await?;
        
        // Verify signature
        self.verify_signature(&plugin_data)?;
        
        // Extract to plugins directory
        self.extract_plugin(&plugin_data)?;
        
        // Run post-install hooks
        self.run_post_install(plugin_name)?;
        
        Ok(())
    }
}
```

### Plugin Development Kit

```bash
# Create new plugin
mmm plugin create my-plugin --template command

# Plugin development commands
mmm plugin build
mmm plugin test
mmm plugin lint
mmm plugin package
mmm plugin publish

# Hot reload during development
mmm plugin dev --watch
```

Plugin template structure:
```
my-plugin/
├── Cargo.toml          # For Rust plugins
├── plugin.toml         # Plugin manifest
├── src/
│   ├── lib.rs         # Plugin entry point
│   └── commands.rs    # Command implementations
├── tests/
│   └── integration.rs
└── README.md
```

### Security Model

```rust
pub struct PermissionManager {
    granted_permissions: HashMap<PluginId, HashSet<Permission>>,
}

pub enum Permission {
    FileSystem { path: PathBuf, access: FileAccess },
    Network { hosts: Vec<String> },
    Environment { vars: Vec<String> },
    Command { executable: String },
    State { scope: StateScope },
}

pub enum FileAccess {
    Read,
    Write,
    Execute,
}

pub enum StateScope {
    Plugin,      // Only plugin's own state
    Project,     // Current project state
    Global,      // Global mmm state
}

// Permission request during plugin installation
impl PermissionManager {
    pub fn request_permissions(&self, plugin: &PluginMetadata) -> Result<bool> {
        println!("Plugin '{}' requests the following permissions:", plugin.name);
        for perm in &plugin.requested_permissions {
            println!("  - {}", perm);
        }
        
        let response = prompt_user("Grant these permissions? [y/N]: ")?;
        Ok(response.to_lowercase() == "y")
    }
}
```