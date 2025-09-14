---
number: 74
title: Environment Variables and Working Directory Control
category: foundation
priority: medium
status: draft
dependencies: []
created: 2025-01-14
---

# Specification 74: Environment Variables and Working Directory Control

**Category**: foundation
**Priority**: medium
**Status**: draft
**Dependencies**: None

## Context

The normalized workflow structure shows support for per-step environment variables and working directories, but these features aren't fully exposed in the workflow configuration. Fine-grained control over execution environment is essential for complex workflows that interact with different tools and services.

## Objective

Implement comprehensive environment variable management and working directory control at global, workflow, and step levels, enabling precise configuration of execution contexts for different commands and phases.

## Requirements

### Functional Requirements
- Global environment variables for entire workflow
- Per-step environment variable override
- Per-step working directory specification
- Environment variable interpolation
- Secret management for sensitive values
- Environment inheritance control
- Dynamic environment based on conditions
- Environment snapshots and restoration
- Cross-platform path handling

### Non-Functional Requirements
- Secure handling of sensitive values
- Efficient environment switching
- Clear environment visibility in logs
- Platform-agnostic path resolution

## Acceptance Criteria

- [ ] Global `env:` section sets workflow-wide variables
- [ ] Step-level `env:` overrides global settings
- [ ] `working_dir:` changes execution directory
- [ ] `${env.VAR}` interpolates environment variables
- [ ] Secrets masked in logs
- [ ] Environment inheritance configurable
- [ ] Conditional environment application
- [ ] Path resolution works cross-platform
- [ ] Environment changes isolated per step
- [ ] Clear logging of environment context

## Technical Details

### Implementation Approach

1. **Environment Configuration Structure**:
   ```rust
   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct EnvironmentConfig {
       /// Global environment variables
       #[serde(default)]
       pub global_env: HashMap<String, EnvValue>,

       /// Secret environment variables (masked in logs)
       #[serde(default)]
       pub secrets: HashMap<String, SecretValue>,

       /// Environment files to load
       #[serde(default)]
       pub env_files: Vec<PathBuf>,

       /// Inherit from parent process
       #[serde(default = "default_true")]
       pub inherit: bool,

       /// Environment profiles
       #[serde(default)]
       pub profiles: HashMap<String, EnvProfile>,

       /// Active profile
       #[serde(skip_serializing_if = "Option::is_none")]
       pub active_profile: Option<String>,
   }

   #[derive(Debug, Clone, Serialize, Deserialize)]
   #[serde(untagged)]
   pub enum EnvValue {
       Static(String),
       Dynamic(DynamicEnv),
       Conditional(ConditionalEnv),
   }

   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct DynamicEnv {
       pub command: String,
       pub cache: bool,
   }

   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct ConditionalEnv {
       pub condition: String,
       pub when_true: String,
       pub when_false: String,
   }

   #[derive(Debug, Clone)]
   pub struct SecretValue {
       encrypted: Vec<u8>,
       provider: SecretProvider,
   }

   #[derive(Debug, Clone, Serialize, Deserialize)]
   pub struct StepEnvironment {
       /// Step-specific environment variables
       #[serde(default)]
       pub env: HashMap<String, String>,

       /// Working directory for this step
       #[serde(skip_serializing_if = "Option::is_none")]
       pub working_dir: Option<PathBuf>,

       /// Clear parent environment
       #[serde(default)]
       pub clear_env: bool,

       /// Temporary environment (restored after step)
       #[serde(default)]
       pub temporary: bool,
   }
   ```

