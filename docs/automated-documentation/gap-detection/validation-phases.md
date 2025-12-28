## Content Sufficiency Validation (Step 0)

**CRITICAL SAFEGUARD**: Before creating any subsection, gap detection validates that sufficient material exists in the codebase to support meaningful documentation.

**Source**: `.claude/commands/prodigy-detect-documentation-gaps.md:166-335`

### Preservation of Single-File Chapters

Gap detection **ALWAYS preserves well-written single-file chapters** (.claude/commands/prodigy-detect-documentation-gaps.md:174-209):

**Preservation Rules**:
- **< 1000 lines AND < 10 H2 sections**: PRESERVE as single-file
- **>= 1000 lines OR >= 10 H2 sections**: Consider subsections for readability

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
- X **DO NOT create subsection**
- **Alternative**: Add as section within parent chapter's index.md
- **Log**: "Warning: Skipping subsection '${SUBSECTION_TITLE}': only ${TOTAL_MENTIONS} mentions, ${ESTIMATED_LINES} estimated lines"
- **Gap Report**: Record as `"action": "skipped_subsection_creation", "reason": "insufficient_content"`

**If TOTAL_MENTIONS >= 5 AND ESTIMATED_LINES >= 50 BUT < 100**:
- ~ Create subsection with "MINIMAL" flag
- Add metadata: `{"content_warning": "minimal", "estimated_lines": ESTIMATED_LINES}`
- Signals to fix phase that limited content is expected

**If TOTAL_MENTIONS >= 10 AND ESTIMATED_LINES >= 100**:
- Check mark **Proceed with full subsection creation**

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
