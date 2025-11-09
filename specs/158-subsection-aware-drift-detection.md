---
number: 158
title: Subsection-Aware Drift Detection
category: optimization
priority: medium
status: draft
dependencies: [157]
created: 2025-01-11
---

# Specification 158: Subsection-Aware Drift Detection

**Category**: optimization
**Priority**: medium
**Status**: draft
**Dependencies**: Spec 157 (mdBook Subsection Organization)

## Context

After implementing Spec 157, large chapters are automatically split into logical subsections with organized directory structures. However, the current drift detection workflow still operates at the **chapter level**:

- `prodigy-chapters.json` defines chapters as single files
- Map phase processes one entire chapter per agent
- Drift analysis treats chapters as atomic units
- Cannot identify which specific subsection has drift

This creates several problems:

1. **Coarse Granularity**: "MapReduce chapter has drift" doesn't tell us if the problem is in DLQ, checkpoints, or performance tuning
2. **Inefficient Fixes**: Must regenerate entire chapter even if only one subsection needs updating
3. **Poor Parallelism**: Fewer work items (chapters) means less parallelization potential
4. **No Structure Intent**: Subsections are created post-hoc by splitting, not designed upfront
5. **Maintenance Burden**: Can't track which subsections correspond to which codebase features

### Current Workflow Flow

```yaml
# Current (Chapter-Level)
setup:
  - analyze features → features.json

map:
  input: chapters.json
  json_path: "$.chapters[*]"  # 10-15 chapter items
  agent_template:
    - analyze chapter drift
    - fix entire chapter

reduce:
  - build book
  - split large chapters (post-hoc)
```

### Desired Workflow Flow

```yaml
# Future (Subsection-Level)
setup:
  - analyze features → features.json
  - define subsection structure → chapters-with-subsections.json

map:
  input: chapters-with-subsections.json
  json_path: "$.chapters[*].subsections[*]"  # 50-100 subsection items!
  agent_template:
    - analyze subsection drift
    - fix specific subsection

reduce:
  - build book
  - verify subsection organization
```

## Objective

Enable drift detection and fixes at the subsection granularity by:
1. Extending `prodigy-chapters.json` schema to define subsections explicitly
2. Updating drift detection commands to analyze subsections independently
3. Modifying map phase to process subsections instead of chapters
4. Maintaining backward compatibility with single-file chapters

This provides better parallelism, targeted fixes, and intentional subsection structure across all documentation books.

## Requirements

### Functional Requirements

1. **Enhanced Chapter Schema**
   - Support both single-file chapters (backward compatible)
   - Support multi-subsection chapters with explicit structure
   - Define subsection-to-feature mappings
   - Specify subsection file paths and topics

2. **Subsection Drift Analysis**
   - Analyze drift at subsection granularity
   - Compare subsection content against feature subset
   - Generate subsection-specific drift reports
   - Identify missing, outdated, or incorrect content per subsection

3. **Targeted Subsection Fixes**
   - Fix individual subsection files independently
   - Update only changed subsections, preserve unchanged ones
   - Maintain cross-references between subsections
   - Validate subsection fixes don't break chapter coherence

4. **Gap Detection for Subsections**
   - Detect missing subsections within existing chapters
   - Auto-generate subsection definitions for new features
   - Suggest subsection organization based on feature categories
   - Create stub subsection files when gaps detected

5. **Backward Compatibility**
   - Support legacy single-file chapter definitions
   - Migrate single-file chapters to subsection structure
   - Preserve existing drift detection for non-subsection chapters
   - Gradual migration path from flat to hierarchical

### Non-Functional Requirements

1. **Performance**: Analyze 100 subsections in parallel faster than 15 sequential chapters
2. **Granularity**: Detect drift at <200 line subsection level vs 1000+ line chapter level
3. **Parallelism**: Support 50+ concurrent agents (more subsections = more parallelism)
4. **Accuracy**: Subsection drift detection as accurate as current chapter-level detection

