//! Performance optimizations for fast startup and incremental processing

use colored::*;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};
use std::collections::{HashSet, HashMap};
use std::fs;
use serde::{Deserialize, Serialize};
use anyhow::Result;
use tokio::task::JoinHandle;

/// Fast startup manager
pub struct FastStartup {
    start_time: Instant,
    initialization_tasks: Vec<JoinHandle<Result<()>>>,
}

impl FastStartup {
    /// Create a new fast startup instance
    pub fn new() -> Self {
        // Print immediately to show responsiveness
        println!("{} {} starting code improvement...", 
            "🚀".bold(), 
            "MMM".cyan().bold()
        );
        
        Self {
            start_time: Instant::now(),
            initialization_tasks: Vec::new(),
        }
    }
    
    /// Add an initialization task to run in background
    pub fn add_task<F>(&mut self, task: F)
    where
        F: std::future::Future<Output = Result<()>> + Send + 'static,
    {
        let handle = tokio::spawn(task);
        self.initialization_tasks.push(handle);
    }
    
    /// Wait for all initialization tasks
    pub async fn wait_for_init(&mut self) -> Result<()> {
        // Show splash while initializing
        self.show_splash();
        
        // Wait for all tasks
        for task in self.initialization_tasks.drain(..) {
            task.await??;
        }
        
        let elapsed = self.start_time.elapsed();
        if elapsed < Duration::from_millis(100) {
            // Super fast init!
            tracing::debug!("Fast startup completed in {:?}", elapsed);
        }
        
        Ok(())
    }
    
    /// Show splash screen while loading
    fn show_splash(&self) {
        // Quick tips while loading
        let tips = [
            "💡 Tip: Use --focus to target specific improvements",
            "💡 Tip: Add mmm.toml for custom settings",
            "💡 Tip: Use --preview for interactive mode",
            "💡 Tip: Run 'mmm install-hooks' for automatic improvements",
        ];
        
        if let Some(tip) = tips.get(rand::random::<usize>() % tips.len()) {
            println!();
            println!("{}", tip.dimmed());
        }
    }
}

/// Incremental processing for changed files only
pub struct IncrementalProcessor {
    last_run_cache: LastRunCache,
    change_detector: ChangeDetector,
}

#[derive(Debug, Serialize, Deserialize)]
struct LastRunCache {
    timestamp: SystemTime,
    processed_files: HashSet<PathBuf>,
    file_hashes: HashMap<PathBuf, u64>,
}

impl IncrementalProcessor {
    /// Create a new incremental processor
    pub fn new() -> Result<Self> {
        let cache_path = Self::cache_path()?;
        let last_run_cache = if cache_path.exists() {
            let content = fs::read_to_string(&cache_path)?;
            serde_json::from_str(&content).unwrap_or_else(|_| LastRunCache {
                timestamp: SystemTime::now(),
                processed_files: HashSet::new(),
                file_hashes: HashMap::new(),
            })
        } else {
            LastRunCache {
                timestamp: SystemTime::now(),
                processed_files: HashSet::new(),
                file_hashes: HashMap::new(),
            }
        };
        
        Ok(Self {
            last_run_cache,
            change_detector: ChangeDetector::new(),
        })
    }
    
    /// Get files that need processing
    pub async fn get_changed_files(&self, project_root: &Path) -> Result<Vec<PathBuf>> {
        let changed = self.change_detector
            .detect_changes(project_root, &self.last_run_cache)
            .await?;
        
        if changed.is_empty() {
            println!("{} No changes since last run - your code is still great!",
                "✨".green()
            );
        } else {
            println!("{} Found {} changed files to improve",
                "📝".cyan(),
                changed.len().to_string().yellow()
            );
        }
        
        Ok(changed)
    }
    
    /// Update cache after processing
    pub fn update_cache(&mut self, processed_files: Vec<PathBuf>) -> Result<()> {
        for file in processed_files {
            self.last_run_cache.processed_files.insert(file.clone());
            
            // Calculate file hash
            if let Ok(content) = fs::read(&file) {
                let hash = Self::calculate_hash(&content);
                self.last_run_cache.file_hashes.insert(file, hash);
            }
        }
        
        self.last_run_cache.timestamp = SystemTime::now();
        
        // Save cache
        let cache_path = Self::cache_path()?;
        let content = serde_json::to_string_pretty(&self.last_run_cache)?;
        fs::write(cache_path, content)?;
        
        Ok(())
    }
    
    /// Get cache file path
    fn cache_path() -> Result<PathBuf> {
        let cache_dir = dirs::cache_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not find cache directory"))?
            .join("mmm");
        
        fs::create_dir_all(&cache_dir)?;
        Ok(cache_dir.join("last_run.json"))
    }
    
    /// Calculate simple hash for content
    fn calculate_hash(content: &[u8]) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        hasher.finish()
    }
}

