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

    // Create Cargo.toml to make it a valid Rust project
    fs::write(
        temp_dir.path().join("Cargo.toml"),
        r#"[package]
name = "test_project"
version = "0.1.0"
edition = "2021"

[dependencies]
"#,
    )
    .unwrap();


    // Run analysis
    let analyzer = ProjectAnalyzer::new();
    let result = analyzer.analyze(temp_dir.path()).await.unwrap();

    // Verify analysis results
    // We created at least 2 files (main.rs and Cargo.toml)
    assert!(result.metadata.files_analyzed >= 2, "Expected at least 2 files, found {}", result.metadata.files_analyzed);
    // Code patterns might be empty for a minimal project
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
    assert_eq!(
        loaded_analysis.metadata.files_analyzed,
        analysis.metadata.files_analyzed
    );
}