## Acceptance Criteria

- [ ] `prodigy-chapters.json` schema extended with optional `subsections` array
- [ ] Single-file chapters still work without subsections (backward compatible)
- [ ] Multi-subsection chapters properly defined with file paths and topics
- [ ] `/prodigy-detect-documentation-gaps` generates subsection definitions
- [ ] `/prodigy-analyze-book-chapter-drift` renamed/updated to handle subsections
- [ ] `/prodigy-fix-chapter-drift` renamed/updated to fix individual subsections
- [ ] Map phase can process `$.chapters[*].subsections[*]` JSONPath
- [ ] Drift reports show subsection-level issues, not just chapter-level
- [ ] Subsection fixes don't break cross-references to other subsections
- [ ] SUMMARY.md correctly reflects subsection hierarchy after fixes
- [ ] Migration command converts single-file chapters to subsection structure
- [ ] Documentation updated with subsection workflow examples

## Technical Details

### Implementation Approach

#### 1. Enhanced Chapter Schema

**New `prodigy-chapters.json` Format**:

```json
{
  "chapters": [
    {
      "id": "workflow-basics",
      "title": "Workflow Basics",
      "type": "single-file",
      "file": "book/src/workflow-basics.md",
      "topics": ["Standard workflows", "Basic structure"],
      "validation": "Check basic workflow syntax"
    },
    {
      "id": "mapreduce",
      "title": "MapReduce Workflows",
      "type": "multi-subsection",
      "index_file": "book/src/mapreduce/index.md",
      "subsections": [
        {
          "id": "structure",
          "title": "Complete Structure",
          "file": "book/src/mapreduce/structure.md",
          "topics": ["setup", "map", "reduce", "phases"],
          "validation": "Check MapReduceWorkflowConfig fields",
          "feature_mapping": ["mapreduce.phases", "mapreduce.configuration"]
        },
        {
          "id": "checkpoint-resume",
          "title": "Checkpoint and Resume",
          "file": "book/src/mapreduce/checkpoint-resume.md",
          "topics": ["checkpoints", "resume", "state", "recovery"],
          "validation": "Check checkpoint structure and resume behavior",
          "feature_mapping": ["mapreduce.checkpoint", "mapreduce.resume"]
        },
        {
          "id": "dlq",
          "title": "Dead Letter Queue",
          "file": "book/src/mapreduce/dlq.md",
          "topics": ["failed items", "retry", "DLQ", "error handling"],
          "validation": "Check DLQ structure and retry commands",
          "feature_mapping": ["mapreduce.dlq", "error_handling.dlq"]
        }
      ]
    }
  ]
}
```

**Schema Fields**:
- `type`: `"single-file"` or `"multi-subsection"`
- `index_file`: Path to index.md for multi-subsection chapters
- `subsections[]`: Array of subsection definitions
- `feature_mapping`: Links subsection to specific features from features.json

#### 2. Updated Command: `/prodigy-analyze-subsection-drift`

**Replaces**: `/prodigy-analyze-book-chapter-drift` (or extends it)

**Parameters**:
```bash
/prodigy-analyze-subsection-drift \
  --project Prodigy \
  --json '{"id": "checkpoint-resume", "title": "Checkpoint and Resume", ...}' \
  --features .prodigy/book-analysis/features.json \
  --chapter-id mapreduce
```

**Algorithm**:
1. Parse subsection JSON from `--json` parameter
2. Load features.json and extract relevant feature subset based on `feature_mapping`
3. Read subsection markdown file
4. Compare subsection content against mapped features
5. Identify subsection-specific drift:
   - Missing content for mapped features
   - Outdated information
   - Incorrect examples
6. Generate drift report for this subsection only:
   - Path: `.prodigy/book-analysis/drift-${chapter_id}-${subsection_id}.json`
   - Focus on subsection-specific issues
   - Include cross-references to related subsections

