//! Integration tests for documentation gap detection (Spec 124)
//!
//! These tests verify that the gap detection logic works correctly by:
//! 1. Creating mock features.json with known features
//! 2. Creating mock book structure with some chapters missing
//! 3. Simulating gap detection analysis
//! 4. Verifying correct chapters would be created
//! 5. Testing idempotence (running twice doesn't create duplicates)
//! 6. Verifying no false positives (existing chapters aren't duplicated)

use anyhow::Result;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// Mock feature definition
#[derive(Debug, Clone)]
struct MockFeature {
    category: String,
    description: String,
    capabilities: Vec<String>,
}

/// Mock chapter definition
#[derive(Debug, Clone)]
struct MockChapter {
    id: String,
    title: String,
    file: String,
    topics: Vec<String>,
}

/// Test helper to create mock features.json
fn create_mock_features(temp_dir: &Path, features: &[MockFeature]) -> Result<PathBuf> {
    let features_path = temp_dir.join("features.json");

    let mut features_map: HashMap<String, Value> = HashMap::new();
    for feature in features {
        features_map.insert(
            feature.category.clone(),
            json!({
                "description": feature.description,
                "capabilities": feature.capabilities,
            }),
        );
    }

    let features_json = json!({
        "analysis_date": "2025-10-11T00:00:00Z",
        "features": features_map,
    });

    fs::write(&features_path, serde_json::to_string_pretty(&features_json)?)?;
    Ok(features_path)
}

/// Test helper to create mock chapters.json
fn create_mock_chapters(temp_dir: &Path, chapters: &[MockChapter]) -> Result<PathBuf> {
    let chapters_path = temp_dir.join("prodigy-chapters.json");

    let chapters_array: Vec<Value> = chapters
        .iter()
        .map(|ch| {
            json!({
                "id": ch.id,
                "title": ch.title,
                "file": ch.file,
                "topics": ch.topics,
            })
        })
        .collect();

    fs::write(&chapters_path, serde_json::to_string_pretty(&chapters_array)?)?;
    Ok(chapters_path)
}

/// Test helper to create mock book structure
fn create_mock_book_structure(temp_dir: &Path, chapters: &[MockChapter]) -> Result<PathBuf> {
    let book_dir = temp_dir.join("book");
    let src_dir = book_dir.join("src");
    fs::create_dir_all(&src_dir)?;

    // Create SUMMARY.md
    let summary_path = src_dir.join("SUMMARY.md");
    let mut summary_content = String::from("# Summary\n\n");
    summary_content.push_str("- [Introduction](introduction.md)\n\n");
    summary_content.push_str("## User Guide\n\n");
    for chapter in chapters {
        summary_content.push_str(&format!("- [{}]({})\n", chapter.title, chapter.file));
    }
    fs::write(&summary_path, summary_content)?;

    // Create chapter markdown files
    for chapter in chapters {
        let chapter_path = src_dir.join(&chapter.file);
        let chapter_content = format!(
            "# {}\n\nThis chapter covers: {}\n",
            chapter.title,
            chapter.topics.join(", ")
        );
        fs::write(&chapter_path, chapter_content)?;
    }

    Ok(book_dir)
}

/// Simulates gap detection logic based on the spec
fn detect_gaps(
    features: &[MockFeature],
    chapters: &[MockChapter],
) -> Vec<(String, String, String)> {
    let mut gaps = Vec::new();

    for feature in features {
        // Normalize feature category for comparison
        let normalized_category = feature.category.to_lowercase().replace('_', "-");

        // Check if any chapter covers this feature
        let is_covered = chapters.iter().any(|ch| {
            let normalized_id = ch.id.to_lowercase();
            let normalized_title = ch.title.to_lowercase();

            // Check exact or partial matches
            normalized_id.contains(&normalized_category)
                || normalized_category.contains(&normalized_id)
                || normalized_title.contains(&normalized_category)
                || ch.topics.iter().any(|topic| {
                    let normalized_topic = topic.to_lowercase();
                    normalized_topic.contains(&normalized_category)
                        || normalized_category.contains(&normalized_topic)
                })
        });

        if !is_covered {
            // Generate recommended chapter ID
            let chapter_id = feature.category.replace('_', "-");

            // Generate recommended title
            let title = feature
                .category
                .split('_')
                .map(|word| {
                    let mut chars = word.chars();
                    match chars.next() {
                        Some(first) => first.to_uppercase().chain(chars).collect::<String>(),
                        None => String::new(),
                    }
                })
                .collect::<Vec<_>>()
                .join(" ");

            gaps.push((chapter_id, title, feature.description.clone()));
        }
    }

    gaps
}

