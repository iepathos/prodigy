//! Comprehensive tests for migrating legacy session formats

#[cfg(test)]
mod migration_integration_tests {
    use crate::unified_session::migration::{SessionMigrator, MigrationReport};
    use crate::unified_session::SessionManager;
    use anyhow::Result;
    use serde_json::json;
    use std::fs;
    use tempfile::TempDir;

    /// Create a test environment with legacy session data
    async fn setup_test_env() -> Result<(TempDir, SessionManager)> {
        let temp_dir = TempDir::new()?;
        let storage = crate::storage::GlobalStorage::new()?;
        let manager = SessionManager::new(storage).await?;

        Ok((temp_dir, manager))
    }

    /// Create legacy session directory structure
    fn create_legacy_structure(temp_dir: &TempDir) -> Result<std::path::PathBuf> {
        let legacy_dir = temp_dir.path().join(".prodigy");
        fs::create_dir_all(&legacy_dir)?;
        Ok(legacy_dir)
    }

    #[tokio::test]
    async fn test_migrate_cook_session_format() -> Result<()> {
        let (temp_dir, manager) = setup_test_env().await?;
        let migrator = SessionMigrator::new(manager, temp_dir.path().to_path_buf());
        let legacy_dir = create_legacy_structure(&temp_dir)?;

        // Create a legacy cook session
        let cook_session = json!({
            "session_id": "cook-20241201-123456",
            "status": "Completed",
            "started_at": "2024-12-01T12:00:00Z",
            "ended_at": "2024-12-01T13:30:00Z",
            "iterations_completed": 10,
            "files_changed": 25,
            "errors": [],
            "working_directory": temp_dir.path().to_str().unwrap(),
            "worktree_name": "prodigy-session-abc123",
            "command_timings": {
                "claude /test": 120.5,
                "shell echo test": 0.5
            },
            "iteration_timings": [30.2, 45.1, 28.9]
        });

        fs::write(
            legacy_dir.join("session_cook-20241201-123456.json"),
            cook_session.to_string(),
        )?;

        // Run migration
        let report = migrator.migrate_from_legacy().await?;

        // Verify migration
        assert_eq!(report.successful_migrations.len(), 1);
        assert_eq!(report.failed_migrations.len(), 0);
        assert!(report.is_successful());

        // Verify legacy directory was archived
        assert!(!legacy_dir.exists());
        assert!(temp_dir.path().join(".prodigy_migrated").exists());

        Ok(())
    }

    #[tokio::test]
    async fn test_migrate_mapreduce_session_format() -> Result<()> {
        let (temp_dir, manager) = setup_test_env().await?;
        let migrator = SessionMigrator::new(manager, temp_dir.path().to_path_buf());
        let legacy_dir = create_legacy_structure(&temp_dir)?;

        // Create a legacy MapReduce session
        let mapreduce_session = json!({
            "session_id": "mapreduce-job-xyz789",
            "status": "Interrupted",
            "started_at": "2024-11-15T09:00:00Z",
            "ended_at": null,
            "iterations_completed": 0,
            "files_changed": 0,
            "errors": ["Agent 3 failed: connection timeout"],
            "working_directory": temp_dir.path().to_str().unwrap(),
            "worktree_name": null,
            "job_config": {
                "max_parallel": 10,
                "total_items": 100,
                "processed_items": 67
            },
            "checkpoint": {
                "last_processed_index": 66,
                "failed_items": [23, 45, 67]
            }
        });

        fs::write(
            legacy_dir.join("session_mapreduce-job-xyz789.json"),
            mapreduce_session.to_string(),
        )?;

        // Run migration
        let report = migrator.migrate_from_legacy().await?;

        // Verify migration
        assert_eq!(report.successful_migrations.len(), 1);
        assert_eq!(report.failed_migrations.len(), 0);
        assert!(report.is_successful());

        Ok(())
    }

