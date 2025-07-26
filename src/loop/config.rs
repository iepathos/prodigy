use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Configuration for iterative improvement loops
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LoopConfig {
    pub target_score: f64,
    pub max_iterations: u32,
    pub scope: Vec<String>,
    pub severity_filter: Vec<SeverityLevel>,
    pub termination_conditions: Vec<TerminationCondition>,
    pub safety_settings: SafetySettings,
    pub workflow_template: String,
}

impl Default for LoopConfig {
    fn default() -> Self {
        Self {
            target_score: 8.5,
            max_iterations: 3,
            scope: vec!["src/".to_string()],
            severity_filter: vec![SeverityLevel::Critical, SeverityLevel::High],
            termination_conditions: vec![
                TerminationCondition::TargetAchieved { threshold: 8.5 },
                TerminationCondition::MaxIterations { count: 3 },
                TerminationCondition::DiminishingReturns {
                    min_improvement: 0.1,
                    consecutive_iterations: 2,
                },
                TerminationCondition::NoAutomatedActions,
            ],
            safety_settings: SafetySettings::default(),
            workflow_template: "code-quality-improvement".to_string(),
        }
    }
}

/// Severity levels for filtering issues
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum SeverityLevel {
    Critical,
    High,
    Medium,
    Low,
}

impl std::str::FromStr for SeverityLevel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "critical" => Ok(Self::Critical),
            "high" => Ok(Self::High),
            "medium" => Ok(Self::Medium),
            "low" => Ok(Self::Low),
            _ => Err(format!("Unknown severity level: {s}")),
        }
    }
}

/// Termination condition types
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum TerminationCondition {
    TargetAchieved {
        threshold: f64,
    },
    MaxIterations {
        count: u32,
    },
    DiminishingReturns {
        min_improvement: f64,
        consecutive_iterations: u32,
    },
    NoAutomatedActions,
    QualityRegression {
        threshold: f64,
    },
    TimeLimit {
        duration: Duration,
    },
    UserIntervention,
}

/// Quality targets for improvement loops
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QualityTarget {
    pub overall_score: f64,
    pub compilation_errors: u32,
    pub test_failures: u32,
    pub critical_issues: u32,
    pub high_priority_issues: u32,
    pub code_coverage: Option<f64>,
    pub complexity_score: Option<f64>,
}

impl Default for QualityTarget {
    fn default() -> Self {
        Self {
            overall_score: 8.5,
            compilation_errors: 0,
            test_failures: 0,
            critical_issues: 0,
            high_priority_issues: 0,
            code_coverage: Some(80.0),
            complexity_score: Some(5.0),
        }
    }
}

/// Safety settings for loop execution
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SafetySettings {
    pub create_git_stash: bool,
    pub validate_compilation: bool,
    pub run_tests: bool,
    pub max_file_changes_per_iteration: usize,
    pub rollback_on_regression: bool,
    pub backup_before_changes: bool,
    pub require_human_approval_threshold: f64,
}

impl Default for SafetySettings {
    fn default() -> Self {
        Self {
            create_git_stash: true,
            validate_compilation: true,
            run_tests: true,
            max_file_changes_per_iteration: 20,
            rollback_on_regression: true,
            backup_before_changes: true,
            require_human_approval_threshold: 5.0,
        }
    }
}
