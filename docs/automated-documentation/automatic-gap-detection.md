## Automatic Gap Detection

Automatic gap detection is a critical component of Prodigy's documentation workflow that identifies undocumented features and automatically creates chapter/subsection definitions with stub markdown files. This ensures comprehensive documentation coverage and prevents features from being implemented without corresponding user guidance.

**Source**: Implemented in `.claude/commands/prodigy-detect-documentation-gaps.md:1-1048` and tested in `tests/documentation_gap_detection_test.rs:1-678`

## Overview

Gap detection runs in the **setup phase** of the book workflow (workflows/book-docs-drift.yml:31-34) and performs several key functions:

1. **Analyzes** features.json (from feature analysis) against existing chapters/subsections
2. **Classifies** gaps by severity (high, medium, low)
3. **Validates** content sufficiency before creating subsections (Step 0)
4. **Syncs** chapters.json with actual file structure (Phase 7.5)
5. **Creates** missing chapter definitions and stub markdown files
6. **Updates** SUMMARY.md with proper hierarchy
7. **Generates** flattened-items.json for the map phase (mandatory)

The gap detection process ensures that:
- Features aren't documented without sufficient codebase material (prevents stub subsections)
- Multi-subsection chapter structures are accurately reflected in chapters.json
- The map phase receives a complete, flat list of all chapters and subsections to process
- Documentation organization matches implementation reality

## Command Usage

**Command**: `/prodigy-detect-documentation-gaps`

**Parameters** (.claude/commands/prodigy-detect-documentation-gaps.md:5-11):
```bash
/prodigy-detect-documentation-gaps \
  --project "Prodigy" \
  --config ".prodigy/book-config.json" \
  --features ".prodigy/book-analysis/features.json" \
  --chapters "workflows/data/prodigy-chapters.json" \
  --book-dir "book"
```

**Workflow Integration** (workflows/book-docs-drift.yml:31-34):
```yaml
setup:
  # Step 1: Analyze features
  - claude: "/prodigy-analyze-features-for-book --project $PROJECT_NAME --config $PROJECT_CONFIG"

  # Step 2: Detect gaps and generate flattened-items.json
  - claude: "/prodigy-detect-documentation-gaps \
      --project $PROJECT_NAME \
      --config $PROJECT_CONFIG \
      --features $FEATURES_PATH \
      --chapters $CHAPTERS_FILE \
      --book-dir $BOOK_DIR"
```

## Gap Severity Classification

Gap detection classifies documentation gaps into three severity levels based on feature importance and documentation completeness (.claude/commands/prodigy-detect-documentation-gaps.md:66-112):

### High Severity (Missing Chapter/Subsection)

**Criteria**:
- Feature area exists in features.json
- NO corresponding chapter OR subsection found
- Major user-facing capability with no guidance

**Example**:
```json
{
  "severity": "high",
  "type": "missing_chapter",
  "feature_category": "agent_merge",
  "feature_description": "Custom merge workflows for map agents",
  "recommended_chapter_id": "agent-merge-workflows",
  "recommended_title": "Agent Merge Workflows"
}
```

**Action**: Create new chapter definition with stub markdown file

### Medium Severity (Incomplete Chapter/Subsection)

**Criteria**:
- Chapter or multi-subsection structure exists for feature area
- But specific sub-capabilities are missing
- Could be addressed by adding subsection or expanding content

**Example**:
- "mapreduce" chapter exists but missing "performance_tuning" subsection

**Action**: Create subsection definition and add to existing multi-subsection chapter

### Low Severity (Minor Gap)

**Criteria**:
- Edge cases or advanced features not documented
- Internal APIs exposed to users
- Less common use cases

**Action**: Log as warning but may not create new content

## Content Sufficiency Validation (Step 0)

**CRITICAL SAFEGUARD**: Before creating any subsection, gap detection validates that sufficient material exists in the codebase to support meaningful documentation.

**Source**: `.claude/commands/prodigy-detect-documentation-gaps.md:166-335`