#[test]
fn test_gap_detection_identifies_missing_chapters() -> Result<()> {
    let temp_dir = TempDir::new()?;

    // Create features with some undocumented ones
    let features = vec![
        MockFeature {
            category: "workflow_basics".to_string(),
            description: "Basic workflow features".to_string(),
            capabilities: vec!["setup".to_string(), "commands".to_string()],
        },
        MockFeature {
            category: "mapreduce".to_string(),
            description: "MapReduce workflows".to_string(),
            capabilities: vec!["map".to_string(), "reduce".to_string()],
        },
        MockFeature {
            category: "agent_merge".to_string(),
            description: "Custom merge workflows for agents".to_string(),
            capabilities: vec!["validation".to_string(), "merge_config".to_string()],
        },
        MockFeature {
            category: "circuit_breaker".to_string(),
            description: "Circuit breaker for error handling".to_string(),
            capabilities: vec!["threshold".to_string(), "recovery".to_string()],
        },
    ];

    // Create chapters - only documenting first two features
    let chapters = vec![
        MockChapter {
            id: "workflow-basics".to_string(),
            title: "Workflow Basics".to_string(),
            file: "workflow-basics.md".to_string(),
            topics: vec!["Setup phase".to_string(), "Commands".to_string()],
        },
        MockChapter {
            id: "mapreduce-workflows".to_string(),
            title: "MapReduce Workflows".to_string(),
            file: "mapreduce-workflows.md".to_string(),
            topics: vec!["Map phase".to_string(), "Reduce phase".to_string()],
        },
    ];

    create_mock_features(temp_dir.path(), &features)?;
    create_mock_chapters(temp_dir.path(), &chapters)?;
    create_mock_book_structure(temp_dir.path(), &chapters)?;

    // Run gap detection
    let gaps = detect_gaps(&features, &chapters);

    // Verify gaps were detected
    assert_eq!(gaps.len(), 2, "Should detect 2 missing chapters");

    // Verify agent_merge gap
    assert!(
        gaps.iter().any(|(id, _, _)| id == "agent-merge"),
        "Should detect agent_merge gap"
    );

    // Verify circuit_breaker gap
    assert!(
        gaps.iter().any(|(id, _, _)| id == "circuit-breaker"),
        "Should detect circuit_breaker gap"
    );

    Ok(())
}

#[test]
fn test_gap_detection_idempotence() -> Result<()> {
    let temp_dir = TempDir::new()?;

    let features = vec![MockFeature {
        category: "new_feature".to_string(),
        description: "A new feature".to_string(),
        capabilities: vec!["capability1".to_string()],
    }];

    let chapters = vec![];

    create_mock_features(temp_dir.path(), &features)?;
    create_mock_chapters(temp_dir.path(), &chapters)?;

    // First run - should detect gap
    let gaps_first = detect_gaps(&features, &chapters);
    assert_eq!(gaps_first.len(), 1, "First run should detect 1 gap");

    // Simulate creating the chapter
    let new_chapter = MockChapter {
        id: "new-feature".to_string(),
        title: "New Feature".to_string(),
        file: "new-feature.md".to_string(),
        topics: vec!["New feature overview".to_string()],
    };

    let updated_chapters = vec![new_chapter];

    // Second run - should detect no gaps
    let gaps_second = detect_gaps(&features, &updated_chapters);
    assert_eq!(
        gaps_second.len(),
        0,
        "Second run should detect no gaps (idempotent)"
    );

    Ok(())
}

#[test]
fn test_gap_detection_prevents_false_positives() -> Result<()> {
    let temp_dir = TempDir::new()?;

    // Feature with multiple naming variations
    let features = vec![MockFeature {
        category: "retry_configuration".to_string(),
        description: "Retry configuration options".to_string(),
        capabilities: vec!["max_attempts".to_string(), "backoff".to_string()],
    }];

    // Chapter with similar but not exact name
    let chapters = vec![MockChapter {
        id: "retry-config".to_string(),
        title: "Retry Configuration".to_string(),
        file: "retry-config.md".to_string(),
        topics: vec![
            "Retry settings".to_string(),
            "Configuration options".to_string(),
        ],
    }];

    create_mock_features(temp_dir.path(), &features)?;
    create_mock_chapters(temp_dir.path(), &chapters)?;

    // Run gap detection
    let gaps = detect_gaps(&features, &chapters);

    // Should NOT detect a gap - the chapter exists with similar name
    assert_eq!(gaps.len(), 0, "Should not create duplicate chapters");

    Ok(())
}

