//! Integration tests for context optimization features

use anyhow::Result;
use mmm::context::{
    debt::{DebtItem, DebtType},
    size_manager::ContextSizeManager,
};
use std::collections::HashMap;
use std::path::PathBuf;
use tempfile::TempDir;

// Test removed as it requires access to private methods
// The duplication detection is tested through the full map_technical_debt flow

// Test removed as it requires access to private methods
// The aggregation is tested through the full map_technical_debt flow

#[tokio::test]
async fn test_context_size_manager() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let context_dir = temp_dir.path().join(".mmm").join("context");
    std::fs::create_dir_all(&context_dir)?;

    // Create test files with different sizes
    std::fs::write(context_dir.join("small.json"), r#"{"data": "small"}"#)?;

    let large_content = format!(r#"{{"data": "{}"}}"#, "x".repeat(400_000));
    std::fs::write(context_dir.join("large.json"), &large_content)?;

    let huge_content = format!(r#"{{"data": "{}"}}"#, "x".repeat(600_000));
    std::fs::write(context_dir.join("huge.json"), &huge_content)?;

    // Analyze sizes
    let manager = ContextSizeManager::new();
    let metadata = manager.analyze_context_sizes(&context_dir)?;

    // Should detect warnings
    assert!(
        !metadata.warnings.is_empty(),
        "Should generate size warnings"
    );
    assert!(metadata
        .warnings
        .iter()
        .any(|w| w.contains("exceeds maximum size")));
    assert!(metadata
        .warnings
        .iter()
        .any(|w| w.contains("Total context size exceeds target")));

    // Should identify largest file
    assert!(metadata.largest_file.is_some());
    assert_eq!(metadata.largest_file.unwrap().filename, "huge.json");

    Ok(())
}

// Test removed - hybrid coverage functionality has been removed
/*
#[tokio::test]
async fn test_hybrid_coverage_prioritization() -> Result<()> {
    let analyzer = BasicHybridCoverageAnalyzer::new();

    // Create test coverage map with gaps
    let mut file_coverage = HashMap::new();
    file_coverage.insert(
        PathBuf::from("src/critical.rs"),
        FileCoverage {
            path: PathBuf::from("src/critical.rs"),
            coverage_percentage: 20.0,
            tested_lines: 20,
            total_lines: 100,
            tested_functions: 2,
            total_functions: 10,
            has_tests: true,
        },
    );
    file_coverage.insert(
        PathBuf::from("src/stable.rs"),
        FileCoverage {
            path: PathBuf::from("src/stable.rs"),
            coverage_percentage: 80.0,
            tested_lines: 80,
            total_lines: 100,
            tested_functions: 8,
            total_functions: 10,
            has_tests: true,
        },
    );

    let coverage_map = TestCoverageMap {
        overall_coverage: 0.5,
        file_coverage,
        untested_functions: vec![],
        critical_paths: vec![],
    };

    // Create gaps separately for the test
    let _critical_gaps = [
        CoverageGap {
            file: PathBuf::from("src/critical.rs"),
            functions: vec!["process_payment".to_string()],
            coverage_percentage: 20.0,
            risk: "High".to_string(),
        },
        CoverageGap {
            file: PathBuf::from("src/stable.rs"),
            functions: vec!["format_output".to_string()],
            coverage_percentage: 80.0,
            risk: "Low".to_string(),
        },
    ];

    // Empty metrics for simplicity
    let metrics_history = vec![];

    // We need to create a modified coverage map with the gaps
    // Since the analyzer extracts gaps from coverage map internally
    // We'll adjust our test to work with that
    let report = analyzer
        .analyze_hybrid_coverage(&PathBuf::from("."), &coverage_map, &metrics_history)
        .await?;

    // Should prioritize critical.rs
    assert!(!report.priority_gaps.is_empty());
    assert_eq!(
        report.priority_gaps[0].gap.file,
        PathBuf::from("src/critical.rs"),
        "Should prioritize low coverage file"
    );

    // Should have recommendations
    let recommendations = report.get_recommendations();
    assert!(!recommendations.is_empty());
    assert!(recommendations[0].contains("critical.rs"));

    Ok(())
}
*/

#[test]
fn test_size_optimization_for_analysis_result() {
    use mmm::context::size_manager::OptimizableForSize;

    // Create a large analysis result
    let mut analysis = create_test_analysis_result();

    // Add many debt items
    for i in 0..1000 {
        analysis.technical_debt.debt_items.push(DebtItem {
            id: format!("item_{i}"),
            title: format!("Debt item {i}"),
            description: "Test".to_string(),
            location: PathBuf::from("test.rs"),
            line_number: Some(i),
            debt_type: DebtType::Todo,
            impact: 5,
            effort: 3,
            tags: vec![],
        });
    }

    let original_count = analysis.technical_debt.debt_items.len();

    // Optimize with 50% reduction
    let optimized = analysis.optimize_for_size(0.5).unwrap();

    // Should have reduced items
    assert!(
        optimized.technical_debt.debt_items.len() < original_count,
        "Should reduce debt items"
    );
    assert!(
        optimized.technical_debt.debt_items.len() >= 100,
        "Should keep at least minimum items"
    );
}

/// Helper to create a test AnalysisResult
fn create_test_analysis_result() -> mmm::context::AnalysisResult {
    use mmm::context::{
        conventions::{NamingRules, NamingStyle, ProjectConventions, TestingConventions},
        debt::TechnicalDebtMap,
        dependencies::DependencyGraph,
        AnalysisMetadata, AnalysisResult, ArchitectureInfo,
    };

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
            priority_queue: std::collections::BinaryHeap::new(),
        },
        test_coverage: None,
        metadata: AnalysisMetadata {
            timestamp: chrono::Utc::now(),
            duration_ms: 1000,
            files_analyzed: 100,
            incremental: false,
            version: "0.1.0".to_string(),
        },
    }
}
