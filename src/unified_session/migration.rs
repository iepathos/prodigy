//! Migration utilities for transitioning from old session formats to unified session

use super::{SessionConfig, SessionId, SessionManager, SessionStatus, SessionType};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::{info, warn};

/// Old session format from legacy system
#[derive(Debug, Deserialize, Serialize)]
struct LegacySessionState {
    session_id: String,
    status: String,
    started_at: chrono::DateTime<chrono::Utc>,
    ended_at: Option<chrono::DateTime<chrono::Utc>>,
    iterations_completed: usize,
    files_changed: usize,
    errors: Vec<String>,
    working_directory: PathBuf,
    worktree_name: Option<String>,
}

/// Migrate session data from old format to new unified format
pub struct SessionMigrator {
    manager: SessionManager,
    project_path: PathBuf,
}

impl SessionMigrator {
    /// Create new migrator
    pub fn new(manager: SessionManager, project_path: PathBuf) -> Self {
        Self {
            manager,
            project_path,
        }
    }

    /// Migrate all sessions from legacy .prodigy directory
    pub async fn migrate_from_legacy(&self) -> Result<MigrationReport> {
        let legacy_dir = self.project_path.join(".prodigy");

        if !legacy_dir.exists() {
            info!("No legacy session directory found, skipping migration");
            return Ok(MigrationReport::default());
        }

        let mut report = MigrationReport::default();

        // Look for session state files
        let entries = std::fs::read_dir(&legacy_dir)?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            // Look for session JSON files
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                let filename = path.file_name().and_then(|s| s.to_str()).unwrap_or("");

                // Skip non-session files
                if !filename.starts_with("session_") && filename != "session_state.json" {
                    continue;
                }

                match self.migrate_single_session(&path).await {
                    Ok(session_id) => {
                        info!("Migrated session: {}", session_id.as_str());
                        report
                            .successful_migrations
                            .push(session_id.as_str().to_string());
                    }
                    Err(e) => {
                        warn!("Failed to migrate session from {}: {}", path.display(), e);
                        report.failed_migrations.push((path.clone(), e.to_string()));
                    }
                }
            }
        }

        // Archive the old directory if migration was successful
        if !report.successful_migrations.is_empty() && report.failed_migrations.is_empty() {
            self.archive_legacy_directory(&legacy_dir)?;
        }

        Ok(report)
    }

    /// Migrate a single session from old format
    async fn migrate_single_session(&self, path: &Path) -> Result<SessionId> {
        // Read legacy session data
        let content =
            std::fs::read_to_string(path).context("Failed to read legacy session file")?;

        let legacy_session: LegacySessionState =
            serde_json::from_str(&content).context("Failed to parse legacy session data")?;

        // Convert status
        let status = match legacy_session.status.as_str() {
            "InProgress" => SessionStatus::Running,
            "Completed" => SessionStatus::Completed,
            "Failed" => SessionStatus::Failed,
            "Interrupted" => SessionStatus::Paused,
            _ => SessionStatus::Running,
        };

        // Extract project name from working directory
        let project_name = self
            .project_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        // Create new session config
        let mut metadata = std::collections::HashMap::new();
        metadata.insert("project_name".to_string(), serde_json::json!(project_name));
        if let Some(worktree_name) = &legacy_session.worktree_name {
            metadata.insert(
                "worktree_name".to_string(),
                serde_json::json!(worktree_name),
            );
        }
        metadata.insert("started_by".to_string(), serde_json::json!("migrated"));
        metadata.insert("tags".to_string(), serde_json::json!(vec!["migrated"]));
        metadata.insert(
            "description".to_string(),
            serde_json::json!(format!(
                "Migrated from legacy session {}",
                legacy_session.session_id
            )),
        );

        let config = SessionConfig {
            session_type: SessionType::Workflow,
            workflow_id: Some(legacy_session.session_id.clone()),
            job_id: None,
            metadata,
        };

        // Create new session
        let session_id = self.manager.create_session(config).await?;

        // Update with legacy data
        let update = super::SessionUpdate::Status(status);
        self.manager.update_session(&session_id, update).await?;

        // Add metadata about legacy session
        let mut metadata = std::collections::HashMap::new();
        metadata.insert(
            "legacy_session_id".to_string(),
            serde_json::json!(legacy_session.session_id),
        );
        metadata.insert(
            "iterations_completed".to_string(),
            serde_json::json!(legacy_session.iterations_completed),
        );
        metadata.insert(
            "files_changed".to_string(),
            serde_json::json!(legacy_session.files_changed),
        );

        if !legacy_session.errors.is_empty() {
            metadata.insert(
                "errors".to_string(),
                serde_json::json!(legacy_session.errors),
            );
        }

        let update = super::SessionUpdate::Metadata(metadata);
        self.manager.update_session(&session_id, update).await?;

        Ok(session_id)
    }

    /// Archive the legacy directory
    fn archive_legacy_directory(&self, legacy_dir: &Path) -> Result<()> {
        let archive_path = legacy_dir.with_file_name(".prodigy_migrated");

        if archive_path.exists() {
            // Remove previous archive
            std::fs::remove_dir_all(&archive_path)?;
        }

        std::fs::rename(legacy_dir, &archive_path)?;
        info!(
            "Archived legacy session directory to {}",
            archive_path.display()
        );

        Ok(())
    }

    /// Check if migration is needed
    pub async fn needs_migration(&self) -> bool {
        let legacy_dir = self.project_path.join(".prodigy");

        if !legacy_dir.exists() {
            return false;
        }

        // Check if there are any session files
        if let Ok(entries) = std::fs::read_dir(&legacy_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("json") {
                    let filename = path.file_name().and_then(|s| s.to_str()).unwrap_or("");

                    if filename.starts_with("session_") || filename == "session_state.json" {
                        return true;
                    }
                }
            }
        }

        false
    }
}