### Preservation of Single-File Chapters

Gap detection **ALWAYS preserves well-written single-file chapters** (.claude/commands/prodigy-detect-documentation-gaps.md:174-209):

**Preservation Rules**:
- **< 1000 lines AND < 10 H2 sections**: PRESERVE as single-file
- **≥ 1000 lines OR ≥ 10 H2 sections**: Consider subsections for readability

**Why**: The original flat documentation structure works well for moderate-sized chapters. Subsections should only be created when they genuinely improve navigation.

### Content Availability Validation

**Step 0a: Discover Codebase Structure** (.claude/commands/prodigy-detect-documentation-gaps.md:211-222)

Before counting content, the command discovers where code and examples are located using language-agnostic patterns:

```bash
# Discover test locations
TEST_DIRS=$(find . -type d -name "*test*" -o -name "*spec*" | grep -v node_modules | grep -v .git | head -5)

# Discover example/workflow/config locations
EXAMPLE_DIRS=$(find . -type d -name "*example*" -o -name "*workflow*" -o -name "*sample*" -o -name "*config*" | grep -v node_modules | grep -v .git | head -5)

# Discover primary source locations (works for Rust, Python, JS, TS, Go, Java)
SOURCE_DIRS=$(find . -type f \( -name "*.rs" -o -name "*.py" -o -name "*.js" -o -name "*.ts" -o -name "*.go" -o -name "*.java" \) | sed 's|/[^/]*$||' | sort -u | grep -v node_modules | grep -v .git | head -10)
```

**Step 0b: Count Potential Content Sources** (.claude/commands/prodigy-detect-documentation-gaps.md:224-255)

For each proposed subsection, the command counts language-agnostic content sources:

```bash
FEATURE_CATEGORY="<feature-category-name>"

# Type definitions (struct, class, interface, enum, type)
TYPE_COUNT=$(rg "(struct|class|interface|type|enum).*${FEATURE_CATEGORY}" --hidden --iglob '!.git' --iglob '!node_modules' -c | awk '{s+=$1} END {print s}')

# Function/method definitions
FUNCTION_COUNT=$(rg "(fn|function|def|func|public|private).*${FEATURE_CATEGORY}" --hidden --iglob '!.git' --iglob '!node_modules' -c | awk '{s+=$1} END {print s}')

# Test mentions in discovered test directories
TEST_COUNT=0
for test_dir in $TEST_DIRS; do
  count=$(rg "${FEATURE_CATEGORY}" "$test_dir" --hidden -c 2>/dev/null | awk '{s+=$1} END {print s}')
  TEST_COUNT=$((TEST_COUNT + count))
done

# Example/config file mentions in discovered example directories
EXAMPLE_COUNT=0
for example_dir in $EXAMPLE_DIRS; do
  count=$(rg "${FEATURE_CATEGORY}" "$example_dir" --hidden -c 2>/dev/null | awk '{s+=$1} END {print s}')
  EXAMPLE_COUNT=$((EXAMPLE_COUNT + count))
done

# Calculate totals
TOTAL_MENTIONS=$((TYPE_COUNT + FUNCTION_COUNT + TEST_COUNT + EXAMPLE_COUNT))

# Estimate documentation lines (rule of thumb)
# Each type = ~30 lines docs, each function = ~10 lines, each example = ~40 lines, each test = ~15 lines
ESTIMATED_LINES=$((TYPE_COUNT * 30 + FUNCTION_COUNT * 10 + EXAMPLE_COUNT * 40 + TEST_COUNT * 15))
```

### Content Sufficiency Thresholds

**MUST HAVE** (to create subsection) - (.claude/commands/prodigy-detect-documentation-gaps.md:259-265):
- `TOTAL_MENTIONS >= 5` - Feature mentioned in at least 5 places
- `ESTIMATED_LINES >= 50` - Can generate at least 50 lines of documentation
- At least ONE of:
  - `TYPE_COUNT >= 1` (has configuration type/struct/class)
  - `EXAMPLE_COUNT >= 1` (has real example/config file)

