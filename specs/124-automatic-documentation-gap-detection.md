---
number: 124
title: Automatic Documentation Gap Detection
category: testing
priority: high
status: draft
dependencies: []
created: 2025-10-11
---

# Specification 124: Automatic Documentation Gap Detection

**Category**: testing
**Priority**: high
**Status**: draft
**Dependencies**: None

## Context

The current book documentation workflow (`workflows/book-docs-drift.yml`) successfully detects and fixes drift in **existing** documentation chapters. However, it has a critical limitation: it only processes chapters that are already defined in `workflows/data/prodigy-chapters.json`. This means:

❌ **Missing capabilities**: New features added to the codebase are not discovered automatically
❌ **Undocumented chapters**: If a feature has no corresponding chapter definition, it won't be documented
❌ **Manual maintenance**: Users must manually update `prodigy-chapters.json` and create stub markdown files
❌ **Incomplete coverage**: Documentation lags behind implementation without visibility

The workflow currently focuses on **drift detection** (updating existing docs) but lacks **gap detection** (finding missing docs).

### Current Workflow Limitations

**Setup Phase** (workflows/book-docs-drift.yml:24-28):
```yaml
setup:
  - shell: "mkdir -p $ANALYSIS_DIR"
  - claude: "/prodigy-analyze-features-for-book --project $PROJECT_NAME --config $PROJECT_CONFIG"
```

The setup phase analyzes the codebase and creates `features.json` with all discovered capabilities, but it doesn't compare this against the existing book structure.

**Map Phase** (workflows/book-docs-drift.yml:31-44):
```yaml
map:
  input: "${CHAPTERS_FILE}"  # Only processes existing chapters
  json_path: "$.chapters[*]"

  agent_template:
    - claude: "/prodigy-analyze-book-chapter-drift ..."
    - claude: "/prodigy-fix-chapter-drift ..."
```

The map phase only iterates over chapters already defined in `prodigy-chapters.json`, so new features never get documented.

### Real-World Impact

**Example scenario**:
1. Developer implements new MapReduce feature: `agent_merge` workflows
2. Code is shipped and working
3. `features.json` correctly identifies the new capability
4. But documentation workflow skips it because no chapter exists for it
5. Users discover the feature by reading code or by accident

This creates a gap between what the system can do and what users know about.

## Objective

Extend the book documentation workflow to automatically detect documentation gaps by:
1. **Analyzing book structure**: Scan existing chapters to identify documented topics
2. **Comparing against features**: Match discovered features from `features.json` against documented topics
3. **Identifying gaps**: Find features/capabilities that lack documentation
4. **Creating chapter definitions**: Generate new entries in `prodigy-chapters.json` for missing topics
5. **Creating stub files**: Initialize empty markdown files in `book/src/` with basic structure
6. **Processing new chapters**: Ensure the map phase processes both existing and newly discovered chapters

This enables the documentation workflow to be fully self-sufficient, discovering and documenting new features without manual intervention.

## Requirements

### Functional Requirements

1. **Book Structure Analysis**
   - Scan `book/src/SUMMARY.md` to extract existing chapter structure
   - Parse each chapter markdown file to identify documented topics
   - Build a map of documented capabilities with their locations
   - Extract section headings and documented features

2. **Gap Detection**
   - Load `features.json` from setup phase analysis
   - Compare feature categories against documented topics
   - Identify features present in code but absent from documentation
   - Categorize gaps by severity (major missing chapter vs minor missing section)
   - Generate gap report with actionable details

3. **Chapter Definition Generation**
   - Create new chapter entries for undocumented features
   - Follow the same structure as existing entries in `prodigy-chapters.json`
   - Generate appropriate chapter IDs, titles, and file paths
   - Define relevant topics and validation criteria
   - Determine logical chapter ordering based on feature relationships

4. **Stub File Creation**
   - Create markdown files in `book/src/` for new chapters
   - Include basic structure: title, introduction, sections
   - Add placeholders for examples and use cases
   - Follow mdBook conventions and existing chapter patterns
   - Ensure files are valid markdown that won't break book build

5. **Workflow Integration**
   - Add gap detection step to setup phase before map begins
   - Update `prodigy-chapters.json` in-place with new chapters
   - Create stub files before map phase processes them
   - Ensure map phase includes newly discovered chapters
   - Commit changes with clear message indicating auto-discovery

