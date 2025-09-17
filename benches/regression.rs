//! Performance regression detection system for Prodigy benchmarks
//! Tracks benchmark baselines and detects performance degradation

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// Benchmark result with statistical information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResult {
    pub name: String,
    pub group: String,
    pub mean_ns: f64,
    pub median_ns: f64,
    pub std_dev_ns: f64,
    pub min_ns: f64,
    pub max_ns: f64,
    pub iterations: u64,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Baseline storage for benchmark results
#[derive(Debug, Serialize, Deserialize)]
pub struct BenchmarkBaseline {
    pub version: String,
    pub commit: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub results: HashMap<String, BenchmarkResult>,
}

/// Regression detection configuration
#[derive(Debug, Clone)]
pub struct RegressionConfig {
    /// Percentage threshold for detecting regression (e.g., 5.0 for 5%)
    pub threshold_percent: f64,
    /// Number of standard deviations for statistical significance
    pub std_dev_multiplier: f64,
    /// Minimum number of iterations for valid comparison
    pub min_iterations: u64,
    /// Path to store baseline files
    pub baseline_dir: PathBuf,
}

impl Default for RegressionConfig {
    fn default() -> Self {
        Self {
            threshold_percent: 5.0,
            std_dev_multiplier: 2.0,
            min_iterations: 30,
            baseline_dir: PathBuf::from(".benchmark-baselines"),
        }
    }
}

/// Performance regression detector
pub struct RegressionDetector {
    config: RegressionConfig,
    current_baseline: Option<BenchmarkBaseline>,
}

impl RegressionDetector {
    pub fn new(config: RegressionConfig) -> Self {
        Self {
            config,
            current_baseline: None,
        }
    }

    /// Load baseline from file
    pub fn load_baseline(&mut self, commit: &str) -> anyhow::Result<()> {
        let baseline_path = self.baseline_path(commit);
        if baseline_path.exists() {
            let content = fs::read_to_string(&baseline_path)?;
            self.current_baseline = Some(serde_json::from_str(&content)?);
        }
        Ok(())
    }

    /// Save baseline to file
    pub fn save_baseline(&self, baseline: &BenchmarkBaseline) -> anyhow::Result<()> {
        fs::create_dir_all(&self.config.baseline_dir)?;
        let baseline_path = self.baseline_path(&baseline.commit);
        let content = serde_json::to_string_pretty(baseline)?;
        fs::write(baseline_path, content)?;
        Ok(())
    }

    /// Compare current results against baseline
    pub fn detect_regressions(&self, current: &BenchmarkBaseline) -> Vec<RegressionReport> {
        let Some(baseline) = &self.current_baseline else {
            return vec![];
        };

        let mut reports = Vec::new();

        for (name, current_result) in &current.results {
            if let Some(baseline_result) = baseline.results.get(name) {
                if let Some(report) = self.analyze_regression(baseline_result, current_result) {
                    reports.push(report);
                }
            }
        }

        reports
    }

    /// Analyze individual benchmark for regression
    fn analyze_regression(
        &self,
        baseline: &BenchmarkResult,
        current: &BenchmarkResult,
    ) -> Option<RegressionReport> {
        // Check minimum iterations
        if current.iterations < self.config.min_iterations {
            return None;
        }

        let percent_change = ((current.mean_ns - baseline.mean_ns) / baseline.mean_ns) * 100.0;

        // Calculate statistical significance
        let combined_std_dev = (baseline.std_dev_ns.powi(2) + current.std_dev_ns.powi(2)).sqrt();
        let z_score = (current.mean_ns - baseline.mean_ns).abs() / combined_std_dev;
        let is_significant = z_score > self.config.std_dev_multiplier;

        // Detect regression
        if percent_change > self.config.threshold_percent && is_significant {
            return Some(RegressionReport {
                benchmark_name: current.name.clone(),
                baseline_mean_ns: baseline.mean_ns,
                current_mean_ns: current.mean_ns,
                percent_change,
                is_regression: true,
                is_significant,
                z_score,
            });
        }

        // Detect improvement (negative regression)
        if percent_change < -self.config.threshold_percent && is_significant {
            return Some(RegressionReport {
                benchmark_name: current.name.clone(),
                baseline_mean_ns: baseline.mean_ns,
                current_mean_ns: current.mean_ns,
                percent_change,
                is_regression: false,
                is_significant,
                z_score,
            });
        }

        None
    }