**SHOULD HAVE** (for quality subsection) - (.claude/commands/prodigy-detect-documentation-gaps.md:266-269):
- `TOTAL_MENTIONS >= 10`
- `ESTIMATED_LINES >= 100`
- `TYPE_COUNT >= 1 AND EXAMPLE_COUNT >= 1` (both type definition and example)

### Decision Tree

**If TOTAL_MENTIONS < 5 OR ESTIMATED_LINES < 50**:
- ✗ **DO NOT create subsection**
- **Alternative**: Add as section within parent chapter's index.md
- **Log**: "⚠ Skipping subsection '${SUBSECTION_TITLE}': only ${TOTAL_MENTIONS} mentions, ${ESTIMATED_LINES} estimated lines"
- **Gap Report**: Record as `"action": "skipped_subsection_creation", "reason": "insufficient_content"`

**If TOTAL_MENTIONS >= 5 AND ESTIMATED_LINES >= 50 BUT < 100**:
- ~ Create subsection with "MINIMAL" flag
- Add metadata: `{"content_warning": "minimal", "estimated_lines": ESTIMATED_LINES}`
- Signals to fix phase that limited content is expected

**If TOTAL_MENTIONS >= 10 AND ESTIMATED_LINES >= 100**:
- ✓ **Proceed with full subsection creation**

### Special Case: Meta-Subsections

Meta-subsections like "Best Practices", "Troubleshooting", and "Examples" use different validation criteria (.claude/commands/prodigy-detect-documentation-gaps.md:306-334):

**Best Practices Subsection**:
```bash
BEST_PRACTICE_COUNT=$(rg "best.practice|pattern|guideline" --hidden --iglob '!.git' --iglob '!node_modules' -i -c | awk '{s+=$1} END {print s}')
# Requirement: BEST_PRACTICE_COUNT >= 3 OR documented patterns in code
```

**Troubleshooting Subsection**:
```bash
ERROR_COUNT=$(rg "error|warn|fail" --hidden --iglob '!.git' --iglob '!node_modules' -c | awk '{s+=$1} END {print s}')
ISSUE_COUNT=$(rg "TODO|FIXME|XXX" --hidden --iglob '!.git' --iglob '!node_modules' -c | awk '{s+=$1} END {print s}')
# Requirement: ERROR_COUNT >= 10 OR ISSUE_COUNT >= 5
```

**Examples Subsection**:
```bash
EXAMPLE_FILE_COUNT=0
for example_dir in $EXAMPLE_DIRS; do
  count=$(find "$example_dir" -type f \( -name "*.yml" -o -name "*.yaml" -o -name "*.json" -o -name "*.toml" \) 2>/dev/null | wc -l)
  EXAMPLE_FILE_COUNT=$((EXAMPLE_FILE_COUNT + count))
done
# Requirement: EXAMPLE_FILE_COUNT >= 2 real config files
```

**If threshold not met**: Add brief section to parent chapter's index.md instead of creating separate subsection.

## Structure Validation (Phase 7.5)

**MANDATORY**: Ensures chapters.json accurately reflects the actual file structure before generating flattened-items.json.

**Source**: `.claude/commands/prodigy-detect-documentation-gaps.md:678-743`

### Validation Process

**Step 1: Scan for Multi-Subsection Directories**

Find all directories under `book/src/` with an `index.md` file and count `.md` subsection files:

```bash
for dir in $(find "${BOOK_DIR}/src/" -maxdepth 1 -type d); do
  if [ -f "${dir}/index.md" ]; then
    SUBSECTION_COUNT=$(find "${dir}" -maxdepth 1 -name "*.md" ! -name "index.md" | wc -l)
    if [ "$SUBSECTION_COUNT" -gt 0 ]; then
      # This is a multi-subsection chapter
      CHAPTER_ID=$(basename "$dir")
      echo "Found multi-subsection chapter: $CHAPTER_ID"
    fi
  fi
done
```

