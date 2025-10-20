// ! Pure functions for aggregating agent results
//!
//! These functions operate on AgentResult data structures to calculate
//! statistics, filter results, and group by error types.

use crate::cook::execution::mapreduce::agent::AgentResult;
use std::collections::HashMap;
use std::time::Duration;

/// Result aggregation summary
#[derive(Debug, Clone)]
pub struct AggregationStats {
    pub total: usize,
    pub successful: usize,
    pub failed: usize,
    pub total_duration: Duration,
    pub avg_duration: Duration,
    pub success_rate: f64,
}

/// Calculate success rate from agent results
///
/// # Arguments
///
/// * `results` - Slice of agent results to analyze
///
/// # Returns
///
/// Success rate as a percentage (0.0-100.0), or 0.0 if results is empty
///
/// # Examples
///
/// ```
/// use prodigy::cook::execution::mapreduce::pure::aggregation::calculate_success_rate;
/// use prodigy::cook::execution::mapreduce::agent::{AgentResult, AgentStatus};
/// use std::time::Duration;
///
/// let results = vec![
///     AgentResult {
///         item_id: "1".to_string(),
///         status: AgentStatus::Success,
///         output: None,
///         commits: vec![],
///         duration: Duration::from_secs(1),
///         error: None,
///         worktree_path: None,
///         branch_name: None,
///         worktree_session_id: None,
///         files_modified: vec![],
///         json_log_location: None,
///     },
///     AgentResult {
///         item_id: "2".to_string(),
///         status: AgentStatus::Failed("error".to_string()),
///         output: None,
///         commits: vec![],
///         duration: Duration::from_secs(1),
///         error: Some("error".to_string()),
///         worktree_path: None,
///         branch_name: None,
///         worktree_session_id: None,
///         files_modified: vec![],
///         json_log_location: None,
///     },
/// ];
/// assert_eq!(calculate_success_rate(&results), 50.0);
/// ```
pub fn calculate_success_rate(results: &[AgentResult]) -> f64 {
    if results.is_empty() {
        return 0.0;
    }
    let successful = results.iter().filter(|r| r.is_success()).count();
    (successful as f64 / results.len() as f64) * 100.0
}

/// Filter successful results
///
/// # Arguments
///
/// * `results` - Slice of agent results to filter
///
/// # Returns
///
/// Vector of references to successful results
pub fn filter_successful(results: &[AgentResult]) -> Vec<&AgentResult> {
    results.iter().filter(|r| r.is_success()).collect()
}

/// Filter failed results
///
/// # Arguments
///
/// * `results` - Slice of agent results to filter
///
/// # Returns
///
/// Vector of references to failed results
pub fn filter_failed(results: &[AgentResult]) -> Vec<&AgentResult> {
    results.iter().filter(|r| !r.is_success()).collect()
}

/// Group results by error type
///
/// # Arguments
///
/// * `results` - Slice of agent results to group
///
/// # Returns
///
/// HashMap mapping error categories to lists of results
pub fn group_by_error(results: &[AgentResult]) -> HashMap<String, Vec<&AgentResult>> {
    let mut groups = HashMap::new();
    for result in results.iter().filter(|r| !r.is_success()) {
        let error_type = categorize_error(&result.error);
        groups
            .entry(error_type)
            .or_insert_with(Vec::new)
            .push(result);
    }
    groups
}

/// Aggregate execution statistics
///
/// # Arguments
///
/// * `results` - Slice of agent results to aggregate
///
/// # Returns
///
/// Aggregation summary with statistics
pub fn aggregate_stats(results: &[AgentResult]) -> AggregationStats {
    let successful = results.iter().filter(|r| r.is_success()).count();
    let failed = results.iter().filter(|r| !r.is_success()).count();
    let total_duration: Duration = results.iter().map(|r| r.duration).sum();

    AggregationStats {
        total: results.len(),
        successful,
        failed,
        total_duration,
        avg_duration: calculate_avg_duration(results),
        success_rate: calculate_success_rate(results),
    }
}

/// Calculate average duration
fn calculate_avg_duration(results: &[AgentResult]) -> Duration {
    if results.is_empty() {
        return Duration::ZERO;
    }
    let total: Duration = results.iter().map(|r| r.duration).sum();
    total / results.len() as u32
}

/// Categorize error type
fn categorize_error(error: &Option<String>) -> String {
    match error {
        None => "unknown".to_string(),
        Some(e) if e.contains("timeout") => "timeout".to_string(),
        Some(e) if e.contains("command failed") => "command_failure".to_string(),
        Some(e) if e.contains("git") => "git_error".to_string(),
        Some(_) => "other".to_string(),
    }
}

/// Collect outputs from successful results
///
/// # Arguments
///
/// * `results` - Slice of agent results
///
/// # Returns
///
/// Vector of output strings from successful results
pub fn collect_outputs(results: &[AgentResult]) -> Vec<String> {
    results
        .iter()
        .filter(|r| r.is_success())
        .filter_map(|r| r.output.clone())
        .collect()
}