**Example Drift Report** (`.prodigy/book-analysis/drift-mapreduce-checkpoint-resume.json`):
```json
{
  "chapter_id": "mapreduce",
  "subsection_id": "checkpoint-resume",
  "subsection_title": "Checkpoint and Resume",
  "subsection_file": "book/src/mapreduce/checkpoint-resume.md",
  "feature_mappings": ["mapreduce.checkpoint", "mapreduce.resume"],
  "drift_detected": true,
  "severity": "medium",
  "issues": [
    {
      "type": "missing_content",
      "severity": "high",
      "section": "Resume Behavior",
      "description": "Missing documentation for session-job ID mapping",
      "feature_reference": "mapreduce.resume.session_job_mapping",
      "fix_suggestion": "Add section explaining bidirectional session-job mapping"
    }
  ]
}
```

#### 3. Updated Command: `/prodigy-fix-subsection-drift`

**Replaces**: `/prodigy-fix-chapter-drift` (or extends it)

**Parameters**:
```bash
/prodigy-fix-subsection-drift \
  --project Prodigy \
  --chapter-id mapreduce \
  --subsection-id checkpoint-resume
```

**Algorithm**:
1. Load drift report: `.prodigy/book-analysis/drift-mapreduce-checkpoint-resume.json`
2. Read subsection file: `book/src/mapreduce/checkpoint-resume.md`
3. Apply fixes to this subsection only
4. Preserve cross-references to other subsections:
   - Links to `dlq.md` stay valid
   - Links to `structure.md` stay valid
5. Validate subsection still fits in chapter context
6. Commit subsection changes:
   ```
   docs: fix drift in MapReduce > Checkpoint and Resume subsection

   - Added session-job ID mapping documentation
   - Updated checkpoint structure examples
   ```

#### 4. Map Phase Workflow Update

**New Workflow** (`book-docs-drift.yml`):

```yaml
map:
  input: "${CHAPTERS_FILE}"
  json_path: "$.chapters[*].subsections[*]"  # NEW: Process subsections!

  agent_template:
    # Analyze subsection drift
    - claude: "/prodigy-analyze-subsection-drift --project $PROJECT_NAME --json '${item}' --features $FEATURES_PATH --chapter-id ${item.parent_chapter_id}"
      commit_required: true

    # Fix subsection drift
    - claude: "/prodigy-fix-subsection-drift --project $PROJECT_NAME --chapter-id ${item.parent_chapter_id} --subsection-id ${item.id}"
      commit_required: true

  max_parallel: ${MAX_PARALLEL}

  # Filter out single-file chapters (process separately or skip)
  filter: "item.type == 'subsection'"
```

**JSONPath Challenge**: Need to include parent chapter ID in subsection items.

**Solution - Preprocessing Step**:
```yaml
setup:
  # ... existing steps ...

  # Flatten subsections with parent context
  - shell: |
      jq '.chapters | map(
        if .type == "multi-subsection" then
          .subsections | map(. + {parent_chapter_id: .id, type: "subsection"})
        else
          [. + {type: "single-file"}]
        end
      ) | flatten' ${CHAPTERS_FILE} > .prodigy/book-analysis/flattened-items.json

map:
  input: ".prodigy/book-analysis/flattened-items.json"
  json_path: "$[*]"

  agent_template:
    - claude: "/prodigy-analyze-item-drift --project $PROJECT_NAME --json '${item}' --features $FEATURES_PATH"
    - claude: "/prodigy-fix-item-drift --project $PROJECT_NAME --json '${item}'"
```

#### 5. Gap Detection for Subsections

**Update**: `/prodigy-detect-documentation-gaps`

**New Behavior**:
1. Analyze features.json as before
2. For each feature area, determine if it should be:
   - New single-file chapter
   - New subsection in existing chapter
   - Update to existing subsection
