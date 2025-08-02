use mmm::context::AnalysisResult;
use mmm::cook::analysis::cache::{AnalysisCache, AnalysisCacheImpl};
use mmm::cook::analysis::runner::{AnalysisRunner, AnalysisRunnerImpl};
use mmm::cook::analysis::AnalysisCoordinator;
use mmm::cook::execution::MockCommandRunner;
use tempfile::TempDir;

#[tokio::test]
async fn test_analysis_coordinator_full_cycle() {
    let temp_dir = TempDir::new().unwrap();
    let cache = AnalysisCacheImpl::new(temp_dir.path());
    let mock_runner = MockCommandRunner::new();
    let runner = AnalysisRunnerImpl::new(mock_runner);

    // Test analyze_project
    let result = runner.run_analysis(temp_dir.path(), false).await;
    assert!(result.is_ok());

    // Test save_analysis
    let analysis = result.unwrap();
    let cache_key = "test_analysis";
    let save_result = cache.put(cache_key, &analysis).await;
    assert!(save_result.is_ok());

    // Test get_cached_analysis
    let cached = cache.get(cache_key).await;
    assert!(cached.is_ok());
    assert!(cached.unwrap().is_some());
}

#[tokio::test]
async fn test_incremental_analysis() {
    let temp_dir = TempDir::new().unwrap();
    let mock_runner = MockCommandRunner::new();
    let runner = AnalysisRunnerImpl::new(mock_runner);

    // Create test files
    std::fs::write(temp_dir.path().join("test.rs"), "fn main() {}").unwrap();

    // Test incremental analysis
    let result = runner.run_analysis(temp_dir.path(), false).await;
    assert!(result.is_ok());
}