/// Collect all commits from successful results
///
/// # Arguments
///
/// * `results` - Slice of agent results
///
/// # Returns
///
/// Vector of all commit hashes
pub fn collect_commits(results: &[AgentResult]) -> Vec<String> {
    results
        .iter()
        .filter(|r| r.is_success())
        .flat_map(|r| r.commits.clone())
        .collect()
}

/// Count total commits across results
///
/// # Arguments
///
/// * `results` - Slice of agent results
///
/// # Returns
///
/// Total number of commits
pub fn count_commits(results: &[AgentResult]) -> usize {
    results.iter().map(|r| r.commits.len()).sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cook::execution::mapreduce::agent::AgentStatus;

    fn create_successful_result(duration_secs: u64) -> AgentResult {
        AgentResult {
            item_id: "test".to_string(),
            status: AgentStatus::Success,
            output: Some("output".to_string()),
            commits: vec!["abc123".to_string()],
            duration: Duration::from_secs(duration_secs),
            error: None,
            worktree_path: None,
            branch_name: None,
            worktree_session_id: None,
            files_modified: vec![],
            json_log_location: None,
            cleanup_status: None,
        }
    }

    fn create_failed_result(error_msg: &str) -> AgentResult {
        AgentResult {
            item_id: "test".to_string(),
            status: AgentStatus::Failed(error_msg.to_string()),
            output: None,
            commits: vec![],
            duration: Duration::from_secs(1),
            error: Some(error_msg.to_string()),
            worktree_path: None,
            branch_name: None,
            worktree_session_id: None,
            files_modified: vec![],
            json_log_location: None,
            cleanup_status: None,
        }
    }

    #[test]
    fn test_calculate_success_rate_all_successful() {
        let results = vec![create_successful_result(1), create_successful_result(2)];
        assert_eq!(calculate_success_rate(&results), 100.0);
    }

    #[test]
    fn test_calculate_success_rate_all_failed() {
        let results = vec![
            create_failed_result("error1"),
            create_failed_result("error2"),
        ];
        assert_eq!(calculate_success_rate(&results), 0.0);
    }

    #[test]
    fn test_calculate_success_rate_empty() {
        assert_eq!(calculate_success_rate(&[]), 0.0);
    }

    #[test]
    fn test_calculate_success_rate_mixed() {
        let results = vec![
            create_successful_result(1),
            create_failed_result("error"),
            create_successful_result(2),
        ];
        assert!((calculate_success_rate(&results) - 66.67).abs() < 0.1);
    }

    #[test]
    fn test_filter_successful() {
        let results = vec![
            create_successful_result(1),
            create_failed_result("error"),
            create_successful_result(2),
        ];
        let successful = filter_successful(&results);
        assert_eq!(successful.len(), 2);
    }

    #[test]
    fn test_filter_failed() {
        let results = vec![
            create_successful_result(1),
            create_failed_result("error"),
            create_successful_result(2),
        ];
        let failed = filter_failed(&results);
        assert_eq!(failed.len(), 1);
    }

    #[test]
    fn test_group_by_error() {
        let results = vec![
            create_failed_result("timeout occurred"),
            create_failed_result("command failed"),
            create_failed_result("timeout again"),
            create_failed_result("git error"),
        ];
        let groups = group_by_error(&results);
        assert_eq!(groups.len(), 3); // timeout, command_failure, git_error
        assert_eq!(groups.get("timeout").unwrap().len(), 2);
        assert_eq!(groups.get("command_failure").unwrap().len(), 1);
        assert_eq!(groups.get("git_error").unwrap().len(), 1);
    }

    #[test]
    fn test_aggregate_stats() {
        let results = vec![
            create_successful_result(10),
            create_failed_result("error"),
            create_successful_result(20),
        ];
        let stats = aggregate_stats(&results);
        assert_eq!(stats.total, 3);
        assert_eq!(stats.successful, 2);
        assert_eq!(stats.failed, 1);
        assert_eq!(stats.total_duration, Duration::from_secs(31));
        // Average duration: 31s / 3 = 10.33s (rounded down to 10s in integer division)
        assert!(stats.avg_duration >= Duration::from_secs(10));
        assert!(stats.avg_duration < Duration::from_secs(11));
    }

    #[test]
    fn test_collect_outputs() {
        let results = vec![
            create_successful_result(1),
            create_failed_result("error"),
            create_successful_result(2),
        ];
        let outputs = collect_outputs(&results);
        assert_eq!(outputs.len(), 2);
        assert_eq!(outputs[0], "output");
    }

    #[test]
    fn test_collect_commits() {
        let results = vec![create_successful_result(1), create_successful_result(2)];
        let commits = collect_commits(&results);
        assert_eq!(commits.len(), 2);
        assert_eq!(commits[0], "abc123");
    }

    #[test]
    fn test_count_commits() {
        let results = vec![
            create_successful_result(1),
            create_failed_result("error"),
            create_successful_result(2),
        ];
        assert_eq!(count_commits(&results), 2);
    }
}