    /// Get path for baseline file
    fn baseline_path(&self, commit: &str) -> PathBuf {
        self.config
            .baseline_dir
            .join(format!("baseline-{}.json", &commit[..8.min(commit.len())]))
    }

    /// Generate trend analysis across multiple commits
    pub fn analyze_trends(&mut self, commits: &[String]) -> TrendAnalysis {
        let mut trends = HashMap::new();

        for commit in commits {
            if let Ok(()) = self.load_baseline(commit) {
                if let Some(baseline) = &self.current_baseline {
                    for (name, result) in &baseline.results {
                        trends
                            .entry(name.clone())
                            .or_insert_with(Vec::new)
                            .push(TrendPoint {
                                commit: commit.clone(),
                                timestamp: result.timestamp,
                                mean_ns: result.mean_ns,
                            });
                    }
                }
            }
        }

        TrendAnalysis { trends }
    }
}

/// Regression detection report
#[derive(Debug, Clone, Serialize)]
pub struct RegressionReport {
    pub benchmark_name: String,
    pub baseline_mean_ns: f64,
    pub current_mean_ns: f64,
    pub percent_change: f64,
    pub is_regression: bool,
    pub is_significant: bool,
    pub z_score: f64,
}

impl RegressionReport {
    /// Format report for display
    pub fn format(&self) -> String {
        let change_type = if self.is_regression {
            "REGRESSION"
        } else {
            "IMPROVEMENT"
        };

        format!(
            "{}: {} - {:.2}% {} (baseline: {:.2}ns, current: {:.2}ns, z-score: {:.2})",
            change_type,
            self.benchmark_name,
            self.percent_change.abs(),
            if self.is_regression {
                "slower"
            } else {
                "faster"
            },
            self.baseline_mean_ns,
            self.current_mean_ns,
            self.z_score
        )
    }
}

/// Trend analysis across multiple commits
#[derive(Debug, Serialize)]
pub struct TrendAnalysis {
    pub trends: HashMap<String, Vec<TrendPoint>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TrendPoint {
    pub commit: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub mean_ns: f64,
}

/// CI integration for automated regression detection
pub struct CIIntegration {
    detector: RegressionDetector,
}

impl Default for CIIntegration {
    fn default() -> Self {
        Self::new()
    }
}

impl CIIntegration {
    pub fn new() -> Self {
        Self {
            detector: RegressionDetector::new(RegressionConfig::default()),
        }
    }

    /// Run regression detection in CI pipeline
    pub fn run_ci_check(&mut self, current_results: BenchmarkBaseline) -> anyhow::Result<bool> {
        // Get base commit (e.g., from environment variable or git)
        let base_commit = std::env::var("BASE_COMMIT").unwrap_or_else(|_| "main".to_string());

        // Load baseline for comparison
        self.detector.load_baseline(&base_commit)?;

        // Detect regressions
        let regressions = self.detector.detect_regressions(&current_results);

        // Report results
        if !regressions.is_empty() {
            println!("\n⚠️  Performance Regressions Detected:\n");
            for report in &regressions {
                if report.is_regression {
                    println!("  ❌ {}", report.format());
                } else {
                    println!("  ✅ {}", report.format());
                }
            }

            // Fail CI if there are actual regressions
            let has_regressions = regressions.iter().any(|r| r.is_regression);
            if has_regressions {
                println!("\n❌ CI Failed: Performance regressions detected");
                return Ok(false);
            }
        } else {
            println!("\n✅ No performance regressions detected");
        }

        // Save current results as potential new baseline
        if std::env::var("UPDATE_BASELINE").is_ok() {
            self.detector.save_baseline(&current_results)?;
            println!("📊 Baseline updated for commit: {}", current_results.commit);
        }

        Ok(true)
    }

