# /prodigy-detect-documentation-gaps

Detect documentation gaps by analyzing the codebase features against existing book chapters, then automatically create chapter definitions and stub markdown files for undocumented features.

## Variables

- `--project <name>` - Project name (e.g., "Prodigy")
- `--config <path>` - Path to book configuration JSON (e.g., ".prodigy/book-config.json")
- `--features <path>` - Path to features.json from setup phase (e.g., ".prodigy/book-analysis/features.json")
- `--chapters <path>` - Path to chapter definitions JSON (e.g., "workflows/data/prodigy-chapters.json")
- `--book-dir <path>` - Book directory path (e.g., "book")

## Execute

### Phase 1: Parse Parameters and Load Data

**Parse Command Arguments:**
Extract all required parameters from the command:
- `--project`: Project name for output messages
- `--config`: Path to book configuration
- `--features`: Path to features.json from setup phase
- `--chapters`: Path to prodigy-chapters.json
- `--book-dir`: Book directory path

**Load Configuration Files:**
Read the following files to understand current state:
1. Read `--features` file to get complete feature inventory from setup phase
2. Read `--chapters` file to get existing chapter definitions
3. Read `${book-dir}/src/SUMMARY.md` to get book structure
4. Read `--config` file to get project settings

### Phase 2: Analyze Existing Documentation Coverage

**Build Documentation Map:**

For each chapter in the chapters JSON file:
1. Extract the chapter ID, title, file path, and topics
2. Read the chapter markdown file to understand documented content
3. Extract section headings and documented capabilities
4. Build a map: `{chapter_id: {title, topics, documented_features}}`

**Normalize Topic Names for Comparison:**

Create normalized versions of all documented topics:
- Convert to lowercase
- Remove punctuation and special characters
- Trim whitespace
- Extract key terms (e.g., "MapReduce Workflows" → "mapreduce", "workflows")

This helps match feature categories against documented topics accurately.

### Phase 3: Identify Documentation Gaps

**Compare Features Against Documentation:**

For each feature area in features.json:
1. Extract the feature category name (e.g., "mapreduce", "command_types", "environment")
2. Check if ANY existing chapter OR subsection covers this feature area
3. Look for matches in:
   - Chapter IDs and subsection IDs
   - Chapter titles and subsection titles
   - Chapter topics list and subsection topics
   - Section headings in markdown files
   - Subsection feature_mapping arrays (if present)

**Classify Gaps by Severity:**

**High Severity (Missing Chapter/Subsection):**
- Feature area exists in features.json
- No corresponding chapter OR subsection found
- Major user-facing capability with no guidance
- Example: "agent_merge" feature exists but no chapter/subsection documents it

**Medium Severity (Incomplete Chapter/Subsection):**
- Chapter or multi-subsection structure exists for the feature area
- But specific sub-capabilities are missing
- Could be addressed by:
  - Adding a new subsection to existing multi-subsection chapter
  - Expanding an existing subsection
  - Adding content to single-file chapter
- Example: "mapreduce" chapter exists but missing "performance_tuning" subsection

**Low Severity (Minor Gap):**
- Edge cases or advanced features not documented
- Internal APIs exposed to users
- Less common use cases

**Generate Gap Report:**

Create a structured JSON report documenting all gaps found:
```json
{
  "analysis_date": "<current-timestamp>",
  "features_analyzed": <total-feature-areas>,
  "documented_topics": <count-of-chapters-and-subsections>,
  "gaps_found": <count-of-gaps>,
  "gaps": [
    {
      "severity": "high|medium|low",
      "type": "missing_chapter|missing_subsection|incomplete_chapter|incomplete_subsection",
      "feature_category": "<feature-area-name>",
      "feature_description": "<brief-description>",
      "recommended_chapter_id": "<chapter-id>",
      "recommended_title": "<chapter-title>",
      "recommended_location": "<file-path>",
      "parent_chapter_id": "<parent-id-if-subsection>",
      "is_subsection": true|false
    }
  ],
  "actions_taken": []
}
```

### Phase 4: Generate Chapter Definitions for Missing Chapters

**For Each High Severity Gap (Missing Chapter):**

1. **Generate Chapter ID:**
   - Convert feature category to kebab-case
   - Example: "agent_merge" → "agent-merge-workflows"
   - Example: "circuit_breaker" → "circuit-breaker"
   - Ensure uniqueness against existing chapter IDs

