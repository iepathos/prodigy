//! Unit tests for session coordinator

#[cfg(test)]
mod tests {
    use crate::cook::coordinators::session::{DefaultSessionCoordinator, SessionCoordinator};
    use crate::cook::session::{SessionStatus, SessionUpdate};
    use crate::testing::mocks::MockSessionManager;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_start_session() {
        // Setup
        let mock_session = Arc::new(MockSessionManager::new());
        let coordinator = DefaultSessionCoordinator::new(mock_session.clone());

        // Test
        let result = coordinator.start_session("test-session-123").await;

        // Verify
        assert!(result.is_ok());
        assert!(mock_session.was_start_called());

        // Verify session ID is stored
        let info = coordinator.get_session_info().await.unwrap();
        assert_eq!(info.session_id, "test-session-123");
    }

    #[tokio::test]
    async fn test_start_session_failure() {
        // Setup
        let mock_session = Arc::new(MockSessionManager::failing());
        let coordinator = DefaultSessionCoordinator::new(mock_session.clone());

        // Test
        let result = coordinator.start_session("test-session").await;

        // Verify
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "Mock failure");
    }

    #[tokio::test]
    async fn test_update_status() {
        // Setup
        let mock_session = Arc::new(MockSessionManager::new());
        let coordinator = DefaultSessionCoordinator::new(mock_session.clone());

        // Start session first
        coordinator.start_session("test-session").await.unwrap();

        // Test different status updates
        let statuses = vec![
            SessionStatus::InProgress,
            SessionStatus::Completed,
            SessionStatus::Failed,
            SessionStatus::Interrupted,
        ];

        for status in statuses {
            let result = coordinator.update_status(status.clone()).await;
            assert!(result.is_ok());

            // Verify update was recorded
            let updates = mock_session.get_update_calls();
            assert!(updates
                .iter()
                .any(|u| matches!(u, SessionUpdate::UpdateStatus(s) if *s == status)));
        }
    }

    #[tokio::test]
    async fn test_track_iteration() {
        // Setup
        let mock_session = Arc::new(MockSessionManager::new());
        let coordinator = DefaultSessionCoordinator::new(mock_session.clone());

        // Start session first
        coordinator.start_session("test-session").await.unwrap();

        // Test tracking multiple iterations
        for i in 1..=5 {
            let result = coordinator.track_iteration(i).await;
            assert!(result.is_ok());
        }

        // Verify all iterations were tracked
        let updates = mock_session.get_update_calls();
        let iteration_count = updates
            .iter()
            .filter(|u| matches!(u, SessionUpdate::IncrementIteration))
            .count();
        assert_eq!(iteration_count, 5);
    }

    #[tokio::test]
    async fn test_complete_session_success() {
        // Setup
        let mock_session = Arc::new(MockSessionManager::new());
        let coordinator = DefaultSessionCoordinator::new(mock_session.clone());

        // Start session and track some work
        coordinator.start_session("test-session").await.unwrap();
        coordinator.track_iteration(1).await.unwrap();
        coordinator.track_iteration(2).await.unwrap();

        // Test successful completion
        let result = coordinator.complete_session(true).await;
        assert!(result.is_ok());

        // Verify status was set to completed
        let updates = mock_session.get_update_calls();
        assert!(updates
            .iter()
            .any(|u| matches!(u, SessionUpdate::UpdateStatus(SessionStatus::Completed))));
    }

    #[tokio::test]
    async fn test_complete_session_failure() {
        // Setup
        let mock_session = Arc::new(MockSessionManager::new());
        let coordinator = DefaultSessionCoordinator::new(mock_session.clone());

        // Start session
        coordinator.start_session("test-session").await.unwrap();

        // Test failed completion
        let result = coordinator.complete_session(false).await;
        assert!(result.is_ok());

        // Verify status was set to failed
        let updates = mock_session.get_update_calls();
        assert!(updates
            .iter()
            .any(|u| matches!(u, SessionUpdate::UpdateStatus(SessionStatus::Failed))));
    }

    #[tokio::test]
    async fn test_get_session_info() {
        // Setup
        let mock_session = Arc::new(MockSessionManager::new());
        let coordinator = DefaultSessionCoordinator::new(mock_session.clone());

        // Test before starting session
        let info = coordinator.get_session_info().await.unwrap();
        assert_eq!(info.session_id, "unknown");
        assert_eq!(info.status, SessionStatus::InProgress);

        // Start session and test again
        coordinator.start_session("my-session").await.unwrap();
        coordinator
            .update_status(SessionStatus::InProgress)
            .await
            .unwrap();

        let info = coordinator.get_session_info().await.unwrap();
        assert_eq!(info.session_id, "my-session");
        assert_eq!(info.status, SessionStatus::InProgress);
    }

    #[tokio::test]
    async fn test_resume_session() {
        // Setup
        let mock_session = Arc::new(MockSessionManager::new());
        let coordinator = DefaultSessionCoordinator::new(mock_session.clone());

        // Start session and track iterations
        coordinator
            .start_session("resumable-session")
            .await
            .unwrap();
        coordinator.track_iteration(1).await.unwrap();
        coordinator.track_iteration(2).await.unwrap();
        coordinator.track_iteration(3).await.unwrap();

        // Test resuming
        let result = coordinator
            .resume_session("resumable-session")
            .await
            .unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap(), 3); // Should return iteration count

        // Complete session and test resuming again
        coordinator.complete_session(true).await.unwrap();
        let result = coordinator
            .resume_session("resumable-session")
            .await
            .unwrap();
        assert!(result.is_none()); // Cannot resume completed session
    }

    #[tokio::test]
    async fn test_session_lifecycle() {
        // Setup
        let mock_session = Arc::new(MockSessionManager::new());
        let coordinator = DefaultSessionCoordinator::new(mock_session.clone());

        // Full lifecycle test
        // 1. Start
        coordinator.start_session("lifecycle-test").await.unwrap();

        // 2. Update status to in progress
        coordinator
            .update_status(SessionStatus::InProgress)
            .await
            .unwrap();

        // 3. Track multiple iterations
        for i in 1..=10 {
            coordinator.track_iteration(i).await.unwrap();
        }

        // 4. Complete successfully
        coordinator.complete_session(true).await.unwrap();

        // Verify final state
        let info = coordinator.get_session_info().await.unwrap();
        assert_eq!(info.session_id, "lifecycle-test");
        assert_eq!(info.status, SessionStatus::Completed);

        // Verify all updates were recorded
        let updates = mock_session.get_update_calls();
        assert!(updates.len() > 10); // At least 10 iterations + status updates
    }

    #[tokio::test]
    async fn test_error_propagation() {
        // Setup with failing mock
        let mock_session = Arc::new(MockSessionManager::failing());
        let coordinator = DefaultSessionCoordinator::new(mock_session.clone());

        // Test that errors propagate correctly
        assert!(coordinator.start_session("test").await.is_err());
        assert!(coordinator
            .update_status(SessionStatus::InProgress)
            .await
            .is_err());
        assert!(coordinator.track_iteration(1).await.is_err());
        assert!(coordinator.complete_session(true).await.is_err());
    }
}
