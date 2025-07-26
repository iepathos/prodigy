use serde_json::json;
use std::sync::Arc;

use crate::{workflow::WorkflowContext, Result};

use super::{
    config::{LoopConfig, SeverityLevel, TerminationCondition},
    engine::IterationEngine,
    session::ReviewData,
};

/// Result of session initialization
#[derive(Debug, Clone)]
pub struct SessionInitResult {
    pub session_id: String,
    pub baseline_metrics: super::metrics::LoopMetrics,
    pub estimated_iterations: u32,
}

/// Result of code review phase
#[derive(Debug, Clone)]
pub struct ReviewResult {
    pub review_id: String,
    pub quality_score: f64,
    pub actionable_items: usize,
    pub automated_fixes: u32,
    pub manual_fixes: u32,
    pub critical_issues: u32,
    pub recommendations: super::session::ReviewRecommendations,
}

/// Result of termination condition check
#[derive(Debug, Clone)]
pub struct TerminationResult {
    pub should_terminate: bool,
    pub reason: String,
    pub final_score: f64,
    pub iterations_completed: u32,
}

/// Workflow step commands for iterative improvement
pub struct LoopCommands {
    engine: Arc<IterationEngine>,
    #[allow(dead_code)]
    workflow_context: Arc<WorkflowContext>,
}

impl LoopCommands {
    pub fn new(engine: Arc<IterationEngine>, workflow_context: Arc<WorkflowContext>) -> Self {
        Self {
            engine,
            workflow_context,
        }
    }

    /// Initialize a new loop session
    pub async fn session_init(&self, params: &WorkflowParameters) -> Result<SessionInitResult> {
        let config = LoopConfig {
            target_score: params.get_float("target_score")?,
            max_iterations: params.get_u32("max_iterations")?,
            scope: params
                .get_string("scope")?
                .split(',')
                .map(|s| s.trim().to_string())
                .collect(),
            severity_filter: self.parse_severity_filter(&params.get_string("severity_filter")?)?,
            termination_conditions: self.build_default_termination_conditions(),
            safety_settings: super::config::SafetySettings::default(),
            workflow_template: "code-quality-improvement".to_string(),
        };

        let session = self.engine.create_session(config).await?;

        Ok(SessionInitResult {
            session_id: session.id.to_string(),
            baseline_metrics: session.baseline_metrics.clone(),
            estimated_iterations: session.estimated_iterations,
        })
    }

    /// Execute Claude review phase
    pub async fn claude_review(&self, session_id: &str, iteration: u32) -> Result<ReviewResult> {
        let session = self.engine.get_session(session_id).await?;
        let scope = session.config.scope.join(" ");

        // Build context for Claude command
        let mut context = WorkflowContext::new();
        context.insert("iteration".to_string(), json!(iteration));
        context.insert("session_id".to_string(), json!(session_id));
        context.insert(
            "previous_results".to_string(),
            json!(session.get_previous_results()),
        );

        // For now, return a mock result
        // In the full implementation, this would execute the structured Claude command
        let mock_review_data = ReviewData {
            review_id: format!(
                "review-{}-{}",
                chrono::Utc::now().timestamp(),
                uuid::Uuid::new_v4()
            ),
            overall_score: 7.5,
            scope: scope.clone(),
            actions: vec![],
            summary: super::session::ReviewSummary {
                total_issues: 5,
                critical: 1,
                high: 2,
                medium: 2,
                low: 0,
                automated_fixes: 3,
                manual_fixes: 2,
                compilation_errors: 0,
                test_failures: 1,
                clippy_warnings: 4,
            },
            metrics: super::session::QualityMetrics {
                code_complexity: 6.2,
                test_coverage: 75.0,
                technical_debt_ratio: 0.15,
                maintainability_index: 7.8,
            },
            recommendations: super::session::ReviewRecommendations {
                next_iteration_focus: "Address test failures and improve coverage".to_string(),
                architecture_improvements: vec![
                    "Consider extracting common functionality".to_string(),
                    "Improve error handling patterns".to_string(),
                ],
                priority_actions: vec!["action_1".to_string(), "action_2".to_string()],
            },
        };

        // Update session with review results
        self.engine
            .update_session_review(session_id, iteration, &mock_review_data)
            .await?;

        Ok(ReviewResult {
            review_id: mock_review_data.review_id,
            quality_score: mock_review_data.overall_score,
            actionable_items: mock_review_data.actions.len(),
            automated_fixes: mock_review_data.summary.automated_fixes,
            manual_fixes: mock_review_data.summary.manual_fixes,
            critical_issues: mock_review_data.summary.critical,
            recommendations: mock_review_data.recommendations,
        })
    }