2. **Generate Chapter Title:**
   - Convert to title case with spaces
   - Add descriptive suffix if needed
   - Example: "agent_merge" → "Agent Merge Workflows"
   - Example: "circuit_breaker" → "Circuit Breaker"

3. **Determine File Path:**
   - Use book_src from config (typically "book/src")
   - Create filename from chapter ID
   - Format: `${book_src}/${chapter_id}.md`
   - Example: "book/src/agent-merge-workflows.md"

4. **Extract Topics from Features:**
   - Look at the feature capabilities in features.json
   - Convert capabilities to topic names
   - Example: For "agent_merge" feature with capabilities ["validation", "merge_config", "error_handling"]
   - Topics: ["Agent merge configuration", "Merge validation", "Error handling in merges"]

5. **Define Validation Criteria:**
   - Create validation string based on feature type
   - Example: "Check that agent_merge syntax and variables are documented"
   - Include references to relevant structs or configs

6. **Create Chapter Definition Structure:**
```json
{
  "id": "<chapter-id>",
  "title": "<chapter-title>",
  "file": "<file-path>",
  "topics": ["<topic-1>", "<topic-2>", ...],
  "validation": "<validation-criteria>",
  "auto_generated": true,
  "source_feature": "<feature-category>"
}
```

### Phase 4a: Generate Subsection Definitions for Missing Subsections

**For Each Medium Severity Gap (Missing Subsection in Existing Chapter):**

When a gap can be filled by adding a subsection to an existing multi-subsection chapter:

**STEP 0: Validate Subsection Necessity (MANDATORY)**

**CRITICAL: Do not create subsections for features with insufficient content.**

**CRITICAL: Do not fragment well-written existing single-file chapters.**

Before creating any subsection definition, perform content availability validation:

**STEP 0-PRECHECK: Check if Existing Single-File Chapter Should Be Preserved**

```bash
# Before proposing subsections for a chapter, check if it already exists as a complete single file
EXISTING_FILE="${BOOK_SRC}/${chapter_id}.md"

if [ -f "$EXISTING_FILE" ]; then
  EXISTING_LINES=$(wc -l < "$EXISTING_FILE")
  EXISTING_H2_COUNT=$(grep -c '^## ' "$EXISTING_FILE" || echo 0)

  # Heuristic: If chapter is moderate size and well-organized, KEEP IT SINGLE-FILE
  if [ "$EXISTING_LINES" -lt 1000 ] && [ "$EXISTING_H2_COUNT" -lt 10 ]; then
    echo "✓ Chapter ${chapter_id} is well-structured as single file"
    echo "  - ${EXISTING_LINES} lines of content"
    echo "  - ${EXISTING_H2_COUNT} major sections"
    echo "✓ PRESERVING single-file structure - will NOT create subsections"

    # Skip subsection creation for this chapter entirely
    SKIP_SUBSECTION_CREATION=true
    continue
  fi

  # If chapter is large and complex, subsections may help
  if [ "$EXISTING_LINES" -ge 1000 ] || [ "$EXISTING_H2_COUNT" -ge 10 ]; then
    echo "ℹ Chapter ${chapter_id} is large (${EXISTING_LINES} lines, ${EXISTING_H2_COUNT} sections)"
    echo "ℹ Subsections may improve readability - proceeding with validation"
  fi
fi
```

**Preservation Guidelines:**

- **ALWAYS preserve single-file chapters < 1000 lines**
- **NEVER create subsections just for the sake of organization**
- **Only create subsections when they genuinely improve navigation for large chapters**
- **Remember: The original flat structure at https://iepathos.github.io/prodigy/ worked well for smaller chapters**

**STEP 0a: Discover Codebase Structure**

```bash
# Discover test locations (common patterns)
TEST_DIRS=$(find . -type d -name "*test*" -o -name "*spec*" | grep -v node_modules | grep -v .git | head -5)

# Discover example/workflow/config locations
EXAMPLE_DIRS=$(find . -type d -name "*example*" -o -name "*workflow*" -o -name "*sample*" -o -name "*config*" | grep -v node_modules | grep -v .git | head -5)

# Discover primary source locations
SOURCE_DIRS=$(find . -type f \( -name "*.rs" -o -name "*.py" -o -name "*.js" -o -name "*.ts" -o -name "*.go" -o -name "*.java" \) | sed 's|/[^/]*$||' | sort -u | grep -v node_modules | grep -v .git | head -10)
```

