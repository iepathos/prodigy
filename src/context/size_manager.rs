//! Context size management for controlling analysis output sizes

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Configuration for context size limits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextSizeConfig {
    /// Maximum size for any single context file in bytes
    pub max_file_size: usize,
    /// Warning threshold as percentage of max size (0.0-1.0)
    pub warning_threshold: f64,
    /// Target total context directory size
    pub target_total_size: usize,
    /// Enable size warnings
    pub enable_warnings: bool,
}

impl Default for ContextSizeConfig {
    fn default() -> Self {
        Self {
            max_file_size: 500_000,       // 500KB per file
            warning_threshold: 0.8,       // Warn at 80% of max
            target_total_size: 1_000_000, // 1MB total
            enable_warnings: true,
        }
    }
}

/// Metadata about context sizes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextSizeMetadata {
    pub file_sizes: Vec<FileSizeInfo>,
    pub total_size: usize,
    pub largest_file: Option<FileSizeInfo>,
    pub size_reduction_applied: bool,
    pub warnings: Vec<String>,
}

/// Information about a single file's size
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSizeInfo {
    pub filename: String,
    pub raw_size: usize,
    pub compressed_size: Option<usize>,
    pub reduction_percentage: Option<f64>,
}

use super::test_coverage::{Criticality, TestCoverageMap};

/// Optimize test coverage data to reduce size while preserving critical information
pub fn optimize_test_coverage(coverage: &TestCoverageMap) -> TestCoverageMap {
    let mut optimized = coverage.clone();

    // Filter file coverage to only include files with < 50% coverage or no tests
    // Note: coverage_percentage in TestCoverageMap is stored as a decimal (0.0-1.0)
    optimized
        .file_coverage
        .retain(|_, file_cov| file_cov.coverage_percentage < 0.5 || !file_cov.has_tests);

    // Prioritize untested functions by criticality
    optimized
        .untested_functions
        .sort_by_key(|f| match f.criticality {
            Criticality::High => 0,
            Criticality::Medium => 1,
            Criticality::Low => 2,
        });

    // Limit untested functions:
    // - Keep all High criticality functions
    // - Keep up to 30 Medium criticality functions
    // - Keep up to 10 Low criticality functions
    let mut high_count = 0;
    let mut medium_count = 0;
    let mut low_count = 0;

    optimized
        .untested_functions
        .retain(|f| match f.criticality {
            Criticality::High => {
                high_count += 1;
                true
            }
            Criticality::Medium => {
                medium_count += 1;
                medium_count <= 30
            }
            Criticality::Low => {
                low_count += 1;
                low_count <= 10
            }
        });

    optimized
}

/// Manager for monitoring and controlling context file sizes
pub struct ContextSizeManager {
    config: ContextSizeConfig,
}

impl Default for ContextSizeManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ContextSizeManager {
    /// Create a new size manager with default config
    pub fn new() -> Self {
        Self {
            config: ContextSizeConfig::default(),
        }
    }

    /// Create a size manager with custom config
    pub fn with_config(config: ContextSizeConfig) -> Self {
        Self { config }
    }

    /// Check if a value would exceed size limits when serialized
    pub fn check_size<T: Serialize>(&self, value: &T, name: &str) -> Result<SizeCheckResult> {
        let serialized = serde_json::to_string(value)
            .with_context(|| format!("Failed to serialize {name} for size check"))?;
        let size = serialized.len();

        let mut result = SizeCheckResult {
            size,
            exceeds_limit: size > self.config.max_file_size,
            warning_threshold_reached: size as f64
                > self.config.max_file_size as f64 * self.config.warning_threshold,
            reduction_needed: false,
            suggested_reduction: 0.0,
        };

        if result.exceeds_limit {
            result.reduction_needed = true;
            result.suggested_reduction = 1.0 - (self.config.max_file_size as f64 / size as f64);
        }

        Ok(result)
    }

    /// Optimize a serializable value to fit within size limits
    pub fn optimize_for_size<T>(&self, value: T, name: &str) -> Result<OptimizedValue<T>>
    where
        T: Serialize + OptimizableForSize,
    {
        let initial_check = self.check_size(&value, name)?;

        if !initial_check.exceeds_limit && !initial_check.warning_threshold_reached {
            return Ok(OptimizedValue {
                value,
                original_size: initial_check.size,
                optimized_size: initial_check.size,
                optimization_applied: false,
            });
        }

        // Apply optimization
        let optimized = value.optimize_for_size(initial_check.suggested_reduction)?;
        let final_check = self.check_size(&optimized, name)?;

        Ok(OptimizedValue {
            value: optimized,
            original_size: initial_check.size,
            optimized_size: final_check.size,
            optimization_applied: true,
        })
    }

    /// Analyze context directory sizes
    pub fn analyze_context_sizes(&self, context_dir: &Path) -> Result<ContextSizeMetadata> {
        let mut file_sizes = Vec::new();
        let mut total_size = 0;
        let mut warnings = Vec::new();

        // Check each JSON file in the context directory
        for entry in std::fs::read_dir(context_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                let metadata = entry.metadata()?;
                let size = metadata.len() as usize;
                total_size += size;

                let filename = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string();

                // Check individual file size
                if size > self.config.max_file_size {
                    warnings.push(format!(
                        "{} exceeds maximum size: {} bytes (limit: {} bytes)",
                        filename, size, self.config.max_file_size
                    ));
                } else if size as f64
                    > self.config.max_file_size as f64 * self.config.warning_threshold
                {
                    warnings.push(format!(
                        "{} approaching size limit: {} bytes ({:.0}% of limit)",
                        filename,
                        size,
                        (size as f64 / self.config.max_file_size as f64) * 100.0
                    ));
                }

                file_sizes.push(FileSizeInfo {
                    filename,
                    raw_size: size,
                    compressed_size: None,
                    reduction_percentage: None,
                });
            }
        }

