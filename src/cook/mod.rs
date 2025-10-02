//! Cook module - orchestrates improvement sessions
//!
//! This module has been refactored to use a component-based architecture
//! with dependency injection for improved testability and maintainability.

pub mod command;
pub mod commit_tracker;
pub mod common_strings;
pub mod coordinators;
pub mod environment;
pub mod execution;
pub mod expression;
pub mod git_ops;
pub mod goal_seek;
pub mod input;
pub mod interaction;
pub mod orchestrator;
pub mod retry;
pub mod retry_state;
pub mod retry_v2;
pub mod session;
pub mod signal_handler;
pub mod workflow;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod mod_tests;

#[cfg(test)]
mod retry_state_tests;

#[cfg(test)]
mod dry_run_tests;

use crate::abstractions::git::RealGitOperations;
use crate::config::{workflow::WorkflowConfig, ConfigLoader};
use crate::unified_session::SessionId;
use anyhow::{anyhow, Context as _, Result};
use std::path::Path;
use std::sync::Arc;

// Re-export key types
pub use command::CookCommand;
pub use environment::{PathResolver, Platform};
pub use orchestrator::{CookConfig, CookOrchestrator, DefaultCookOrchestrator};

/// Main entry point for cook operations
pub async fn cook(mut cmd: CookCommand) -> Result<()> {
    // Save the original directory before any path changes
    let original_dir = std::env::current_dir()?;

    // Determine project path
    let project_path = if let Some(ref path) = cmd.path {
        // Expand tilde notation if present
        let expanded_path = if path.to_string_lossy().starts_with("~/") {
            let home = directories::BaseDirs::new()
                .ok_or_else(|| anyhow!("Could not determine base directories"))?
                .home_dir()
                .to_path_buf();
            home.join(
                path.strip_prefix("~/")
                    .context("Failed to strip ~/ prefix")?,
            )
        } else {
            path.clone()
        };

        // Resolve to absolute path
        let absolute_path = if expanded_path.is_absolute() {
            expanded_path
        } else {
            original_dir.join(&expanded_path)
        };

        // Validate path exists and is a directory
        if !absolute_path.exists() {
            return Err(anyhow!("Directory not found: {}", absolute_path.display()));
        }
        if !absolute_path.is_dir() {
            return Err(anyhow!(
                "Path is not a directory: {}",
                absolute_path.display()
            ));
        }

        // Check if it's a git repository (only required if using worktree or git operations)
        // Skip this check for batch/exec commands that don't need git
        let is_temp_workflow = cmd
            .playbook
            .to_str()
            .map(|s| s.contains("/tmp/") || s.contains("/var/folders/") || s.contains("Temp"))
            .unwrap_or(false);
        // Always require git except for temporary workflows
        let requires_git = !is_temp_workflow;
        if requires_git && !absolute_path.join(".git").exists() {
            return Err(anyhow!("Not a git repository: {}", absolute_path.display()));
        }

        // Change to the specified directory
        std::env::set_current_dir(&absolute_path).with_context(|| {
            format!("Failed to change to directory: {}", absolute_path.display())
        })?;

        absolute_path
    } else {
        original_dir.clone()
    };

    // Make playbook path absolute if it's relative (based on original directory)
    if !cmd.playbook.is_absolute() {
        cmd.playbook = original_dir.join(&cmd.playbook);
    }

    // Load configuration
    let config_loader = ConfigLoader::new().await?;
    config_loader
        .load_with_explicit_path(&project_path, None)
        .await?;
    let _config = config_loader.get_config();

    // Load workflow - this handles both regular and MapReduce workflows
    let (workflow, mapreduce_config) = load_workflow_with_mapreduce(&cmd).await?;

    // Create orchestrator with all dependencies
    let orchestrator = create_orchestrator(&project_path, &cmd).await?;

    // Create cook configuration
    let mut cook_config = CookConfig {
        command: cmd,
        project_path: Arc::new(project_path),
        workflow: Arc::new(workflow),
        mapreduce_config: None,
    };

    // If this is a MapReduce workflow, we need special handling
    if let Some(mr_config) = mapreduce_config {
        // Store the MapReduce config for the orchestrator to use
        cook_config.mapreduce_config = Some(Arc::new(mr_config));
    }

    // Run the orchestrator
    orchestrator.run(cook_config).await
}