**STEP 0b: Count Potential Content Sources (Language-Agnostic)**

```bash
# 1. Count potential content sources in codebase
FEATURE_CATEGORY="<feature-category-name>"

# Type definitions (struct, class, interface, enum, type)
TYPE_COUNT=$(rg "(struct|class|interface|type|enum).*${FEATURE_CATEGORY}" --hidden --iglob '!.git' --iglob '!node_modules' -c | awk '{s+=$1} END {print s}')

# Function/method definitions
FUNCTION_COUNT=$(rg "(fn|function|def|func|public|private).*${FEATURE_CATEGORY}" --hidden --iglob '!.git' --iglob '!node_modules' -c | awk '{s+=$1} END {print s}')

# Test mentions (search in discovered test directories)
TEST_COUNT=0
for test_dir in $TEST_DIRS; do
  count=$(rg "${FEATURE_CATEGORY}" "$test_dir" --hidden -c 2>/dev/null | awk '{s+=$1} END {print s}')
  TEST_COUNT=$((TEST_COUNT + count))
done

# Example/config file mentions (search in discovered example directories)
EXAMPLE_COUNT=0
for example_dir in $EXAMPLE_DIRS; do
  count=$(rg "${FEATURE_CATEGORY}" "$example_dir" --hidden -c 2>/dev/null | awk '{s+=$1} END {print s}')
  EXAMPLE_COUNT=$((EXAMPLE_COUNT + count))
done

TOTAL_MENTIONS=$((TYPE_COUNT + FUNCTION_COUNT + TEST_COUNT + EXAMPLE_COUNT))

# 2. Estimate potential content lines
# Rule of thumb: Each type = ~30 lines docs, each function = ~10 lines, each example = ~40 lines
ESTIMATED_LINES=$((TYPE_COUNT * 30 + FUNCTION_COUNT * 10 + EXAMPLE_COUNT * 40 + TEST_COUNT * 15))
```

**Content Sufficiency Thresholds:**

**MUST HAVE (to create subsection):**
- `TOTAL_MENTIONS >= 5` - Feature mentioned in at least 5 places in codebase
- `ESTIMATED_LINES >= 50` - Can generate at least 50 lines of grounded documentation
- At least ONE of:
  - `TYPE_COUNT >= 1` (has configuration type/struct/class)
  - `EXAMPLE_COUNT >= 1` (has real example/config file)

**SHOULD HAVE (for good subsection):**
- `TOTAL_MENTIONS >= 10`
- `ESTIMATED_LINES >= 100`
- `TYPE_COUNT >= 1 AND EXAMPLE_COUNT >= 1` (both type definition and example)

**Decision Tree:**

**If TOTAL_MENTIONS < 5 OR ESTIMATED_LINES < 50:**
- **DO NOT create subsection**
- **Instead:** Add note to gap report:
  ```json
  {
    "action": "skipped_subsection_creation",
    "feature_category": "${FEATURE_CATEGORY}",
    "reason": "insufficient_content",
    "details": {
      "total_mentions": TOTAL_MENTIONS,
      "estimated_lines": ESTIMATED_LINES,
      "recommendation": "Merge into parent chapter section or mark as planned feature"
    }
  }
  ```
- **Alternative:** Add as a section within parent chapter's index.md, not separate subsection
- **Log warning:** "⚠ Skipping subsection '${SUBSECTION_TITLE}': only ${TOTAL_MENTIONS} mentions, ${ESTIMATED_LINES} estimated lines"

**If TOTAL_MENTIONS >= 5 AND ESTIMATED_LINES >= 50 BUT < 100:**
- Create subsection with "MINIMAL" flag
- Add to subsection metadata:
  ```json
  {
    "content_warning": "minimal",
    "estimated_lines": ESTIMATED_LINES,
    "total_mentions": TOTAL_MENTIONS
  }
  ```
- This signals to fix phase that limited content is expected

**If TOTAL_MENTIONS >= 10 AND ESTIMATED_LINES >= 100:**
- Proceed with full subsection creation (continue to Step 1 below)

**Special Case: "Best Practices", "Troubleshooting", "Examples" Subsections**

These meta-subsections have different validation:

