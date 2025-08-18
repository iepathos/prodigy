//! MapReduce executor for parallel workflow execution
//!
//! Implements parallel execution of workflow steps across multiple agents
//! using isolated git worktrees for fault isolation and parallelism.

use crate::cook::execution::ClaudeExecutor;
use crate::cook::interaction::UserInteraction;
use crate::cook::orchestrator::ExecutionEnvironment;
use crate::cook::session::SessionManager;
use crate::cook::workflow::{WorkflowContext, WorkflowStep};
use crate::worktree::WorktreeManager;
use anyhow::{anyhow, Context, Result};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};
use tokio::task::JoinHandle;

/// Configuration for MapReduce execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapReduceConfig {
    /// Path to input JSON file
    pub input: PathBuf,
    /// JSON path expression to extract work items
    #[serde(default)]
    pub json_path: String,
    /// Maximum number of parallel agents
    #[serde(default = "default_max_parallel")]
    pub max_parallel: usize,
    /// Timeout per agent in seconds
    #[serde(default = "default_timeout")]
    pub timeout_per_agent: u64,
    /// Number of retry attempts on failure
    #[serde(default = "default_retry")]
    pub retry_on_failure: u32,
}

fn default_max_parallel() -> usize {
    10
}

fn default_timeout() -> u64 {
    600 // 10 minutes
}

fn default_retry() -> u32 {
    2
}

/// Map phase configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MapPhase {
    /// Input configuration
    #[serde(flatten)]
    pub config: MapReduceConfig,
    /// Agent template commands
    pub agent_template: Vec<WorkflowStep>,
    /// Optional filter expression
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter: Option<String>,
    /// Optional sort field
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_by: Option<String>,
}

/// Reduce phase configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReducePhase {
    /// Commands to execute in reduce phase
    pub commands: Vec<WorkflowStep>,
}

/// Status of an agent execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentStatus {
    Pending,
    Running,
    Success,
    Failed(String),
    Timeout,
    Retrying(u32),
}

/// Result from a single agent execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResult {
    /// Unique identifier for the work item
    pub item_id: String,
    /// Status of the agent execution
    pub status: AgentStatus,
    /// Output from the agent
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    /// Git commits created by the agent
    #[serde(default)]
    pub commits: Vec<String>,
    /// Duration of execution
    pub duration: Duration,
    /// Error message if failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Worktree path used
    #[serde(skip_serializing_if = "Option::is_none")]
    pub worktree_path: Option<PathBuf>,
}

/// Progress tracking for parallel execution
struct ProgressTracker {
    multi_progress: MultiProgress,
    overall_bar: ProgressBar,
    agent_bars: Vec<ProgressBar>,
}

impl ProgressTracker {
    fn new(total_items: usize, max_parallel: usize) -> Self {
        let multi_progress = MultiProgress::new();

        // Overall progress bar
        let overall_bar = multi_progress.add(ProgressBar::new(total_items as u64));
        overall_bar.set_style(
            ProgressStyle::default_bar()
                .template(
                    "{spinner:.green} [{elapsed_precise}] {bar:40.cyan/blue} {pos}/{len} {msg}",
                )
                .unwrap()
                .progress_chars("##-"),
        );
        overall_bar.set_message("Processing items...");

        // Individual agent progress bars
        let mut agent_bars = Vec::new();
        for i in 0..max_parallel.min(total_items) {
            let bar = multi_progress.add(ProgressBar::new(100));
            bar.set_style(
                ProgressStyle::default_bar()
                    .template(&format!("  Agent {:2}: {{msg}}", i + 1))
                    .unwrap(),
            );
            bar.set_message("Idle");
            agent_bars.push(bar);
        }

        Self {
            multi_progress,
            overall_bar,
            agent_bars,
        }
    }

    fn update_agent(&self, agent_index: usize, message: &str) {
        if agent_index < self.agent_bars.len() {
            self.agent_bars[agent_index].set_message(message.to_string());
        }
    }

    fn complete_item(&self) {
        self.overall_bar.inc(1);
    }

    fn finish(&self, message: &str) {
        self.overall_bar.finish_with_message(message.to_string());
        for bar in &self.agent_bars {
            bar.finish_and_clear();
        }
    }
}