**Step 2: Compare Against chapters.json**

For each discovered multi-subsection chapter:
1. Look up definition in chapters.json
2. Check if `type` field is "multi-subsection" or "single-file"
3. **If type is "single-file" or missing**: MISMATCH - add to mismatches list
4. **If type is "multi-subsection"**: Compare subsection counts
   - If counts don't match: MISMATCH

**Step 3: Check for Orphaned Single-File Definitions**

For each chapter with `type: "single-file"`:
1. Check if expected file (`book/src/chapter-id.md`) exists
2. Check if directory (`book/src/chapter-id/`) exists instead
3. **If file missing but directory exists**: MISMATCH

**Step 4: Auto-Migrate Mismatched Chapters**

For each mismatched chapter:
1. Scan directory to discover all subsection files
2. For each `.md` file (excluding `index.md`):
   - Extract subsection ID from filename (remove `.md`)
   - Read file and extract title from first H1/H2 heading
   - Extract topics from section headings
   - Create subsection definition
3. Update chapter in chapters.json:
   - Change `type` to "multi-subsection"
   - Change `file` to `index_file` (pointing to `index.md`)
   - Add `subsections` array with all discovered subsections
   - Preserve existing `topics` and `validation` fields
4. Write updated chapters.json to disk
5. Record migration in gap report

### Example Migration

**Before** (chapters.json - incorrect):
```json
{
  "id": "mapreduce",
  "title": "MapReduce Workflows",
  "file": "mapreduce.md",
  "type": "single-file",
  "topics": ["Map phase", "Reduce phase"]
}
```

**Actual File Structure** (reality):
```
book/src/mapreduce/
├── index.md
├── checkpoint-and-resume.md
├── performance-tuning.md
└── worktree-isolation.md
```

**After Migration** (chapters.json - corrected):
```json
{
  "id": "mapreduce",
  "title": "MapReduce Workflows",
  "index_file": "mapreduce/index.md",
  "type": "multi-subsection",
  "topics": ["Map phase", "Reduce phase"],
  "subsections": [
    {
      "id": "checkpoint-and-resume",
      "title": "Checkpoint and Resume",
      "file": "mapreduce/checkpoint-and-resume.md"
    },
    {
      "id": "performance-tuning",
      "title": "Performance Tuning",
      "file": "mapreduce/performance-tuning.md"
    },
    {
      "id": "worktree-isolation",
      "title": "Worktree Isolation",
      "file": "mapreduce/worktree-isolation.md"
    }
  ]
}
```

**Commit**: Structure fixes are committed BEFORE generating flattened-items.json with message: "docs: sync chapters.json with actual file structure"

## Flattened Items Generation (Phase 8)

**CRITICAL**: This file MUST be generated regardless of whether gaps are found. The map phase depends on it.

**Source**: `.claude/commands/prodigy-detect-documentation-gaps.md:744-827`

### Purpose

Creates a flat array of all chapters and subsections for parallel processing in the map phase. This enables each map agent to work on a single chapter or subsection independently.

### Processing Logic

```
For each chapter in chapters.json:
  If type == "multi-subsection":
    For each subsection in chapter.subsections:
      Create item with parent metadata
      Add to flattened array

  If type == "single-file":
    Create item with type marker
    Add to flattened array
```

### Output Structure

**File**: `.prodigy/book-analysis/flattened-items.json`