```bash
# For "best-practices" subsection (search across codebase)
BEST_PRACTICE_COUNT=$(rg "best.practice|pattern|guideline" --hidden --iglob '!.git' --iglob '!node_modules' -i -c | awk '{s+=$1} END {print s}')

# For "troubleshooting" subsection
ERROR_COUNT=$(rg "error|warn|fail" --hidden --iglob '!.git' --iglob '!node_modules' -c | awk '{s+=$1} END {print s}')
ISSUE_COUNT=$(rg "TODO|FIXME|XXX" --hidden --iglob '!.git' --iglob '!node_modules' -c | awk '{s+=$1} END {print s}')

# For "examples" subsection (search discovered example directories)
EXAMPLE_FILE_COUNT=0
for example_dir in $EXAMPLE_DIRS; do
  count=$(find "$example_dir" -type f \( -name "*.yml" -o -name "*.yaml" -o -name "*.json" -o -name "*.toml" \) 2>/dev/null | wc -l)
  EXAMPLE_FILE_COUNT=$((EXAMPLE_FILE_COUNT + count))
done
```

**Requirements for meta-subsections:**
- **Best Practices:** `BEST_PRACTICE_COUNT >= 3` OR documented patterns in code
- **Troubleshooting:** `ERROR_COUNT >= 10` OR `ISSUE_COUNT >= 5`
- **Examples:** `EXAMPLE_FILE_COUNT >= 2` real example/config files

**If meta-subsection doesn't meet threshold:**
- **DO NOT create separate subsection**
- **Instead:** Add brief section to parent chapter's index.md
- **Example:** "## Best Practices" section with 3-5 bullets, not separate file

1. **Identify Target Chapter:**
   - Determine which existing chapter should contain this subsection
   - Check if chapter is already `type: "multi-subsection"`
   - If chapter is `type: "single-file"`, consider migration (see Phase 10)

2. **Generate Subsection ID:**
   - Convert feature category to kebab-case
   - Example: "performance_tuning" → "performance-tuning"
   - Example: "agent_isolation" → "agent-isolation"
   - Ensure uniqueness within chapter's subsections

3. **Generate Subsection Title:**
   - Convert to title case with spaces
   - Example: "performance_tuning" → "Performance Tuning"
   - Example: "agent_isolation" → "Agent Isolation"

4. **Determine Subsection File Path:**
   - Use pattern: `${book_src}/${parent_chapter_id}/${subsection_id}.md`
   - Example: "book/src/mapreduce/performance-tuning.md"
   - Ensure parent directory exists (it should if chapter is multi-subsection)

5. **Extract Topics from Features:**
   - Look at feature capabilities in features.json
   - Convert to topic names relevant to subsection
   - Example: For "performance_tuning" with capabilities ["parallelism", "resource_limits"]
   - Topics: ["parallel execution", "resource management", "performance optimization"]

6. **Define Feature Mapping:**
   - List specific feature paths this subsection should document
   - Example: `["mapreduce.performance", "mapreduce.resource_limits"]`
   - This enables focused drift detection

7. **Define Validation Criteria:**
   - Create validation string based on subsection focus
   - Example: "Check performance tuning options and best practices documented"
   - Include references to relevant configuration

8. **Create Subsection Definition Structure:**
```json
{
  "id": "<subsection-id>",
  "title": "<subsection-title>",
  "file": "<subsection-file-path>",
  "topics": ["<topic-1>", "<topic-2>", ...],
  "validation": "<validation-criteria>",
  "feature_mapping": ["<feature-path-1>", "<feature-path-2>", ...],
  "auto_generated": true,
  "source_feature": "<feature-category>",
  "content_estimate": {
    "estimated_lines": ESTIMATED_LINES,
    "total_mentions": TOTAL_MENTIONS,
    "type_count": TYPE_COUNT,
    "example_count": EXAMPLE_COUNT,
    "test_count": TEST_COUNT
  }
}
```

### Phase 5: Update Chapter Definitions File and Generate Flattened Output

**Read Existing Chapters:**
Load the current contents of the chapters JSON file (e.g., workflows/data/prodigy-chapters.json)

**For New Chapters:**

**Check for Duplicates:**
- Verify the chapter ID doesn't already exist
- Check that the file path isn't already in use
- Normalize and compare titles to avoid near-duplicates

