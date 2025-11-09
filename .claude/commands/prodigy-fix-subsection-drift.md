# /prodigy-fix-subsection-drift

Fix documentation drift for a specific chapter or subsection based on its drift analysis report.

This command supports both single-file chapters (backward compatible) and individual subsections within multi-subsection chapters.

## Variables

- `--project <name>` - Project name (e.g., "Prodigy", "Debtmap")
- `--json <item>` - JSON object containing chapter or subsection details
- `--chapter-id <id>` - (Optional) Chapter ID for subsections
- `--subsection-id <id>` - (Optional) Subsection ID if fixing a subsection

## Execute

### Phase 1: Understand Context

You are fixing documentation drift for either a full chapter or a single subsection. The analysis phase has already created a drift report. Your job is to:
1. Read the drift report
2. Fix all identified issues
3. Update the documentation file
4. Preserve cross-references (especially important for subsections)
5. Commit the changes

**Important for Subsections:**
- Only update the specific subsection file
- Preserve links to other subsections
- Maintain subsection scope and focus
- Don't accidentally modify sibling subsections

### Phase 2: Parse Input and Load Drift Report

**Extract Parameters:**
```bash
PROJECT_NAME="<value from --project parameter>"
ITEM_JSON="<value from --json parameter, may be empty>"
CHAPTER_ID="<value from --chapter-id parameter, may be empty>"
SUBSECTION_ID="<value from --subsection-id parameter, may be empty>"
```

**Determine Item Type and IDs:**

If `ITEM_JSON` is provided:
```bash
ITEM_TYPE=$(echo "$ITEM_JSON" | jq -r '.type // "single-file"')
ITEM_ID=$(echo "$ITEM_JSON" | jq -r '.id')

if [ "$ITEM_TYPE" = "subsection" ]; then
  PARENT_CHAPTER_ID=$(echo "$ITEM_JSON" | jq -r '.parent_chapter_id')
  SUBSECTION_ID="$ITEM_ID"
else
  CHAPTER_ID="$ITEM_ID"
fi
```

If using separate parameters:
```bash
if [ -n "$SUBSECTION_ID" ]; then
  ITEM_TYPE="subsection"
  PARENT_CHAPTER_ID="$CHAPTER_ID"
else
  ITEM_TYPE="chapter"
fi
```

**Determine Drift Report Path:**

**For Subsections:**
- Pattern: `.prodigy/book-analysis/drift-${PARENT_CHAPTER_ID}-${SUBSECTION_ID}.json`
- Example: `.prodigy/book-analysis/drift-mapreduce-checkpoint-and-resume.json`

**For Single-File Chapters:**
- Pattern: `.prodigy/book-analysis/drift-${CHAPTER_ID}.json`
- Example: `.prodigy/book-analysis/drift-workflow-basics.json`

**Load Drift Report:**
Read the drift report JSON file to extract:
- `item_file` or `chapter_file` or `subsection_file`: Path to markdown file
- `issues[]`: List of drift issues with fix suggestions
- `severity`: Overall drift severity
- `improvement_suggestions[]`: Additional recommendations
- `cross_references[]`: Related subsections (for subsections)
- `feature_mappings[]`: Scoped features (for subsections)

### Phase 3: Analyze Drift Issues

**Parse Issues:**
For each issue in the drift report:
- Identify section that needs updating
- Understand what content is missing/outdated/incorrect
- Review `fix_suggestion` and `source_reference`
- Check `current_content` vs `should_be` if provided

**Prioritize Fixes:**
1. **Critical severity** - Missing entire sections, completely outdated
2. **High severity** - Major features undocumented, incorrect examples
3. **Medium severity** - Incomplete explanations, minor inaccuracies
4. **Low severity** - Style issues, missing cross-references

### Phase 3.5: Extract Real Examples from Codebase (MANDATORY)

**CRITICAL: ALL documentation content must be grounded in actual codebase implementation.**

**Step 1: Identify What Needs Code Examples**

From the drift report issues, identify what requires code validation:
- Struct definitions and field names
- YAML syntax and configuration options
- CLI command syntax
- Enum variants
- Function signatures
- Workflow examples

**Step 2: Search for Source Definitions**

For each feature being documented, **MANDATORY searches:**

**A. Find Struct Definitions:**
```bash
# Search for the actual struct in source code
rg "struct\s+${StructName}" src/ --type rust

# Extract field names and types
rg "pub\s+\w+:" src/path/to/file.rs -A 5

# Example: For retry configuration
rg "struct\s+RetryConfig" src/ --type rust
rg "pub\s+(max_attempts|backoff|initial_delay):" src/config/ -A 2
```

