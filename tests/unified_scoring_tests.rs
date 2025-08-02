//! Tests for unified scoring system

use mmm::metrics::ImprovementMetrics;
use mmm::scoring::ProjectHealthScore;

#[test]
fn test_unified_scoring_from_metrics() {
    let mut metrics = ImprovementMetrics::default();

    // Set up test data
    metrics.test_coverage = 75.0;
    metrics.lint_warnings = 5;
    metrics.code_duplication = 10.0;
    metrics.doc_coverage = 60.0;
    metrics.type_coverage = 90.0;

    // Calculate unified score
    let health_score = ProjectHealthScore::from_metrics(&metrics);

    // Verify overall score is in expected range
    assert!(health_score.overall > 60.0 && health_score.overall < 80.0);

    // Verify components are correctly calculated
    assert_eq!(health_score.components.test_coverage, Some(75.0));
    assert!(health_score.components.code_quality.is_some());
    assert!(health_score.components.maintainability.is_some());
    assert_eq!(health_score.components.documentation, Some(60.0));
    assert_eq!(health_score.components.type_safety, Some(90.0));
}

#[test]
fn test_unified_scoring_missing_data() {
    let metrics = ImprovementMetrics::default();

    // Calculate unified score with default (empty) metrics
    let health_score = ProjectHealthScore::from_metrics(&metrics);

    // Should handle missing data gracefully
    assert!(health_score.overall >= 0.0);
    assert!(health_score.overall <= 100.0);

    // Some components should be None
    assert_eq!(health_score.components.test_coverage, None);
    assert_eq!(health_score.components.documentation, None);
    assert_eq!(health_score.components.type_safety, None);
}

#[test]
fn test_improvement_suggestions() {
    let mut metrics = ImprovementMetrics::default();
    metrics.test_coverage = 30.0;
    metrics.doc_coverage = 20.0;
    metrics.lint_warnings = 50;

    let health_score = ProjectHealthScore::from_metrics(&metrics);
    let suggestions = health_score.get_improvement_suggestions();

    // Should provide relevant suggestions
    assert!(!suggestions.is_empty());
    assert!(suggestions.iter().any(|s| s.contains("test coverage")));
    assert!(suggestions.iter().any(|s| s.contains("documentation")));
}

#[test]
fn test_metrics_update_health_score() {
    let mut metrics = ImprovementMetrics::default();
    metrics.test_coverage = 80.0;
    metrics.type_coverage = 85.0;
    metrics.doc_coverage = 70.0;

    // Update health score
    metrics.update_health_score();

    // Verify health score was updated
    assert!(metrics.health_score.is_some());
    let health_score = metrics.health_score.as_ref().unwrap();
    assert!(health_score.overall > 70.0);
}

#[test]
fn test_consistent_scoring() {
    // Test that the same metrics always produce the same score
    let mut metrics1 = ImprovementMetrics::default();
    metrics1.test_coverage = 65.0;
    metrics1.lint_warnings = 10;
    metrics1.code_duplication = 5.0;

    let mut metrics2 = ImprovementMetrics::default();
    metrics2.test_coverage = 65.0;
    metrics2.lint_warnings = 10;
    metrics2.code_duplication = 5.0;

    let score1 = ProjectHealthScore::from_metrics(&metrics1);
    let score2 = ProjectHealthScore::from_metrics(&metrics2);

    // Scores should be identical
    assert_eq!(score1.overall, score2.overall);
    assert_eq!(
        score1.components.test_coverage,
        score2.components.test_coverage
    );
    assert_eq!(
        score1.components.code_quality,
        score2.components.code_quality
    );
}