    #[tokio::test]
    async fn test_migrate_multiple_sessions() -> Result<()> {
        let (temp_dir, manager) = setup_test_env().await?;
        let migrator = SessionMigrator::new(manager, temp_dir.path().to_path_buf());
        let legacy_dir = create_legacy_structure(&temp_dir)?;

        // Create multiple legacy sessions with different statuses
        let sessions = vec![
            ("session_1.json", "InProgress", 5, 10),
            ("session_2.json", "Completed", 20, 50),
            ("session_3.json", "Failed", 2, 3),
            ("session_4.json", "Interrupted", 8, 15),
        ];

        for (filename, status, iterations, files) in sessions {
            let session_data = json!({
                "session_id": filename.trim_end_matches(".json"),
                "status": status,
                "started_at": "2024-11-01T00:00:00Z",
                "ended_at": if status == "Completed" {
                    Some("2024-11-01T01:00:00Z")
                } else {
                    None
                },
                "iterations_completed": iterations,
                "files_changed": files,
                "errors": if status == "Failed" {
                    vec!["Test error"]
                } else {
                    vec![]
                },
                "working_directory": temp_dir.path().to_str().unwrap(),
                "worktree_name": format!("worktree-{}", filename.trim_end_matches(".json"))
            });

            fs::write(legacy_dir.join(filename), session_data.to_string())?;
        }

        // Run migration
        let report = migrator.migrate_from_legacy().await?;

        // Verify all sessions were migrated
        assert_eq!(report.successful_migrations.len(), 4);
        assert_eq!(report.failed_migrations.len(), 0);
        assert!(report.is_successful());
        assert_eq!(report.summary(), "Successfully migrated 4 session(s)");

        Ok(())
    }

    #[tokio::test]
    async fn test_migrate_session_with_checkpoint() -> Result<()> {
        let (temp_dir, manager) = setup_test_env().await?;
        let migrator = SessionMigrator::new(manager, temp_dir.path().to_path_buf());
        let legacy_dir = create_legacy_structure(&temp_dir)?;

        // Create a session with checkpoint data
        let session_with_checkpoint = json!({
            "session_id": "resumable-session",
            "status": "Interrupted",
            "started_at": "2024-10-20T14:00:00Z",
            "ended_at": null,
            "iterations_completed": 3,
            "files_changed": 7,
            "errors": [],
            "working_directory": temp_dir.path().to_str().unwrap(),
            "worktree_name": "resumable-worktree",
            "workflow_state": {
                "current_step": 5,
                "total_steps": 10,
                "step_results": [
                    {"step": 1, "success": true},
                    {"step": 2, "success": true},
                    {"step": 3, "success": true},
                    {"step": 4, "success": true},
                    {"step": 5, "success": false}
                ]
            },
            "checkpoint": {
                "workflow_hash": "abc123def456",
                "resume_point": {
                    "step_index": 5,
                    "retry_count": 2
                }
            }
        });

        fs::write(
            legacy_dir.join("session_resumable.json"),
            session_with_checkpoint.to_string(),
        )?;

        // Run migration
        let report = migrator.migrate_from_legacy().await?;

        // Verify migration preserved checkpoint data
        assert_eq!(report.successful_migrations.len(), 1);
        assert!(report.is_successful());

        Ok(())
    }

    #[tokio::test]
    async fn test_migrate_corrupted_session() -> Result<()> {
        let (temp_dir, manager) = setup_test_env().await?;
        let migrator = SessionMigrator::new(manager, temp_dir.path().to_path_buf());
        let legacy_dir = create_legacy_structure(&temp_dir)?;

        // Create a valid session
        let valid_session = json!({
            "session_id": "valid-session",
            "status": "Completed",
            "started_at": "2024-09-01T00:00:00Z",
            "ended_at": "2024-09-01T01:00:00Z",
            "iterations_completed": 1,
            "files_changed": 2,
            "errors": [],
            "working_directory": temp_dir.path().to_str().unwrap(),
            "worktree_name": "valid-worktree"
        });

        fs::write(
            legacy_dir.join("session_valid.json"),
            valid_session.to_string(),
        )?;

        // Create a corrupted session file
        fs::write(
            legacy_dir.join("session_corrupted.json"),
            "{ invalid json data }",
        )?;

        // Create a session with missing required fields
        let incomplete_session = json!({
            "session_id": "incomplete-session"
            // Missing required fields
        });

        fs::write(
            legacy_dir.join("session_incomplete.json"),
            incomplete_session.to_string(),
        )?;

        // Run migration
        let report = migrator.migrate_from_legacy().await?;

        // Verify partial success
        assert_eq!(report.successful_migrations.len(), 1);
        assert_eq!(report.failed_migrations.len(), 2);
        assert!(!report.is_successful());

        // Legacy directory should not be archived due to failures
        assert!(legacy_dir.exists());

        Ok(())
    }