        // Check total size
        if total_size > self.config.target_total_size {
            warnings.push(format!(
                "Total context size exceeds target: {} bytes (target: {} bytes)",
                total_size, self.config.target_total_size
            ));
        }

        // Find largest file
        let largest_file = file_sizes.iter().max_by_key(|f| f.raw_size).cloned();

        Ok(ContextSizeMetadata {
            file_sizes,
            total_size,
            largest_file,
            size_reduction_applied: false,
            warnings,
        })
    }

    /// Print size warnings if enabled
    pub fn print_warnings(&self, metadata: &ContextSizeMetadata) {
        if !self.config.enable_warnings || metadata.warnings.is_empty() {
            return;
        }

        eprintln!("\n⚠️  Context Size Warnings:");
        for warning in &metadata.warnings {
            eprintln!("  - {warning}");
        }
        eprintln!();
    }
}

/// Result of a size check
#[derive(Debug)]
pub struct SizeCheckResult {
    pub size: usize,
    pub exceeds_limit: bool,
    pub warning_threshold_reached: bool,
    pub reduction_needed: bool,
    pub suggested_reduction: f64,
}

/// A value that has been optimized for size
#[derive(Debug)]
pub struct OptimizedValue<T> {
    pub value: T,
    pub original_size: usize,
    pub optimized_size: usize,
    pub optimization_applied: bool,
}

/// Trait for types that can be optimized to reduce size
pub trait OptimizableForSize: Sized {
    /// Optimize the value to reduce its serialized size
    fn optimize_for_size(self, reduction_factor: f64) -> Result<Self>;
}

// Implement OptimizableForSize for AnalysisResult
impl OptimizableForSize for super::AnalysisResult {
    fn optimize_for_size(mut self, reduction_factor: f64) -> Result<Self> {
        // Reduce technical debt items
        if reduction_factor > 0.0 {
            let target_items =
                ((1.0 - reduction_factor) * self.technical_debt.debt_items.len() as f64) as usize;
            self.technical_debt
                .debt_items
                .truncate(target_items.max(100));

            // Reduce duplication map entries
            let target_dups = ((1.0 - reduction_factor)
                * self.technical_debt.duplication_map.len() as f64)
                as usize;
            let mut dup_entries: Vec<_> = self.technical_debt.duplication_map.drain().collect();
            dup_entries.truncate(target_dups.max(50));
            self.technical_debt.duplication_map = dup_entries.into_iter().collect();

            // Clear priority queue and rebuild with reduced items
            self.technical_debt.priority_queue.clear();
            for item in &self.technical_debt.debt_items {
                self.technical_debt.priority_queue.push(item.clone());
            }
        }

        Ok(self)
    }
}

// Implement OptimizableForSize for TechnicalDebtMap
impl OptimizableForSize for super::TechnicalDebtMap {
    fn optimize_for_size(mut self, reduction_factor: f64) -> Result<Self> {
        if reduction_factor > 0.0 {
            // Reduce debt items
            let target_items = ((1.0 - reduction_factor) * self.debt_items.len() as f64) as usize;
            self.debt_items.sort_by(|a, b| b.cmp(a)); // Sort by priority
            self.debt_items.truncate(target_items.max(50));

            // Reduce hotspots
            let target_hotspots = ((1.0 - reduction_factor) * self.hotspots.len() as f64) as usize;
            self.hotspots
                .sort_by_key(|h| std::cmp::Reverse(h.complexity));
            self.hotspots.truncate(target_hotspots.max(20));

            // Reduce duplication entries
            let target_dups =
                ((1.0 - reduction_factor) * self.duplication_map.len() as f64) as usize;
            let mut scored_dups: Vec<_> = self
                .duplication_map
                .into_iter()
                .map(|(hash, blocks)| {
                    let score = blocks.len() as f32
                        * (blocks
                            .first()
                            .map(|b| b.end_line - b.start_line)
                            .unwrap_or(0) as f32);
                    (score, hash, blocks)
                })
                .collect();
            scored_dups.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

            self.duplication_map = scored_dups
                .into_iter()
                .take(target_dups.max(30))
                .map(|(_, hash, blocks)| (hash, blocks))
                .collect();

            // Rebuild priority queue
            self.priority_queue.clear();
            for item in &self.debt_items {
                self.priority_queue.push(item.clone());
            }
        }

        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_size_check() {
        let manager = ContextSizeManager::new();

        // Small value should pass
        let small_value = vec![1, 2, 3];
        let result = manager.check_size(&small_value, "test").unwrap();
        assert!(!result.exceeds_limit);
        assert!(!result.warning_threshold_reached);

        // Large value should trigger warning/limit
        let large_value = vec![0u8; 600_000];
        let result = manager.check_size(&large_value, "test").unwrap();
        assert!(result.exceeds_limit);
        assert!(result.reduction_needed);
    }

    #[test]
    fn test_config_defaults() {
        let config = ContextSizeConfig::default();
        assert_eq!(config.max_file_size, 500_000);
        assert_eq!(config.warning_threshold, 0.8);
        assert_eq!(config.target_total_size, 1_000_000);
        assert!(config.enable_warnings);
    }
}