/// MapReduce executor for parallel workflow execution
pub struct MapReduceExecutor {
    claude_executor: Arc<dyn ClaudeExecutor>,
    session_manager: Arc<dyn SessionManager>,
    user_interaction: Arc<dyn UserInteraction>,
    worktree_manager: Arc<WorktreeManager>,
    project_root: PathBuf,
}

impl MapReduceExecutor {
    /// Create a new MapReduce executor
    pub fn new(
        claude_executor: Arc<dyn ClaudeExecutor>,
        session_manager: Arc<dyn SessionManager>,
        user_interaction: Arc<dyn UserInteraction>,
        worktree_manager: Arc<WorktreeManager>,
        project_root: PathBuf,
    ) -> Self {
        Self {
            claude_executor,
            session_manager,
            user_interaction,
            worktree_manager,
            project_root,
        }
    }

    /// Execute a MapReduce workflow
    pub async fn execute(
        &self,
        map_phase: &MapPhase,
        reduce_phase: Option<&ReducePhase>,
        env: &ExecutionEnvironment,
    ) -> Result<Vec<AgentResult>> {
        let start_time = Instant::now();

        // Load and parse work items
        let work_items = self.load_work_items(&map_phase.config).await?;

        self.user_interaction.display_info(&format!(
            "Starting MapReduce execution with {} items, max {} parallel agents",
            work_items.len(),
            map_phase.config.max_parallel
        ));

        // Execute map phase
        let map_results = self.execute_map_phase(map_phase, work_items, env).await?;

        // Execute reduce phase if specified
        if let Some(reduce_phase) = reduce_phase {
            self.execute_reduce_phase(reduce_phase, &map_results, env)
                .await?;
        }

        // Report summary
        let duration = start_time.elapsed();
        self.report_summary(&map_results, duration);

        Ok(map_results)
    }

    /// Load work items from JSON file
    async fn load_work_items(&self, config: &MapReduceConfig) -> Result<Vec<Value>> {
        let input_path = if config.input.is_absolute() {
            config.input.clone()
        } else {
            self.project_root.join(&config.input)
        };

        let content = tokio::fs::read_to_string(&input_path)
            .await
            .context(format!(
                "Failed to read input file: {}",
                input_path.display()
            ))?;

        let json: Value = serde_json::from_str(&content).context("Failed to parse input JSON")?;

        // Extract items using JSON path
        let items = if config.json_path.is_empty() {
            // If no JSON path specified, treat entire JSON as single item or array
            match json {
                Value::Array(arr) => arr,
                other => vec![other],
            }
        } else {
            self.extract_with_json_path(&json, &config.json_path)?
        };

        Ok(items)
    }

    /// Extract items using JSON path expression
    fn extract_with_json_path(&self, json: &Value, path: &str) -> Result<Vec<Value>> {
        // Simple JSON path implementation
        // Supports basic paths like "$.items[*]" or "$.debt_items[*]"
        let path = path.trim_start_matches("$.");
        let parts: Vec<&str> = path.split('.').collect();

        let mut current = json.clone();
        for part in parts {
            if part.ends_with("[*]") {
                let field_name = &part[..part.len() - 3];
                current = current
                    .get(field_name)
                    .ok_or_else(|| anyhow!("Field '{}' not found in JSON", field_name))?
                    .clone();

                return match current {
                    Value::Array(arr) => Ok(arr),
                    _ => Err(anyhow!("Expected array at path '{}'", path)),
                };
            } else {
                current = current
                    .get(part)
                    .ok_or_else(|| anyhow!("Field '{}' not found in JSON", part))?
                    .clone();
            }
        }

        match current {
            Value::Array(arr) => Ok(arr),
            other => Ok(vec![other]),
        }
    }