    /// Generate performance report
    pub fn generate_report(&mut self, commits: Vec<String>) -> String {
        let trends = self.detector.analyze_trends(&commits);

        let mut report = String::from("# Performance Trend Report\n\n");

        for (benchmark, points) in &trends.trends {
            report.push_str(&format!("## {}\n\n", benchmark));
            report.push_str("| Commit | Timestamp | Mean (ns) | Change |\n");
            report.push_str("|--------|-----------|-----------|--------|\n");

            let mut prev_mean = None;
            for point in points {
                let change = if let Some(prev) = prev_mean {
                    let pct = ((point.mean_ns - prev) / prev) * 100.0;
                    format!("{:+.2}%", pct)
                } else {
                    "baseline".to_string()
                };

                report.push_str(&format!(
                    "| {} | {} | {:.2} | {} |\n",
                    &point.commit[..8.min(point.commit.len())],
                    point.timestamp.format("%Y-%m-%d"),
                    point.mean_ns,
                    change
                ));

                prev_mean = Some(point.mean_ns);
            }
            report.push('\n');
        }

        report
    }
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn test_regression_detection() {
        let config = RegressionConfig {
            threshold_percent: 5.0,
            std_dev_multiplier: 2.0,
            min_iterations: 30,
            baseline_dir: PathBuf::from("/tmp/bench-test"),
        };

        let detector = RegressionDetector::new(config);

        let baseline = BenchmarkResult {
            name: "test_bench".to_string(),
            group: "test".to_string(),
            mean_ns: 1000.0,
            median_ns: 990.0,
            std_dev_ns: 50.0,
            min_ns: 900.0,
            max_ns: 1100.0,
            iterations: 100,
            timestamp: chrono::Utc::now(),
        };

        let current = BenchmarkResult {
            name: "test_bench".to_string(),
            group: "test".to_string(),
            mean_ns: 1100.0, // 10% slower
            median_ns: 1090.0,
            std_dev_ns: 55.0,
            min_ns: 1000.0,
            max_ns: 1200.0,
            iterations: 100,
            timestamp: chrono::Utc::now(),
        };

        let report = detector.analyze_regression(&baseline, &current);
        assert!(report.is_some());

        let report = report.unwrap();
        assert!(report.is_regression);
        assert!(report.percent_change > 5.0);
    }

    #[test]
    fn test_improvement_detection() {
        let config = RegressionConfig::default();
        let detector = RegressionDetector::new(config);

        let baseline = BenchmarkResult {
            name: "test_bench".to_string(),
            group: "test".to_string(),
            mean_ns: 1000.0,
            median_ns: 990.0,
            std_dev_ns: 50.0,
            min_ns: 900.0,
            max_ns: 1100.0,
            iterations: 100,
            timestamp: chrono::Utc::now(),
        };

        let current = BenchmarkResult {
            name: "test_bench".to_string(),
            group: "test".to_string(),
            mean_ns: 900.0, // 10% faster
            median_ns: 890.0,
            std_dev_ns: 45.0,
            min_ns: 800.0,
            max_ns: 1000.0,
            iterations: 100,
            timestamp: chrono::Utc::now(),
        };

        let report = detector.analyze_regression(&baseline, &current);
        assert!(report.is_some());

        let report = report.unwrap();
        assert!(!report.is_regression);
        assert!(report.percent_change < -5.0);
    }
}

// Criterion benchmark setup
use criterion::{criterion_group, criterion_main, Criterion};

fn bench_regression_detection(c: &mut Criterion) {
    c.bench_function("regression_detection", |b| {
        let config = RegressionConfig::default();
        let detector = RegressionDetector::new(config);

        let baseline = BenchmarkResult {
            name: "test_benchmark".to_string(),
            group: "test_group".to_string(),
            mean_ns: 1000.0,
            median_ns: 950.0,
            std_dev_ns: 50.0,
            min_ns: 900.0,
            max_ns: 1100.0,
            iterations: 100,
            timestamp: chrono::Utc::now(),
        };

        let current = BenchmarkResult {
            name: "test_benchmark".to_string(),
            group: "test_group".to_string(),
            mean_ns: 1100.0,
            median_ns: 1050.0,
            std_dev_ns: 55.0,
            min_ns: 990.0,
            max_ns: 1210.0,
            iterations: 100,
            timestamp: chrono::Utc::now(),
        };

        b.iter(|| detector.analyze_regression(&baseline, &current));
    });
}

criterion_group!(benches, bench_regression_detection);
criterion_main!(benches);