/// Create the orchestrator with all dependencies
/// Create session management components
async fn create_session_components(
    project_path: &Path,
) -> Result<(
    SessionId,
    Arc<crate::unified_session::SessionManager>,
    Arc<crate::unified_session::CookSessionAdapter>,
)> {
    let session_id = SessionId::new();
    let storage = crate::storage::GlobalStorage::new()?;
    let storage2 = crate::storage::GlobalStorage::new()?;
    let unified_manager = Arc::new(crate::unified_session::SessionManager::new(storage2).await?);
    let session_manager = Arc::new(
        crate::unified_session::CookSessionAdapter::new(project_path.to_path_buf(), storage)
            .await?,
    );
    Ok((session_id, unified_manager, session_manager))
}

/// Create event logger for session
async fn create_event_logger(
    project_path: &Path,
    session_id: &str,
) -> Option<Arc<crate::cook::execution::events::EventLogger>> {
    match crate::storage::create_global_event_logger(project_path, session_id).await {
        Ok(logger) => Some(Arc::new(logger)),
        Err(e) => {
            tracing::warn!(
                "Failed to create event logger for session {}: {}",
                session_id,
                e
            );
            None
        }
    }
}

async fn create_orchestrator(
    project_path: &Path,
    cmd: &CookCommand,
) -> Result<Arc<dyn CookOrchestrator>> {
    // Create shared dependencies
    let git_operations = Arc::new(RealGitOperations::new());
    let subprocess = Arc::new(crate::subprocess::SubprocessManager::production());

    // Create runners - use multiple instances since RealCommandRunner is not Clone
    let command_runner1 = execution::runner::RealCommandRunner::new();
    let command_runner2 = execution::runner::RealCommandRunner::new();

    // Create base components
    let config_loader = Arc::new(ConfigLoader::new().await?);
    let worktree_manager = Arc::new(crate::worktree::WorktreeManager::new(
        project_path.to_path_buf(),
        subprocess.as_ref().clone(),
    )?);

    // Create session components
    let (session_id, unified_manager, session_manager) =
        create_session_components(project_path).await?;

    // Create user interaction with verbosity from command args
    let verbosity = interaction::VerbosityLevel::from_args(cmd.verbosity, cmd.quiet);
    let user_interaction = Arc::new(interaction::DefaultUserInteraction::with_verbosity(
        verbosity,
    ));

    // Create executors
    let command_executor = Arc::new(command_runner1);

    // Create event logger for Claude streaming logs
    let event_logger = create_event_logger(project_path, &session_id.to_string()).await;

    let claude_executor = Arc::new({
        let mut executor = execution::claude::ClaudeExecutorImpl::new(command_runner2)
            .with_verbosity(cmd.verbosity);
        if let Some(logger) = event_logger {
            executor = executor.with_event_logger(logger);
        }
        executor
    });

    // Create environment coordinator
    let _environment_coordinator = Arc::new(coordinators::DefaultEnvironmentCoordinator::new(
        config_loader,
        worktree_manager,
        git_operations.clone(),
    ));

    // Create session coordinator using UnifiedSessionManager directly
    let _session_coordinator = Arc::new(coordinators::DefaultSessionCoordinator::new(
        unified_manager.clone(),
        project_path.to_path_buf(),
    ));

    // Create execution coordinator
    let _execution_coordinator = Arc::new(coordinators::DefaultExecutionCoordinator::new(
        command_executor.clone(),
        claude_executor.clone(),
        subprocess.clone(),
    ));

    // Create workflow executor with dry_run support
    let workflow_executor: Arc<dyn workflow::WorkflowExecutor> = Arc::new(
        workflow::WorkflowExecutorImpl::new(
            claude_executor.clone(),
            session_manager.clone(),
            user_interaction.clone(),
        )
        .with_dry_run(cmd.dry_run),
    );

    // Create workflow coordinator
    let _workflow_coordinator = Arc::new(coordinators::DefaultWorkflowCoordinator::new(
        workflow_executor.clone(),
        user_interaction.clone(),
    ));

    // Create orchestrator with correct trait implementations
    Ok(Arc::new(DefaultCookOrchestrator::new(
        session_manager.clone(),
        command_executor.clone(),
        claude_executor.clone(),
        user_interaction.clone(),
        git_operations,
        (*subprocess).clone(),
    )))
}

