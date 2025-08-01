//! Analysis caching implementation

use crate::context::AnalysisResult;
use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use std::path::{Path, PathBuf};
use tokio::fs;

/// Trait for analysis caching
#[async_trait]
pub trait AnalysisCache: Send + Sync {
    /// Get cached analysis if valid
    async fn get(&self, key: &str) -> Result<Option<AnalysisResult>>;

    /// Store analysis in cache
    async fn put(&self, key: &str, analysis: &AnalysisResult) -> Result<()>;

    /// Check if cache entry is valid
    async fn is_valid(&self, key: &str, max_age: Duration) -> Result<bool>;

    /// Clear all cache entries
    async fn clear(&self) -> Result<()>;

    /// Clear specific cache entry
    async fn remove(&self, key: &str) -> Result<()>;
}

/// File-based analysis cache implementation
pub struct AnalysisCacheImpl {
    cache_dir: PathBuf,
}

impl AnalysisCacheImpl {
    /// Create a new analysis cache
    pub fn new(project_path: &Path) -> Self {
        Self {
            cache_dir: project_path.join(".mmm/cache/analysis"),
        }
    }

    /// Get cache file path for a key
    fn cache_path(&self, key: &str) -> PathBuf {
        // Hash the key to avoid filesystem issues
        let hash = format!("{:x}", md5::compute(key));
        self.cache_dir.join(format!("{hash}.json"))
    }

    /// Ensure cache directory exists
    async fn ensure_cache_dir(&self) -> Result<()> {
        fs::create_dir_all(&self.cache_dir).await?;
        Ok(())
    }
}

#[async_trait]
impl AnalysisCache for AnalysisCacheImpl {
    async fn get(&self, key: &str) -> Result<Option<AnalysisResult>> {
        let cache_path = self.cache_path(key);

        if !cache_path.exists() {
            return Ok(None);
        }

        match fs::read_to_string(&cache_path).await {
            Ok(content) => {
                match serde_json::from_str::<AnalysisResult>(&content) {
                    Ok(analysis) => Ok(Some(analysis)),
                    Err(e) => {
                        // Invalid cache entry, remove it
                        let _ = fs::remove_file(&cache_path).await;
                        eprintln!("Invalid cache entry {key}: {e}");
                        Ok(None)
                    }
                }
            }
            Err(_) => Ok(None),
        }
    }

    async fn put(&self, key: &str, analysis: &AnalysisResult) -> Result<()> {
        self.ensure_cache_dir().await?;

        let cache_path = self.cache_path(key);
        let json = serde_json::to_string_pretty(analysis)?;
        fs::write(&cache_path, json).await?;

        Ok(())
    }

    async fn is_valid(&self, key: &str, max_age: Duration) -> Result<bool> {
        let cache_path = self.cache_path(key);

        if !cache_path.exists() {
            return Ok(false);
        }

        // Check file modification time
        let metadata = fs::metadata(&cache_path).await?;
        if let Ok(modified) = metadata.modified() {
            if let Ok(modified_time) = modified.duration_since(std::time::UNIX_EPOCH) {
                let modified_datetime =
                    DateTime::<Utc>::from(std::time::UNIX_EPOCH + modified_time);
                let age = Utc::now().signed_duration_since(modified_datetime);
                return Ok(age < max_age);
            }
        }

        // If we can't determine age, consider it invalid
        Ok(false)
    }

    async fn clear(&self) -> Result<()> {
        if self.cache_dir.exists() {
            fs::remove_dir_all(&self.cache_dir).await?;
        }
        Ok(())
    }

    async fn remove(&self, key: &str) -> Result<()> {
        let cache_path = self.cache_path(key);
        if cache_path.exists() {
            fs::remove_file(&cache_path).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::AnalysisMetadata;
    use tempfile::TempDir;

    fn create_test_analysis() -> AnalysisResult {
        use crate::context::{
            conventions::{NamingRules, NamingStyle, TestingConventions},
            ArchitectureInfo, DependencyGraph, ProjectConventions, TechnicalDebtMap,
        };
        use std::collections::{BinaryHeap, HashMap};

        AnalysisResult {
            dependency_graph: DependencyGraph {
                nodes: HashMap::new(),
                edges: vec![],
                cycles: vec![],
                layers: vec![],
            },
            architecture: ArchitectureInfo {
                patterns: vec![],
                layers: vec![],
                components: HashMap::new(),
                violations: vec![],
            },
            conventions: ProjectConventions {
                naming_patterns: NamingRules {
                    file_naming: NamingStyle::SnakeCase,
                    function_naming: NamingStyle::SnakeCase,
                    variable_naming: NamingStyle::SnakeCase,
                    type_naming: NamingStyle::PascalCase,
                    constant_naming: NamingStyle::ScreamingSnakeCase,
                },
                code_patterns: HashMap::new(),
                test_patterns: TestingConventions {
                    test_file_pattern: "*_test.rs".to_string(),
                    test_function_prefix: "test_".to_string(),
                    test_module_pattern: "tests".to_string(),
                    assertion_style: "assert!".to_string(),
                },
                project_idioms: vec![],
            },
            technical_debt: TechnicalDebtMap {
                debt_items: vec![],
                hotspots: vec![],
                duplication_map: HashMap::new(),
                priority_queue: BinaryHeap::new(),
            },
            test_coverage: None,
            hybrid_coverage: None,
            metadata: AnalysisMetadata {
                timestamp: Utc::now(),
                duration_ms: 100,
                files_analyzed: 10,
                incremental: false,
                version: "0.1.0".to_string(),
            },
        }
    }

    #[tokio::test]
    async fn test_cache_basic_operations() {
        let temp_dir = TempDir::new().unwrap();
        let cache = AnalysisCacheImpl::new(temp_dir.path());

        let analysis = create_test_analysis();
        let key = "test-key";

        // Test put and get
        cache.put(key, &analysis).await.unwrap();
        let retrieved = cache.get(key).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().metadata.files_analyzed, 10);

        // Test remove
        cache.remove(key).await.unwrap();
        let retrieved = cache.get(key).await.unwrap();
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_cache_validity() {
        let temp_dir = TempDir::new().unwrap();
        let cache = AnalysisCacheImpl::new(temp_dir.path());

        let analysis = create_test_analysis();
        let key = "validity-test";

        cache.put(key, &analysis).await.unwrap();

        // Should be valid for 1 hour
        let valid = cache.is_valid(key, Duration::hours(1)).await.unwrap();
        assert!(valid);

        // Should not be valid for 0 seconds
        let valid = cache.is_valid(key, Duration::seconds(0)).await.unwrap();
        assert!(!valid);
    }

    #[tokio::test]
    async fn test_cache_clear() {
        let temp_dir = TempDir::new().unwrap();
        let cache = AnalysisCacheImpl::new(temp_dir.path());

        let analysis = create_test_analysis();

        // Add multiple entries
        cache.put("key1", &analysis).await.unwrap();
        cache.put("key2", &analysis).await.unwrap();

        // Clear cache
        cache.clear().await.unwrap();

        // Both should be gone
        assert!(cache.get("key1").await.unwrap().is_none());
        assert!(cache.get("key2").await.unwrap().is_none());
    }
}