3. Generate subsection definitions:
   ```json
   {
     "action": "create_subsection",
     "chapter_id": "mapreduce",
     "subsection": {
       "id": "agent-merge",
       "title": "Agent Merge Workflows",
       "file": "book/src/mapreduce/agent-merge.md",
       "topics": ["merge config", "merge variables", "merge validation"],
       "feature_mapping": ["mapreduce.merge", "advanced.merge_workflows"]
     }
   }
   ```
4. Update `prodigy-chapters.json` with new subsection definitions
5. Create stub subsection markdown files
6. Update SUMMARY.md with new subsection entry

### Architecture Changes

**Modified Commands**:
```
.claude/commands/
├── prodigy-analyze-subsection-drift.md  # NEW or updated
├── prodigy-fix-subsection-drift.md      # NEW or updated
└── prodigy-detect-documentation-gaps.md # UPDATED
```

**Schema Migration**:
```
workflows/data/
├── prodigy-chapters.json          # SCHEMA UPDATED
└── prodigy-chapters-v1.json       # BACKUP of old format
```

**Drift Reports**:
```
.prodigy/book-analysis/
├── drift-{chapter-id}-{subsection-id}.json  # NEW format
└── drift-{chapter-id}.json                  # OLD format (deprecated)
```

### Data Structures

**Chapter Definition (Unified)**:
```rust
#[derive(Serialize, Deserialize)]
struct ChapterDefinition {
    id: String,
    title: String,
    #[serde(rename = "type")]
    chapter_type: ChapterType,

    // For single-file chapters
    #[serde(skip_serializing_if = "Option::is_none")]
    file: Option<PathBuf>,

    // For multi-subsection chapters
    #[serde(skip_serializing_if = "Option::is_none")]
    index_file: Option<PathBuf>,

    #[serde(skip_serializing_if = "Option::is_none")]
    subsections: Option<Vec<SubsectionDefinition>>,

    topics: Vec<String>,
    validation: String,
}

#[derive(Serialize, Deserialize)]
enum ChapterType {
    #[serde(rename = "single-file")]
    SingleFile,
    #[serde(rename = "multi-subsection")]
    MultiSubsection,
}

#[derive(Serialize, Deserialize)]
struct SubsectionDefinition {
    id: String,
    title: String,
    file: PathBuf,
    topics: Vec<String>,
    validation: String,
    feature_mapping: Vec<String>,  // Links to features.json paths

    // Added during preprocessing for map phase
    #[serde(skip_serializing_if = "Option::is_none")]
    parent_chapter_id: Option<String>,
}
```

**Drift Report (Subsection-Level)**:
```rust
#[derive(Serialize, Deserialize)]
struct SubsectionDriftReport {
    chapter_id: String,
    subsection_id: String,
    subsection_title: String,
    subsection_file: PathBuf,
    feature_mappings: Vec<String>,
    drift_detected: bool,
    severity: DriftSeverity,
    issues: Vec<DriftIssue>,
    cross_references: Vec<String>,  // Other subsections referenced
}
```

### APIs and Interfaces

**Command Signature Changes**:

```bash
# Old (Chapter-level)
/prodigy-analyze-book-chapter-drift \
  --project Prodigy \
  --json '{"id": "mapreduce", "file": "book/src/mapreduce.md", ...}' \
  --features .prodigy/book-analysis/features.json

# New (Subsection-level)
/prodigy-analyze-subsection-drift \
  --project Prodigy \
  --chapter-id mapreduce \
  --json '{"id": "checkpoint-resume", "file": "book/src/mapreduce/checkpoint-resume.md", ...}' \
  --features .prodigy/book-analysis/features.json
```

**Backward Compatibility Mode**:
```bash
# For single-file chapters, command auto-detects and uses chapter-level analysis
/prodigy-analyze-subsection-drift \
  --project Prodigy \
  --json '{"id": "intro", "type": "single-file", "file": "book/src/intro.md", ...}' \
  --features .prodigy/book-analysis/features.json
# Internally: Falls back to chapter-level drift detection
```