**B. Find Enum Variants:**
```bash
# Search for enum definitions
rg "enum\s+${EnumName}" src/ --type rust

# Get all variants
rg "^\s+\w+," src/path/to/enum.rs

# Example: For backoff strategies
rg "enum\s+BackoffStrategy" src/ --type rust
```

**C. Find Real Usage in Tests:**
```bash
# Search test files for actual usage
rg "${FeatureName}" tests/ --type rust -A 10

# Example: How RetryConfig is actually constructed
rg "RetryConfig\s*\{" tests/ -A 10
```

**D. Find Real Workflow Examples:**
```bash
# Search existing workflows for real examples
rg "${yaml_field_name}" workflows/ --type yaml -A 5

# Example: Find retry configuration in real workflows
rg "retry:" workflows/ -A 5
rg "max_attempts:" workflows/ -A 2
```

**E. Find Existing Documentation Examples:**
```bash
# Check if other chapters have validated examples
rg "${feature_name}" book/src/ --type md -A 10

# Only reuse these if they reference source code
```

**Step 3: Validate All Examples**

**For YAML Examples:**
```bash
# Check field names exist in struct
# Pattern: For each field in YAML example, verify it exists in source

# Example validation:
# YAML shows:   retry_config:
# Source check: rg "pub retry_config:" src/
# Result:       MUST FIND MATCH or DON'T USE
```

**For Code Examples:**
```bash
# Verify enum variants exist
# Pattern: Check each variant mentioned

# Example:
# Docs show:   backoff: exponential
# Source check: rg "Exponential" src/config/retry.rs
# Result:       MUST MATCH EXACTLY (case-sensitive)
```

**For CLI Commands:**
```bash
# Verify command syntax from help text
# Pattern: Run actual command if possible, or check CLI parser

# Example:
# Docs show:   prodigy run workflow.yml --profile prod
# Source check: rg "profile" src/cli.rs
# Result:       Verify flag exists and format is correct
```

**Step 4: Extract Real Examples**

**Template for Code-Grounded Examples:**
```markdown
## Configuration

The `RetryConfig` struct defines retry behavior (src/config/retry.rs:45):

\`\`\`yaml
retry_config:
  max_attempts: 3           # Maximum retry attempts (default: 3)
  initial_delay_ms: 100     # Initial delay in milliseconds (default: 100)
  backoff: exponential      # Backoff strategy: exponential, linear, fibonacci
  max_delay_ms: 60000       # Maximum delay cap (default: 60000)
\`\`\`

**Source**: Extracted from `RetryConfig` struct in src/config/retry.rs:45-52

**Backoff Strategies** (from src/config/retry.rs:BackoffStrategy enum):
- `exponential` - Delay doubles each retry (2^n * initial_delay)
- `linear` - Delay increases linearly (n * initial_delay)
- `fibonacci` - Delay follows fibonacci sequence

## Real-World Example

From tests/integration/retry_test.rs:78-92:

\`\`\`yaml
name: reliable-workflow
retry_config:
  max_attempts: 5
  initial_delay_ms: 500
  backoff: exponential
  max_delay_ms: 30000
\`\`\`
```

**Step 5: Rules for Content Creation**

**ALWAYS:**
- Include source file references for all examples (e.g., "src/config/retry.rs:45")
- Link to actual test files for real-world examples
- Verify field names match struct definitions exactly
- Verify enum variants match source code exactly (case-sensitive)
- Extract examples from actual workflow files in workflows/
- Note which features are optional vs required based on struct definition

**NEVER:**
- Invent plausible-looking YAML syntax
- Guess field names or types
- Create examples from "common patterns" unless proven in codebase
- Use syntax from other tools or projects
- Assume features exist without verification
- Document features that don't exist in the codebase

**If No Example Exists:**
```markdown
## Usage

This feature is defined in src/path/to/file.rs but no example workflows currently use it.

See the struct definition for available fields:
- [Source Code](../src/path/to/file.rs:line)

**Note**: If you implement a workflow using this feature, please contribute an example!
```

**Step 6: Create Evidence File**

For each subsection/chapter, create a temporary evidence file documenting sources:

```bash
# Create evidence file
cat > .prodigy/book-analysis/evidence-${ITEM_ID}.md <<EOF
# Evidence for ${ITEM_TITLE}

## Source Definitions Found
- RetryConfig struct: src/config/retry.rs:45
- BackoffStrategy enum: src/config/retry.rs:88
- retry_config field: src/config/workflow.rs:123

## Test Examples Found
- tests/integration/retry_test.rs:78 (complete workflow)
- tests/unit/config_test.rs:45 (struct construction)

## Workflow Examples Found
- workflows/prodigy-ci.yml:23 (retry_config usage)

## Documentation References
- book/src/error-handling.md:156 (related concept)

## Validation Results
✓ All YAML fields verified against struct
✓ All enum variants match source
✓ CLI syntax verified against clap definitions
✗ No real-world workflow examples found (using test example instead)
EOF
```

This evidence file helps verify all content is grounded and provides audit trail.

### Phase 4: Fix the Documentation

**Read Current File:**
Read the markdown file from the drift report.

**Apply Fixes Based on Item Type:**

**For Subsections:**

1. **Maintain Subsection Scope:**
   - Only add content related to `feature_mappings`
   - Don't document features outside subsection scope
   - Keep content focused on subsection topics

2. **Preserve Cross-References:**
   - Maintain links to sibling subsections
   - Verify cross-references listed in drift report
   - Add new cross-references if needed
   - Example: Checkpoint subsection links to DLQ subsection

3. **Respect Chapter Context:**
   - Ensure subsection fits within parent chapter
   - Don't duplicate content from other subsections
   - Reference related subsections instead of duplicating

4. **Update Subsection Structure:**
   - Keep consistent heading levels (typically H2 and H3)
   - Maintain standard subsection structure
   - Follow parent chapter organization

**For Single-File Chapters:**

1. **Comprehensive Coverage:**
   - Address all major features in chapter scope
   - Ensure broad topic coverage
   - Include complete feature documentation

2. **Chapter Organization:**
   - Maintain logical flow and structure
   - Keep clear introduction and summary
   - Organize sections appropriately

**Common Fix Patterns (Both Types):**

**Missing Content Issues:**
- Add missing section/content
- Follow fix_suggestion guidance
- Include code examples
- Add cross-references

**Outdated Information Issues:**
- Update outdated content
- Replace old syntax with current
- Update examples to match implementation
- Add version notes if needed

**Incorrect Examples Issues:**
- Fix broken examples
- Verify syntax is correct
- Test examples work with current code
- Add explanatory comments

**Incomplete Explanation Issues:**
- Expand brief explanations
- Add practical examples
- Include use cases
- Link to relevant source code

**Preserve Good Content:**
- Keep content from `positive_aspects`
- Maintain chapter/subsection structure and flow
- Preserve working examples
- Keep helpful diagrams

**Apply Improvement Suggestions:**
- Add cross-references
- Include best practices
- Add troubleshooting tips
- Improve organization if needed

### Phase 5: Quality Checks

**For Subsections:**
- Verify content stays within subsection scope
- Check cross-references to other subsections are valid
- Ensure no duplication with sibling subsections
- Validate subsection fits in chapter context

**For Chapters:**
- Verify comprehensive topic coverage
- Check overall structure is logical
- Ensure proper introduction and conclusion

**General Checks:**
- All critical and high severity issues addressed
- All topics from metadata covered
- Examples are practical and current
- Cross-references are valid
- Content is accurate against source code
- Field names and types are correct
- Examples parse correctly
- CLI commands match current syntax

### Phase 5.5: Validate Minimum Content Requirements (MANDATORY)

**CRITICAL: Subsections and chapters MUST meet minimum quality standards before committing.**

**Step 1: Count Lines and Content**

```bash
# Get actual content line count (excluding blank lines and single-word headers)
LINE_COUNT=$(grep -v '^$' ${ITEM_FILE} | grep -v '^#\s*$' | wc -l)
HEADING_COUNT=$(grep '^##' ${ITEM_FILE} | wc -l)
CODE_BLOCK_COUNT=$(grep '```' ${ITEM_FILE} | wc -l)
```

**Step 2: Minimum Content Thresholds**

**For Subsections:**
- **Minimum 50 lines** of actual content (excluding blank lines)
- **Minimum 3 level-2 headings** (## sections)
- **Minimum 2 code examples** (``` blocks)
- **Minimum 1 source reference** to codebase files

**For Single-File Chapters:**
- **Minimum 100 lines** of actual content
- **Minimum 5 level-2 headings**
- **Minimum 3 code examples**
- **Minimum 2 source references**

**Step 3: Content Completeness Check**