/// Load workflow configuration with MapReduce support
async fn load_workflow_with_mapreduce(
    cmd: &CookCommand,
) -> Result<(
    WorkflowConfig,
    Option<crate::config::MapReduceWorkflowConfig>,
)> {
    // Always load from playbook since it's required
    load_playbook_with_mapreduce(&cmd.playbook).await
}

/// Load workflow configuration (backward compatibility)
#[allow(dead_code)]
async fn load_workflow(
    cmd: &CookCommand,
    _config: &crate::config::Config,
) -> Result<WorkflowConfig> {
    // Always load from playbook since it's required
    load_playbook(&cmd.playbook).await
}

/// Load workflow configuration from a playbook file with MapReduce support
async fn load_playbook_with_mapreduce(
    path: &Path,
) -> Result<(
    WorkflowConfig,
    Option<crate::config::MapReduceWorkflowConfig>,
)> {
    let content = tokio::fs::read_to_string(path)
        .await
        .context(format!("Failed to read playbook file: {}", path.display()))?;

    // Try to parse as YAML first, then fall back to JSON
    if path.extension().and_then(|s| s.to_str()) == Some("yml")
        || path.extension().and_then(|s| s.to_str()) == Some("yaml")
    {
        // First check if it's a MapReduce workflow by looking for mode: mapreduce
        if content.contains("mode: mapreduce") || content.contains("mode: \"mapreduce\"") {
            // Try to parse as MapReduce workflow
            match crate::config::parse_mapreduce_workflow(&content) {
                Ok(mapreduce_config) => {
                    // Return empty workflow config with the MapReduce config
                    Ok((
                        WorkflowConfig {
                            commands: vec![],
                            env: None,
                            secrets: None,
                            env_files: None,
                            profiles: None,
                            merge: None,
                        },
                        Some(mapreduce_config),
                    ))
                }
                Err(e) => {
                    let mut error_msg =
                        format!("Failed to parse MapReduce workflow: {}\n", path.display());
                    error_msg.push_str(&format!("\nOriginal error: {e}"));
                    error_msg.push_str("\n\nHint: Check that your MapReduce workflow follows the correct structure:");
                    error_msg.push_str("\n  - name, mode, map (required)");
                    error_msg.push_str("\n  - setup, reduce (optional)");
                    error_msg.push_str(
                        "\n  - map.agent_template.commands should be a list of WorkflowSteps",
                    );
                    Err(anyhow!(error_msg))
                }
            }
        } else {
            // Try to parse as regular workflow
            match serde_yaml::from_str::<WorkflowConfig>(&content) {
                Ok(config) => Ok((config, None)),
                Err(e) => {
                    // Try to provide more helpful error messages
                    let mut error_msg =
                        format!("Failed to parse YAML playbook: {}\n", path.display());

                    // Extract line and column info if available
                    if let Some(location) = e.location() {
                        error_msg.push_str(&format!(
                            "Error at line {}, column {}\n",
                            location.line(),
                            location.column()
                        ));

                        // Try to show the problematic line
                        if let Some(line) = content.lines().nth(location.line().saturating_sub(1)) {
                            error_msg.push_str(&format!("Problematic line: {line}\n"));
                            if location.column() > 0 {
                                error_msg.push_str(&format!(
                                    "{}^\n",
                                    " ".repeat(location.column().saturating_sub(1))
                                ));
                            }
                        }
                    }

                    error_msg.push_str(&format!("\nOriginal error: {e}"));

                    // Add hints for common issues with context from file
                    error_msg.push_str("\n\n=== FILE CONTENT ===");
                    error_msg.push_str("\nShowing file structure (first 10 non-empty lines):");
                    let mut shown = 0;
                    for (idx, line) in content.lines().enumerate() {
                        if shown >= 10 {
                            break;
                        }
                        if !line.trim().is_empty() {
                            error_msg.push_str(&format!("\n  {:3} | {}", idx + 1, line));
                            shown += 1;
                        }
                    }

                    // Provide helpful structure hints
                    if content.contains("claude:") || content.contains("shell:") {
                        error_msg.push_str("\n\n=== SUPPORTED FORMATS ===");
                        error_msg.push_str("\nProdigy supports two workflow formats:");
                        error_msg.push_str("\n\n1. Direct array (no wrapper):");
                        error_msg.push_str("\n   - shell: \"command1\"");
                        error_msg.push_str("\n   - claude: \"/command2\"");
                        error_msg.push_str("\n\n2. Object with commands field:");
                        error_msg.push_str("\n   commands:");
                        error_msg.push_str("\n     - shell: \"command1\"");
                        error_msg.push_str("\n     - claude: \"/command2\"");
                        error_msg.push_str("\n\nThe parse error above indicates the YAML structure doesn't match either format.");
                        error_msg.push_str("\nCheck for: indentation errors, missing fields, or invalid YAML syntax.");
                    }

                    Err(anyhow!(error_msg))
                }
            }
        }
    } else {
        // Default to JSON parsing
        match serde_json::from_str::<WorkflowConfig>(&content) {
            Ok(config) => Ok((config, None)),
            Err(e) => {
                let mut error_msg = format!("Failed to parse JSON playbook: {}\n", path.display());

                // JSON errors usually include line/column info
                error_msg.push_str(&format!("Error: {e}"));

                Err(anyhow!(error_msg))
            }
        }
    }
}

