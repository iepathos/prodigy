use anyhow::Result;
use std::time::Instant;
use tracing::{debug, info, warn};

use crate::cook::execution::{CommandExecutor, ExecutionContext};

use super::{AttemptRecord, GoalSeekConfig, GoalSeekResult};
use super::validator::{ScoreExtractor, ValidationResult};

pub struct GoalSeekEngine {
    command_executor: Box<dyn CommandExecutor>,
    attempt_history: Vec<AttemptRecord>,
}

impl GoalSeekEngine {
    pub fn new(command_executor: Box<dyn CommandExecutor>) -> Self {
        Self {
            command_executor,
            attempt_history: Vec::new(),
        }
    }
    
    pub async fn seek(&mut self, config: GoalSeekConfig) -> Result<GoalSeekResult> {
        let mut attempt = 0;
        let start_time = Instant::now();
        let mut best_score = 0;
        let mut last_output = String::new();
        
        info!("🎯 Goal: {}", config.goal);
        info!("🎚️  Threshold: {}", config.threshold);
        
        loop {
            attempt += 1;
            info!("🔄 Attempt {} of {}", attempt, config.max_attempts);
            
            // Check limits
            if attempt > config.max_attempts {
                return Ok(GoalSeekResult::MaxAttemptsReached { 
                    attempts: attempt - 1,
                    best_score,
                    last_output,
                });
            }
            
            if let Some(timeout) = config.timeout_seconds {
                if start_time.elapsed().as_secs() > timeout {
                    return Ok(GoalSeekResult::Timeout {
                        attempts: attempt - 1,
                        best_score,
                        elapsed: start_time.elapsed(),
                    });
                }
            }
            
            // Execute command (handles both initial attempt and refinement based on context)
            let command = config.claude.as_ref()
                .map(|c| format!("claude {}", c))
                .or_else(|| config.shell.clone())
                .ok_or_else(|| anyhow::anyhow!("Goal seek must have either 'claude' or 'shell' command"))?;
            
            debug!("Executing: {}", command);
            let attempt_result = self.execute_command_with_context(&command, attempt > 1).await?;
            
            // Validate result
            debug!("Validating with: {}", config.validate);
            let validation = self.validate_attempt(&config, &attempt_result).await?;
            
            // Record attempt
            self.attempt_history.push(AttemptRecord {
                attempt,
                score: validation.score,
                output: validation.output.clone(),
                timestamp: Instant::now(),
            });
            
            // Update best score
            if validation.score > best_score {
                best_score = validation.score;
                last_output = validation.output.clone();
            }
            
            info!("📊 Score: {}/100 (threshold: {})", validation.score, config.threshold);
            
            // Check if goal achieved
            if validation.score >= config.threshold {
                return Ok(GoalSeekResult::Success {
                    attempts: attempt,
                    final_score: validation.score,
                    execution_time: start_time.elapsed(),
                });
            }
            
            // Check for convergence (no improvement in last 3 attempts)
            if attempt >= 3 && self.is_converged() {
                return Ok(GoalSeekResult::Converged {
                    attempts: attempt,
                    final_score: validation.score,
                    reason: "No improvement in last 3 attempts".to_string(),
                });
            }
            
            // If not final attempt, prepare for refinement
            if attempt < config.max_attempts {
                info!("🔧 Refining for next attempt...");
            }
        }
    }
    
    async fn execute_command_with_context(&self, command: &str, has_validation_context: bool) -> Result<String> {
        // Don't parse the command - pass it as-is to the executor
        if command.trim().is_empty() {
            return Err(anyhow::anyhow!("Empty command"));
        }
        
        // Create execution context with validation data if available
        let mut context = ExecutionContext::default();
        
        if has_validation_context && !self.attempt_history.is_empty() {
            // Add validation context from previous attempt
            let last_attempt = &self.attempt_history[self.attempt_history.len() - 1];
            
            // Commands can access validation context via environment variables
            context.env_vars.insert(
                "PRODIGY_VALIDATION_SCORE".to_string(), 
                last_attempt.score.to_string()
            );
            context.env_vars.insert(
                "PRODIGY_VALIDATION_OUTPUT".to_string(), 
                last_attempt.output.clone()
            );
            
            // Try to extract gaps if validation output was JSON
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&last_attempt.output) {
                if let Some(gaps) = json.get("gaps") {
                    context.env_vars.insert(
                        "PRODIGY_VALIDATION_GAPS".to_string(),
                        gaps.to_string()
                    );
                }
            }
        }
        
