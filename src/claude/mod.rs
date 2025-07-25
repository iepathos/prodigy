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
            model_selector: ModelSelector::new(config.default_model),
        })
    }

    /// Execute a Claude command
    pub async fn execute_command(&mut self, command: &str, args: Vec<String>) -> Result<String> {
        // Get command configuration
        let cmd_config = self.commands.get_command(command)?;

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

        // Execute API call
        let response = self.client.complete(&context, &model).await?;

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

    fn estimate_tokens(&self, text: &str) -> usize {
        // Simple estimation: ~4 characters per token
        text.len() / 4
    }
}
