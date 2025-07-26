use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::config::LoopConfig;
use super::metrics::LoopMetrics;

/// State of a loop session
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum SessionState {
    Initializing,
    Running,
    Paused,
    Completed,
    Failed,
    Cancelled,
}

impl std::fmt::Display for SessionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Initializing => write!(f, "initializing"),
            Self::Running => write!(f, "running"),
            Self::Paused => write!(f, "paused"),
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl std::str::FromStr for SessionState {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "initializing" => Ok(Self::Initializing),
            "running" => Ok(Self::Running),
            "paused" => Ok(Self::Paused),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            _ => Err(format!("Unknown session state: {s}")),
        }
    }
}

/// A loop session represents a complete iterative improvement cycle
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LoopSession {
    pub id: Uuid,
    pub config: LoopConfig,
    pub status: SessionState,
    pub current_iteration: u32,
    pub baseline_metrics: LoopMetrics,
    pub current_metrics: LoopMetrics,
    pub iterations: Vec<IterationData>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
    pub estimated_iterations: u32,
}

impl LoopSession {
    pub fn new(config: LoopConfig, baseline_metrics: LoopMetrics) -> Self {
        let id = Uuid::new_v4();
        let now = Utc::now();

        Self {
            id,
            config,
            status: SessionState::Initializing,
            current_iteration: 0,
            baseline_metrics: baseline_metrics.clone(),
            current_metrics: baseline_metrics,
            iterations: Vec::new(),
            created_at: now,
            updated_at: now,
            completed_at: None,
            error: None,
            estimated_iterations: 3, // Default estimate
        }
    }

    pub fn get_previous_results(&self) -> Vec<&IterationData> {
        self.iterations.iter().collect()
    }

    pub fn get_current_metrics(&self) -> &LoopMetrics {
        &self.current_metrics
    }

    pub fn update_status(&mut self, status: SessionState) {
        if matches!(
            status,
            SessionState::Completed | SessionState::Failed | SessionState::Cancelled
        ) {
            self.completed_at = Some(Utc::now());
        }

        self.status = status;
        self.updated_at = Utc::now();
    }

    pub fn add_iteration(&mut self, iteration: IterationData) {
        self.current_iteration = iteration.iteration_number;
        self.current_metrics = iteration.metrics.clone();
        self.iterations.push(iteration);
        self.updated_at = Utc::now();
    }

    pub fn set_error(&mut self, error: String) {
        self.error = Some(error);
        self.status = SessionState::Failed;
        self.updated_at = Utc::now();
        self.completed_at = Some(Utc::now());
    }
}

/// Data for a single iteration within a loop session
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IterationData {
    pub id: Uuid,
    pub session_id: Uuid,
    pub iteration_number: u32,
    pub review_results: Option<ReviewData>,
    pub improvement_results: Option<ImprovementData>,
    pub validation_results: Option<ValidationData>,
    pub metrics: LoopMetrics,
    pub duration: std::time::Duration,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
}

/// Review results from Claude review phase
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReviewData {
    pub review_id: String,
    pub overall_score: f64,
    pub scope: String,
    pub actions: Vec<ReviewAction>,
    pub summary: ReviewSummary,
    pub metrics: QualityMetrics,
    pub recommendations: ReviewRecommendations,
}

/// Individual review action
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReviewAction {
    pub id: String,
    pub action_type: String,
    pub severity: String,
    pub file: String,
    pub line: Option<u32>,
    pub line_range: Option<(u32, u32)>,
    pub title: String,
    pub description: String,
    pub suggestion: String,
    pub automated: bool,
    pub estimated_effort: String,
    pub category: String,
    pub impact: String,
}

/// Summary of review results
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReviewSummary {
    pub total_issues: u32,
    pub critical: u32,
    pub high: u32,
    pub medium: u32,
    pub low: u32,
    pub automated_fixes: u32,
    pub manual_fixes: u32,
    pub compilation_errors: u32,
    pub test_failures: u32,
    pub clippy_warnings: u32,
}

/// Quality metrics from review
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QualityMetrics {
    pub code_complexity: f64,
    pub test_coverage: f64,
    pub technical_debt_ratio: f64,
    pub maintainability_index: f64,
}

/// Review recommendations
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReviewRecommendations {
    pub next_iteration_focus: String,
    pub architecture_improvements: Vec<String>,
    pub priority_actions: Vec<String>,
}

/// Improvement results from Claude improvement phase
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImprovementData {
    pub changes_applied: u32,
    pub files_modified: Vec<String>,
    pub actions_attempted: u32,
    pub actions_successful: u32,
    pub actions_failed: u32,
    pub error_messages: Vec<String>,
}

/// Validation results after improvements
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ValidationData {
    pub compilation_passed: bool,
    pub tests_passed: bool,
    pub linting_passed: bool,
    pub has_issues: bool,
    pub error_count: u32,
    pub warning_count: u32,
    pub test_results: Option<TestResults>,
}

/// Test execution results
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TestResults {
    pub total_tests: u32,
    pub passed: u32,
    pub failed: u32,
    pub skipped: u32,
    pub coverage: Option<f64>,
}

/// Summary of a loop session for listing
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LoopSessionSummary {
    pub id: Uuid,
    pub status: SessionState,
    pub current_iteration: u32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