/// Report of migration results
#[derive(Debug, Default)]
pub struct MigrationReport {
    pub successful_migrations: Vec<String>,
    pub failed_migrations: Vec<(PathBuf, String)>,
}

impl MigrationReport {
    /// Check if migration was fully successful
    pub fn is_successful(&self) -> bool {
        !self.successful_migrations.is_empty() && self.failed_migrations.is_empty()
    }

    /// Get total number of sessions processed
    pub fn total_processed(&self) -> usize {
        self.successful_migrations.len() + self.failed_migrations.len()
    }

    /// Generate a summary message
    pub fn summary(&self) -> String {
        if self.total_processed() == 0 {
            return "No sessions found to migrate".to_string();
        }

        let success_count = self.successful_migrations.len();
        let failure_count = self.failed_migrations.len();

        if failure_count == 0 {
            format!("Successfully migrated {} session(s)", success_count)
        } else {
            format!(
                "Migrated {} session(s), {} failed",
                success_count, failure_count
            )
        }
    }
}

/// Auto-migrate sessions on startup
pub async fn auto_migrate(
    manager: SessionManager,
    project_path: PathBuf,
) -> Result<Option<MigrationReport>> {
    let migrator = SessionMigrator::new(manager, project_path);

    if !migrator.needs_migration().await {
        return Ok(None);
    }

    info!("Detected legacy session data, starting migration...");
    let report = migrator.migrate_from_legacy().await?;

    if report.is_successful() {
        info!("{}", report.summary());
    } else {
        warn!("{}", report.summary());
        for (path, error) in &report.failed_migrations {
            warn!("  Failed: {} - {}", path.display(), error);
        }
    }

    Ok(Some(report))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_migration_detection() {
        let temp_dir = TempDir::new().unwrap();
        let storage = crate::storage::GlobalStorage::new().unwrap();
        let manager = SessionManager::new(storage).await.unwrap();

        let migrator = SessionMigrator::new(manager, temp_dir.path().to_path_buf());

        // No migration needed for empty directory
        assert!(!migrator.needs_migration().await);

        // Create legacy directory with session file
        let legacy_dir = temp_dir.path().join(".prodigy");
        std::fs::create_dir_all(&legacy_dir).unwrap();

        let session_data = r#"{
            "session_id": "test-123",
            "status": "Completed",
            "started_at": "2024-01-01T00:00:00Z",
            "ended_at": "2024-01-01T01:00:00Z",
            "iterations_completed": 5,
            "files_changed": 10,
            "errors": [],
            "working_directory": "/test",
            "worktree_name": "test-worktree"
        }"#;

        std::fs::write(legacy_dir.join("session_state.json"), session_data).unwrap();

        // Now migration is needed
        assert!(migrator.needs_migration().await);
    }

    #[tokio::test]
    async fn test_migration_report() {
        let mut report = MigrationReport::default();

        assert_eq!(report.total_processed(), 0);
        assert_eq!(report.summary(), "No sessions found to migrate");

        report.successful_migrations.push("session-1".to_string());
        report.successful_migrations.push("session-2".to_string());

        assert!(report.is_successful());
        assert_eq!(report.total_processed(), 2);
        assert_eq!(report.summary(), "Successfully migrated 2 session(s)");

        report
            .failed_migrations
            .push((PathBuf::from("failed.json"), "Parse error".to_string()));

        assert!(!report.is_successful());
        assert_eq!(report.total_processed(), 3);
        assert_eq!(report.summary(), "Migrated 2 session(s), 1 failed");
    }
}