/// Load workflow configuration from a playbook file  
async fn load_playbook(path: &Path) -> Result<WorkflowConfig> {
    let content = tokio::fs::read_to_string(path)
        .await
        .context(format!("Failed to read playbook file: {}", path.display()))?;

    // Try to parse as YAML first, then fall back to JSON
    if path.extension().and_then(|s| s.to_str()) == Some("yml")
        || path.extension().and_then(|s| s.to_str()) == Some("yaml")
    {
        // Try to parse as regular workflow first
        match serde_yaml::from_str::<WorkflowConfig>(&content) {
            Ok(config) => Ok(config),
            Err(e) => {
                // Try to provide more helpful error messages
                let mut error_msg = format!("Failed to parse YAML playbook: {}\n", path.display());

                // Extract line and column info if available
                if let Some(location) = e.location() {
                    error_msg.push_str(&format!(
                        "Error at line {}, column {}\n",
                        location.line(),
                        location.column()
                    ));

                    // Try to show the problematic line
                    if let Some(line) = content.lines().nth(location.line().saturating_sub(1)) {
                        error_msg.push_str(&format!("Problematic line: {line}\n"));
                        if location.column() > 0 {
                            error_msg.push_str(&format!(
                                "{}^\n",
                                " ".repeat(location.column().saturating_sub(1))
                            ));
                        }
                    }
                }

                error_msg.push_str(&format!("\nOriginal error: {e}"));

                // Add hints for common issues with context from file
                error_msg.push_str("\n\n=== FILE CONTENT ===");
                error_msg.push_str("\nShowing file structure (first 10 non-empty lines):");
                let mut shown = 0;
                for (idx, line) in content.lines().enumerate() {
                    if shown >= 10 {
                        break;
                    }
                    if !line.trim().is_empty() {
                        error_msg.push_str(&format!("\n  {:3} | {}", idx + 1, line));
                        shown += 1;
                    }
                }

                // Provide helpful structure hints
                if content.contains("claude:") || content.contains("shell:") {
                    error_msg.push_str("\n\n=== SUPPORTED FORMATS ===");
                    error_msg.push_str("\nProdigy supports two workflow formats:");
                    error_msg.push_str("\n\n1. Direct array (no wrapper):");
                    error_msg.push_str("\n   - shell: \"command1\"");
                    error_msg.push_str("\n   - claude: \"/command2\"");
                    error_msg.push_str("\n\n2. Object with commands field:");
                    error_msg.push_str("\n   commands:");
                    error_msg.push_str("\n     - shell: \"command1\"");
                    error_msg.push_str("\n     - claude: \"/command2\"");
                    error_msg.push_str("\n\nThe parse error above indicates the YAML structure doesn't match either format.");
                    error_msg.push_str(
                        "\nCheck for: indentation errors, missing fields, or invalid YAML syntax.",
                    );
                }

                Err(anyhow!(error_msg))
            }
        }
    } else {
        // Default to JSON parsing
        match serde_json::from_str::<WorkflowConfig>(&content) {
            Ok(config) => Ok(config),
            Err(e) => {
                let mut error_msg = format!("Failed to parse JSON playbook: {}\n", path.display());

                // JSON errors usually include line/column info
                error_msg.push_str(&format!("Error: {e}"));

                Err(anyhow!(error_msg))
            }
        }
    }
}