        // Execute with context - pass full command string
        let result = self.command_executor.execute(command, &[], context).await?;
        
        if !result.success {
            warn!("Command failed: {}", result.stderr);
        }
        
        Ok(result.stdout)
    }
    
    async fn validate_attempt(
        &self,
        config: &GoalSeekConfig,
        _attempt_output: &str
    ) -> Result<ValidationResult> {
        // Execute validation command (validation doesn't need context)
        let validation_output = self.execute_command_with_context(&config.validate, false).await?;
        
        // Parse validation result
        ScoreExtractor::parse_structured_validation(&validation_output, config.threshold)
    }
    
    fn is_converged(&self) -> bool {
        if self.attempt_history.len() < 3 {
            return false;
        }
        
        let recent: Vec<u32> = self.attempt_history
            .iter()
            .rev()
            .take(3)
            .map(|r| r.score)
            .collect();
            
        // Check if scores are very similar (within 2 points)
        let max_score = recent.iter().max().unwrap_or(&0);
        let min_score = recent.iter().min().unwrap_or(&0);
        
        max_score - min_score <= 2
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cook::execution::ExecutionResult;
    use crate::testing::mocks::CommandExecutorMock;

    #[tokio::test]
    async fn test_goal_seek_success() {
        let mut mock_executor = CommandExecutorMock::new();
        mock_executor.add_response("test-cmd", ExecutionResult {
            stdout: "test output".to_string(),
            stderr: String::new(),
            exit_code: Some(0),
            success: true,
        });
        mock_executor.add_response("validate-cmd", ExecutionResult {
            stdout: "score: 95".to_string(),
            stderr: String::new(),
            exit_code: Some(0),
            success: true,
        });
        
        let config = GoalSeekConfig {
            goal: "Test goal".to_string(),
            claude: None,
            shell: Some("test-cmd".to_string()),
            validate: "validate-cmd".to_string(),
            threshold: 90,
            max_attempts: 3,
            timeout_seconds: Some(60),
            fail_on_incomplete: Some(false),
        };
        
        let mut engine = GoalSeekEngine::new(Box::new(mock_executor));
        let result = engine.seek(config).await.unwrap();
        
        assert!(matches!(result, GoalSeekResult::Success { .. }));
    }

    #[tokio::test]
    async fn test_convergence_detection() {
        let mut mock_executor = CommandExecutorMock::new();
        // Mock responses that show no improvement
        mock_executor.add_response("test-cmd", ExecutionResult {
            stdout: "test".to_string(),
            stderr: String::new(),
            exit_code: Some(0),
            success: true,
        });
        mock_executor.add_response("validate-cmd", ExecutionResult {
            stdout: "score: 80".to_string(),
            stderr: String::new(),
            exit_code: Some(0),
            success: true,
        });
        
        let _config = GoalSeekConfig {
            goal: "Test convergence".to_string(),
            claude: None,
            shell: Some("test-cmd".to_string()),
            validate: "validate-cmd".to_string(),
            threshold: 95,
            max_attempts: 5,
            timeout_seconds: None,
            fail_on_incomplete: Some(false),
        };
        
        let mut engine = GoalSeekEngine::new(Box::new(mock_executor));
        
        // Simulate convergence by adding similar scores
        engine.attempt_history = vec![
            AttemptRecord {
                attempt: 1,
                score: 80,
                output: "score: 80".to_string(),
                timestamp: Instant::now(),
            },
            AttemptRecord {
                attempt: 2,
                score: 81,
                output: "score: 81".to_string(),
                timestamp: Instant::now(),
            },
            AttemptRecord {
                attempt: 3,
                score: 80,
                output: "score: 80".to_string(),
                timestamp: Instant::now(),
            },
        ];
        
        assert!(engine.is_converged());
    }
}