2. **Environment Manager**:
   ```rust
   pub struct EnvironmentManager {
       base_env: HashMap<String, String>,
       secrets: SecretStore,
       profiles: HashMap<String, EnvProfile>,
       current_dir: PathBuf,
       env_stack: Vec<EnvironmentSnapshot>,
   }

   #[derive(Clone)]
   struct EnvironmentSnapshot {
       env: HashMap<String, String>,
       working_dir: PathBuf,
   }

   impl EnvironmentManager {
       pub async fn setup_step_environment(
           &mut self,
           step: &WorkflowStep,
           global_config: &EnvironmentConfig,
       ) -> Result<EnvironmentContext> {
           // Start with base environment
           let mut env = if step.clear_env {
               HashMap::new()
           } else if global_config.inherit {
               self.get_inherited_env()?
           } else {
               self.base_env.clone()
           };

           // Apply global environment
           for (key, value) in &global_config.global_env {
               let resolved = self.resolve_env_value(value).await?;
               env.insert(key.clone(), resolved);
           }

           // Apply profile if active
           if let Some(profile_name) = &global_config.active_profile {
               self.apply_profile(&mut env, profile_name)?;
           }

           // Apply step-specific environment
           if let Some(step_env) = &step.env {
               for (key, value) in step_env {
                   env.insert(key.clone(), self.interpolate(value)?);
               }
           }

           // Load secrets
           for (key, secret) in &global_config.secrets {
               let value = self.secrets.decrypt(secret).await?;
               env.insert(key.clone(), value);
           }

           // Set working directory
           let working_dir = if let Some(dir) = &step.working_dir {
               self.resolve_path(dir)?
           } else {
               self.current_dir.clone()
           };

           // Save snapshot if temporary
           if step.temporary {
               self.env_stack.push(EnvironmentSnapshot {
                   env: self.base_env.clone(),
                   working_dir: self.current_dir.clone(),
               });
           }

           Ok(EnvironmentContext {
               env,
               working_dir,
               secrets: self.get_secret_keys(&global_config.secrets),
           })
       }

       pub async fn restore_environment(&mut self) -> Result<()> {
           if let Some(snapshot) = self.env_stack.pop() {
               self.base_env = snapshot.env;
               self.current_dir = snapshot.working_dir;
           }
           Ok(())
       }

       async fn resolve_env_value(&self, value: &EnvValue) -> Result<String> {
           match value {
               EnvValue::Static(s) => Ok(s.clone()),
               EnvValue::Dynamic(d) => {
                   let output = Command::new("sh")
                       .arg("-c")
                       .arg(&d.command)
                       .output()
                       .await?;
                   Ok(String::from_utf8(output.stdout)?.trim().to_string())
               }
               EnvValue::Conditional(c) => {
                   let condition_met = self.evaluate_condition(&c.condition)?;
                   Ok(if condition_met {
                       c.when_true.clone()
                   } else {
                       c.when_false.clone()
                   })
               }
           }
       }

       fn resolve_path(&self, path: &Path) -> Result<PathBuf> {
           if path.is_absolute() {
               Ok(path.to_path_buf())
           } else {
               Ok(self.current_dir.join(path))
           }
       }
   }
   ```

3. **Command Execution with Environment**:
   ```rust
   impl CommandExecutor {
       pub async fn execute_with_environment(
           &self,
           command: &str,
           context: &EnvironmentContext,
       ) -> Result<CommandResult> {
           let mut cmd = Command::new("sh");
           cmd.arg("-c").arg(command);

           // Set environment variables
           cmd.env_clear();
           for (key, value) in &context.env {
               // Mask secrets in logs
               if context.secrets.contains(key) {
                   debug!("Setting secret environment variable: {}", key);
               } else {
                   debug!("Setting environment variable: {}={}", key, value);
               }
               cmd.env(key, value);
           }

           // Set working directory
           cmd.current_dir(&context.working_dir);
           info!("Executing in directory: {}", context.working_dir.display());

           // Execute command
           let output = cmd.output().await?;

           // Mask secrets in output
           let stdout = self.mask_secrets(
               String::from_utf8_lossy(&output.stdout).to_string(),
               &context.secrets,
           );
           let stderr = self.mask_secrets(
               String::from_utf8_lossy(&output.stderr).to_string(),
               &context.secrets,
           );

           Ok(CommandResult {
               exit_code: output.status.code().unwrap_or(-1),
               stdout,
               stderr,
               success: output.status.success(),
           })
       }

       fn mask_secrets(&self, text: String, secret_keys: &[String]) -> String {
           let mut masked = text;
           for key in secret_keys {
               if let Ok(value) = std::env::var(key) {
                   masked = masked.replace(&value, "***MASKED***");
               }
           }
           masked
       }
   }
   ```