/// Legacy function for backward compatibility
/// Delegates to the new orchestrator
pub async fn run_improvement_loop(
    cmd: CookCommand,
    _session: &crate::worktree::WorktreeSession,
    _worktree_manager: &crate::worktree::WorktreeManager,
    _verbose: bool,
) -> Result<()> {
    // Simply delegate to the new cook function
    cook(cmd).await
}

#[cfg(test)]
mod cook_tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_create_orchestrator() {
        let temp_dir = TempDir::new().unwrap();
        let cmd = CookCommand {
            playbook: PathBuf::from("test.yml"),
            path: None,
            max_iterations: 1,
            map: vec![],
            args: vec![],
            fail_fast: false,
            auto_accept: false,
            metrics: false,
            resume: None,
            verbosity: 0,
            quiet: false,
            dry_run: false,
        };
        let orchestrator = create_orchestrator(temp_dir.path(), &cmd).await.unwrap();

        // Should create orchestrator successfully - just check it exists by trying to drop it
        drop(orchestrator);
    }

    #[tokio::test]
    async fn test_load_workflow_default() {
        let temp_dir = TempDir::new().unwrap();
        let playbook_path = temp_dir.path().join("test.yml");

        // Create a simple test workflow
        let workflow_content = r#"commands:
  - "prodigy-code-review"
  - name: "prodigy-lint"
    focus: "performance"
"#;
        tokio::fs::write(&playbook_path, workflow_content)
            .await
            .unwrap();

        let cmd = CookCommand {
            playbook: playbook_path,
            path: None,
            max_iterations: 5,
            map: vec![],
            args: vec![],
            fail_fast: false,
            metrics: false,
            auto_accept: false,
            resume: None,
            verbosity: 0,
            quiet: false,
            dry_run: false,
        };

        let config = crate::config::Config::default();
        let workflow = load_workflow(&cmd, &config).await.unwrap();

        // Should load default workflow
        assert!(!workflow.commands.is_empty());
        assert_eq!(workflow.commands.len(), 2);
    }

    #[tokio::test]
    async fn test_load_mapreduce_workflow() {
        let temp_dir = TempDir::new().unwrap();
        let playbook_path = temp_dir.path().join("mapreduce.yml");

        // Create a MapReduce workflow matching the debtmap-mapreduce.yml structure
        let workflow_content = r#"name: test-mapreduce
mode: mapreduce

setup:
  - shell: "echo setup"

map:
  input: test.json
  json_path: "$.items[*]"
  agent_template:
    commands:
      - claude: "/fix-item ${item.id}"
      - shell: "echo test"
  max_parallel: 5

reduce:
  commands:
    - claude: "/summarize ${map.results}"
    - shell: "echo done"
"#;
        tokio::fs::write(&playbook_path, workflow_content)
            .await
            .unwrap();

        // Try to load the MapReduce workflow
        let result = load_playbook_with_mapreduce(&playbook_path).await;

        // Debug the error if it fails
        match &result {
            Ok((workflow, mapreduce_config)) => {
                println!("Successfully loaded MapReduce workflow");
                // Should have empty workflow commands and a MapReduce config
                assert_eq!(workflow.commands.len(), 0);
                assert!(mapreduce_config.is_some(), "Should have MapReduce config");
                let mr_config = mapreduce_config.as_ref().unwrap();
                assert_eq!(mr_config.name, "test-mapreduce");
                assert_eq!(mr_config.mode, "mapreduce");
            }
            Err(e) => {
                panic!("Failed to load MapReduce workflow: {e:#}");
            }
        }
    }

    #[tokio::test]
    async fn test_load_debtmap_mapreduce_workflow() {
        let temp_dir = TempDir::new().unwrap();
        let playbook_path = temp_dir.path().join("debtmap-mapreduce.yml");

        // Use the exact content from the problematic file
        let workflow_content = r#"name: debtmap-parallel-elimination
mode: mapreduce

# Setup phase: Analyze the codebase and generate debt items
setup:
  - shell: "just coverage-lcov"
    
  - shell: "debtmap analyze . --lcov target/coverage/info.lcov --output debtmap.json --format json && git add debtmap.json && git commit -m 'Add debtmap.json'"
    commit_required: true

# Map phase: Process each debt item in parallel
map:
  # Input configuration - debtmap.json contains items array
  input: debtmap.json
  json_path: "$.items[*]"
  
  # Commands to execute for each debt item
  agent_template:
    commands:
      # Fix the specific debt item
      - claude: "/fix-debt-item --file ${item.location.file} --function ${item.location.function} --line ${item.location.line} --score ${item.unified_score.final_score}"
        capture_output: true
        timeout: 300
      
      # Run tests to verify the fix
      - shell: "just test"
        on_failure:
          claude: "/prodigy-debug-test-failure --output '${shell.output}'"
          max_attempts: 2
          fail_workflow: false
      
      # Run linting
      - shell: "just fmt && just lint"
        on_failure:
          claude: "/prodigy-lint '${shell.output}'"
          max_attempts: 2
          fail_workflow: false
  
  # Parallelization settings
  max_parallel: 5  # Run up to 5 agents in parallel
  
  # Process high-score items first
  filter: "unified_score.final_score >= 5"  # Only process items with score >= 5
  sort_by: "unified_score.final_score DESC"  # Process highest score items first
  max_items: 10  # Limit to 10 items per run

# Reduce phase: Aggregate results and finalize
reduce:
  commands:
    # Generate summary report
    - claude: "/summarize-debt-fixes --results '${map.results}' --successful ${map.successful} --failed ${map.failed}"
      capture_output: true
    
    # Run full test suite after all fixes
    - shell: "just test"
      on_failure:
        claude: "/prodigy-debug-test-failure --output '${shell.output}'"
        max_attempts: 3
        fail_workflow: true  # Fail if tests don't pass after merging
    
    # Run formatting and linting
    - shell: "just fmt && just lint"
      capture_output: None
    
    # Regenerate debt analysis to see improvement
    - claude: "/debtmap --compare-before"
      capture_output: true
    
    # Create final commit
    - shell: |
        git add -A && git commit -m "fix: eliminate ${map.successful} technical debt items via MapReduce
        
        Processed ${map.total} debt items in parallel:
        - Successfully fixed: ${map.successful} items
        - Failed to fix: ${map.failed} items
        
        This commit represents the aggregated work of multiple parallel agents."
      commit_required: true
"#;
        tokio::fs::write(&playbook_path, workflow_content)
            .await
            .unwrap();

        // Try to load the MapReduce workflow
        let result = load_playbook_with_mapreduce(&playbook_path).await;

        // Debug the error if it fails
        match &result {
            Ok((workflow, mapreduce_config)) => {
                println!("Successfully loaded debtmap MapReduce workflow");
                assert_eq!(workflow.commands.len(), 0);
                assert!(mapreduce_config.is_some(), "Should have MapReduce config");
                let mr_config = mapreduce_config.as_ref().unwrap();
                assert_eq!(mr_config.name, "debtmap-parallel-elimination");
                assert_eq!(mr_config.mode, "mapreduce");
            }
            Err(e) => {
                panic!("Failed to load debtmap MapReduce workflow: {e:#}");
            }
        }
    }

    #[tokio::test]
    async fn test_yaml_error_messages() {
        let temp_dir = TempDir::new().unwrap();

        // Test case 1: Invalid YAML syntax
        let playbook_path = temp_dir.path().join("invalid.yml");
        let invalid_content = r#"commands:
  - claude: "/prodigy-coverage"
    id: coverage
      commit_required: false  # Wrong indentation
"#;
        tokio::fs::write(&playbook_path, invalid_content)
            .await
            .unwrap();

        let err = load_playbook(&playbook_path).await.unwrap_err();
        let err_msg = err.to_string();
        assert!(err_msg.contains("Error at line"));
        assert!(err_msg.contains("column"));
        assert!(err_msg.contains("commit_required: false"));

        // Test case 2: Wrong structure that triggers new syntax hint
        let playbook_path2 = temp_dir.path().join("new_syntax.yml");
        let new_syntax_content = r#"commands:
  - claude: "/prodigy-coverage"
    outputs:
      spec:
        file_pattern: "*.md"
      invalid_field:  # Wrong field at wrong level
        something: true
"#;
        tokio::fs::write(&playbook_path2, new_syntax_content)
            .await
            .unwrap();

        let err2 = load_playbook(&playbook_path2).await.unwrap_err();
        let err_msg2 = err2.to_string();
        assert!(err_msg2.contains("claude:") || err_msg2.contains("shell:"));
    }

    #[tokio::test]
    async fn test_run_improvement_loop() {
        // Create a test playbook
        let temp_dir = TempDir::new().unwrap();
        let playbook_path = temp_dir.path().join("test.yml");

        // Create a minimal workflow
        let workflow_content = r#"commands:
  - "prodigy-lint"
"#;
        tokio::fs::write(&playbook_path, workflow_content)
            .await
            .unwrap();

        // Create test command
        let cmd = CookCommand {
            playbook: playbook_path,
            path: Some(temp_dir.path().to_path_buf()),
            max_iterations: 1,
            map: vec![],
            args: vec![],
            fail_fast: false,
            metrics: false,
            auto_accept: false,
            resume: None,
            verbosity: 0,
            quiet: false,
            dry_run: false,
        };

        // Create dummy session and worktree manager (not used in the function)
        let session = crate::worktree::WorktreeSession::new(
            "test-session".to_string(),
            "test-branch".to_string(),
            temp_dir.path().to_path_buf(),
        );
        let subprocess = crate::subprocess::SubprocessManager::production();
        let worktree_manager =
            crate::worktree::WorktreeManager::new(temp_dir.path().to_path_buf(), subprocess)
                .unwrap();

        // Note: This will fail in tests because no Claude API is available
        // but we're just testing that the function delegates correctly
        let result = run_improvement_loop(cmd, &session, &worktree_manager, false).await;

        // Should fail due to missing Claude API, but that's expected
        assert!(result.is_err());
    }
}
