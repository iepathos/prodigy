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
    assert!(!result.conventions.code_patterns.is_empty());
}

#[tokio::test]
async fn test_context_save_and_load_integration() {
    use mmm::context::{load_analysis, save_analysis};

    // Test saving and loading analysis data
    let temp_dir = TempDir::new().unwrap();
    let project_path = temp_dir.path();
    
    // First create a context dir
    let context_dir = project_path.join(".mmm").join("context");
    fs::create_dir_all(&context_dir).unwrap();

    // Run actual analysis to get valid structure
    let analyzer = ProjectAnalyzer::new();
    let analysis = analyzer.analyze(project_path).await.unwrap();

    // Save analysis
    save_analysis(project_path, &analysis).unwrap();

    // Load analysis
    let loaded = load_analysis(project_path).unwrap();
    assert!(loaded.is_some());

    let loaded_analysis = loaded.unwrap();
    assert_eq!(loaded_analysis.metadata.files_analyzed, analysis.metadata.files_analyzed);
}