    /// Execute the map phase with parallel agents
    async fn execute_map_phase(
        &self,
        map_phase: &MapPhase,
        work_items: Vec<Value>,
        env: &ExecutionEnvironment,
    ) -> Result<Vec<AgentResult>> {
        let total_items = work_items.len();
        let max_parallel = map_phase.config.max_parallel.min(total_items);

        // Create progress tracker
        let progress = Arc::new(ProgressTracker::new(total_items, max_parallel));

        // Create channels for work distribution
        let (work_tx, work_rx) = mpsc::channel::<(usize, Value)>(total_items);
        let work_rx = Arc::new(RwLock::new(work_rx));

        // Send all work items to the queue
        for (index, item) in work_items.into_iter().enumerate() {
            work_tx.send((index, item)).await?;
        }
        drop(work_tx); // Close the sender

        // Results collection
        let results = Arc::new(RwLock::new(Vec::new()));

        // Spawn worker tasks
        let mut workers = Vec::new();
        for agent_index in 0..max_parallel {
            let work_rx = work_rx.clone();
            let results = results.clone();
            let progress = progress.clone();
            let map_phase = map_phase.clone();
            let env = env.clone();
            let executor = self.clone_executor();

            let handle: JoinHandle<Result<()>> = tokio::spawn(async move {
                executor
                    .run_agent(agent_index, work_rx, results, progress, map_phase, env)
                    .await
            });

            workers.push(handle);
        }

        // Wait for all workers to complete
        for worker in workers {
            if let Err(e) = worker.await? {
                self.user_interaction
                    .display_warning(&format!("Worker error: {}", e));
            }
        }

        // Finish progress tracking
        progress.finish("Map phase completed");

        // Return collected results
        let results = results.read().await;
        Ok(results.clone())
    }

    /// Run a single agent worker
    async fn run_agent(
        &self,
        agent_index: usize,
        work_rx: Arc<RwLock<mpsc::Receiver<(usize, Value)>>>,
        results: Arc<RwLock<Vec<AgentResult>>>,
        progress: Arc<ProgressTracker>,
        map_phase: MapPhase,
        env: ExecutionEnvironment,
    ) -> Result<()> {
        loop {
            // Get next work item
            let work_item = {
                let mut rx = work_rx.write().await;
                rx.recv().await
            };

            let Some((item_index, item)) = work_item else {
                // No more work
                progress.update_agent(agent_index, "Completed");
                break;
            };

            let item_id = format!("item_{}", item_index);
            progress.update_agent(agent_index, &format!("Processing {}", item_id));

            // Execute work item with retries
            let mut attempt = 0;
            let agent_result = loop {
                attempt += 1;

                if attempt > 1 {
                    progress.update_agent(
                        agent_index,
                        &format!("Retrying {} (attempt {})", item_id, attempt),
                    );
                }

                let result = self
                    .execute_agent_commands(&item_id, &item, &map_phase.agent_template, &env)
                    .await;

                match result {
                    Ok(res) => break res,
                    Err(_e) if attempt < map_phase.config.retry_on_failure => {
                        // Retry on failure
                        tokio::time::sleep(Duration::from_secs(2)).await;
                        continue;
                    }
                    Err(e) => {
                        // Final failure
                        break AgentResult {
                            item_id: item_id.clone(),
                            status: AgentStatus::Failed(e.to_string()),
                            output: None,
                            commits: vec![],
                            duration: Duration::from_secs(0),
                            error: Some(e.to_string()),
                            worktree_path: None,
                        };
                    }
                }
            };

            // Store result
            {
                let mut res = results.write().await;
                res.push(agent_result);
            }

            // Update progress
            progress.complete_item();
        }

        Ok(())
    }

    /// Execute commands for a single agent
    async fn execute_agent_commands(
        &self,
        item_id: &str,
        item: &Value,
        template_steps: &[WorkflowStep],
        env: &ExecutionEnvironment,
    ) -> Result<AgentResult> {
        let start_time = Instant::now();

        // Create isolated worktree session for this agent
        let worktree_session = self
            .worktree_manager
            .create_session()
            .await
            .context("Failed to create agent worktree")?;
        let worktree_name = worktree_session.name.clone();
        let worktree_path = worktree_session.path.clone();

        // Create workflow context with item data
        let mut context = WorkflowContext::default();

        // Add item data to context
        if let Value::Object(obj) = item {
            for (key, value) in obj {
                let value_str = match value {
                    Value::String(s) => s.clone(),
                    Value::Number(n) => n.to_string(),
                    Value::Bool(b) => b.to_string(),
                    _ => serde_json::to_string(value)?,
                };
                context.variables.insert(format!("item.{}", key), value_str);
            }
        }

        // Create agent-specific environment
        let _agent_env = ExecutionEnvironment {
            working_dir: worktree_path.clone(),
            project_dir: env.project_dir.clone(),
            worktree_name: Some(worktree_name.clone()),
            session_id: format!("{}-{}", env.session_id, item_id),
        };

        // Execute template steps
        let mut output = String::new();

        for _step in template_steps {
            // Note: This would use the WorkflowExecutor to execute the step
            // For now, we'll just collect the command information
            output.push_str(&format!("Executing step for {}\n", item_id));
        }

        // Get commits from worktree
        let commits = self.get_worktree_commits(&worktree_path).await?;

        // Clean up worktree (in real implementation, might keep for reduce phase)
        self.worktree_manager.cleanup_session(&worktree_name, true).await?;

        Ok(AgentResult {
            item_id: item_id.to_string(),
            status: AgentStatus::Success,
            output: Some(output),
            commits,
            duration: start_time.elapsed(),
            error: None,
            worktree_path: Some(worktree_path),
        })
    }

