//! Claude integration module for MMM
//!
//! Provides sophisticated Claude CLI integration with prompt engineering,
//! context management, response processing, and conversation memory.

pub mod api;
pub mod cache;
pub mod commands;
pub mod context;
pub mod memory;
pub mod models;
pub mod prompt;
pub mod response;
pub mod token;

pub use api::ClaudeClient;
pub use cache::ResponseCache;
pub use commands::CommandRegistry;
pub use context::{ContextItem, ContextManager, Priority};
pub use memory::ConversationMemory;
pub use models::ModelSelector;
pub use prompt::{PromptEngine, PromptTemplate};
pub use response::{ParsedResponse, ResponseProcessor};
pub use token::TokenTracker;

use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Main Claude integration configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeConfig {
    /// API key for Claude
    pub api_key: String,

    /// Default model to use
    pub default_model: String,

    /// Token limits
    pub daily_token_limit: Option<usize>,

    /// Cache directory
    pub cache_dir: PathBuf,

    /// Max context window size
    pub max_context_tokens: usize,

    /// Retry configuration
    pub max_retries: u32,
    pub retry_delay_ms: u64,
}

impl Default for ClaudeConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            default_model: "claude-3-sonnet-20240229".to_string(),
            daily_token_limit: None,
            cache_dir: PathBuf::from(".mmm/claude_cache"),
            max_context_tokens: 100000,
            max_retries: 3,
            retry_delay_ms: 1000,
        }
    }
}

/// Main Claude integration manager
pub struct ClaudeManager {
    pub client: ClaudeClient,
    pub prompt_engine: PromptEngine,
    pub context_manager: ContextManager,
    pub response_processor: ResponseProcessor,
    pub token_tracker: TokenTracker,
    pub memory: ConversationMemory,
    pub commands: CommandRegistry,
    pub cache: ResponseCache,
    pub model_selector: ModelSelector,
    metrics: Arc<Mutex<ClaudeMetrics>>,
}

#[derive(Debug, Default)]
struct ClaudeMetrics {
    response_times: Vec<f64>,
    error_count: u64,
    current_model: String,
}

impl ClaudeManager {
    /// Create a new Claude manager
    pub fn new(config: ClaudeConfig) -> Result<Self> {
        Ok(Self {
            client: ClaudeClient::new(config.api_key, config.max_retries, config.retry_delay_ms)?,
            prompt_engine: PromptEngine::new()?,
            context_manager: ContextManager::new(config.max_context_tokens),
            response_processor: ResponseProcessor::new(),
            token_tracker: TokenTracker::new(config.daily_token_limit)?,
            memory: ConversationMemory::new(),
            commands: CommandRegistry::new()?,
            cache: ResponseCache::new(config.cache_dir)?,
            model_selector: ModelSelector::new(config.default_model.clone()),
            metrics: Arc::new(Mutex::new(ClaudeMetrics {
                response_times: Vec::new(),
                error_count: 0,
                current_model: config.default_model,
            })),
        })
    }

    /// Execute a Claude command
    pub async fn execute_command(&mut self, command: &str, args: Vec<String>) -> Result<String> {
        // Get command configuration
        let cmd_config = self.commands.get_command(command)?;

        // Check if this is a Claude CLI command
        if cmd_config.task_type == "claude-cli" {
            return self.execute_claude_cli_command(command, args).await;
        }

        // Prepare prompt from template
        let prompt = self
            .prompt_engine
            .render_template(&cmd_config.template, args)?;

        // Build context
        let context = self.context_manager.build_context(&prompt)?;

        // Check token limits
        let estimated_tokens = self.estimate_tokens(&context);
        self.token_tracker.can_proceed(estimated_tokens)?;

        // Check cache
        if let Some(cached) = self.cache.get_cached(&context) {
            return Ok(cached.response);
        }

        // Select appropriate model
        let model = self.model_selector.select_for_task(&cmd_config.task_type)?;

        // Track model selection
        {
            let mut metrics = self.metrics.lock().await;
            metrics.current_model = model.clone();
        }

        // Execute API call with timing
        let start = std::time::Instant::now();
        let response = match self.client.complete(&context, &model).await {
            Ok(resp) => resp,
            Err(e) => {
                let mut metrics = self.metrics.lock().await;
                metrics.error_count += 1;
                return Err(e);
            }
        };
        let duration = start.elapsed();

        // Record response time
        {
            let mut metrics = self.metrics.lock().await;
            metrics.response_times.push(duration.as_millis() as f64);
            // Keep only last 1000 response times
            if metrics.response_times.len() > 1000 {
                metrics.response_times.remove(0);
            }
        }

        // Process response
        let parsed = self.response_processor.process(&response.content)?;

        // Update memory
        self.memory.add_exchange(&prompt, &parsed)?;

        // Track tokens
        self.token_tracker.record_usage(response.tokens_used)?;

        // Cache if appropriate
        if self.cache.should_cache(&response) {
            self.cache.store(&context, &response)?;
        }

        Ok(parsed.content)
    }

    /// Execute a Claude CLI slash command
    async fn execute_claude_cli_command(
        &mut self,
        command: &str,
        args: Vec<String>,
    ) -> Result<String> {
        use tokio::process::Command;

        // Map MMM command to Claude CLI slash command
        let slash_command = match command {
            "lint" | "claude-lint" => "/lint",
            "review" | "claude-review" => "/review",
            "implement-spec" | "claude-implement-spec" => "/implement-spec",
            "add-spec" | "claude-add-spec" => "/add-spec",
            _ => {
                return Err(crate::error::Error::NotFound(format!(
                    "Unknown Claude CLI command: {}",
                    command
                )))
            }
        };

        // Build command with arguments
        let mut cmd_args = vec![slash_command.to_string()];
        cmd_args.extend(args);

        // Execute Claude CLI
        let output = Command::new("claude")
            .args(&cmd_args)
            .output()
            .await
            .map_err(|e| {
                crate::error::Error::Other(format!("Failed to execute Claude CLI: {}", e))
            })?;

        if output.status.success() {
            let result = String::from_utf8_lossy(&output.stdout).to_string();

            // Track this as a successful Claude interaction
            self.token_tracker
                .record_usage(self.estimate_tokens(&result))?;

            Ok(result)
        } else {
            let error = String::from_utf8_lossy(&output.stderr);
            Err(crate::error::Error::Other(format!(
                "Claude CLI command failed: {}",
                error
            )))
        }
    }

    fn estimate_tokens(&self, text: &str) -> usize {
        // Simple estimation: ~4 characters per token
        text.len() / 4
    }

    /// Get token usage information
    pub async fn get_token_usage(&self) -> Result<TokenUsage> {
        self.token_tracker.get_usage().await
    }

    /// Get response times
    pub async fn get_response_times(&self) -> Result<Vec<f64>> {
        let metrics = self.metrics.lock().await;
        Ok(metrics.response_times.clone())
    }

    /// Get error count
    pub async fn get_error_count(&self) -> Result<u64> {
        let metrics = self.metrics.lock().await;
        Ok(metrics.error_count)
    }

    /// Get current model
    pub async fn get_current_model(&self) -> Result<String> {
        let metrics = self.metrics.lock().await;
        Ok(metrics.current_model.clone())
    }

    /// Generate a response (simplified interface)
    pub async fn generate_response(&self, prompt: &str) -> Result<String> {
        // This is a simplified method for basic prompts
        let response = self
            .client
            .complete(prompt, &self.model_selector.get_default_model()?)
            .await?;
        Ok(response.content)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
}