**Append New Chapters:**
- Add new chapter definitions to the chapters array

**Record Action:**
```json
{
  "action": "created_chapter_definition",
  "chapter_id": "<chapter-id>",
  "file_path": "workflows/data/prodigy-chapters.json"
}
```

**For New Subsections:**

**Find Target Chapter:**
- Locate the parent chapter by ID in chapters array
- Verify chapter type is "multi-subsection"
- If chapter is "single-file", log warning and skip (requires migration first)

**Check for Duplicate Subsections:**
- Check if subsection ID already exists in chapter's subsections array
- Verify file path is unique within chapter
- Compare titles to avoid near-duplicates

**Append Subsection to Chapter:**
- Add subsection definition to chapter's subsections array
- Maintain array order (alphabetical or logical)

**Record Action:**
```json
{
  "action": "created_subsection_definition",
  "chapter_id": "<parent-chapter-id>",
  "subsection_id": "<subsection-id>",
  "file_path": "workflows/data/prodigy-chapters.json"
}
```

**Write Updated Chapter Definitions:**
Write the complete chapters JSON back to disk with proper formatting (if any gaps were found):
- Use 2-space indentation
- Maintain JSON structure
- Preserve existing chapters and subsections
- Keep subsection order within chapters

**Note**: The flattened-items.json generation has moved to Phase 8 to ensure it always executes.

### Phase 6: Create Stub Markdown Files

**For Each New Chapter and Subsection:**

**For New Chapters:**

1. **Determine Stub Content:**
   Generate markdown following this template structure:

```markdown
# {Chapter Title}

{Brief introduction explaining the purpose of this feature/capability}

## Overview

{High-level description of what this feature enables}

## Configuration

{If applicable, configuration options and syntax}

```yaml
# Example configuration
```

## Use Cases

### {Use Case 1 Name}

{Description and example}

## Examples

### Basic Example

```yaml
# Example workflow
```

## Best Practices

- {Best practice 1}
- {Best practice 2}

## Troubleshooting

### Common Issues

**Issue**: {Common problem}
**Solution**: {How to fix}

## See Also

- [Related documentation](link)
```

2. **Customize Content for Feature:**
   - Use chapter title from definition
   - Reference the feature category from features.json
   - Include relevant configuration examples
   - Add placeholders for sections

3. **Create File:**
   - Write stub markdown to the file path defined in chapter definition
   - Ensure directory exists (book/src should already exist)
   - Use proper markdown formatting

4. **Validate Markdown:**
   - Ensure the file is valid markdown
   - Check that it won't break mdbook build
   - Verify all syntax is correct

5. **Record Action:**
```json
{
  "action": "created_stub_file",
  "file_path": "<file-path>",
  "type": "chapter"
}
```

**For New Subsections:**

1. **Determine Stub Content:**
   Generate markdown following this subsection template:

```markdown
# {Subsection Title}

{Brief introduction explaining this specific aspect of the parent chapter}

## Overview

{Focused description of what this subsection covers within the chapter context}

## Configuration

{If applicable, specific configuration options for this feature}

```yaml
# Example configuration
```

## Usage

### Basic Usage

{Simple example demonstrating the core functionality}

```yaml
# Example
```

### Advanced Usage

{More complex examples if applicable}

## Best Practices

- {Best practice 1}
- {Best practice 2}
- {Best practice 3}

## Common Issues

**Issue**: {Common problem specific to this subsection}
**Solution**: {How to fix}

## Related Subsections

- [Related Subsection 1](../related-subsection.md)
- [Related Subsection 2](../another-subsection.md)
```

2. **Customize Content for Subsection:**
   - Use subsection title from definition
   - Reference feature_mapping features from features.json
   - Include subsection-specific topics
   - Add cross-references to related subsections
   - Keep content focused on subsection scope

3. **Create Subsection File:**
   - Write stub markdown to subsection file path
   - Ensure parent chapter directory exists (e.g., book/src/mapreduce/)
   - Use proper markdown formatting

4. **Validate Markdown:**
   - Ensure valid markdown syntax
   - Check won't break mdbook build
   - Verify cross-references use correct paths

5. **Record Action:**
```json
{
  "action": "created_stub_file",
  "file_path": "<subsection-file-path>",
  "type": "subsection",
  "parent_chapter_id": "<parent-chapter-id>"
}
```

### Phase 7: Update SUMMARY.md