4. **Cross-Platform Path Resolution**:
   ```rust
   pub struct PathResolver {
       platform: Platform,
   }

   impl PathResolver {
       pub fn resolve(&self, path: &str) -> PathBuf {
           let expanded = self.expand_variables(path);
           let normalized = self.normalize_separators(&expanded);

           match self.platform {
               Platform::Windows => self.resolve_windows(&normalized),
               Platform::Unix => self.resolve_unix(&normalized),
           }
       }

       fn expand_variables(&self, path: &str) -> String {
           let mut result = path.to_string();

           // Expand ~ to home directory
           if result.starts_with("~/") {
               if let Ok(home) = std::env::var("HOME") {
                   result = result.replacen("~", &home, 1);
               }
           }

           // Expand environment variables
           lazy_static! {
               static ref ENV_VAR_RE: Regex = Regex::new(r"\$\{([^}]+)\}").unwrap();
           }

           for cap in ENV_VAR_RE.captures_iter(&result.clone()) {
               let var_name = &cap[1];
               if let Ok(value) = std::env::var(var_name) {
                   result = result.replace(&cap[0], &value);
               }
           }

           result
       }

       fn normalize_separators(&self, path: &str) -> String {
           match self.platform {
               Platform::Windows => path.replace('/', "\\"),
               Platform::Unix => path.replace('\\', "/"),
           }
       }
   }
   ```

### Architecture Changes
- Add `EnvironmentManager` to execution context
- Integrate with command executors
- Add secret management system
- Implement environment profiles
- Add path resolution utilities

### Data Structures
```yaml
# Global environment configuration
env:
  NODE_ENV: production
  API_URL: https://api.example.com
  WORKERS:
    command: "nproc"
    cache: true

secrets:
  API_KEY: ${vault:api/keys/production}
  DB_PASSWORD: ${env:SECRET_DB_PASS}

env_files:
  - .env.production
  - config/environment.yml

profiles:
  development:
    NODE_ENV: development
    API_URL: http://localhost:3000
    DEBUG: "true"

  testing:
    NODE_ENV: test
    API_URL: http://localhost:4000
    COVERAGE: "true"

# Step with environment override
tasks:
  - name: "Build application"
    shell: "npm run build"
    env:
      BUILD_TARGET: production
      OPTIMIZE: "true"
    working_dir: ./frontend

  - name: "Run tests"
    shell: "pytest"
    working_dir: ./backend
    env:
      PYTHONPATH: ./src:./tests
      TEST_ENV: "true"
    clear_env: false
    temporary: true  # Restore after step

  - name: "Deploy"
    shell: "./deploy.sh"
    working_dir: ${env.DEPLOY_DIR}
    env:
      DEPLOY_ENV:
        condition: "${branch} == 'main'"
        when_true: "production"
        when_false: "staging"
```

## Dependencies

- **Prerequisites**: None
- **Affected Components**:
  - `src/cook/execution/` - Environment application
  - `src/config/workflow.rs` - Environment configuration
  - `src/subprocess/` - Command execution with env
- **External Dependencies**: Platform-specific path handling

## Testing Strategy

- **Unit Tests**:
  - Environment variable resolution
  - Path normalization
  - Secret masking
  - Profile application
- **Integration Tests**:
  - End-to-end environment management
  - Working directory changes
  - Secret handling
  - Cross-platform paths
- **Security Tests**:
  - Secret masking in logs
  - Environment isolation
  - Sensitive value handling

## Documentation Requirements

- **Code Documentation**: Document environment resolution
- **User Documentation**:
  - Environment configuration guide
  - Secret management
  - Cross-platform considerations
  - Best practices
- **Architecture Updates**: Add environment flow to architecture

## Implementation Notes

- Use OS-specific APIs for secure environment handling
- Cache dynamic environment values when specified
- Support .env file formats
- Integrate with secret management systems
- Future: Container-based isolation

## Migration and Compatibility

- Workflows without env config use system environment
- Gradual adoption of environment features
- Backwards compatible with current execution
- Clear migration path for environment management