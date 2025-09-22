//! Runtime initialization and setup
//!
//! This module handles application startup, storage migration, and runtime setup.

use crate::app::{config::AppConfig, logging::init_logging};
use anyhow::Result;
use tracing::{debug, error};

/// Initialize the application with proper logging and configuration
pub async fn initialize_app(config: AppConfig) -> Result<()> {
    // Initialize logging first
    init_logging(&config);

    // Perform automatic migration from local to global storage if needed
    if let Err(e) = check_and_migrate_storage().await {
        error!("Storage migration failed: {}", e);
        // Continue anyway - migration is best effort
    }

    Ok(())
}

/// Check for local storage and migrate to global if found
pub async fn check_and_migrate_storage() -> Result<()> {
    use crate::storage::GlobalStorage;
    use crate::unified_session::{migration::auto_migrate, SessionManager};
    use std::env;

    // Get current directory as project path
    let project_path = env::current_dir()?;

    // Create session manager and run auto-migration
    let storage = GlobalStorage::new()?;
    let manager = SessionManager::new(storage).await?;

    if let Some(report) = auto_migrate(manager, project_path).await? {
        debug!("Migration completed: {}", report.summary());
    }

    Ok(())
}