**Read Current SUMMARY.md:**
Load the book's SUMMARY.md file to understand structure

**Parse Structure:**
Identify sections:
- Introduction (always at top)
- User Guide (basic features)
- Advanced Topics (complex features)
- Reference (examples, troubleshooting)

**For New Chapters:**

1. **Classify New Chapters:**
   - Basic workflow features → User Guide
   - Advanced features (retry, error handling, composition) → Advanced Topics
   - Examples and troubleshooting → Reference

2. **Determine Insertion Point:**
   - Maintain alphabetical order by title
   - Or maintain logical order based on dependencies
   - Insert after similar topics

3. **Insert Chapter Entries:**
   Add entries in markdown list format:
   ```markdown
   - [Chapter Title](chapter-file.md)
   ```

**For New Subsections:**

1. **Locate Parent Chapter:**
   - Find the parent chapter entry in SUMMARY.md
   - Check if chapter already has nested subsections

2. **Add Subsection as Nested List Item:**
   ```markdown
   - [Parent Chapter](parent/index.md)
     - [Subsection 1](parent/subsection-1.md)
     - [New Subsection](parent/new-subsection.md)
   ```

3. **Maintain Subsection Order:**
   - Keep alphabetical or logical ordering within chapter
   - Ensure indentation is correct (2-4 spaces)
   - Follow existing subsection format in SUMMARY.md

**Write Updated SUMMARY.md:**
Write the modified SUMMARY.md back to disk

**Record Action:**
```json
{
  "action": "updated_summary",
  "file_path": "book/src/SUMMARY.md",
  "items_added": [
    {"type": "chapter", "id": "..."},
    {"type": "subsection", "parent": "...", "id": "..."}
  ]
}
```

### Phase 7.5: Validate and Sync Chapter Structure with Reality (MANDATORY)

**CRITICAL: Ensure chapters.json accurately reflects the actual file structure.**

Before generating flattened-items.json, validate that all chapter definitions match reality.

**Step 1: Scan for Multi-Subsection Directories**

1. Find all directories under `${BOOK_DIR}/src/` that contain an `index.md` file
2. For each directory found:
   - Count how many `.md` files exist (excluding `index.md`)
   - If count > 0, this is a multi-subsection chapter
   - Record the chapter ID (directory name) in a list of discovered multi-subsection chapters

**Step 2: Compare Against chapters.json Definitions**

For each discovered multi-subsection chapter:
1. Look up how it's defined in `$CHAPTERS_FILE`
2. Check if `type` field is "multi-subsection" or "single-file"
3. **If type is "single-file" or missing**: This is a MISMATCH - add to mismatches list
4. **If type is "multi-subsection"**: Count subsections in chapters.json and compare to actual file count
   - If counts don't match, add to mismatches list

**Step 3: Check for Orphaned Single-File Definitions**

For each chapter in chapters.json that has `type: "single-file"`:
1. Check if the expected file (e.g., `book/src/chapter-id.md`) exists
2. Check if a directory with that name exists (e.g., `book/src/chapter-id/`)
3. **If file doesn't exist but directory does**: Add to mismatches list

**Step 4: Auto-Migrate Mismatched Chapters**

For each chapter in the mismatches list:
1. Scan the chapter directory to discover all subsection files
2. For each subsection `.md` file (excluding `index.md`):
   - Extract subsection ID from filename (remove `.md` extension)
   - Read the file and extract title from first H1 or H2 heading
   - If no heading found, convert filename to Title Case
   - Extract topics from section headings (H2/H3) in the file
   - Create subsection definition object with: id, title, file path, topics, validation
3. Build complete subsections array from all discovered subsections
4. Update the chapter in chapters.json:
   - Change `type` to "multi-subsection"
   - Change `file` to `index_file` pointing to `book/src/{chapter-id}/index.md`
   - Add `subsections` array with all discovered subsections
   - Preserve existing `topics` and `validation` fields
5. Write updated chapters.json to disk
6. Record this migration in MIGRATION_ACTIONS list

**Step 5: Add Structure Validation to Gap Report**

Add a `structure_validation` section to the gap report JSON:
- `mismatches_found`: Count of chapters that needed migration
- `mismatched_chapters`: Array of chapter IDs that were migrated
- `migrations_performed`: Array of migration actions taken
- `validation_timestamp`: Current timestamp