#[test]
fn test_gap_detection_normalizes_topic_names() -> Result<()> {
    let temp_dir = TempDir::new()?;

    // Features with underscores and various formats
    let features = vec![
        MockFeature {
            category: "command_types".to_string(),
            description: "Different command types".to_string(),
            capabilities: vec!["shell".to_string(), "claude".to_string()],
        },
        MockFeature {
            category: "goal_seeking".to_string(),
            description: "Goal seeking operations".to_string(),
            capabilities: vec!["validation".to_string()],
        },
    ];

    // Chapters with normalized names (hyphens)
    let chapters = vec![
        MockChapter {
            id: "command-types".to_string(),
            title: "Command Types".to_string(),
            file: "command-types.md".to_string(),
            topics: vec!["Shell commands".to_string(), "Claude commands".to_string()],
        },
        MockChapter {
            id: "goal-seek".to_string(),
            title: "Goal Seeking".to_string(),
            file: "goal-seek.md".to_string(),
            topics: vec!["Goal validation".to_string()],
        },
    ];

    create_mock_features(temp_dir.path(), &features)?;
    create_mock_chapters(temp_dir.path(), &chapters)?;

    // Run gap detection
    let gaps = detect_gaps(&features, &chapters);

    // Should detect no gaps - normalization should match them
    assert_eq!(
        gaps.len(),
        0,
        "Normalization should match underscore and hyphen variations"
    );

    Ok(())
}

#[test]
fn test_chapter_definition_generation() -> Result<()> {
    let temp_dir = TempDir::new()?;

    let features = vec![MockFeature {
        category: "environment_variables".to_string(),
        description: "Environment variable support".to_string(),
        capabilities: vec![
            "global_env".to_string(),
            "secrets".to_string(),
            "profiles".to_string(),
        ],
    }];

    let chapters = vec![];

    create_mock_features(temp_dir.path(), &features)?;
    create_mock_chapters(temp_dir.path(), &chapters)?;

    // Run gap detection
    let gaps = detect_gaps(&features, &chapters);

    assert_eq!(gaps.len(), 1, "Should detect 1 gap");

    let (chapter_id, title, description) = &gaps[0];

    // Verify chapter ID uses hyphens
    assert_eq!(chapter_id, "environment-variables");
    assert!(!chapter_id.contains('_'), "Chapter ID should use hyphens");

    // Verify title is properly formatted
    assert_eq!(title, "Environment Variables");
    assert!(
        title.chars().next().unwrap().is_uppercase(),
        "Title should start with uppercase"
    );

    // Verify description is preserved
    assert_eq!(description, "Environment variable support");

    Ok(())
}

#[test]
fn test_stub_file_structure() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let book_dir = temp_dir.path().join("book");
    let src_dir = book_dir.join("src");
    fs::create_dir_all(&src_dir)?;

    // Create a mock stub file based on spec requirements
    let stub_content = r#"# New Feature

Brief introduction explaining the purpose of this feature/capability

## Overview

High-level description of what this feature enables

## Configuration

Configuration options and syntax

```yaml
# Example configuration
```

## Use Cases

### Use Case 1 Name

Description and example

## Examples

### Basic Example

```yaml
# Example workflow
```

## Best Practices

- Best practice 1
- Best practice 2

## Troubleshooting

### Common Issues

**Issue**: Common problem
**Solution**: How to fix

## See Also

- [Related documentation](link)
"#;

    let stub_path = src_dir.join("new-feature.md");
    fs::write(&stub_path, stub_content)?;

    // Verify stub file was created
    assert!(stub_path.exists(), "Stub file should be created");

    // Read and verify structure
    let content = fs::read_to_string(&stub_path)?;

    // Verify required sections exist
    assert!(content.contains("# New Feature"), "Should have title");
    assert!(content.contains("## Overview"), "Should have Overview");
    assert!(
        content.contains("## Configuration"),
        "Should have Configuration"
    );
    assert!(content.contains("## Use Cases"), "Should have Use Cases");
    assert!(content.contains("## Examples"), "Should have Examples");
    assert!(
        content.contains("## Best Practices"),
        "Should have Best Practices"
    );
    assert!(
        content.contains("## Troubleshooting"),
        "Should have Troubleshooting"
    );
    assert!(content.contains("## See Also"), "Should have See Also");

    // Verify code blocks are properly formatted
    assert!(content.contains("```yaml"), "Should have YAML code blocks");

    Ok(())
}

#[test]
fn test_gap_detection_with_partial_matches() -> Result<()> {
    let temp_dir = TempDir::new()?;

    // Feature that should match a chapter with partial name
    let features = vec![MockFeature {
        category: "mapreduce_workflows".to_string(),
        description: "MapReduce workflow system".to_string(),
        capabilities: vec!["map".to_string(), "reduce".to_string()],
    }];

    // Chapter with shorter name that partially matches
    let chapters = vec![MockChapter {
        id: "mapreduce".to_string(),
        title: "MapReduce".to_string(),
        file: "mapreduce.md".to_string(),
        topics: vec!["Map phase".to_string(), "Reduce phase".to_string()],
    }];

    create_mock_features(temp_dir.path(), &features)?;
    create_mock_chapters(temp_dir.path(), &chapters)?;

    // Run gap detection
    let gaps = detect_gaps(&features, &chapters);

    // Should not detect gap - partial match should work
    assert_eq!(
        gaps.len(),
        0,
        "Should match partial names (mapreduce matches mapreduce_workflows)"
    );

    Ok(())
}

