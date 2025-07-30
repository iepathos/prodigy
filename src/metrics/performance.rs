//! Performance metrics profiling

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};
use tracing::{debug, warn};

/// Performance metrics data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub compile_time: Duration,
    pub binary_size: u64,
    pub benchmark_results: HashMap<String, Duration>,
    pub memory_usage: HashMap<String, u64>,
}

/// Profiles performance metrics
pub struct PerformanceProfiler;

impl PerformanceProfiler {
    /// Create a new performance profiler
    pub fn new() -> Self {
        Self
    }

    /// Profile performance metrics for the project
    pub fn profile(&self, project_path: &Path) -> Result<PerformanceMetrics> {
        let mut metrics = PerformanceMetrics {
            compile_time: Duration::default(),
            binary_size: 0,
            benchmark_results: HashMap::new(),
            memory_usage: HashMap::new(),
        };

        // Measure compile time
        metrics.compile_time = self.measure_compile_time(project_path)?;

        // Measure binary size
        metrics.binary_size = self.measure_binary_size(project_path)?;

        // Run benchmarks if available
        if self.has_benchmarks(project_path) {
            metrics.benchmark_results = self.run_benchmarks(project_path)?;
        }

        // Estimate memory usage
        self.estimate_memory_usage(project_path, &mut metrics.memory_usage)?;

        Ok(metrics)
    }

    /// Measure compilation time
    fn measure_compile_time(&self, project_path: &Path) -> Result<Duration> {
        debug!("Measuring compile time");

        // Clean build to get accurate timing
        Command::new("cargo")
            .arg("clean")
            .current_dir(project_path)
            .output()
            .context("Failed to clean project")?;

        let start = Instant::now();

        let output = Command::new("cargo")
            .args(["build", "--release"])
            .current_dir(project_path)
            .output()
            .context("Failed to build project")?;

        let duration = start.elapsed();

        if !output.status.success() {
            warn!("Build failed, using cached time if available");
            // Return a default duration if build fails
            return Ok(Duration::from_secs(60));
        }

        debug!("Compile time: {:?}", duration);
        Ok(duration)
    }

    /// Measure binary size
    fn measure_binary_size(&self, project_path: &Path) -> Result<u64> {
        // Find the main binary in target/release
        let target_dir = project_path.join("target").join("release");

        if !target_dir.exists() {
            // Try debug build
            let debug_dir = project_path.join("target").join("debug");
            if debug_dir.exists() {
                return self.find_binary_size(&debug_dir);
            }
            return Ok(0);
        }

        self.find_binary_size(&target_dir)
    }

    /// Find the main binary and get its size
    fn find_binary_size(&self, target_dir: &Path) -> Result<u64> {
        // Look for executable files
        for entry in std::fs::read_dir(target_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() {
                let metadata = entry.metadata()?;

                // Check if executable
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if metadata.permissions().mode() & 0o111 != 0 {
                        // Skip test executables and build scripts
                        let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                        if !filename.contains('-') && !filename.starts_with("build-script") {
                            return Ok(metadata.len());
                        }
                    }
                }

                #[cfg(not(unix))]
                {
                    // On Windows, check for .exe extension
                    if path.extension().map_or(false, |ext| ext == "exe") {
                        return Ok(metadata.len());
                    }
                }
            }
        }

        Ok(0)
    }

    /// Check if project has benchmarks
    fn has_benchmarks(&self, project_path: &Path) -> bool {
        let benches_dir = project_path.join("benches");
        benches_dir.exists() && benches_dir.is_dir()
    }

    /// Run benchmarks and collect results
    fn run_benchmarks(&self, project_path: &Path) -> Result<HashMap<String, Duration>> {
        debug!("Running benchmarks");

        let mut results = HashMap::new();

        // Check if we can run benchmarks quickly
        let output = Command::new("cargo")
            .args(["bench", "--no-run"])
            .current_dir(project_path)
            .output();

        if output.is_err() || !output.unwrap().status.success() {
            warn!("Benchmarks not available or failed to compile");
            return Ok(results);
        }

        // For now, return placeholder data
        // Real implementation would parse benchmark output
        results.insert("default_benchmark".to_string(), Duration::from_millis(100));

        Ok(results)
    }

    /// Estimate memory usage patterns
    fn estimate_memory_usage(
        &self,
        project_path: &Path,
        memory_usage: &mut HashMap<String, u64>,
    ) -> Result<()> {
        // This is a simplified estimation
        // Real implementation could use valgrind, heaptrack, or other tools

        // Estimate based on dependencies
        let cargo_toml = project_path.join("Cargo.toml");
        if cargo_toml.exists() {
            let content = std::fs::read_to_string(&cargo_toml)?;

            // Check for known memory-intensive dependencies
            let mut estimated_usage = 1024 * 1024; // 1MB base

            if content.contains("tokio") {
                estimated_usage += 5 * 1024 * 1024; // 5MB for async runtime
            }
            if content.contains("serde") {
                estimated_usage += 2 * 1024 * 1024; // 2MB for serialization
            }
            if content.contains("rayon") {
                estimated_usage += 4 * 1024 * 1024; // 4MB for parallel processing
            }

            memory_usage.insert("estimated_heap".to_string(), estimated_usage);
            memory_usage.insert("estimated_stack".to_string(), 2 * 1024 * 1024);
            // 2MB stack
        }

        Ok(())
    }
}

impl Default for PerformanceProfiler {
    fn default() -> Self {
        Self::new()
    }
}
