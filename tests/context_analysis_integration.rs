use mmm::context::{ContextAnalyzer, ProjectAnalyzer};
use std::fs;
use tempfile::TempDir;

#[tokio::test]
async fn test_context_analysis_integration() {
    // Create test project structure
    let temp_dir = TempDir::new().unwrap();
    let src_dir = temp_dir.path().join("src");
    fs::create_dir(&src_dir).unwrap();

    // Create test files
    fs::write(
        src_dir.join("main.rs"),
        r#"
        fn main() {
            println!("Hello, world!");
        }
        
        fn untested_function() -> Result<(), String> {
            Err("Not implemented".to_string())
        }
    "#,
    )
    .unwrap();

    // Run analysis
    let analyzer = ProjectAnalyzer::new();
    let result = analyzer.analyze(temp_dir.path()).await.unwrap();

    // Verify analysis results
    assert!(result.metadata.files_analyzed > 0);
    assert!(result.conventions.code_patterns.len() > 0);
}