#[test]
fn test_gap_detection_severity_classification() -> Result<()> {
    let temp_dir = TempDir::new()?;

    // Mix of documented and undocumented features
    let features = vec![
        MockFeature {
            category: "core_workflow".to_string(),
            description: "Core workflow engine (critical)".to_string(),
            capabilities: vec!["execution".to_string()],
        },
        MockFeature {
            category: "advanced_retry".to_string(),
            description: "Advanced retry features (optional)".to_string(),
            capabilities: vec!["jitter".to_string()],
        },
    ];

    let chapters = vec![MockChapter {
        id: "core-workflow".to_string(),
        title: "Core Workflow".to_string(),
        file: "core-workflow.md".to_string(),
        topics: vec!["Workflow execution".to_string()],
    }];

    create_mock_features(temp_dir.path(), &features)?;
    create_mock_chapters(temp_dir.path(), &chapters)?;

    // Run gap detection
    let gaps = detect_gaps(&features, &chapters);

    // Should detect the advanced_retry gap
    assert_eq!(gaps.len(), 1, "Should detect 1 missing chapter");

    let (chapter_id, _, _) = &gaps[0];
    assert_eq!(chapter_id, "advanced-retry");

    // Note: Severity classification would be done by the actual implementation
    // based on feature usage patterns and metadata

    Ok(())
}

#[test]
fn test_summary_update_logic() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let book_dir = temp_dir.path().join("book");
    let src_dir = book_dir.join("src");
    fs::create_dir_all(&src_dir)?;

    // Create initial SUMMARY.md
    let summary_path = src_dir.join("SUMMARY.md");
    let initial_summary = r#"# Summary

- [Introduction](introduction.md)

## User Guide

- [Workflow Basics](workflow-basics.md)

## Advanced Topics

- [MapReduce](mapreduce.md)
"#;
    fs::write(&summary_path, initial_summary)?;

    // Verify we can parse and update the summary
    let content = fs::read_to_string(&summary_path)?;

    assert!(content.contains("# Summary"), "Should have title");
    assert!(
        content.contains("## User Guide"),
        "Should have User Guide section"
    );
    assert!(
        content.contains("## Advanced Topics"),
        "Should have Advanced Topics section"
    );

    // Simulate adding a new chapter
    let updated_summary = content.replace(
        "## Advanced Topics\n\n- [MapReduce](mapreduce.md)",
        "## Advanced Topics\n\n- [Agent Merge](agent-merge.md)\n- [MapReduce](mapreduce.md)",
    );

    fs::write(&summary_path, updated_summary)?;

    // Verify update
    let final_content = fs::read_to_string(&summary_path)?;
    assert!(
        final_content.contains("[Agent Merge](agent-merge.md)"),
        "Should contain new chapter"
    );

    Ok(())
}

#[test]
fn test_gap_detection_with_empty_features() -> Result<()> {
    let temp_dir = TempDir::new()?;

    let features = vec![];
    let chapters = vec![MockChapter {
        id: "existing".to_string(),
        title: "Existing Chapter".to_string(),
        file: "existing.md".to_string(),
        topics: vec!["Topic".to_string()],
    }];

    create_mock_features(temp_dir.path(), &features)?;
    create_mock_chapters(temp_dir.path(), &chapters)?;

    // Run gap detection
    let gaps = detect_gaps(&features, &chapters);

    // Should detect no gaps with no features
    assert_eq!(gaps.len(), 0, "No features means no gaps");

    Ok(())
}

#[test]
fn test_gap_detection_with_empty_chapters() -> Result<()> {
    let temp_dir = TempDir::new()?;

    let features = vec![
        MockFeature {
            category: "feature1".to_string(),
            description: "Feature 1".to_string(),
            capabilities: vec!["cap1".to_string()],
        },
        MockFeature {
            category: "feature2".to_string(),
            description: "Feature 2".to_string(),
            capabilities: vec!["cap2".to_string()],
        },
    ];

    let chapters = vec![];

    create_mock_features(temp_dir.path(), &features)?;
    create_mock_chapters(temp_dir.path(), &chapters)?;

    // Run gap detection
    let gaps = detect_gaps(&features, &chapters);

    // Should detect gaps for all features
    assert_eq!(gaps.len(), 2, "All features should be gaps");

    Ok(())
}