/// Change detector for finding modified files
struct ChangeDetector {
    ignore_patterns: Vec<String>,
}

impl ChangeDetector {
    fn new() -> Self {
        Self {
            ignore_patterns: vec![
                ".git".to_string(),
                "target".to_string(),
                "node_modules".to_string(),
                ".mmm/cache".to_string(),
                "*.log".to_string(),
            ],
        }
    }
    
    /// Detect changed files since last run
    async fn detect_changes(
        &self,
        root: &Path,
        cache: &LastRunCache,
    ) -> Result<Vec<PathBuf>> {
        let mut changed_files = Vec::new();
        
        // Use git if available for faster detection
        if self.has_git(root).await {
            changed_files = self.git_changed_files(root, cache.timestamp).await?;
        } else {
            // Fall back to filesystem scanning
            changed_files = self.scan_changed_files(root, cache).await?;
        }
        
        // Filter ignored patterns
        changed_files.retain(|path| {
            !self.ignore_patterns.iter().any(|pattern| {
                path.to_string_lossy().contains(pattern)
            })
        });
        
        Ok(changed_files)
    }
    
    /// Check if directory has git
    async fn has_git(&self, root: &Path) -> bool {
        root.join(".git").exists()
    }
    
    /// Get changed files using git
    async fn git_changed_files(
        &self,
        root: &Path,
        since: SystemTime,
    ) -> Result<Vec<PathBuf>> {
        use tokio::process::Command;
        
        let since_timestamp = since
            .duration_since(SystemTime::UNIX_EPOCH)?
            .as_secs();
        
        let output = Command::new("git")
            .arg("diff")
            .arg("--name-only")
            .arg("--relative")
            .arg(format!("--since={}", since_timestamp))
            .current_dir(root)
            .output()
            .await?;
        
        if !output.status.success() {
            return Ok(Vec::new());
        }
        
        let files = String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(|line| root.join(line))
            .collect();
        
        Ok(files)
    }
    
    /// Scan filesystem for changed files
    async fn scan_changed_files(
        &self,
        root: &Path,
        cache: &LastRunCache,
    ) -> Result<Vec<PathBuf>> {
        use tokio::fs;
        
        let mut changed = Vec::new();
        let mut entries = fs::read_dir(root).await?;
        
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            let metadata = entry.metadata().await?;
            
            if metadata.is_file() {
                // Check if file was modified after last run
                if let Ok(modified) = metadata.modified() {
                    if modified > cache.timestamp {
                        // Also check if content actually changed
                        if let Some(&old_hash) = cache.file_hashes.get(&path) {
                            let content = fs::read(&path).await?;
                            let new_hash = IncrementalProcessor::calculate_hash(&content);
                            
                            if new_hash != old_hash {
                                changed.push(path);
                            }
                        } else {
                            // New file
                            changed.push(path);
                        }
                    }
                }
            }
        }
        
        Ok(changed)
    }
}

/// Lazy loading for deferred initialization
pub struct LazyLoader<T> {
    loader: Option<Box<dyn FnOnce() -> T + Send>>,
    value: Option<T>,
}

impl<T> LazyLoader<T> {
    /// Create a new lazy loader
    pub fn new<F>(loader: F) -> Self
    where
        F: FnOnce() -> T + Send + 'static,
    {
        Self {
            loader: Some(Box::new(loader)),
            value: None,
        }
    }
    
    /// Get the value, loading if necessary
    pub fn get(&mut self) -> &T {
        if self.value.is_none() {
            if let Some(loader) = self.loader.take() {
                self.value = Some(loader());
            }
        }
        
        self.value.as_ref().unwrap()
    }
}

/// Parallel task executor for concurrent operations
pub struct ParallelExecutor {
    max_concurrent: usize,
}

impl ParallelExecutor {
    /// Create a new parallel executor
    pub fn new(max_concurrent: usize) -> Self {
        Self { max_concurrent }
    }
    
    /// Execute tasks in parallel with progress
    pub async fn execute<T, F, Fut>(
        &self,
        tasks: Vec<T>,
        task_fn: F,
        progress_message: &str,
    ) -> Result<Vec<Result<()>>>
    where
        T: Send + 'static,
        F: Fn(T) -> Fut + Clone + Send + 'static,
        Fut: std::future::Future<Output = Result<()>> + Send,
    {
        use futures::stream::{self, StreamExt};
        
        let total = tasks.len();
        let progress = crate::developer_experience::progress::progress_bar(
            total as u64,
            progress_message,
        );
        
        let results: Vec<Result<()>> = stream::iter(tasks)
            .map(|task| {
                let task_fn = task_fn.clone();
                let progress = progress.clone();
                
                async move {
                    let result = task_fn(task).await;
                    progress.inc(1);
                    result
                }
            })
            .buffer_unordered(self.max_concurrent)
            .collect()
            .await;
        
        progress.finish_with_message("Complete");
        Ok(results)
    }
}

use std::collections::HashMap;