### Non-Functional Requirements

1. **Accuracy**: Gap detection must minimize false positives and false negatives
2. **Idempotence**: Running gap detection multiple times should not create duplicates
3. **Performance**: Analysis should complete in reasonable time (<30 seconds for typical project)
4. **Maintainability**: Use configurable thresholds and categorization rules
5. **User Experience**: Provide clear output showing discovered gaps and actions taken
6. **Backward Compatibility**: Existing workflows continue to work without modification

## Acceptance Criteria

- [ ] New slash command `/prodigy-detect-documentation-gaps` implemented
- [ ] Gap detection analyzes `book/src/SUMMARY.md` and all referenced chapter files
- [ ] Comparison logic matches features.json categories against documented topics
- [ ] Gap report generated showing:
  - Missing chapter-level topics (high severity)
  - Missing section-level topics within existing chapters (medium severity)
  - Count of gaps by category
  - Recommendations for chapter structure
- [ ] New chapter definitions added to `prodigy-chapters.json` with proper structure
- [ ] Stub markdown files created in `book/src/` with:
  - Chapter title and introduction
  - Section placeholders based on feature categories
  - Example blocks and use case sections
  - Valid markdown that passes mdbook build
- [ ] Updated `workflows/book-docs-drift.yml` includes gap detection in setup phase
- [ ] Map phase processes both existing and newly created chapters
- [ ] Changes committed with message: "docs: auto-discover missing chapters for [feature names]"
- [ ] No false positives (creating duplicate chapters for existing topics)
- [ ] No false negatives (missing obvious undocumented features)
- [ ] Gap detection works with generic configuration (not Prodigy-specific)
- [ ] Documentation updated explaining gap detection capability
- [ ] Tests verify gap detection logic and chapter generation

## Technical Details

### Implementation Approach

**Phase 1: Create Gap Detection Slash Command**

Implement `.claude/commands/prodigy-detect-documentation-gaps.md` with the following workflow:

1. **Parse Parameters**:
   - `--project`: Project name
   - `--config`: Path to book configuration
   - `--features`: Path to features.json from setup phase
   - `--chapters`: Path to prodigy-chapters.json
   - `--book-dir`: Book directory path

2. **Load Existing Documentation Structure**:
   - Read `book/src/SUMMARY.md` to get chapter list
   - Parse each chapter file to extract topics
   - Build map: `{topic: {chapter_id, file_path, sections}}`

3. **Load Feature Inventory**:
   - Read features.json from setup phase
   - Extract all feature categories and capabilities
   - Build map: `{feature_category: [capabilities]}`

4. **Perform Gap Analysis**:
   - For each feature category in features.json
   - Check if topic exists in documentation map
   - If missing → classify as "missing chapter" gap
   - If exists but capabilities missing → classify as "incomplete chapter" gap
   - Generate structured gap report

5. **Create Chapter Definitions**:
   - For each missing chapter gap
   - Generate chapter definition following prodigy-chapters.json format
   - Determine chapter ID from feature category
   - Create appropriate title from category name
   - Define file path in book/src/
   - List topics from feature capabilities
   - Add validation criteria

6. **Update prodigy-chapters.json**:
   - Read existing chapters
   - Append new chapter definitions
   - Maintain proper JSON structure
   - Write back to file

7. **Create Stub Markdown Files**:
   - For each new chapter
   - Create file at defined path in book/src/
   - Generate structure from template
   - Include sections for each topic
   - Add placeholder content
   - Ensure valid markdown

8. **Update SUMMARY.md**:
   - Read existing SUMMARY.md
   - Determine appropriate insertion point
   - Add entries for new chapters
   - Maintain proper nesting and order
   - Write back to file

**Phase 2: Update Workflow Configuration**

Modify `workflows/book-docs-drift.yml`:

```yaml
setup:
  - shell: "mkdir -p $ANALYSIS_DIR"

  # Step 1: Analyze codebase features
  - claude: "/prodigy-analyze-features-for-book --project $PROJECT_NAME --config $PROJECT_CONFIG"

  # Step 2: Detect documentation gaps and create missing chapters (NEW)
  - claude: "/prodigy-detect-documentation-gaps --project $PROJECT_NAME --config $PROJECT_CONFIG --features $FEATURES_PATH --chapters $CHAPTERS_FILE --book-dir $BOOK_DIR"
    commit_required: true  # Commit new chapters before map phase
```

