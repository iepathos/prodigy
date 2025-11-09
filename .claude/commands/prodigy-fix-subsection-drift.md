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