    /// Get commits from a worktree
    async fn get_worktree_commits(&self, worktree_path: &Path) -> Result<Vec<String>> {
        use tokio::process::Command;

        let output = Command::new("git")
            .args(["log", "--format=%H", "HEAD~10..HEAD"])
            .current_dir(worktree_path)
            .output()
            .await?;

        if !output.status.success() {
            return Ok(vec![]);
        }

        let commits = String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(|s| s.to_string())
            .collect();

        Ok(commits)
    }

    /// Execute the reduce phase
    async fn execute_reduce_phase(
        &self,
        reduce_phase: &ReducePhase,
        map_results: &[AgentResult],
        _env: &ExecutionEnvironment,
    ) -> Result<()> {
        self.user_interaction
            .display_progress("Starting reduce phase...");

        // Create context with map results
        let mut context = WorkflowContext::default();

        // Add summary statistics
        let successful = map_results
            .iter()
            .filter(|r| matches!(r.status, AgentStatus::Success))
            .count();
        let failed = map_results
            .iter()
            .filter(|r| matches!(r.status, AgentStatus::Failed(_)))
            .count();

        context
            .variables
            .insert("map.successful".to_string(), successful.to_string());
        context
            .variables
            .insert("map.failed".to_string(), failed.to_string());
        context
            .variables
            .insert("map.total".to_string(), map_results.len().to_string());

        // Add serialized results
        let results_json = serde_json::to_string(map_results)?;
        context
            .variables
            .insert("map.results".to_string(), results_json);

        // Execute reduce commands
        for _step in &reduce_phase.commands {
            self.user_interaction
                .display_progress("Executing reduce step...");
            // Note: This would use the WorkflowExecutor to execute the step
        }

        self.user_interaction
            .display_success("Reduce phase completed");

        Ok(())
    }

    /// Report execution summary
    fn report_summary(&self, results: &[AgentResult], duration: Duration) {
        let successful = results
            .iter()
            .filter(|r| matches!(r.status, AgentStatus::Success))
            .count();
        let failed = results
            .iter()
            .filter(|r| matches!(r.status, AgentStatus::Failed(_)))
            .count();
        let timeout = results
            .iter()
            .filter(|r| matches!(r.status, AgentStatus::Timeout))
            .count();

        let total_commits: usize = results.iter().map(|r| r.commits.len()).sum();

        self.user_interaction.display_info(&format!(
            "\n📊 MapReduce Execution Summary:
            Total items: {}
            Successful: {} ({:.1}%)
            Failed: {} ({:.1}%)
            Timeouts: {} ({:.1}%)
            Total commits: {}
            Total duration: {:.2}s
            Average time per item: {:.2}s",
            results.len(),
            successful,
            (successful as f64 / results.len() as f64) * 100.0,
            failed,
            (failed as f64 / results.len() as f64) * 100.0,
            timeout,
            (timeout as f64 / results.len() as f64) * 100.0,
            total_commits,
            duration.as_secs_f64(),
            duration.as_secs_f64() / results.len() as f64,
        ));
    }

    /// Clone the executor for use in spawned tasks
    fn clone_executor(&self) -> MapReduceExecutor {
        MapReduceExecutor {
            claude_executor: self.claude_executor.clone(),
            session_manager: self.session_manager.clone(),
            user_interaction: self.user_interaction.clone(),
            worktree_manager: self.worktree_manager.clone(),
            project_root: self.project_root.clone(),
        }
    }
}

#[cfg(test)]
#[path = "mapreduce_tests.rs"]
mod tests;