## Dependencies

### Prerequisites
- **Spec 157**: mdBook Subsection Organization (subsection structure must exist)

### Affected Components
- `.claude/commands/prodigy-analyze-book-chapter-drift.md` → renamed/extended
- `.claude/commands/prodigy-fix-chapter-drift.md` → renamed/extended
- `.claude/commands/prodigy-detect-documentation-gaps.md` → updated
- `workflows/data/prodigy-chapters.json` → schema updated
- `workflows/book-docs-drift.yml` → map phase updated

### External Dependencies
- jq: For JSON preprocessing to flatten subsections
- JSONPath: For extracting subsections from nested structure

## Testing Strategy

### Unit Tests
- Parse multi-subsection chapter definitions correctly
- Parse single-file chapter definitions (backward compatibility)
- Extract subsections with parent chapter context
- Generate subsection-specific drift reports
- Update individual subsection files without affecting others

### Integration Tests

1. **Multi-Subsection Drift Detection**:
   - Create chapter with 5 subsections
   - Modify codebase feature mapped to subsection 3
   - Run drift detection
   - Verify only subsection 3 shows drift
   - Fix subsection 3
   - Verify other subsections unchanged

2. **Single-File Backward Compatibility**:
   - Define chapter with `type: "single-file"`
   - Run drift detection
   - Verify chapter-level analysis still works
   - Verify no errors or broken behavior

3. **Gap Detection for Subsections**:
   - Add new feature to features.json
   - Run gap detection
   - Verify new subsection definition created in existing chapter
   - Verify stub file created in correct subdirectory
   - Verify SUMMARY.md updated with new subsection

4. **Map Phase Parallelism**:
   - Create book with 10 chapters, each with 5 subsections (50 items)
   - Run workflow with max_parallel: 10
   - Verify all 50 subsections processed
   - Verify drift reports generated for each
   - Verify fixes applied independently

### Performance Tests
- Process 100 subsections faster than 20 chapters sequentially
- Parallel processing with 50 agents completes in <5 minutes
- Subsection drift analysis completes in <10 seconds per subsection

### User Acceptance
- Run on prodigy book with MapReduce, Commands, Variables chapters split
- Verify subsection-level drift detection more accurate than chapter-level
- Confirm targeted fixes update only changed subsections
- Validate cross-references between subsections preserved

## Documentation Requirements

### Code Documentation
- Document new chapter schema with examples
- Explain subsection vs chapter-level processing
- Document feature_mapping usage and best practices
- Provide migration guide from old to new schema

### User Documentation
- Update automated-documentation.md with subsection workflow
- Add examples of multi-subsection chapter definitions
- Explain when to use subsections vs single files
- Document gap detection for subsections

### Architecture Updates
- Update CLAUDE.md with new command signatures
- Document preprocessing step for flattening subsections
- Explain feature_mapping concept and usage

## Implementation Notes

### Feature Mapping Strategy

**Mapping Subsections to Features**:
- Each subsection's `feature_mapping` array links to paths in features.json
- Example: `["mapreduce.checkpoint", "mapreduce.resume"]`
- During drift analysis, only compare against these feature subsets
- More accurate drift detection (less noise from unrelated features)

**Example**:
```json
// features.json
{
  "mapreduce": {
    "checkpoint": { "storage": "~/.prodigy/state/", ... },
    "resume": { "commands": ["prodigy resume-job"], ... },
    "dlq": { "storage": "~/.prodigy/dlq/", ... }
  }
}

// Subsection definition
{
  "id": "checkpoint-resume",
  "feature_mapping": ["mapreduce.checkpoint", "mapreduce.resume"]
}

// Drift analysis: Compare subsection only against checkpoint + resume features
```

### Preprocessing for Map Phase

**Challenge**: JSONPath `$.chapters[*].subsections[*]` doesn't preserve parent context.

