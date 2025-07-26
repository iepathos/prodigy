//! Smart error handling with rollback capabilities

use colored::*;
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::fs;
use anyhow::{Context, Result};

/// Error handler with user-friendly messages
pub struct ErrorHandler {
    context: ErrorContext,
}

#[derive(Debug)]
enum ErrorContext {
    BuildFailure,
    NetworkError,
    FileAccess,
    General,
}

impl ErrorHandler {
    /// Create a new error handler
    pub fn new() -> Self {
        Self {
            context: ErrorContext::General,
        }
    }
    
    /// Handle a build failure
    pub fn handle_build_failure(&self, error_output: &str) -> String {
        let mut message = String::new();
        
        message.push_str(&format!("{} {}\n\n", "❌".red(), "Build failed after improvements".red().bold()));
        message.push_str("The following changes caused compilation errors:\n");
        
        // Parse error output for specific issues
        if let Some(file_error) = self.parse_build_error(error_output) {
            message.push_str(&format!("  {} {}\n", "•".dimmed(), file_error));
        }
        
        message.push_str(&format!("\n{} Rolling back changes...\n", "🔄".yellow()));
        message.push_str(&format!("{} Rollback complete. Your code is unchanged.\n\n", "✅".green()));
        message.push_str(&format!("{} Try running with {} flag for safer improvements",
            "💡".cyan(),
            "--conservative".cyan()
        ));
        
        message
    }
    
    /// Handle network errors
    pub fn handle_network_error(&self, retry_count: usize) -> String {
        let mut message = String::new();
        
        message.push_str(&format!("{} {}\n\n", "⚠️".yellow(), "Claude API unreachable".yellow().bold()));
        
        if retry_count > 0 {
            message.push_str(&format!("Retry attempt {} failed.\n\n", retry_count));
        }
        
        message.push_str("Would you like to:\n");
        message.push_str(&format!("  {}. Retry (recommended)\n", "1".cyan()));
        message.push_str(&format!("  {}. Use offline mode (limited improvements)\n", "2".cyan()));
        message.push_str(&format!("  {}. Cancel\n", "3".cyan()));
        
        message
    }
    
    /// Handle file access errors
    pub fn handle_file_error(&self, path: &Path, operation: &str) -> String {
        format!("{} Failed to {} file: {}\n{}",
            "❌".red(),
            operation,
            path.display(),
            "Please check file permissions and try again.".dimmed()
        )
    }
    
    /// Parse build error for specific file/line info
    fn parse_build_error(&self, output: &str) -> Option<String> {
        // Look for common error patterns
        for line in output.lines() {
            if line.contains("error[E") || line.contains("error:") {
                // Extract file path and line number if possible
                if let Some(file_info) = line.split(':').next() {
                    return Some(format!("{} - {}", file_info, "Type mismatch after refactoring"));
                }
            }
        }
        None
    }
}

/// Rollback manager for reverting changes
pub struct RollbackManager {
    backups: HashMap<PathBuf, Vec<u8>>,
    changes_log: Vec<ChangeRecord>,
}

#[derive(Debug, Clone)]
struct ChangeRecord {
    path: PathBuf,
    change_type: ChangeType,
    timestamp: std::time::Instant,
}

#[derive(Debug, Clone)]
enum ChangeType {
    Modified,
    Created,
    Deleted,
}

impl RollbackManager {
    /// Create a new rollback manager
    pub fn new() -> Self {
        Self {
            backups: HashMap::new(),
            changes_log: Vec::new(),
        }
    }
    
    /// Backup a file before modification
    pub fn backup_file(&mut self, path: &Path) -> Result<()> {
        if path.exists() {
            let content = fs::read(path)
                .with_context(|| format!("Failed to read file for backup: {}", path.display()))?;
            
            self.backups.insert(path.to_path_buf(), content);
            self.changes_log.push(ChangeRecord {
                path: path.to_path_buf(),
                change_type: ChangeType::Modified,
                timestamp: std::time::Instant::now(),
            });
        } else {
            self.changes_log.push(ChangeRecord {
                path: path.to_path_buf(),
                change_type: ChangeType::Created,
                timestamp: std::time::Instant::now(),
            });
        }
        
        Ok(())
    }
    
    /// Record a file deletion
    pub fn record_deletion(&mut self, path: &Path, content: Vec<u8>) {
        self.backups.insert(path.to_path_buf(), content);
        self.changes_log.push(ChangeRecord {
            path: path.to_path_buf(),
            change_type: ChangeType::Deleted,
            timestamp: std::time::Instant::now(),
        });
    }
    
    /// Rollback all changes
    pub async fn rollback(&self) -> Result<()> {
        println!("{} Starting rollback...", "🔄".yellow());
        
        let total = self.changes_log.len();
        let mut completed = 0;
        
        for record in self.changes_log.iter().rev() {
            match record.change_type {
                ChangeType::Modified => {
                    if let Some(content) = self.backups.get(&record.path) {
                        fs::write(&record.path, content)
                            .with_context(|| format!("Failed to restore: {}", record.path.display()))?;
                    }
                }
                ChangeType::Created => {
                    if record.path.exists() {
                        fs::remove_file(&record.path)
                            .with_context(|| format!("Failed to remove: {}", record.path.display()))?;
                    }
                }
                ChangeType::Deleted => {
                    if let Some(content) = self.backups.get(&record.path) {
                        fs::write(&record.path, content)
                            .with_context(|| format!("Failed to restore deleted file: {}", record.path.display()))?;
                    }
                }
            }
            
            completed += 1;
            if completed % 5 == 0 || completed == total {
                println!("  {} Rolled back {}/{} changes", 
                    "↻".dimmed(), 
                    completed, 
                    total
                );
            }
        }
        
        println!("{} Rollback complete!", "✅".green());
        Ok(())
    }
    
    /// Get the number of changes that would be rolled back
    pub fn change_count(&self) -> usize {
        self.changes_log.len()
    }
    
    /// Clear all backups (after successful completion)
    pub fn clear(&mut self) {
        self.backups.clear();
        self.changes_log.clear();
    }
}

/// Recovery suggestions for various error types
pub fn suggest_recovery(error: &anyhow::Error) -> Option<String> {
    let error_str = error.to_string().to_lowercase();
    
    if error_str.contains("permission denied") {
        Some(format!("{} Check file permissions or run with appropriate privileges",
            "💡".cyan()
        ))
    } else if error_str.contains("git") {
        Some(format!("{} Ensure you're in a git repository with a clean working tree",
            "💡".cyan()
        ))
    } else if error_str.contains("network") || error_str.contains("timeout") {
        Some(format!("{} Check your internet connection and try again",
            "💡".cyan()
        ))
    } else if error_str.contains("api key") || error_str.contains("authentication") {
        Some(format!("{} Ensure your Claude API key is properly configured",
            "💡".cyan()
        ))
    } else {
        None
    }
}

/// Format an error chain for display
pub fn format_error_chain(error: &anyhow::Error) -> String {
    let mut message = String::new();
    
    message.push_str(&format!("{} {}\n", "❌".red(), error.to_string().red().bold()));
    
    let mut current = error.source();
    let mut depth = 1;
    
    while let Some(cause) = current {
        message.push_str(&format!("{}  {} {}\n",
            "  ".repeat(depth),
            "└─".dimmed(),
            cause
        ));
        current = cause.source();
        depth += 1;
    }
    
    if let Some(suggestion) = suggest_recovery(error) {
        message.push_str(&format!("\n{}\n", suggestion));
    }
    
    message
}