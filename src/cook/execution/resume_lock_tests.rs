//! Unit tests for resume lock functionality

#[cfg(test)]
mod tests {
    use super::super::resume_lock::{is_process_running, ResumeLockData, ResumeLockManager};
    use chrono::Utc;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_acquire_lock_success() {
        let temp_dir = TempDir::new().unwrap();
        let manager = ResumeLockManager::new(temp_dir.path().to_path_buf()).unwrap();

        let lock = manager.acquire_lock("test-job").await;
        assert!(lock.is_ok());
    }

    #[tokio::test]
    async fn test_acquire_lock_fails_when_held() {
        let temp_dir = TempDir::new().unwrap();
        let manager = ResumeLockManager::new(temp_dir.path().to_path_buf()).unwrap();

        let _lock1 = manager.acquire_lock("test-job").await.unwrap();
        let lock2 = manager.acquire_lock("test-job").await;

        assert!(lock2.is_err());
        let error_msg = lock2.unwrap_err().to_string();
        assert!(error_msg.contains("already in progress"));
    }

    #[tokio::test]
    async fn test_lock_released_on_drop() {
        let temp_dir = TempDir::new().unwrap();
        let manager = ResumeLockManager::new(temp_dir.path().to_path_buf()).unwrap();

        {
            let _lock = manager.acquire_lock("test-job").await.unwrap();
            // Lock is held
        } // Lock dropped here

        // Should be able to acquire again
        let lock2 = manager.acquire_lock("test-job").await;
        assert!(lock2.is_ok());
    }

    #[tokio::test]
    async fn test_stale_lock_cleanup() {
        let temp_dir = TempDir::new().unwrap();
        let manager = ResumeLockManager::new(temp_dir.path().to_path_buf()).unwrap();

        // Create lock with fake PID (guaranteed not running)
        let lock_path = temp_dir.path().join("resume_locks/test-job.lock");
        std::fs::create_dir_all(lock_path.parent().unwrap()).unwrap();

        let stale_lock = ResumeLockData {
            job_id: "test-job".to_string(),
            process_id: 999999, // Fake PID
            hostname: "test-host".to_string(),
            acquired_at: Utc::now(),
        };
        std::fs::write(&lock_path, serde_json::to_string(&stale_lock).unwrap()).unwrap();

        // Try to acquire - should clean up stale lock and succeed
        let lock = manager.acquire_lock("test-job").await;
        assert!(lock.is_ok());
    }

    #[test]
    fn test_is_process_running_current_process() {
        let current_pid = std::process::id();
        assert!(is_process_running(current_pid));
    }

    #[test]
    fn test_is_process_running_fake_process() {
        let fake_pid = 999999;
        assert!(!is_process_running(fake_pid));
    }

    #[test]
    fn test_lock_data_serialization() {
        let lock_data = ResumeLockData::new("test-job".to_string());
        let json = serde_json::to_string(&lock_data).unwrap();
        let deserialized: ResumeLockData = serde_json::from_str(&json).unwrap();

        assert_eq!(lock_data.job_id, deserialized.job_id);
        assert_eq!(lock_data.process_id, deserialized.process_id);
    }

    #[tokio::test]
    async fn test_multiple_jobs_different_locks() {
        let temp_dir = TempDir::new().unwrap();
        let manager = ResumeLockManager::new(temp_dir.path().to_path_buf()).unwrap();

        // Can acquire locks for different jobs simultaneously
        let _lock1 = manager.acquire_lock("job-1").await.unwrap();
        let _lock2 = manager.acquire_lock("job-2").await.unwrap();

        // Both locks should be held
        assert!(manager.acquire_lock("job-1").await.is_err());
        assert!(manager.acquire_lock("job-2").await.is_err());
    }

    #[tokio::test]
    async fn test_lock_error_message_helpful() {
        let temp_dir = TempDir::new().unwrap();
        let manager = ResumeLockManager::new(temp_dir.path().to_path_buf()).unwrap();

        let _lock1 = manager.acquire_lock("test-job").await.unwrap();
        let lock2 = manager.acquire_lock("test-job").await;

        assert!(lock2.is_err());
        let error = lock2.unwrap_err().to_string();

        // Check that error message contains helpful information
        assert!(error.contains("already in progress"));
        assert!(error.contains("test-job"));
        assert!(error.contains("PID"));
    }

    #[tokio::test]
    async fn test_lock_survives_manager_recreation() {
        let temp_dir = TempDir::new().unwrap();

        {
            let manager = ResumeLockManager::new(temp_dir.path().to_path_buf()).unwrap();
            let _lock = manager.acquire_lock("test-job").await.unwrap();

            // Create new manager instance
            let manager2 = ResumeLockManager::new(temp_dir.path().to_path_buf()).unwrap();

            // Should not be able to acquire lock from different manager instance
            assert!(manager2.acquire_lock("test-job").await.is_err());
        }

        // After lock dropped, new manager should be able to acquire
        let manager3 = ResumeLockManager::new(temp_dir.path().to_path_buf()).unwrap();
        assert!(manager3.acquire_lock("test-job").await.is_ok());
    }
}
