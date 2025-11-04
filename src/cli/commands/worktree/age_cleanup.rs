//! Age-based worktree cleanup logic
//!
//! This module handles cleanup of worktrees based on age,
//! separating pure logic from I/O operations.

use anyhow::Result;
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use std::time::Duration;

use crate::worktree::manager::WorktreeManager;
use crate::worktree::WorktreeSession;

/// Pure function to calculate worktree age
///
/// Returns the age of a worktree session in seconds.
pub fn calculate_age_seconds(created_at: DateTime<Utc>, now: DateTime<Utc>) -> i64 {
    let age = now.signed_duration_since(created_at);
    age.num_seconds()
}

/// Pure function to check if worktree is too old
///
/// Returns true if the worktree age exceeds the maximum age.
pub fn is_worktree_too_old(
    created_at: DateTime<Utc>,
    now: DateTime<Utc>,
    max_age: Duration,
) -> bool {
    let age_seconds = calculate_age_seconds(created_at, now);
    age_seconds as u64 > max_age.as_secs()
}

/// Pure function to format age in hours
///
/// Converts a chrono Duration to hours for display.
pub fn format_age_hours(age: ChronoDuration) -> i64 {
    age.num_hours()
}

/// Pure function to filter sessions by age
///
/// Returns only sessions that exceed the max age.
pub fn filter_old_sessions(
    sessions: Vec<WorktreeSession>,
    max_age: Duration,
    now: DateTime<Utc>,
) -> Vec<WorktreeSession> {
    sessions
        .into_iter()
        .filter(|session| is_worktree_too_old(session.created_at, now, max_age))
        .collect()
}

/// Clean up old worktrees
///
/// This is the main orchestrator for age-based cleanup.
pub async fn cleanup_old_worktrees(
    manager: &WorktreeManager,
    max_age: Duration,
    force: bool,
    dry_run: bool,
) -> Result<()> {
    let sessions = manager.list_sessions().await?;
    let now = Utc::now();

    // Filter to old sessions
    let old_sessions = filter_old_sessions(sessions, max_age, now);

    if old_sessions.is_empty() {
        println!("No worktrees older than the specified age.");
        return Ok(());
    }

    let mut cleaned = 0;

    for session in old_sessions {
        let age = now.signed_duration_since(session.created_at);
        let age_hours = format_age_hours(age);

        if dry_run {
            println!(
                "DRY RUN: Would remove worktree '{}' (age: {} hours)",
                session.name, age_hours
            );
        } else {
            println!(
                "Removing old worktree '{}' (age: {} hours)",
                session.name, age_hours
            );
            manager.cleanup_session(&session.name, force).await?;
            cleaned += 1;
        }
    }

    if !dry_run {
        println!("Cleaned {} old worktrees", cleaned);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_age_seconds() {
        let now = Utc::now();
        let created = now - ChronoDuration::hours(2);
        let age = calculate_age_seconds(created, now);

        // Should be approximately 2 hours = 7200 seconds
        assert!((7199..=7201).contains(&age), "Age was {}", age);
    }

    #[test]
    fn test_is_worktree_too_old_exceeds() {
        let now = Utc::now();
        let created = now - ChronoDuration::hours(25);
        let max_age = Duration::from_secs(24 * 3600); // 24 hours

        assert!(is_worktree_too_old(created, now, max_age));
    }

    #[test]
    fn test_is_worktree_too_old_within() {
        let now = Utc::now();
        let created = now - ChronoDuration::hours(23);
        let max_age = Duration::from_secs(24 * 3600); // 24 hours

        assert!(!is_worktree_too_old(created, now, max_age));
    }

    #[test]
    fn test_is_worktree_too_old_exact_boundary() {
        let now = Utc::now();
        let created = now - ChronoDuration::hours(24);
        let max_age = Duration::from_secs(24 * 3600); // 24 hours

        // At exact boundary, should not be considered too old
        assert!(!is_worktree_too_old(created, now, max_age));
    }

    #[test]
    fn test_format_age_hours() {
        let age = ChronoDuration::hours(48) + ChronoDuration::minutes(30);
        let hours = format_age_hours(age);
        assert_eq!(hours, 48);
    }

    #[test]
    fn test_filter_old_sessions_empty() {
        let sessions = vec![];
        let max_age = Duration::from_secs(3600);
        let now = Utc::now();

        let filtered = filter_old_sessions(sessions, max_age, now);
        assert_eq!(filtered.len(), 0);
    }

    #[test]
    fn test_filter_old_sessions_all_old() {
        let now = Utc::now();
        let sessions = vec![
            WorktreeSession {
                name: "old1".to_string(),
                path: "/tmp/old1".into(),
                branch: "main".to_string(),
                created_at: now - ChronoDuration::hours(25),
            },
            WorktreeSession {
                name: "old2".to_string(),
                path: "/tmp/old2".into(),
                branch: "main".to_string(),
                created_at: now - ChronoDuration::hours(26),
            },
        ];
        let max_age = Duration::from_secs(24 * 3600); // 24 hours

        let filtered = filter_old_sessions(sessions, max_age, now);
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn test_filter_old_sessions_mixed() {
        let now = Utc::now();
        let sessions = vec![
            WorktreeSession {
                name: "old".to_string(),
                path: "/tmp/old".into(),
                branch: "main".to_string(),
                created_at: now - ChronoDuration::hours(25),
            },
            WorktreeSession {
                name: "new".to_string(),
                path: "/tmp/new".into(),
                branch: "main".to_string(),
                created_at: now - ChronoDuration::hours(1),
            },
        ];
        let max_age = Duration::from_secs(24 * 3600); // 24 hours

        let filtered = filter_old_sessions(sessions, max_age, now);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "old");
    }
}