**Phase 3: Stub Template Design**

Create reusable template for stub markdown files:

```markdown
# {Chapter Title}

{Brief introduction explaining the purpose of this feature/capability}

## Overview

{High-level description of what this feature enables}

## Configuration

{Configuration options and syntax if applicable}

```yaml
# Example configuration
```

## Use Cases

### {Use Case 1 Name}

{Description and example}

### {Use Case 2 Name}

{Description and example}

## Examples

### Basic Example

```yaml
# Example workflow
```

### Advanced Example

```yaml
# More complex example
```

## Best Practices

- {Best practice 1}
- {Best practice 2}

## Troubleshooting

### Common Issues

**Issue**: {Common problem}
**Solution**: {How to fix}

## See Also

- [Related Chapter 1](../path/to/chapter.md)
- [Related Chapter 2](../path/to/chapter.md)
```

### Architecture Changes

**New Files**:
- `.claude/commands/prodigy-detect-documentation-gaps.md` - New slash command
- `workflows/data/stub-chapter-template.md` (optional) - Template for consistency

**Modified Files**:
- `workflows/book-docs-drift.yml` - Add gap detection step to setup phase
- `workflows/data/prodigy-chapters.json` - Updated automatically by gap detection
- `book/src/SUMMARY.md` - Updated automatically with new chapters
- `book/src/*.md` - New stub files created as needed

**No Code Changes**: This is entirely workflow and command-based implementation.

### Data Structures

**Gap Report Structure** (generated during analysis):

```json
{
  "analysis_date": "2025-10-11T10:30:00Z",
  "gaps_found": 3,
  "gaps": [
    {
      "severity": "high",
      "type": "missing_chapter",
      "feature_category": "agent_merge",
      "feature_description": "Custom merge workflows for map agents",
      "recommended_chapter_id": "agent-merge-workflows",
      "recommended_title": "Agent Merge Workflows",
      "recommended_location": "book/src/agent-merge-workflows.md"
    },
    {
      "severity": "medium",
      "type": "incomplete_chapter",
      "feature_category": "retry_config",
      "existing_chapter_id": "retry-configuration",
      "missing_topics": ["jitter", "retry_budget"],
      "recommendation": "Add sections for missing retry capabilities"
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
      "file_path": "book/src/agent-merge-workflows.md"
    },
    {
      "action": "updated_summary",
      "file_path": "book/src/SUMMARY.md"
    }
  ]
}
```

**Chapter Definition Structure** (added to prodigy-chapters.json):

```json
{
  "id": "agent-merge-workflows",
  "title": "Agent Merge Workflows",
  "file": "book/src/agent-merge-workflows.md",
  "topics": [
    "agent_merge configuration",
    "Merge validation",
    "Agent-specific variables",
    "Error handling in merges"
  ],
  "validation": "Check that agent_merge syntax and variables are documented",
  "auto_generated": true,
  "source_feature": "agent_merge"
}
```

### APIs and Interfaces

**Slash Command Interface**:

```bash
/prodigy-detect-documentation-gaps \
  --project "Prodigy" \
  --config ".prodigy/book-config.json" \
  --features ".prodigy/book-analysis/features.json" \
  --chapters "workflows/data/prodigy-chapters.json" \
  --book-dir "book"
```

**Gap Detection Output** (to terminal):

```
📊 Documentation Gap Analysis
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Features Analyzed: 12
Documented Topics: 9
Gaps Found: 3

🔴 High Severity Gaps (Missing Chapters):
  • agent_merge - Custom merge workflows for map agents
  • circuit_breaker - Workflow error circuit breaking

🟡 Medium Severity Gaps (Incomplete Chapters):
  • retry-configuration - Missing: jitter, retry_budget

✅ Actions Taken:
  ✓ Created chapter definition: agent-merge-workflows
  ✓ Created stub file: book/src/agent-merge-workflows.md
  ✓ Updated SUMMARY.md with new chapter
  ✓ Committed changes

📝 Next Steps:
  The map phase will now process these new chapters to populate content.
```

## Dependencies

**Prerequisites**: None (builds on existing book workflow infrastructure)

**Affected Components**:
- `workflows/book-docs-drift.yml` - Main documentation workflow
- `workflows/data/prodigy-chapters.json` - Chapter definitions
- `book/src/SUMMARY.md` - Book table of contents
- `.claude/commands/prodigy-detect-documentation-gaps.md` - New command