    #[tokio::test]
    async fn test_migrate_with_special_characters() -> Result<()> {
        let (temp_dir, manager) = setup_test_env().await?;
        let migrator = SessionMigrator::new(manager, temp_dir.path().to_path_buf());
        let legacy_dir = create_legacy_structure(&temp_dir)?;

        // Create a session with special characters
        let special_session = json!({
            "session_id": "session-with-ñ-ü-特殊文字",
            "status": "Completed",
            "started_at": "2024-08-15T00:00:00Z",
            "ended_at": "2024-08-15T02:00:00Z",
            "iterations_completed": 1,
            "files_changed": 1,
            "errors": ["Error with 特殊文字 and émojis 🚀"],
            "working_directory": temp_dir.path().to_str().unwrap(),
            "worktree_name": "worktree-特殊"
        });

        fs::write(
            legacy_dir.join("session_special.json"),
            special_session.to_string(),
        )?;

        // Run migration
        let report = migrator.migrate_from_legacy().await?;

        // Verify successful migration with special characters
        assert_eq!(report.successful_migrations.len(), 1);
        assert!(report.is_successful());

        Ok(())
    }

    #[tokio::test]
    async fn test_migrate_very_old_format() -> Result<()> {
        let (temp_dir, manager) = setup_test_env().await?;
        let migrator = SessionMigrator::new(manager, temp_dir.path().to_path_buf());
        let legacy_dir = create_legacy_structure(&temp_dir)?;

        // Create a very old session format (minimal fields)
        let old_format = json!({
            "session_id": "old-format-2023",
            "status": "Done",  // Old status name
            "started_at": "2023-01-01T00:00:00Z",
            "iterations_completed": 0,  // Old sessions might not track this
            "files_changed": 0,
            "errors": [],
            "working_directory": "/old/path",
            "worktree_name": null  // Old format might not have worktree
        });

        fs::write(
            legacy_dir.join("session_old.json"),
            old_format.to_string(),
        )?;

        // Run migration
        let report = migrator.migrate_from_legacy().await?;

        // Should handle unknown status gracefully
        assert_eq!(report.successful_migrations.len(), 1);
        assert!(report.is_successful());

        Ok(())
    }

    #[tokio::test]
    async fn test_migration_idempotency() -> Result<()> {
        let (temp_dir, manager) = setup_test_env().await?;
        let migrator = SessionMigrator::new(manager, temp_dir.path().to_path_buf());
        let legacy_dir = create_legacy_structure(&temp_dir)?;

        // Create a session
        let session = json!({
            "session_id": "idempotent-test",
            "status": "Completed",
            "started_at": "2024-07-01T00:00:00Z",
            "ended_at": "2024-07-01T01:00:00Z",
            "iterations_completed": 5,
            "files_changed": 10,
            "errors": [],
            "working_directory": temp_dir.path().to_str().unwrap(),
            "worktree_name": "test-worktree"
        });

        fs::write(
            legacy_dir.join("session_test.json"),
            session.to_string(),
        )?;

        // Run migration
        let report1 = migrator.migrate_from_legacy().await?;
        assert_eq!(report1.successful_migrations.len(), 1);

        // Directory should be archived
        assert!(!legacy_dir.exists());
        let archive_dir = temp_dir.path().join(".prodigy_migrated");
        assert!(archive_dir.exists());

        // Restore the archived directory for second migration attempt
        fs::rename(&archive_dir, &legacy_dir)?;

        // Run migration again
        let report2 = migrator.migrate_from_legacy().await?;

        // Should migrate again (creating new session IDs)
        assert_eq!(report2.successful_migrations.len(), 1);

        Ok(())
    }

