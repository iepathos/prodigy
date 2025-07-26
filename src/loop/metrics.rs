use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use uuid::Uuid;

/// Loop-specific metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopMetrics {
    pub session_id: Uuid,
    pub iteration: u32,
    pub quality_score: f64,
    pub score_improvement: f64,
    pub actions_applied: u32,
    pub actions_successful: u32,
    pub files_modified: u32,
    pub compilation_status: bool,
    pub test_status: bool,
    pub duration: Duration,
    pub timestamp: DateTime<Utc>,
    pub detailed_metrics: DetailedMetrics,
}

impl LoopMetrics {
    pub fn new(session_id: Uuid, iteration: u32) -> Self {
        Self {
            session_id,
            iteration,
            quality_score: 0.0,
            score_improvement: 0.0,
            actions_applied: 0,
            actions_successful: 0,
            files_modified: 0,
            compilation_status: true,
            test_status: true,
            duration: Duration::default(),
            timestamp: Utc::now(),
            detailed_metrics: DetailedMetrics::default(),
        }
    }

    pub fn calculate_improvement(&self, previous: &LoopMetrics) -> f64 {
        self.quality_score - previous.quality_score
    }

    pub fn success_rate(&self) -> f64 {
        if self.actions_applied == 0 {
            0.0
        } else {
            self.actions_successful as f64 / self.actions_applied as f64
        }
    }
}

/// Detailed metrics for comprehensive analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetailedMetrics {
    pub code_complexity: f64,
    pub test_coverage: f64,
    pub technical_debt_ratio: f64,
    pub maintainability_index: f64,
    pub cyclomatic_complexity: f64,
    pub lines_of_code: u32,
    pub documentation_coverage: f64,
    pub issue_breakdown: IssueBreakdown,
    pub performance_metrics: PerformanceMetrics,
}

impl Default for DetailedMetrics {
    fn default() -> Self {
        Self {
            code_complexity: 0.0,
            test_coverage: 0.0,
            technical_debt_ratio: 0.0,
            maintainability_index: 0.0,
            cyclomatic_complexity: 0.0,
            lines_of_code: 0,
            documentation_coverage: 0.0,
            issue_breakdown: IssueBreakdown::default(),
            performance_metrics: PerformanceMetrics::default(),
        }
    }
}

/// Breakdown of issues by type and severity
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct IssueBreakdown {
    pub critical_issues: u32,
    pub high_priority_issues: u32,
    pub medium_priority_issues: u32,
    pub low_priority_issues: u32,
    pub compilation_errors: u32,
    pub test_failures: u32,
    pub linting_warnings: u32,
    pub security_issues: u32,
    pub performance_issues: u32,
    pub accessibility_issues: u32,
}


/// Performance-related metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub build_time: Duration,
    pub test_execution_time: Duration,
    pub bundle_size: u64,
    pub memory_usage: u64,
    pub cpu_utilization: f64,
}

impl Default for PerformanceMetrics {
    fn default() -> Self {
        Self {
            build_time: Duration::default(),
            test_execution_time: Duration::default(),
            bundle_size: 0,
            memory_usage: 0,
            cpu_utilization: 0.0,
        }
    }
}

/// Result of a complete iteration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IterationResult {
    pub session_id: Uuid,
    pub iteration: u32,
    pub status: IterationStatus,
    pub metrics: LoopMetrics,
    pub duration: Duration,
    pub actions_taken: Vec<ActionResult>,
    pub validation_results: ValidationResults,
    pub error: Option<String>,
}

/// Status of an iteration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum IterationStatus {
    Completed,
    Failed,
    PartiallyCompleted,
    Skipped,
}

/// Result of a specific action taken during iteration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionResult {
    pub action_id: String,
    pub action_type: String,
    pub file_path: String,
    pub success: bool,
    pub error_message: Option<String>,
    pub changes_made: Vec<String>,
    pub duration: Duration,
}

/// Comprehensive validation results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResults {
    pub overall_success: bool,
    pub compilation_result: CompilationResult,
    pub test_result: TestResult,
    pub linting_result: LintingResult,
    pub security_scan_result: Option<SecurityScanResult>,
}

/// Compilation validation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompilationResult {
    pub success: bool,
    pub error_count: u32,
    pub warning_count: u32,
    pub errors: Vec<CompilationError>,
    pub duration: Duration,
}

/// Individual compilation error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompilationError {
    pub file: String,
    pub line: u32,
    pub column: u32,
    pub message: String,
    pub severity: String,
}

/// Test execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    pub success: bool,
    pub total_tests: u32,
    pub passed: u32,
    pub failed: u32,
    pub skipped: u32,
    pub coverage: Option<f64>,
    pub failed_tests: Vec<FailedTest>,
    pub duration: Duration,
}

/// Individual failed test
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailedTest {
    pub name: String,
    pub file: String,
    pub error_message: String,
    pub duration: Duration,
}

/// Linting result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LintingResult {
    pub success: bool,
    pub error_count: u32,
    pub warning_count: u32,
    pub issues: Vec<LintingIssue>,
    pub duration: Duration,
}

/// Individual linting issue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LintingIssue {
    pub file: String,
    pub line: u32,
    pub column: u32,
    pub rule: String,
    pub message: String,
    pub severity: String,
    pub fixable: bool,
}

/// Security scan result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityScanResult {
    pub success: bool,
    pub vulnerabilities_found: u32,
    pub critical_vulnerabilities: u32,
    pub high_vulnerabilities: u32,
    pub medium_vulnerabilities: u32,
    pub low_vulnerabilities: u32,
    pub vulnerabilities: Vec<SecurityVulnerability>,
    pub duration: Duration,
}

/// Individual security vulnerability
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityVulnerability {
    pub id: String,
    pub severity: String,
    pub title: String,
    pub description: String,
    pub file: String,
    pub line: Option<u32>,
    pub cwe_id: Option<String>,
    pub recommendation: String,
}

/// Loop session report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopReport {
    pub session_id: Uuid,
    pub total_iterations: u32,
    pub initial_score: f64,
    pub final_score: f64,
    pub total_improvement: f64,
    pub total_actions: u32,
    pub success_rate: f64,
    pub total_duration: Duration,
    pub files_affected: u32,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}