**External Dependencies**:
- `jq` (for JSON manipulation in shell steps)
- `mdbook` (existing, for validation)

## Testing Strategy

### Unit Tests

**Test 1: Gap Detection Logic**
- Given: features.json with 5 feature categories
- Given: Documentation covering 3 of those categories
- When: Gap detection runs
- Then: 2 gaps identified with correct severity

**Test 2: Chapter Definition Generation**
- Given: Missing feature "agent_merge"
- When: Chapter definition created
- Then: Definition follows correct structure
- Then: Contains appropriate topics and validation

**Test 3: Stub File Creation**
- Given: Chapter definition for "agent-merge-workflows"
- When: Stub file created
- Then: File is valid markdown
- Then: mdbook build succeeds
- Then: File contains expected sections

**Test 4: Idempotence**
- Given: Gap detection runs successfully
- When: Gap detection runs again immediately
- Then: No duplicate chapters created
- Then: No modifications to existing files

**Test 5: False Positive Prevention**
- Given: Feature "mapreduce" with existing chapter
- When: Gap detection runs
- Then: No new "mapreduce" chapter created
- Then: Existing chapter recognized

### Integration Tests

**Test 1: Full Workflow Execution**
```bash
# Clean state
rm -rf .prodigy/book-analysis book/src/agent-merge-workflows.md

# Run full documentation workflow
prodigy run workflows/book-docs-drift.yml

# Verify outcomes
test -f .prodigy/book-analysis/gap-report.json
test -f book/src/agent-merge-workflows.md
grep -q "agent-merge-workflows" workflows/data/prodigy-chapters.json
mdbook build book/  # Should succeed
```

**Test 2: New Feature → Documentation**
1. Add new feature to codebase (e.g., new command type)
2. Run documentation workflow
3. Verify new chapter created
4. Verify stub file contains relevant sections
5. Verify map phase processes new chapter
6. Verify final documentation includes new content

**Test 3: Partial Documentation**
1. Create chapter file but don't add to SUMMARY.md
2. Run gap detection
3. Verify SUMMARY.md updated with missing chapter
4. Verify no duplicate chapter created

### User Acceptance

1. **Developer adds new feature**:
   - Implements new MapReduce capability
   - Runs documentation workflow
   - Verifies documentation automatically created
   - Reviews and approves generated stub content

2. **Documentation maintainer**:
   - Reviews gap report output
   - Validates chapter definitions are appropriate
   - Confirms stub files follow conventions
   - Verifies book builds without errors

3. **End user**:
   - Reads generated documentation
   - Finds new features documented
   - Examples and use cases are present
   - No broken links or build errors

## Documentation Requirements

### Code Documentation

**Slash Command Documentation** (`.claude/commands/prodigy-detect-documentation-gaps.md`):
- Comprehensive execute section with step-by-step process
- Examples of gap analysis and chapter generation
- Explanation of severity classification
- Guidance on template customization

### User Documentation

**Update `book/src/automated-documentation.md`**:

Add new section:

```markdown
## Automatic Gap Detection

The documentation workflow now automatically discovers undocumented features
and creates stub chapters for them.

### How It Works

1. **Feature Analysis**: The setup phase analyzes your codebase to identify
   all capabilities and features.

2. **Gap Detection**: Compares discovered features against existing book
   chapters to find documentation gaps.

3. **Stub Generation**: Creates new chapter definitions and markdown files
   for missing topics.

4. **Content Population**: The map phase processes both existing and new
   chapters to populate them with accurate, up-to-date content.

### Gap Severity Levels

- **High Severity**: Missing entire chapters for major features
- **Medium Severity**: Existing chapters missing important sections
- **Low Severity**: Minor features or edge cases not documented

### Customization

You can customize gap detection behavior in `.prodigy/book-config.json`:

```json
{
  "gap_detection": {
    "enabled": true,
    "min_severity": "medium",
    "auto_create_stubs": true,
    "template_path": "workflows/data/stub-template.md"
  }
}
```

### Manual Review

While gap detection is automatic, we recommend:
- Review generated stub files before final merge
- Customize section structure if needed
- Add project-specific examples
- Validate technical accuracy
```

**Update `CLAUDE.md`**:

Add to "Best Practices" section:

```markdown
## Documentation Maintenance

The book documentation workflow (`workflows/book-docs-drift.yml`) now includes:
- **Drift Detection**: Updates existing chapters to match current implementation
- **Gap Detection**: Discovers and documents new features automatically
- **Stub Generation**: Creates initial chapter structure for new topics

This ensures documentation stays complete and accurate as the codebase evolves.
```

### Architecture Updates

No `ARCHITECTURE.md` updates needed - this is workflow infrastructure, not core application architecture.

## Implementation Notes

### Gap Classification Logic

**High Severity** (Missing Chapter):
- Feature category in features.json has no corresponding chapter
- Major capability with no documentation entry
- User-facing feature without guidance

**Medium Severity** (Incomplete Chapter):
- Chapter exists but missing documented sub-capabilities
- Example: Retry chapter exists but doesn't document jitter

**Low Severity** (Minor Gap):
- Edge cases or advanced features
- Internal implementation details exposed as API
- Less common use cases

### Chapter ID Generation

Convert feature category to chapter ID:
- Remove underscores: `agent_merge` → `agent merge`
- Convert to kebab-case: `agent merge` → `agent-merge`
- Append context if needed: `agent-merge` → `agent-merge-workflows`
- Ensure uniqueness against existing IDs

### SUMMARY.md Insertion Logic

Determine where to insert new chapters:
1. Parse SUMMARY.md structure (User Guide, Advanced Topics, Reference)
2. Classify new chapter based on feature complexity
3. Insert in appropriate section
4. Maintain alphabetical order within section

### False Positive Prevention

To avoid creating duplicate chapters:
- Normalize topic names before comparison (lowercase, trim, remove punctuation)
- Check for partial matches (e.g., "mapreduce" matches "MapReduce Workflows")
- Use configurable similarity threshold
- Manual review before final commit

### Template Customization

Users can provide custom stub templates:
- Specify template path in book-config.json
- Template uses mustache-style variables: `{{chapter_title}}`, `{{topics}}`
- Fallback to default template if custom not found

## Migration and Compatibility

### Breaking Changes

**None** - This is an additive feature.

### Compatibility Considerations

- Existing workflows continue to work without gap detection
- Gap detection only runs if new command invoked
- Stub files are valid markdown that won't break builds
- Can disable gap detection via configuration

### Migration Path

**Enabling Gap Detection**:

1. **Update workflow file**:
   ```bash
   # Add gap detection step to workflows/book-docs-drift.yml
   # (see Phase 2 in Implementation Approach)
   ```

2. **Configure behavior** (optional):
   ```json
   // .prodigy/book-config.json
   {
     "gap_detection": {
       "enabled": true,
       "auto_create_stubs": true
     }
   }
   ```

3. **Run workflow**:
   ```bash
   prodigy run workflows/book-docs-drift.yml
   ```

4. **Review generated chapters**:
   ```bash
   git diff workflows/data/prodigy-chapters.json
   git diff book/src/SUMMARY.md
   ls -la book/src/*.md | grep "$(date +%Y-%m-%d)"
   ```

5. **Commit results**:
   ```bash
   git add workflows/data/prodigy-chapters.json book/src/
   git commit -m "docs: auto-discover missing chapters"
   ```

**Disabling Gap Detection**:

Simply remove the gap detection step from the workflow file. Existing chapters remain unaffected.

## Success Metrics

- [ ] Gap detection runs successfully in CI/CD
- [ ] No false positives (duplicate chapters) in 10 test runs
- [ ] New features automatically documented within 1 workflow run
- [ ] Gap detection completes in <30 seconds for Prodigy codebase
- [ ] Generated stubs follow consistent structure
- [ ] Book builds successfully after gap detection
- [ ] User feedback confirms improved documentation coverage
- [ ] Reduced manual chapter creation time (estimate: 80% reduction)

## Future Enhancements

### Out of Scope for This Spec

- **AI-powered content generation**: Automatically filling stub content (currently manual)
- **Cross-project gap detection**: Comparing documentation across related projects
- **Interactive gap review**: CLI tool for reviewing and approving gaps before creation
- **Gap detection metrics**: Dashboard showing documentation coverage over time
- **Semantic gap analysis**: Using LLM to identify conceptual documentation gaps

These features may be added in future specifications as the documentation workflow matures.