**Step 6: Commit Structure Fixes (if any)**

If any structure mismatches were found and fixed:
1. Stage the updated chapters.json file
2. Create a commit with message:
   - Title: "docs: sync chapters.json with actual file structure"
   - Body: List of migrated chapters and explanation
3. This commit must happen BEFORE generating flattened-items.json

### Phase 8: Save Gap Report, Generate Flattened Items, and Commit Changes

**STEP 1: Generate Flattened Items for Map Phase (MANDATORY)**

This step MUST execute regardless of whether gaps were found:

1. Read the chapters file from `--chapters` parameter
2. Process each chapter to create flattened array:
   - For `type == "multi-subsection"`: Extract each subsection with parent metadata
   - For `type == "single-file"`: Include chapter with type marker
3. Write to `.prodigy/book-analysis/flattened-items.json`

Example structure:
```json
[
  {
    "id": "workflow-basics",
    "title": "Workflow Basics",
    "file": "book/src/workflow-basics.md",
    "topics": [...],
    "validation": "...",
    "type": "single-file"
  },
  {
    "id": "checkpoint-and-resume",
    "title": "Checkpoint and Resume",
    "file": "book/src/mapreduce/checkpoint-and-resume.md",
    "parent_chapter_id": "mapreduce",
    "parent_chapter_title": "MapReduce Workflows",
    "type": "subsection",
    "topics": [...],
    "feature_mapping": [...]
  }
]
```

**STEP 2: Write Gap Report**

Save the gap report to disk for auditing:
- Path: `.prodigy/book-analysis/gap-report.json` (or equivalent for project)
- Include all gaps found and actions taken
- Use proper JSON formatting

**STEP 3: Display Summary to User**

Print a formatted summary:
```
📊 Documentation Gap Analysis
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Features Analyzed: {count}
Documented Topics: {count}
Gaps Found: {count}

🔴 High Severity Gaps (Missing Chapters):
  • {feature_category} - {description}

🟡 Medium Severity Gaps (Incomplete Chapters):
  • {chapter_id} - Missing: {missing_topics}

✅ Actions Taken:
  ✓ Generated flattened-items.json for map phase
  ✓ Created {count} chapter definitions (if gaps found)
  ✓ Created {count} stub files (if gaps found)
  ✓ Updated SUMMARY.md (if gaps found)

📝 Next Steps:
  The map phase will now process these chapters to detect drift.
```

**Stage and Commit Changes:**

**CRITICAL**: The flattened-items.json file MUST be committed, as the map phase depends on it.

If running in automation mode (PRODIGY_AUTOMATION=true):

**If gaps were found:**
1. Stage all modified files:
   - Updated prodigy-chapters.json
   - New stub markdown files
   - Updated SUMMARY.md
   - Gap report
   - **flattened-items.json (REQUIRED)**

2. Create commit with message:
   - Format: "docs: auto-discover missing chapters for [feature names]"
   - Example: "docs: auto-discover missing chapters for agent-merge-workflows, circuit-breaker"
   - Include brief summary of actions taken

**If NO gaps were found (still need to commit flattened-items.json):**
1. Stage generated files:
   - flattened-items.json (REQUIRED for map phase)
   - Gap report (shows 0 gaps found)

2. Create commit with message:
   - Format: "docs: regenerate flattened-items.json for drift detection (no gaps found)"
   - Include count of chapters/subsections to be processed

### Phase 9: Validation and Quality Checks

**Verify No False Positives:**
- Check that no duplicate chapters were created
- Ensure existing chapters weren't unnecessarily modified
- Verify chapter IDs are unique

**Verify No False Negatives:**
- Check that all obvious undocumented features were detected
- Compare feature areas against documented topics
- Ensure classification (high/medium/low) is appropriate

**Test Book Build:**
Run mdbook build to ensure:
- All new stub files are valid markdown
- SUMMARY.md references are correct
- No broken links created
- Book compiles successfully

If build fails:
- Identify the issue
- Fix the problematic file(s)
- Re-run build validation

### Phase 10: Idempotence Check

**Design for Repeated Execution:**
Gap detection should be idempotent:
- Running it multiple times should not create duplicates
- Already-created chapters should be recognized
- No unnecessary modifications

**Implementation:**
- Always check for existing chapters before creating new ones
- Use normalized comparison for topic matching
- Skip chapters that already exist with the same ID
- Only create chapters for truly missing features