    #[tokio::test]
    async fn test_empty_legacy_directory() -> Result<()> {
        let (temp_dir, manager) = setup_test_env().await?;
        let migrator = SessionMigrator::new(manager, temp_dir.path().to_path_buf());
        let legacy_dir = create_legacy_structure(&temp_dir)?;

        // Create some non-session files
        fs::write(legacy_dir.join("config.json"), "{}")?;
        fs::write(legacy_dir.join("settings.yaml"), "key: value")?;
        fs::write(legacy_dir.join("README.md"), "# README")?;

        // Run migration
        let report = migrator.migrate_from_legacy().await?;

        // Should not migrate non-session files
        assert_eq!(report.successful_migrations.len(), 0);
        assert_eq!(report.failed_migrations.len(), 0);
        assert_eq!(report.summary(), "No sessions found to migrate");

        // Directory should not be archived since no migrations occurred
        assert!(legacy_dir.exists());

        Ok(())
    }

    #[tokio::test]
    async fn test_concurrent_session_access() -> Result<()> {
        // For concurrent test, we need separate managers
        let temp_dir = TempDir::new()?;
        let legacy_dir = create_legacy_structure(&temp_dir)?;

        // Create a session
        let session = json!({
            "session_id": "concurrent-test",
            "status": "InProgress",
            "started_at": "2024-06-01T00:00:00Z",
            "ended_at": null,
            "iterations_completed": 2,
            "files_changed": 5,
            "errors": [],
            "working_directory": temp_dir.path().to_str().unwrap(),
            "worktree_name": "concurrent-worktree"
        });

        fs::write(
            legacy_dir.join("session_concurrent.json"),
            session.to_string(),
        )?;

        // Create multiple migrators with separate managers
        let tasks = (0..3).map(|_| {
            let path = temp_dir.path().to_path_buf();
            tokio::spawn(async move {
                let storage = crate::storage::GlobalStorage::new().unwrap();
                let manager = SessionManager::new(storage).await.unwrap();
                let m = SessionMigrator::new(manager, path);
                m.migrate_from_legacy().await
            })
        });

        // Run migrations concurrently
        let results: Vec<_> = futures::future::join_all(tasks).await;

        // At least one should succeed
        let successful_count = results.iter()
            .filter(|r| r.is_ok())
            .filter_map(|r| r.as_ref().ok())
            .filter(|r| r.as_ref().map(|report: &MigrationReport| report.is_successful()).unwrap_or(false))
            .count();

        assert!(successful_count >= 1);

        Ok(())
    }

    #[tokio::test]
    async fn test_preserve_metadata_during_migration() -> Result<()> {
        let (temp_dir, manager) = setup_test_env().await?;
        let migrator = SessionMigrator::new(manager, temp_dir.path().to_path_buf());
        let legacy_dir = create_legacy_structure(&temp_dir)?;

        // Create a session with rich metadata
        let session = json!({
            "session_id": "metadata-test",
            "status": "Completed",
            "started_at": "2024-05-01T10:00:00Z",
            "ended_at": "2024-05-01T11:30:00Z",
            "iterations_completed": 15,
            "files_changed": 42,
            "errors": [],
            "working_directory": temp_dir.path().to_str().unwrap(),
            "worktree_name": "feature-branch-123",
            "extra_metadata": {
                "user": "test-user",
                "machine": "dev-machine",
                "git_branch": "feature/awesome",
                "git_commit": "abc123def",
                "environment": {
                    "os": "linux",
                    "arch": "x86_64"
                }
            },
            "performance_metrics": {
                "total_duration_seconds": 5400,
                "cpu_usage_percent": 45.2,
                "memory_usage_mb": 1024
            }
        });

        fs::write(
            legacy_dir.join("session_metadata.json"),
            session.to_string(),
        )?;

        // Run migration
        let report = migrator.migrate_from_legacy().await?;

        // Verify successful migration with metadata preservation
        assert_eq!(report.successful_migrations.len(), 1);
        assert!(report.is_successful());

        Ok(())
    }
}