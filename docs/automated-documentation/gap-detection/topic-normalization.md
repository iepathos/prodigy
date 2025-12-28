## Topic Normalization

Gap detection uses normalization logic to accurately match feature categories against documented topics (.claude/commands/prodigy-detect-documentation-gaps.md:42-50):

### Normalization Steps

1. Convert to lowercase
2. Remove punctuation and special characters
3. Trim whitespace
4. Extract key terms from compound names

### Examples

```
"MapReduce Workflows"     → ["mapreduce", "workflows"]
"agent_merge"             → "agent-merge"
"command-types"           → "command-types"
"Validation Operations"   → ["validation", "operations"]
```

### Matching Logic

For each feature area in features.json, the command checks if any of these match:
1. Chapter ID contains normalized_category
2. normalized_category contains Chapter ID
3. Chapter title contains normalized_category
4. Chapter topics contain normalized_category
5. Section headings in markdown match normalized_category
6. Subsection feature_mapping arrays match

**Test Case** (tests/documentation_gap_detection_test.rs:236-274):
```rust
#[test]
fn test_gap_detection_normalizes_topic_names() -> Result<()> {
    // Features with underscores
    let features = vec![
        MockFeature {
            category: "command_types".to_string(),
            // ...
        },
    ];

    // Chapters with normalized names (hyphens)
    let chapters = vec![
        MockChapter {
            id: "command-types".to_string(),  // Hyphen vs underscore
            // ...
        },
    ];

    let gaps = detect_gaps(&features, &chapters);

    // Result: No gaps because normalization matches them
    assert_eq!(gaps.len(), 0, "Normalization should match underscore and hyphen variations");

    Ok(())
}
```

## Idempotence

Gap detection can be run multiple times safely without creating duplicate chapters or subsections (.claude/commands/prodigy-detect-documentation-gaps.md:867-887).

### Idempotence Guarantees

1. **Checks for existing chapters** before creating
2. **Uses normalized comparison** for matching
3. **Skips already-created chapters**
4. **Can run repeatedly** without side effects

### Test Case

**Source**: tests/documentation_gap_detection_test.rs:236-274

```rust
#[test]
fn test_gap_detection_idempotence() -> Result<()> {
    let features = vec![MockFeature {
        category: "new_feature".to_string(),
        description: "A new feature".to_string(),
        capabilities: vec!["capability1".to_string()],
    }];

    // First run with no chapters
    let gaps_first = detect_gaps(&features, &vec![]);
    assert_eq!(gaps_first.len(), 1, "First run detects 1 gap");

    // Simulate creating the chapter
    let updated_chapters = vec![MockChapter {
        id: "new-feature".to_string(),
        title: "New Feature".to_string(),
        file: "new-feature.md".to_string(),
        topics: vec!["New feature overview".to_string()],
    }];

    // Second run with the new chapter
    let gaps_second = detect_gaps(&features, &updated_chapters);
    assert_eq!(gaps_second.len(), 0, "Second run detects no gaps");

    Ok(())
}
```
