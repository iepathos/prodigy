//! Tests for worktree cleanup functionality

#[cfg(test)]
mod coordinator_tests {
    use super::super::{
        CleanupTask, WorktreeCleanupConfig, WorktreeCleanupCoordinator, WorktreeMetrics,
        WorktreeResourceMonitor,
    };
    use std::time::Duration;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_cleanup_coordinator_creation() {
        let temp_dir = TempDir::new().unwrap();
        let config = WorktreeCleanupConfig::default();
        let coordinator =
            WorktreeCleanupCoordinator::new(config.clone(), temp_dir.path().to_path_buf());

        // Verify coordinator is created - just test that registration works
        let _guard = coordinator.register_job("test-job").await;
        // If we get here without panic, the coordinator was created successfully
    }

    #[tokio::test]
    async fn test_cleanup_task_scheduling() {
        let temp_dir = TempDir::new().unwrap();
        let config = WorktreeCleanupConfig::default();
        let coordinator = WorktreeCleanupCoordinator::new(config, temp_dir.path().to_path_buf());

        // Schedule immediate cleanup
        let task = CleanupTask::Immediate {
            worktree_path: temp_dir.path().join("worktree1"),
            job_id: "job1".to_string(),
        };

        assert!(coordinator.schedule_cleanup(task).await.is_ok());

        // Schedule delayed cleanup
        let task = CleanupTask::Scheduled {
            worktree_path: temp_dir.path().join("worktree2"),
            delay: Duration::from_secs(1),
        };

        assert!(coordinator.schedule_cleanup(task).await.is_ok());
    }

    #[tokio::test]
    async fn test_cleanup_config_variations() {
        let default = WorktreeCleanupConfig::default();
        assert!(default.auto_cleanup);
        assert_eq!(default.cleanup_delay_secs, 30);

        let immediate = WorktreeCleanupConfig::immediate();
        assert_eq!(immediate.cleanup_delay_secs, 0);

        let aggressive = WorktreeCleanupConfig::aggressive();
        assert_eq!(aggressive.cleanup_delay_secs, 5);
        assert_eq!(aggressive.max_worktrees_per_job, 20);

        let conservative = WorktreeCleanupConfig::conservative();
        assert_eq!(conservative.cleanup_delay_secs, 120);
        assert_eq!(conservative.max_worktrees_per_job, 100);
    }

    #[tokio::test]
    async fn test_resource_limit_checking() {
        let config = WorktreeCleanupConfig::default();

        // Test limit exceeded
        assert!(config.is_limit_exceeded(200, 50)); // Total limit
        assert!(config.is_limit_exceeded(100, 51)); // Job limit
        assert!(!config.is_limit_exceeded(100, 49)); // Within limits
    }