**Example**:
```json
[
  {
    "id": "workflow-basics",
    "title": "Workflow Basics",
    "file": "book/src/workflow-basics.md",
    "topics": [
      "Setup phase",
      "Command types",
      "Variable interpolation"
    ],
    "validation": "Check that workflow syntax and variable documentation are complete",
    "type": "single-file"
  },
  {
    "id": "checkpoint-and-resume",
    "title": "Checkpoint and Resume",
    "file": "book/src/mapreduce/checkpoint-and-resume.md",
    "parent_chapter_id": "mapreduce",
    "parent_chapter_title": "MapReduce Workflows",
    "type": "subsection",
    "topics": [
      "Checkpoint creation",
      "Resume behavior",
      "State preservation"
    ],
    "validation": "Check that checkpoint mechanism and resume procedures are documented",
    "feature_mapping": [
      "mapreduce.checkpoint",
      "mapreduce.resume"
    ]
  },
  {
    "id": "performance-tuning",
    "title": "Performance Tuning",
    "file": "book/src/mapreduce/performance-tuning.md",
    "parent_chapter_id": "mapreduce",
    "parent_chapter_title": "MapReduce Workflows",
    "type": "subsection",
    "topics": [
      "Parallel execution",
      "Resource limits"
    ],
    "feature_mapping": [
      "mapreduce.performance",
      "mapreduce.resource_limits"
    ]
  }
]
```

### Map Phase Integration

The map phase consumes flattened-items.json (workflows/book-docs-drift.yml:36-48):

```yaml
map:
  input: "${ANALYSIS_DIR}/flattened-items.json"
  json_path: "$[*]"  # Each item is a chapter or subsection

  agent_template:
    # Analyze drift for this specific chapter/subsection
    - claude: "/prodigy-analyze-subsection-drift --project $PROJECT_NAME --json '${item}' --features $FEATURES_PATH"

    # Fix drift for this specific chapter/subsection
    - claude: "/prodigy-fix-subsection-drift --project $PROJECT_NAME --json '${item}'"
```

**Why Required**: Without flattened-items.json, the map phase cannot parallelize drift analysis and fixing across chapters/subsections.

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
"Goal Seeking Operations" → ["goal", "seeking", "operations"]
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

## Gap Report Structure

**Output**: `.prodigy/book-analysis/gap-report.json`

### Example Report

```json
{
  "analysis_date": "2025-11-09T12:34:56Z",
  "features_analyzed": 12,
  "documented_topics": 10,
  "gaps_found": 2,
  "gaps": [
    {
      "severity": "high",
      "type": "missing_chapter",
      "feature_category": "agent_merge",
      "feature_description": "Custom merge workflows for map agents",
      "recommended_chapter_id": "agent-merge-workflows",
      "recommended_title": "Agent Merge Workflows",
      "recommended_location": "book/src/agent-merge-workflows.md",
      "is_subsection": false
    },
    {
      "severity": "high",
      "type": "missing_chapter",
      "feature_category": "circuit_breaker",
      "feature_description": "Circuit breaker for error handling",
      "recommended_chapter_id": "circuit-breaker",
      "recommended_title": "Circuit Breaker",
      "recommended_location": "book/src/circuit-breaker.md",
      "is_subsection": false
    }
  ],
  "actions_taken": [
    {
      "action": "created_chapter_definition",
      "chapter_id": "agent-merge-workflows",
      "file_path": "workflows/data/prodigy-chapters.json"
    },
    {
      "action": "created_stub_file",
      "file_path": "book/src/agent-merge-workflows.md",
      "type": "chapter"
    },
    {
      "action": "updated_summary",
      "file_path": "book/src/SUMMARY.md",
      "items_added": [
        {"type": "chapter", "id": "agent-merge-workflows"}
      ]
    }
  ],
  "structure_validation": {
    "mismatches_found": 1,
    "mismatched_chapters": ["mapreduce"],
    "migrations_performed": [
      {
        "chapter_id": "mapreduce",
        "action": "migrated_to_multi_subsection",
        "subsections_discovered": 3
      }
    ],
    "validation_timestamp": "2025-11-09T12:34:56Z"
  }
}
```

## Execution Progress

When gap detection runs, it displays progress through multiple phases:

