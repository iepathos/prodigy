//! Pure predicate functions for session filtering

use super::state::{SessionFilter, SessionStatus, SessionType, UnifiedSession};
use chrono::{DateTime, Utc};

/// Check if session matches status filter (pure predicate)
pub fn matches_status_filter(session: &UnifiedSession, filter: &Option<SessionStatus>) -> bool {
    match filter {
        Some(status) => session.status == *status,
        None => true,
    }
}

/// Check if session matches type filter (pure predicate)
pub fn matches_type_filter(session: &UnifiedSession, filter: &Option<SessionType>) -> bool {
    let session_type = if session.workflow_data.is_some() {
        SessionType::Workflow
    } else {
        SessionType::MapReduce
    };

    match filter {
        Some(filter_type) => session_type == *filter_type,
        None => true,
    }
}

/// Check if session matches time filter (pure predicate)
pub fn matches_time_filter(
    session: &UnifiedSession,
    after: &Option<DateTime<Utc>>,
    before: &Option<DateTime<Utc>>,
) -> bool {
    let after_check = match after {
        Some(time) => session.started_at >= *time,
        None => true,
    };

    let before_check = match before {
        Some(time) => session.started_at <= *time,
        None => true,
    };

    after_check && before_check
}

/// Check if session matches worktree filter (pure predicate)
pub fn matches_worktree_filter(session: &UnifiedSession, worktree_name: &Option<String>) -> bool {
    match worktree_name {
        Some(name) => {
            if let Some(workflow_data) = &session.workflow_data {
                workflow_data.worktree_name.as_ref() == Some(name)
            } else {
                false
            }
        }
        None => true,
    }
}

/// Apply session filter (pure predicate combining all filters)
pub fn apply_session_filter(session: &UnifiedSession, filter: &SessionFilter) -> bool {
    matches_status_filter(session, &filter.status)
        && matches_type_filter(session, &filter.session_type)
        && matches_time_filter(session, &filter.after, &filter.before)
        && matches_worktree_filter(session, &filter.worktree_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_session() -> UnifiedSession {
        UnifiedSession::new_workflow("test-workflow".to_string(), "test".to_string())
    }

    #[test]
    fn test_matches_status_filter_some() {
        let session = create_test_session();
        assert!(matches_status_filter(
            &session,
            &Some(SessionStatus::Initializing)
        ));
        assert!(!matches_status_filter(
            &session,
            &Some(SessionStatus::Running)
        ));
    }

    #[test]
    fn test_matches_status_filter_none() {
        let session = create_test_session();
        assert!(matches_status_filter(&session, &None));
    }

    #[test]
    fn test_matches_type_filter() {
        let session = create_test_session();
        assert!(matches_type_filter(&session, &Some(SessionType::Workflow)));
        assert!(!matches_type_filter(
            &session,
            &Some(SessionType::MapReduce)
        ));
        assert!(matches_type_filter(&session, &None));
    }

    #[test]
    fn test_matches_time_filter() {
        let session = create_test_session();
        let now = Utc::now();
        let past = now - chrono::Duration::hours(1);
        let future = now + chrono::Duration::hours(1);

        assert!(matches_time_filter(&session, &Some(past), &None));
        assert!(!matches_time_filter(&session, &Some(future), &None));
        assert!(matches_time_filter(&session, &None, &Some(future)));
        assert!(!matches_time_filter(&session, &None, &Some(past)));
        assert!(matches_time_filter(&session, &None, &None));
    }

    #[test]
    fn test_matches_worktree_filter() {
        let session = create_test_session();

        // Session has no worktree name set, so should not match any specific name
        assert!(!matches_worktree_filter(
            &session,
            &Some("test-worktree".to_string())
        ));
        // But should match None filter
        assert!(matches_worktree_filter(&session, &None));
    }

    #[test]
    fn test_apply_session_filter() {
        let session = create_test_session();

        let filter = SessionFilter {
            status: Some(SessionStatus::Initializing),
            session_type: Some(SessionType::Workflow),
            after: None,
            before: None,
            worktree_name: None,
            limit: None,
        };

        assert!(apply_session_filter(&session, &filter));

        let filter_no_match = SessionFilter {
            status: Some(SessionStatus::Running),
            ..Default::default()
        };

        assert!(!apply_session_filter(&session, &filter_no_match));
    }
}