**Solution**: Setup phase preprocesses chapters.json:
```bash
jq '.chapters | map(
  if .type == "multi-subsection" then
    .subsections | map(. + {
      parent_chapter_id: parent.id,
      parent_chapter_title: parent.title,
      type: "subsection"
    })
  else
    [. + {type: "single-file"}]
  end
) | flatten' chapters.json > flattened-items.json
```

**Result**: Flat array of items with full context:
```json
[
  {"type": "single-file", "id": "intro", "file": "intro.md", ...},
  {"type": "subsection", "id": "checkpoint-resume", "parent_chapter_id": "mapreduce", ...},
  {"type": "subsection", "id": "dlq", "parent_chapter_id": "mapreduce", ...}
]
```

### Migration Path

**Phase 1: Opt-in Subsections**
- Existing chapters remain single-file
- New chapters can use multi-subsection format
- Both formats coexist in same chapters.json

**Phase 2: Gradual Migration**
- Migration command converts single-file to multi-subsection:
  ```bash
  /prodigy-migrate-chapter-to-subsections \
    --chapter-id mapreduce \
    --chapters-file workflows/data/prodigy-chapters.json
  ```
- Reads existing split structure from Spec 157
- Generates subsection definitions based on actual files
- Updates chapters.json schema

**Phase 3: Full Adoption**
- All large chapters migrated to subsections
- Single-file reserved for small chapters (<300 lines)
- Workflow standardized across projects

## Migration and Compatibility

### Backward Compatibility

**Single-File Chapters**:
- Still work without subsections
- `type: "single-file"` explicitly marks these
- Commands auto-detect and use chapter-level processing
- No breaking changes for existing workflows

**Legacy chapters.json**:
```json
{
  "chapters": [
    {
      "id": "intro",
      "title": "Introduction",
      "file": "book/src/intro.md",
      "topics": ["Overview"],
      "validation": "Check intro content"
    }
  ]
}
```

**Migrated chapters.json**:
```json
{
  "chapters": [
    {
      "id": "intro",
      "title": "Introduction",
      "type": "single-file",
      "file": "book/src/intro.md",
      "topics": ["Overview"],
      "validation": "Check intro content"
    }
  ]
}
```

### Breaking Changes

**None for users** - This is an opt-in enhancement.

**For workflow maintainers**:
- Must add preprocessing step if using subsections
- Must update map phase JSONPath if using subsections
- Must migrate chapters.json schema (additive, not breaking)

### Migration Command

**Create**: `/prodigy-migrate-chapter-to-subsections`

```bash
/prodigy-migrate-chapter-to-subsections \
  --chapter-id mapreduce \
  --chapters-file workflows/data/prodigy-chapters.json \
  --book-dir book
```

**Algorithm**:
1. Check if chapter already has subsections (skip if so)
2. Check if chapter directory exists: `book/src/mapreduce/`
3. Scan directory for subsection files
4. Generate subsection definitions based on files found
5. Analyze each subsection to extract topics
6. Map subsections to features (heuristic or manual)
7. Update chapters.json with multi-subsection definition
8. Commit changes

## Success Metrics

- **Granularity**: 90% of drift detected at subsection level vs chapter level
- **Parallelism**: 3x more work items (subsections) than chapters
- **Accuracy**: 95% of drift issues mapped to correct subsection
- **Performance**: Process 100 subsections in <5 minutes with 20 parallel agents
- **Coverage**: All large chapters (>400 lines) have subsection definitions

## Future Enhancements

### Spec 159: AI-Driven Section Grouping
- Use Spec 158's subsection structure as foundation
- Apply AI to optimize subsection organization
- Ensure consistent structure across projects

### Cross-Project Templates
- Define standard subsection templates by chapter type
- Example: All "configuration" chapters have same subsection structure
- Example: All "CLI reference" chapters organized consistently

### Subsection Analytics
- Track which subsections have most drift
- Identify subsections needing more examples
- Measure subsection quality and completeness
