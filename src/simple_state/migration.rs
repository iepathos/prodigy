//! Migration support from SQLite to JSON state

use anyhow::{Context, Result};
use chrono::Utc;
use sqlx::sqlite::SqlitePool;
use std::path::Path;

use super::{StateManager, LearningManager};
use super::types::{State, SessionRecord, Improvement, SessionMetrics};

/// Migrate from SQLite database to JSON state
pub async fn migrate_from_sqlite(db_path: &Path) -> Result<()> {
    println!("🔄 Starting migration from SQLite to JSON state...");
    
    // Connect to SQLite database
    let pool = SqlitePool::connect(&format!("sqlite:{}", db_path.display()))
        .await
        .context("Failed to connect to SQLite database")?;
    
    // Initialize new state system
    super::init()?;
    let mut state_mgr = StateManager::new()?;
    let mut learning_mgr = LearningManager::load()?;
    
    // Migrate project info
    let project_id = migrate_project_info(&pool, &mut state_mgr).await?;
    println!("✓ Migrated project info (ID: {})", project_id);
    
    // Migrate sessions
    let session_count = migrate_sessions(&pool, &mut state_mgr, &mut learning_mgr).await?;
    println!("✓ Migrated {} sessions", session_count);
    
    // Save final state
    state_mgr.save()?;
    learning_mgr.save()?;
    
    println!("✅ Migration completed successfully!");
    println!("   You can now delete the old database file: {}", db_path.display());
    
    Ok(())
}

/// Migrate project information
async fn migrate_project_info(pool: &SqlitePool, state_mgr: &mut StateManager) -> Result<String> {
    // Try to get project info from database
    let project = sqlx::query!(
        "SELECT id, name, created_at FROM projects LIMIT 1"
    )
    .fetch_optional(pool)
    .await?;
    
    if let Some(proj) = project {
        state_mgr.state_mut().project_id = proj.id;
        Ok(proj.id)
    } else {
        // No project in database, use default
        Ok(state_mgr.state().project_id.clone())
    }
}

/// Migrate session records
async fn migrate_sessions(
    pool: &SqlitePool,
    state_mgr: &mut StateManager,
    learning_mgr: &mut LearningManager,
) -> Result<u32> {
    let mut count = 0;
    
    // Get all sessions
    let sessions = sqlx::query!(
        r#"
        SELECT 
            id as session_id,
            started_at,
            completed_at,
            initial_score,
            final_score,
            status
        FROM improvement_sessions
        ORDER BY started_at
        "#
    )
    .fetch_all(pool)
    .await?;
    
    for session in sessions {
        // Skip incomplete sessions
        if session.status != "completed" {
            continue;
        }
        
        // Get improvements for this session
        let improvements = sqlx::query!(
            r#"
            SELECT 
                improvement_type,
                file_path,
                line_number,
                description,
                0.1 as impact
            FROM improvements
            WHERE session_id = ?
            "#,
            session.session_id
        )
        .fetch_all(pool)
        .await?;
        
        // Convert to our format
        let session_record = SessionRecord {
            session_id: session.session_id,
            started_at: session.started_at.parse().unwrap_or_else(|_| Utc::now()),
            completed_at: session.completed_at
                .and_then(|s| s.parse().ok())
                .unwrap_or_else(Utc::now),
            initial_score: session.initial_score.unwrap_or(0.0) as f32,
            final_score: session.final_score.unwrap_or(0.0) as f32,
            improvements: improvements
                .into_iter()
                .map(|imp| Improvement {
                    improvement_type: imp.improvement_type.unwrap_or_else(|| "unknown".to_string()),
                    file: imp.file_path.unwrap_or_else(|| "unknown".to_string()),
                    line: imp.line_number.map(|n| n as u32),
                    description: imp.description.unwrap_or_else(|| "No description".to_string()),
                    impact: imp.impact as f32,
                })
                .collect(),
            files_changed: Vec::new(), // Not tracked in old system
            metrics: SessionMetrics {
                duration_seconds: 300, // Default 5 minutes
                claude_calls: 1,
                tokens_used: 0,
            },
        };
        
        // Record in new system
        state_mgr.record_session(session_record.clone())?;
        
        // Update learning data
        for improvement in &session_record.improvements {
            learning_mgr.record_improvement(improvement)?;
        }
        
        count += 1;
    }
    
    Ok(count)
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
    std::fs::copy(db_path, &backup_path)
        .context("Failed to backup database")?;
    println!("📦 Database backed up to: {}", backup_path.display());
    Ok(())
}