**Validation:**
If gap detection runs and finds no gaps:
- Print message: "✅ No documentation gaps found - all features are documented"
- Do not modify chapter definitions file
- **IMPORTANT**: Still generate flattened-items.json from existing chapters for map phase
- Exit successfully

**CRITICAL**: The flattened-items.json file must ALWAYS be generated, even when no gaps are found. This file is required by the map phase to process all chapters for drift detection. Generate it from the existing chapters.json file in Phase 5, regardless of whether gaps were detected.

### Error Handling

**Handle Missing Files Gracefully:**
- If features.json doesn't exist → error: "Feature analysis must run first"
- If chapters.json doesn't exist → create empty structure
- If SUMMARY.md doesn't exist → error: "Book structure missing"
- If config file missing → use sensible defaults

**Handle Invalid JSON:**
- Validate JSON structure before parsing
- Provide clear error messages for malformed files
- Don't proceed with gap detection if data is invalid

**Handle File Write Failures:**
- Check if book/src directory exists and is writable
- Verify permissions before writing files
- Roll back changes if commits fail

**Record Failures:**
Include in gap report if any steps fail:
```json
{
  "errors": [
    {
      "phase": "stub_creation",
      "error": "Failed to write file: permission denied",
      "file_path": "book/src/agent-merge-workflows.md"
    }
  ]
}
```

### Quality Guidelines

**Accuracy:**
- Minimize false positives (no duplicate chapters)
- Minimize false negatives (catch all undocumented features)
- Use fuzzy matching for topic comparison
- Consider synonyms and variations

**User Experience:**
- Provide clear, actionable output
- Show progress during analysis
- Summarize actions taken
- Explain what will happen next

**Maintainability:**
- Use configurable thresholds for gap classification
- Support customization via book-config.json
- Make template structure configurable
- Keep logic modular and testable

**Performance:**
- Complete analysis in <30 seconds for typical projects
- Minimize file I/O operations
- Cache parsed markdown content
- Process chapters in parallel if needed

### Configuration Options (Future Enhancement)

The book-config.json could support gap detection settings:

```json
{
  "gap_detection": {
    "enabled": true,
    "min_severity": "medium",
    "auto_create_stubs": true,
    "template_path": "workflows/data/stub-template.md",
    "similarity_threshold": 0.8,
    "dry_run": false
  }
}
```

For now, use sensible defaults:
- enabled: true
- min_severity: "high"
- auto_create_stubs: true
- similarity_threshold: 0.7 (fuzzy matching threshold)

### Success Indicators

Gap detection is successful when:
- All undocumented features are identified
- New chapter definitions are valid and complete
- Stub markdown files are properly formatted
- SUMMARY.md structure is maintained
- Book builds without errors
- No duplicate chapters created
- Changes are committed cleanly
- **`.prodigy/book-analysis/flattened-items.json` file is created** (REQUIRED)

### Output Format

The command should output progress and results clearly:

**During Execution:**
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

📝 Creating missing chapters...
   ✓ Generated definition: agent-merge-workflows
   ✓ Created stub: book/src/agent-merge-workflows.md
   ✓ Generated definition: circuit-breaker
   ✓ Created stub: book/src/circuit-breaker.md
   ✓ Updated SUMMARY.md

💾 Committing changes...
   ✓ Staged 4 files
   ✓ Committed: docs: auto-discover missing chapters for agent-merge-workflows, circuit-breaker
```

**Final Summary:**
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
  ✓ Committed changes

📝 Next Steps:
  The map phase will now process these new chapters to populate content.
  Review the generated stubs and customize as needed.
```

## FINAL CHECKLIST

Before completing this command, verify:

1. ✅ Gap report saved to `.prodigy/book-analysis/gap-report.json`
2. ✅ **`.prodigy/book-analysis/flattened-items.json` created (MANDATORY - even if no gaps found)**
3. ✅ Chapter definitions updated (if gaps found)
4. ✅ Stub files created (if gaps found)
5. ✅ SUMMARY.md updated (if gaps found)
6. ✅ Changes committed (if any files modified)

**CRITICAL**: Step 2 (flattened-items.json) is REQUIRED for the workflow to proceed to the map phase. This file must contain all chapters and subsections in a flat array format, ready for parallel processing.