    /// Check termination conditions
    pub async fn check_termination_conditions(
        &self,
        session_id: &str,
    ) -> Result<TerminationResult> {
        let session = self.engine.get_session(session_id).await?;
        let current_metrics = session.get_current_metrics();

        for condition in &session.config.termination_conditions {
            if let Some(reason) = self
                .evaluate_condition(condition, &session, current_metrics)
                .await?
            {
                return Ok(TerminationResult {
                    should_terminate: true,
                    reason,
                    final_score: current_metrics.quality_score,
                    iterations_completed: session.current_iteration,
                });
            }
        }

        Ok(TerminationResult {
            should_terminate: false,
            reason: "Continue iteration".to_string(),
            final_score: current_metrics.quality_score,
            iterations_completed: session.current_iteration,
        })
    }

    /// Parse severity filter string into enum values
    fn parse_severity_filter(&self, filter: &str) -> Result<Vec<SeverityLevel>> {
        filter
            .split(',')
            .map(|s| s.trim().parse())
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| crate::Error::Config(format!("Invalid severity level: {e}")))
    }

    /// Build default termination conditions
    fn build_default_termination_conditions(&self) -> Vec<TerminationCondition> {
        vec![
            TerminationCondition::TargetAchieved { threshold: 8.5 },
            TerminationCondition::MaxIterations { count: 3 },
            TerminationCondition::DiminishingReturns {
                min_improvement: 0.1,
                consecutive_iterations: 2,
            },
            TerminationCondition::NoAutomatedActions,
        ]
    }

    /// Evaluate a specific termination condition
    async fn evaluate_condition(
        &self,
        condition: &TerminationCondition,
        session: &super::session::LoopSession,
        current_metrics: &super::metrics::LoopMetrics,
    ) -> Result<Option<String>> {
        match condition {
            TerminationCondition::TargetAchieved { threshold } => {
                if current_metrics.quality_score >= *threshold {
                    Ok(Some(format!("Target score of {threshold:.1} achieved")))
                } else {
                    Ok(None)
                }
            }
            TerminationCondition::MaxIterations { count } => {
                if session.current_iteration >= *count {
                    Ok(Some(format!("Maximum iterations ({count}) reached")))
                } else {
                    Ok(None)
                }
            }
            TerminationCondition::DiminishingReturns {
                min_improvement,
                consecutive_iterations,
            } => {
                // Check if the last N iterations had minimal improvement
                if session.iterations.len() >= *consecutive_iterations as usize {
                    let recent_improvements: Vec<f64> = session
                        .iterations
                        .iter()
                        .rev()
                        .take(*consecutive_iterations as usize)
                        .map(|iter| iter.metrics.score_improvement)
                        .collect();

                    let all_below_threshold = recent_improvements
                        .iter()
                        .all(|&improvement| improvement < *min_improvement);

                    if all_below_threshold {
                        Ok(Some(format!(
                            "Diminishing returns detected: {consecutive_iterations} consecutive iterations with <{min_improvement:.1} improvement"
                        )))
                    } else {
                        Ok(None)
                    }
                } else {
                    Ok(None)
                }
            }
            TerminationCondition::NoAutomatedActions => {
                if current_metrics.actions_applied == 0 {
                    Ok(Some("No automated actions available".to_string()))
                } else {
                    Ok(None)
                }
            }
            TerminationCondition::QualityRegression { threshold } => {
                if current_metrics.score_improvement < -*threshold {
                    Ok(Some(format!(
                        "Quality regression detected: {:.1}",
                        current_metrics.score_improvement
                    )))
                } else {
                    Ok(None)
                }
            }
            TerminationCondition::TimeLimit { duration: _ } => {
                // TODO: Implement time-based termination
                Ok(None)
            }
            TerminationCondition::UserIntervention => {
                // TODO: Implement user intervention detection
                Ok(None)
            }
        }
    }
}

/// Simplified workflow parameters for this implementation
pub struct WorkflowParameters {
    params: std::collections::HashMap<String, serde_json::Value>,
}

impl Default for WorkflowParameters {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkflowParameters {
    pub fn new() -> Self {
        Self {
            params: std::collections::HashMap::new(),
        }
    }

    pub fn get_float(&self, key: &str) -> Result<f64> {
        self.params
            .get(key)
            .and_then(|v| v.as_f64())
            .ok_or_else(|| {
                crate::Error::Config(format!("Missing or invalid float parameter: {key}"))
            })
    }

    pub fn get_u32(&self, key: &str) -> Result<u32> {
        self.params
            .get(key)
            .and_then(|v| v.as_u64())
            .map(|v| v as u32)
            .ok_or_else(|| {
                crate::Error::Config(format!("Missing or invalid u32 parameter: {key}"))
            })
    }

    pub fn get_string(&self, key: &str) -> Result<String> {
        self.params
            .get(key)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| {
                crate::Error::Config(format!("Missing or invalid string parameter: {key}"))
            })
    }

    pub fn insert(&mut self, key: String, value: serde_json::Value) {
        self.params.insert(key, value);
    }
}
