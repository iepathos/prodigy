//! Claude API client with retry logic

use crate::error::{Error, Result};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::sleep;

/// Claude API response
#[derive(Debug, Serialize, Deserialize)]
pub struct ClaudeResponse {
    pub content: String,
    pub tokens_used: usize,
    pub model: String,
    pub stop_reason: Option<String>,
}

/// Claude API request
#[derive(Debug, Serialize)]
struct ClaudeRequest {
    model: String,
    messages: Vec<Message>,
    max_tokens: usize,
    temperature: f32,
    system: Option<String>,
}

#[derive(Debug, Serialize)]
struct Message {
    role: String,
    content: String,
}

/// Claude API client with retry logic
pub struct ClaudeClient {
    client: Client,
    api_key: String,
    max_retries: u32,
    retry_delay_ms: u64,
}

impl ClaudeClient {
    /// Create a new Claude client
    pub fn new(api_key: String, max_retries: u32, retry_delay_ms: u64) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .map_err(|e| Error::Config(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self {
            client,
            api_key,
            max_retries,
            retry_delay_ms,
        })
    }

    /// Complete a prompt with Claude
    pub async fn complete(&self, prompt: &str, model: &str) -> Result<ClaudeResponse> {
        let request = ClaudeRequest {
            model: model.to_string(),
            messages: vec![Message {
                role: "user".to_string(),
                content: prompt.to_string(),
            }],
            max_tokens: 4096,
            temperature: 0.7,
            system: None,
        };

        let mut retry_count = 0;
        loop {
            match self.make_request(&request).await {
                Ok(response) => return Ok(response),
                Err(e) => {
                    if retry_count >= self.max_retries {
                        return Err(e);
                    }

                    // Check if error is retryable
                    if !self.is_retryable_error(&e) {
                        return Err(e);
                    }

                    retry_count += 1;
                    let delay = self.calculate_backoff(retry_count);
                    sleep(Duration::from_millis(delay)).await;
                }
            }
        }
    }

    /// Make a single API request
    async fn make_request(&self, request: &ClaudeRequest) -> Result<ClaudeResponse> {
        let response = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(request)
            .send()
            .await
            .map_err(|e| Error::External(format!("API request failed: {}", e)))?;

        match response.status() {
            StatusCode::OK => {
                let api_response: ApiResponse = response
                    .json()
                    .await
                    .map_err(|e| Error::External(format!("Failed to parse response: {}", e)))?;

                Ok(ClaudeResponse {
                    content: api_response
                        .content
                        .first()
                        .map(|c| c.text.clone())
                        .unwrap_or_default(),
                    tokens_used: api_response.usage.input_tokens + api_response.usage.output_tokens,
                    model: api_response.model,
                    stop_reason: api_response.stop_reason,
                })
            }
            StatusCode::TOO_MANY_REQUESTS => {
                Err(Error::External("Rate limit exceeded".to_string()))
            }
            StatusCode::UNAUTHORIZED => Err(Error::Config("Invalid API key".to_string())),
            status => {
                let error_text = response.text().await.unwrap_or_default();
                Err(Error::External(format!(
                    "API error {}: {}",
                    status, error_text
                )))
            }
        }
    }

    /// Check if an error is retryable
    fn is_retryable_error(&self, error: &Error) -> bool {
        match error {
            Error::External(msg) => {
                msg.contains("Rate limit") || msg.contains("timeout") || msg.contains("connection")
            }
            _ => false,
        }
    }

    /// Calculate exponential backoff delay
    fn calculate_backoff(&self, retry_count: u32) -> u64 {
        self.retry_delay_ms * 2u64.pow(retry_count - 1)
    }
}

// Internal API response structures
#[derive(Debug, Deserialize)]
struct ApiResponse {
    content: Vec<Content>,
    model: String,
    stop_reason: Option<String>,
    usage: Usage,
}

#[derive(Debug, Deserialize)]
struct Content {
    text: String,
}

#[derive(Debug, Deserialize)]
struct Usage {
    input_tokens: usize,
    output_tokens: usize,
}
