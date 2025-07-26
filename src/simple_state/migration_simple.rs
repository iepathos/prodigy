//! Simplified migration support (without SQLx compile-time requirements)

use anyhow::Result;
use std::path::Path;

/// Migrate from SQLite database to JSON state
pub async fn migrate_from_sqlite(_db_path: &Path) -> Result<()> {
    // Temporarily disabled due to SQLx compile-time requirements
    Err(anyhow::anyhow!(
        "SQLite migration is temporarily disabled. Please use the new JSON state system directly."
    ))
}

/// Check if migration is needed
pub async fn needs_migration() -> bool {
    // Check if old database exists and new state doesn't
    let db_exists = Path::new(".mmm/mmm.db").exists();
    let state_exists = Path::new(".mmm/state.json").exists();

    db_exists && !state_exists
}

/// Backup old database before migration
pub async fn backup_database(db_path: &Path) -> Result<()> {
    let backup_path = db_path.with_extension("db.backup");
    std::fs::copy(db_path, &backup_path)?;
    println!("📦 Database backed up to: {}", backup_path.display());
    Ok(())
}