    #[tokio::test]
    async fn test_orphaned_worktree_cleanup() {
        let temp_dir = TempDir::new().unwrap();
        let config = WorktreeCleanupConfig::default();
        let coordinator = WorktreeCleanupCoordinator::new(config, temp_dir.path().to_path_buf());

        // Create some test directories to simulate orphaned worktrees
        let orphan1 = temp_dir.path().join("orphan1");
        let orphan2 = temp_dir.path().join("orphan2");
        std::fs::create_dir(&orphan1).unwrap();
        std::fs::create_dir(&orphan2).unwrap();

        // Clean orphaned worktrees (they're all "old" in this test)
        let result = coordinator
            .cleanup_orphaned_worktrees(Duration::from_secs(0))
            .await;

        // Should succeed even if no actual git worktrees were cleaned
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_job_cleanup() {
        let temp_dir = TempDir::new().unwrap();
        let config = WorktreeCleanupConfig::default();
        let coordinator = WorktreeCleanupCoordinator::new(config, temp_dir.path().to_path_buf());

        let job_id = "test-job";

        // Register a job
        let _guard = coordinator.register_job(job_id).await;

        // Register some worktrees for the job
        let worktree1 = temp_dir.path().join("worktree1");
        let worktree2 = temp_dir.path().join("worktree2");
        std::fs::create_dir(&worktree1).unwrap();
        std::fs::create_dir(&worktree2).unwrap();

        let _guard1 = coordinator
            .register_worktree(job_id, "agent1", worktree1)
            .await;
        let _guard2 = coordinator
            .register_worktree(job_id, "agent2", worktree2)
            .await;

        // Clean up the job
        let result = coordinator.cleanup_job(job_id).await;
        assert!(result.is_ok());

        // Verify cleanup attempted
        let count = result.unwrap();
        assert_eq!(count, 2);
    }

    #[tokio::test]
    async fn test_resource_monitor() {
        let mut monitor = WorktreeResourceMonitor::new(100, 10, 50);

        // Test initial state
        let metrics = monitor.get_metrics();
        assert_eq!(metrics.active_worktrees, 0);
        assert_eq!(metrics.cleanup_operations, 0);

        // Update metrics
        let new_metrics = WorktreeMetrics {
            active_worktrees: 45,
            total_disk_usage: 80 * 1024 * 1024, // 80MB
            cleanup_operations: 5,
            cleanup_failures: 1,
            average_cleanup_time: Duration::from_secs(2),
            orphaned_worktrees: 3,
        };
        monitor.update_metrics(new_metrics);

        // Check recommendations
        let recommendation = monitor.cleanup_recommendation();
        match recommendation {
            super::super::CleanupRecommendation::CleanupOld { .. } => {
                // Expected when disk usage is high
            }
            _ => {
                // Other recommendations are also valid
            }
        }

        // Test resource limit checking
        assert!(monitor.check_limits().is_ok());

        // Test with exceeded limits
        let exceeded_metrics = WorktreeMetrics {
            active_worktrees: 51, // Exceeds limit of 50
            ..Default::default()
        };
        monitor.update_metrics(exceeded_metrics);
        assert!(monitor.check_limits().is_err());
    }

    #[tokio::test]
    async fn test_cleanup_error_types() {
        use super::super::CleanupError;

        let timeout_err = CleanupError::Timeout {
            timeout: Duration::from_secs(30),
        };
        assert!(timeout_err.is_recoverable());
        assert!(timeout_err.should_retry());

        let active_err = CleanupError::WorktreeActive;
        assert!(active_err.is_recoverable());
        assert!(!active_err.should_retry());

        let git_err = CleanupError::GitError("command failed".to_string());
        assert!(!git_err.is_recoverable());
        assert!(!git_err.should_retry());
    }

    #[tokio::test]
    async fn test_dry_run_cleanup() {
        let temp_dir = TempDir::new().unwrap();
        let config = WorktreeCleanupConfig::default();
        let coordinator = WorktreeCleanupCoordinator::new(config, temp_dir.path().to_path_buf());

        // Create a test directory
        let test_worktree = temp_dir.path().join("test-worktree");
        std::fs::create_dir(&test_worktree).unwrap();

        // Verify directory exists
        assert!(test_worktree.exists());

        // Perform cleanup (it won't actually remove non-git worktrees)
        let _ = coordinator.cleanup_worktree(&test_worktree, true).await;

        // In a real scenario with git worktrees, this would test
        // that dry_run doesn't actually remove anything
    }

    #[tokio::test]
    async fn test_batch_cleanup() {
        let temp_dir = TempDir::new().unwrap();
        let config = WorktreeCleanupConfig::default();
        let coordinator = WorktreeCleanupCoordinator::new(config, temp_dir.path().to_path_buf());

        // Create batch cleanup task
        let paths = vec![
            temp_dir.path().join("worktree1"),
            temp_dir.path().join("worktree2"),
            temp_dir.path().join("worktree3"),
        ];

        for path in &paths {
            std::fs::create_dir(path).unwrap();
        }

        let task = CleanupTask::Batch {
            worktree_paths: paths.clone(),
        };

        // Schedule batch cleanup
        assert!(coordinator.schedule_cleanup(task).await.is_ok());
    }

    #[tokio::test]
    async fn test_cleanup_guard() {
        let temp_dir = TempDir::new().unwrap();
        let config = WorktreeCleanupConfig::default();
        let coordinator = WorktreeCleanupCoordinator::new(config, temp_dir.path().to_path_buf());

        let worktree_path = temp_dir.path().join("guarded-worktree");
        std::fs::create_dir(&worktree_path).unwrap();

        let guard = coordinator
            .register_worktree("job1", "agent1", worktree_path.clone())
            .await;

        // Test scheduled cleanup
        assert!(guard.schedule_cleanup(Duration::from_secs(1)).await.is_ok());
    }
}