**Verify all drift issues addressed:**
```bash
# Count issues by severity from drift report
CRITICAL_ISSUES=$(jq '.issues[] | select(.severity == "critical")' ${DRIFT_REPORT} | jq -s length)
HIGH_ISSUES=$(jq '.issues[] | select(.severity == "high")' ${DRIFT_REPORT} | jq -s length)

# ALL critical and high severity issues MUST be resolved
# Check that updated file addresses each issue's section
```

**Required sections for subsections:**
- Overview/Introduction (what this subsection covers)
- Configuration or Syntax (if applicable)
- At least one practical example
- Best practices or common patterns (if material exists in codebase)
- Cross-references to related subsections (if applicable)

**Step 4: Validation Decision Tree**

**If content meets ALL thresholds:**
- Proceed to Phase 6 (commit)

**If content is too short (< 50 lines for subsection, < 100 for chapter):**

1. **Check if content genuinely doesn't exist in codebase:**
   ```bash
   # Count how many source files relate to this feature
   SOURCE_FILE_COUNT=$(rg "${feature_name}" src/ tests/ -l | wc -l)

   # If < 3 source files, feature may be too small for subsection
   ```

2. **If feature is genuinely small (<3 source files, <50 lines possible):**
   - Add a prominent note at the top:
   ```markdown
   # ${SUBSECTION_TITLE}

   > **Note**: This feature has minimal implementation. Consider reviewing:
   > - ${PARENT_CHAPTER_ID}/index.md for overview
   > - Source: src/path/to/implementation.rs

   ## Overview

   ${Brief description}

   ## Configuration

   ${Minimal config example from source}

   ## See Also

   - [Related feature](../related.md)
   ```
   - Add warning to commit message: "MINIMAL CONTENT - feature has limited implementation"

3. **If content SHOULD exist but you couldn't find it:**
   - DO NOT COMMIT stub/minimal content
   - Instead, create a TODO file:
   ```bash
   cat > ${ITEM_FILE}.TODO <<EOF
   # TODO: ${SUBSECTION_TITLE}

   This subsection needs substantial content but insufficient material was found in the codebase.

   ## Issues Identified (from drift report)
   $(jq '.issues[] | "- [\(.severity)] \(.description)"' ${DRIFT_REPORT})

   ## Searches Performed
   - Searched src/ for structs: ${SEARCHES_DONE}
   - Searched tests/ for examples: ${TEST_SEARCHES}
   - Searched workflows/ for usage: ${WORKFLOW_SEARCHES}

   ## Next Steps
   1. Verify feature is implemented (check if feature_mapping is correct)
   2. If implemented, search with different keywords
   3. If not implemented, remove subsection or mark as "Planned Feature"
   4. If implemented but undocumented in code, add rustdoc first

   ## Drift Report
   See: ${DRIFT_REPORT}
   EOF
   ```
   - Log error message:
   ```
   ❌ Cannot fix ${ITEM_TYPE} '${ITEM_TITLE}': insufficient content found in codebase

   Created TODO file: ${ITEM_FILE}.TODO

   Possible reasons:
   1. Feature not yet implemented
   2. Feature_mapping in chapter definition is incorrect
   3. Search keywords need adjustment
   4. Feature exists but needs better code documentation

   Recommended action: Review drift report and verify feature exists
   ```
   - EXIT WITHOUT COMMITTING

**Step 5: Example Quality Validation**

For each code example in the updated documentation:

```bash
# Verify example has source attribution
grep -q "Source:" ${ITEM_FILE} || echo "WARNING: Example missing source attribution"

# Verify example references actual files
grep "src/" ${ITEM_FILE} | while read -r source_ref; do
  # Extract file path from reference
  FILE_PATH=$(echo "$source_ref" | grep -oP 'src/[^:)]+')
  if [ -n "$FILE_PATH" ] && [ ! -f "$FILE_PATH" ]; then
    echo "ERROR: Referenced file does not exist: $FILE_PATH"
  fi
done
```

**All examples MUST:**
- Have a source attribution comment (e.g., "Source: src/config/retry.rs:45")
- Reference files that actually exist
- Use field names that exist in source code
- Use enum variants that match source code exactly

**Step 6: Validation Summary**

Create validation summary for commit message:

```bash
cat > .prodigy/book-analysis/validation-${ITEM_ID}.txt <<EOF
# Validation Summary for ${ITEM_TITLE}

## Content Metrics
- Lines of content: ${LINE_COUNT} (minimum: ${MIN_LINES})
- Headings: ${HEADING_COUNT} (minimum: ${MIN_HEADINGS})
- Code examples: ${CODE_BLOCK_COUNT} (minimum: ${MIN_EXAMPLES})
- Source references: ${SOURCE_REF_COUNT} (minimum: ${MIN_SOURCES})

## Drift Issues Resolved
- Critical: ${CRITICAL_FIXED}/${CRITICAL_ISSUES}
- High: ${HIGH_FIXED}/${HIGH_ISSUES}
- Medium: ${MEDIUM_FIXED}/${MEDIUM_ISSUES}
- Low: ${LOW_FIXED}/${LOW_ISSUES}

## Code Validation
- All struct fields verified: ${STRUCT_VALIDATION}
- All enum variants verified: ${ENUM_VALIDATION}
- All examples have source attribution: ${SOURCE_ATTRIBUTION}
- All referenced files exist: ${FILE_EXISTENCE}

## Quality Gates
✓ Meets minimum content requirements
✓ All critical issues resolved
✓ All high severity issues resolved
✓ All examples grounded in codebase
✓ All source references validated

Status: READY TO COMMIT
EOF
```

**If ANY quality gate fails:**
- DO NOT proceed to commit
- Create detailed TODO file explaining what's missing
- Exit with error message showing validation failures

### Phase 6: Commit the Fix

**Write Updated Documentation:**
Use the Edit tool to update the file with all fixes applied.

**Create Descriptive Commit:**

**For Subsections:**
```bash
CRITICAL_COUNT=<count of critical issues>
HIGH_COUNT=<count of high issues>
TOTAL_ISSUES=<total issues fixed>
SUBSECTION_TITLE="<from drift report>"
PARENT_CHAPTER_TITLE="<parent chapter title>"

git add <subsection_file>
git commit -m "docs: fix ${PROJECT_NAME} subsection '${PARENT_CHAPTER_TITLE} > ${SUBSECTION_TITLE}'

Fixed ${TOTAL_ISSUES} drift issues (${CRITICAL_COUNT} critical, ${HIGH_COUNT} high)

Key updates:
- <list 3-5 most important fixes>

Subsection scope: <feature mappings>
Cross-references preserved: <related subsections>"
```

**For Single-File Chapters:**
```bash
CHAPTER_TITLE="<from drift report>"

git add <chapter_file>
git commit -m "docs: fix ${PROJECT_NAME} book chapter '${CHAPTER_TITLE}'

Fixed ${TOTAL_ISSUES} drift issues (${CRITICAL_COUNT} critical, ${HIGH_COUNT} high)

Key updates:
- <list 3-5 most important fixes>

All examples verified against current implementation."
```

### Phase 7: Validation

**The fix should:**
1. Address all critical and high severity issues
2. Update outdated information to match current code
3. Fix all broken examples
4. Add missing content for major features
5. Preserve positive aspects from drift report
6. Include clear, tested examples
7. Be committed with descriptive message
8. Maintain subsection scope (for subsections)
9. Preserve cross-references (for subsections)

**Don't:**
- Skip critical issues due to complexity
- Add speculative content not in codebase
- Break existing working content
- Remove helpful examples or explanations
- Make unrelated changes
- Document features outside subsection scope (for subsections)
- Duplicate content from other subsections

### Phase 8: Summary Output

**For Subsections:**
```
✅ Fixed drift in ${PARENT_CHAPTER_TITLE} > ${SUBSECTION_TITLE}

Issues addressed:
- ${CRITICAL_COUNT} critical
- ${HIGH_COUNT} high
- ${MEDIUM_COUNT} medium
- ${LOW_COUNT} low

Changes:
- <brief summary of major updates>

Subsection updated: ${SUBSECTION_FILE}
Feature scope: ${FEATURE_MAPPINGS}
Cross-references: ${CROSS_REFS}
```

**For Single-File Chapters:**
```
✅ Fixed drift in ${CHAPTER_TITLE}

Issues addressed:
- ${CRITICAL_COUNT} critical
- ${HIGH_COUNT} high
- ${MEDIUM_COUNT} medium
- ${LOW_COUNT} low

Changes:
- <brief summary of major updates>

Chapter updated: ${CHAPTER_FILE}
```

## Notes

### Subsection-Specific Notes
- Each subsection runs in a separate map agent worktree
- Focus only on the assigned subsection
- Don't modify other subsections even if issues noticed
- Preserve all cross-references to sibling subsections
- Maintain subsection boundaries and scope
- Commits merge to parent worktree automatically

### General Notes
- This command runs during the **map phase** in a separate worktree
- Focus on accuracy - verify against source code
- Include practical, copy-paste ready examples
- Cross-reference related documentation
- The reduce phase handles any merge conflicts