```
🔍 Analyzing documentation coverage...
   ✓ Loaded 12 feature areas from features.json
   ✓ Loaded 10 existing chapters
   ✓ Parsed SUMMARY.md structure

📊 Comparing features against documentation...
   ✓ Analyzed workflow_basics: documented ✓
   ✓ Analyzed mapreduce: documented ✓
   ⚠ Analyzed agent_merge: not documented (gap detected)
   ✓ Analyzed command_types: documented ✓
   ⚠ Analyzed circuit_breaker: not documented (gap detected)

🔍 Validating chapter structure (Phase 7.5)...
   ✓ Scanning for multi-subsection directories
   ✓ Comparing against chapters.json definitions
   ⚠ Found mismatch in mapreduce chapter (was single-file, now multi-subsection)
   ✓ Auto-migrated mapreduce chapter structure

📝 Creating missing chapters...
   ✓ Generated definition: agent-merge-workflows
   ✓ Created stub: book/src/agent-merge-workflows.md
   ✓ Generated definition: circuit-breaker
   ✓ Created stub: book/src/circuit-breaker.md
   ✓ Updated SUMMARY.md

💾 Generating flattened items for map phase...
   ✓ Processed 1 single-file chapter (workflow-basics)
   ✓ Processed 3 subsections from mapreduce chapter
   ✓ Processed 10 additional chapters/subsections
   ✓ Generated .prodigy/book-analysis/flattened-items.json

💾 Committing changes...
   ✓ Staged 6 files
   ✓ Committed: docs: auto-discover missing chapters for agent-merge-workflows, circuit-breaker
   ✓ Committed: docs: sync chapters.json with actual file structure
```

### Final Summary

```
📊 Documentation Gap Analysis Complete
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Features Analyzed: 12
Documented Topics: 10
Gaps Found: 2

🔴 High Severity Gaps (Missing Chapters): 2
  • agent_merge - Custom merge workflows for map agents
  • circuit_breaker - Workflow error circuit breaking

✅ Actions Taken:
  ✓ Created 2 chapter definitions in workflows/data/prodigy-chapters.json
  ✓ Created 2 stub files in book/src/
  ✓ Updated book/src/SUMMARY.md
  ✓ Generated flattened-items.json with 14 items
  ✓ Auto-migrated 1 chapter structure
  ✓ Committed changes (2 commits)

📝 Next Steps:
  The map phase will now process 14 chapters/subsections to populate content.
  Review the generated stubs and customize as needed.
```

## Error Handling

**Source**: `.claude/commands/prodigy-detect-documentation-gaps.md:889-919`

### Common Errors

**Missing features.json**:
- **Cause**: Feature analysis step hasn't run yet
- **Solution**: Ensure `/prodigy-analyze-features-for-book` runs before gap detection in setup phase
- **Error Message**: "Error: features.json not found at {path}. Run feature analysis first."

**Missing/Invalid chapters.json**:
- **Cause**: Chapter definitions file doesn't exist or has invalid JSON
- **Solution**: Create valid chapters.json or fix JSON syntax errors
- **Recovery**: Gap detection can initialize empty chapters.json if needed

**File Write Failures**:
- **Cause**: Permission issues or disk full
- **Solution**: Check directory permissions and disk space
- **Rollback**: Gap detection records partial state in gap report for manual cleanup

**Invalid JSON Handling**:
- **Cause**: Malformed JSON in input files
- **Solution**: Validate JSON with `jq` before running workflow
- **Error Recording**: Details added to gap report for debugging

## Testing

Gap detection has comprehensive test coverage in `tests/documentation_gap_detection_test.rs:1-678`:

### Test Coverage

**Core Functionality**:
- Identifying missing chapters (tests/documentation_gap_detection_test.rs:1-50)
- Idempotence behavior (tests/documentation_gap_detection_test.rs:236-274)
- Topic normalization logic (tests/documentation_gap_detection_test.rs:275-320)
- Chapter definition generation (tests/documentation_gap_detection_test.rs:321-370)

**Edge Cases**:
- False positive prevention via normalization
- Handling chapters with multiple topics
- Subsection discovery and validation
- Structure migration for multi-subsection chapters

**Quality Assurance**:
- Stub file structure validation
- SUMMARY.md update correctness
- Gap report JSON schema validation
