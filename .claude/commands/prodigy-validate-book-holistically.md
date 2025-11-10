# /prodigy-validate-book-holistically

Perform holistic validation of the entire book after map phase completes. This validates cross-cutting concerns that individual subsection fixes cannot detect.

## Variables

- `--project <name>` - Project name (e.g., "Prodigy", "Debtmap")
- `--book-dir <path>` - Path to book directory (default: "book")
- `--output <path>` - Path to write validation report (default: ".prodigy/book-validation.json")
- `--auto-fix <boolean>` - Automatically fix issues found (default: false)

## Execute

### Phase 1: Understand Context

You are performing **holistic validation** of the entire book after the map phase has updated individual chapters/subsections. The map phase focuses on individual files and cannot detect:

1. **Redundancy across chapters** - Multiple files with overlapping Best Practices
2. **Structural inconsistencies** - Some chapters use subsections, others don't
3. **Navigation patterns** - Circular or redundant "See Also" links
4. **Content distribution** - Best Practices scattered vs centralized
5. **Chapter fragmentation** - Too many tiny subsections

**Your Goal**: Identify these cross-cutting issues and either fix them automatically or report them for manual review.

### Phase 2: Extract Parameters

```bash
PROJECT_NAME="${project:?Error: --project is required}"
BOOK_DIR="${book_dir:-book}"
OUTPUT="${output:-.prodigy/book-validation.json}"
AUTO_FIX="${auto_fix:-false}"
```

**Validate Inputs:**
```bash
if [ ! -d "$BOOK_DIR/src" ]; then
    echo "Error: Book directory not found: $BOOK_DIR/src"
    exit 1
fi
```

### Phase 3: Scan Book Structure

**Step 1: Build Chapter Inventory**

Scan `$BOOK_DIR/src/SUMMARY.md` to understand chapter structure:

```bash
# Extract all chapters and their subsections
CHAPTERS=$(grep -E '^\s*-\s+\[' "$BOOK_DIR/src/SUMMARY.md")
```

For each chapter, determine:
1. **Type**: `single-file` (e.g., `error-handling.md`) or `multi-subsection` (e.g., `environment/index.md`)
2. **Subsection count**: How many files under this chapter
3. **Has dedicated best-practices.md**: Check if `{chapter}/best-practices.md` exists
4. **Has dedicated troubleshooting.md**: Check if `{chapter}/troubleshooting.md` exists

**Step 2: Identify All Files with "Best Practices" Sections**

```bash
# Find all markdown files with Best Practices sections
find "$BOOK_DIR/src" -name "*.md" -type f -exec grep -l "^## Best Practices\|^### Best Practices" {} \; > /tmp/bp-files.txt
```

For each file:
1. **File path** relative to `$BOOK_DIR/src/`
2. **File type**: index.md, subsection, standalone, dedicated best-practices.md
3. **Parent chapter**: If subsection, which chapter does it belong to
4. **Line range**: Where the Best Practices section starts/ends

### Phase 4: Detect Anti-Patterns

#### Anti-Pattern 1: Redundant Best Practices Sections

**Issue**: Subsection files have "Best Practices" sections when their chapter has a dedicated `best-practices.md`.

**Detection Logic:**
```bash
# For each file with Best Practices section
while read -r FILE; do
  # Get parent chapter directory
  CHAPTER_DIR=$(dirname "$FILE")

  # Check if this is a subsection (not index.md, not standalone)
  if [[ "$FILE" != */index.md ]] && [[ "$FILE" == */* ]]; then
    # Check if chapter has dedicated best-practices.md
    if [ -f "$CHAPTER_DIR/best-practices.md" ]; then
      echo "REDUNDANT: $FILE has Best Practices but $CHAPTER_DIR/best-practices.md exists"
    fi
  fi
done < /tmp/bp-files.txt
```

**Report Format:**
```json
{
  "type": "redundant_best_practices",
  "severity": "high",
  "files": [
    {
      "file": "environment/index.md",
      "lines": [244, 265],
      "redundant_with": "environment/best-practices.md",
      "recommendation": "Remove section, content covered in dedicated file"
    },
    {
      "file": "retry-configuration/retry-budget.md",
      "lines": [129, 184],
      "redundant_with": "retry-configuration/best-practices.md",
      "recommendation": "Remove section, migrate useful content to dedicated file"
    }
  ]
}
```

#### Anti-Pattern 2: Best Practices in Technical Reference Pages

**Issue**: Technical reference pages (syntax, configuration, API) have Best Practices sections.

**Detection Logic:**
```bash
# Identify technical reference pages
while read -r FILE; do
  # Check file content for reference page indicators
  if grep -qi "syntax\|reference\|configuration\|fields\|options\|parameters" "$FILE" | head -5; then
    # Check if it's a pure reference page (not a guide)
    GUIDE_INDICATORS=$(grep -ci "tutorial\|guide\|walkthrough\|example" "$FILE" | head -5)
    if [ "$GUIDE_INDICATORS" -lt 3 ]; then
      echo "REFERENCE_PAGE: $FILE is technical reference with Best Practices section"
    fi
  fi
done < /tmp/bp-files.txt
```

**Report Format:**
```json
{
  "type": "best_practices_in_reference",
  "severity": "medium",
  "files": [
    {
      "file": "workflow-basics/command-level-options.md",
      "lines": [468, 527],
      "file_type": "technical_reference",
      "recommendation": "Remove Best Practices section - this is API documentation"
    }
  ]
}
```

#### Anti-Pattern 3: Circular "See Also" References

**Issue**: Subsection A links to B, B links to A, creating circular navigation without hierarchy.

**Detection Logic:**
```bash
# Extract all "See Also" links from all files
find "$BOOK_DIR/src" -name "*.md" -type f | while read -r FILE; do
  # Find "See Also" section and extract links
  sed -n '/^## See Also/,/^##/p' "$FILE" | grep -oP '\[.*?\]\(\K[^\)]+' | while read -r LINK; do
    # Resolve relative link
    TARGET=$(cd "$(dirname "$FILE")" && realpath --relative-to="$BOOK_DIR/src" "$LINK" 2>/dev/null)
    echo "$FILE -> $TARGET"
  done
done > /tmp/see-also-graph.txt

# Detect circular references
# If A -> B and B -> A, report as circular
```

**Report Format:**
```json
{
  "type": "circular_see_also",
  "severity": "low",
  "patterns": [
    {
      "files": ["mapreduce/checkpoint-and-resume.md", "mapreduce/dead-letter-queue-dlq.md"],
      "description": "Mutual references without explaining specific relationship"
    }
  ]
}
```

#### Anti-Pattern 4: Generic "See Also" Lists

**Issue**: Files list every other subsection in the chapter without explaining why.

**Detection Logic:**
```bash
# For each file with "See Also" section
find "$BOOK_DIR/src" -name "*.md" -type f | while read -r FILE; do
  # Count links in "See Also" section
  LINK_COUNT=$(sed -n '/^## See Also/,/^##/p' "$FILE" | grep -c '^\s*-')

  # If more than 5 links, likely a generic list
  if [ "$LINK_COUNT" -gt 5 ]; then
    # Check if links have explanations (text after the link)
    EXPLAINED_LINKS=$(sed -n '/^## See Also/,/^##/p' "$FILE" | grep -c '\](.*) -')

    if [ "$EXPLAINED_LINKS" -lt "$((LINK_COUNT / 2))" ]; then
      echo "GENERIC_SEE_ALSO: $FILE lists $LINK_COUNT links without explanations"
    fi
  fi
done
```

**Report Format:**
```json
{
  "type": "generic_see_also",
  "severity": "low",
  "files": [
    {
      "file": "mapreduce/checkpoint-and-resume.md",
      "link_count": 8,
      "explained_count": 2,
      "recommendation": "Reduce to 3-4 most relevant links with specific relationships"
    }
  ]
}
```

#### Anti-Pattern 5: Over-Fragmented Chapters

**Issue**: Chapters with too many subsections (>10) or subsections with minimal content (<100 lines).

**Detection Logic:**
```bash
# For each multi-subsection chapter
find "$BOOK_DIR/src" -type d -mindepth 1 | while read -r CHAPTER_DIR; do
  # Count subsection files (exclude index.md)
  SUBSECTION_COUNT=$(find "$CHAPTER_DIR" -name "*.md" -not -name "index.md" | wc -l)

  if [ "$SUBSECTION_COUNT" -gt 10 ]; then
    # Check average file size
    AVG_LINES=$(find "$CHAPTER_DIR" -name "*.md" -not -name "index.md" -exec wc -l {} \; | awk '{sum+=$1; count++} END {print sum/count}')

    if [ "$AVG_LINES" -lt 100 ]; then
      echo "OVER_FRAGMENTED: $CHAPTER_DIR has $SUBSECTION_COUNT subsections averaging $AVG_LINES lines"
    fi
  fi
done
```

**Report Format:**
```json
{
  "type": "over_fragmented_chapter",
  "severity": "medium",
  "chapters": [
    {
      "chapter": "retry-configuration",
      "subsection_count": 15,
      "average_lines": 87,
      "recommendation": "Consolidate related subsections - target 6-8 focused subsections"
    }
  ]
}
```

#### Anti-Pattern 6: Stub Navigation Files

**Issue**: Files that are just navigation boilerplate (<50 lines, mostly links).

**Detection Logic:**
```bash
# Find small files
find "$BOOK_DIR/src" -name "*.md" -type f -exec sh -c 'wc -l "$1" | awk "\$1 < 50 {print \$2}"' _ {} \; | while read -r FILE; do
  # Check if file is mostly links
  LINK_COUNT=$(grep -c '^\s*-\s*\[.*\](' "$FILE")
  LINE_COUNT=$(wc -l < "$FILE")

  # If more than 50% links, it's a navigation stub
  if [ "$((LINK_COUNT * 2))" -gt "$LINE_COUNT" ]; then
    echo "STUB_FILE: $FILE is only $LINE_COUNT lines with $LINK_COUNT links"
  fi
done
```

**Report Format:**
```json
{
  "type": "stub_navigation_file",
  "severity": "medium",
  "files": [
    {
      "file": "composition/related-chapters.md",
      "lines": 14,
      "link_percentage": 71,
      "recommendation": "Consolidate into composition/index.md"
    }
  ]
}
```

### Phase 5: Generate Holistic Validation Report

**Compile All Findings:**

```json
{
  "validation_timestamp": "2025-01-10T15:30:00Z",
  "project": "$PROJECT_NAME",
  "book_dir": "$BOOK_DIR",
  "total_files": 147,
  "total_chapters": 15,
  "issues_found": [
    {/* Anti-Pattern 1 findings */},
    {/* Anti-Pattern 2 findings */},
    {/* Anti-Pattern 3 findings */},
    {/* Anti-Pattern 4 findings */},
    {/* Anti-Pattern 5 findings */},
    {/* Anti-Pattern 6 findings */}
  ],
  "summary": {
    "redundant_best_practices": 6,
    "best_practices_in_reference": 6,
    "circular_see_also": 12,
    "generic_see_also": 30,
    "over_fragmented_chapters": 3,
    "stub_navigation_files": 8
  },
  "recommendations": [
    "Remove 6 redundant Best Practices sections",
    "Remove 6 Best Practices sections from technical reference pages",
    "Consolidate 3 over-fragmented chapters",
    "Merge 8 stub navigation files into chapter indexes"
  ]
}
```

**Write Report:**
```bash
cat > "$OUTPUT" <<EOF
{validation report JSON}
EOF

echo "✓ Holistic validation complete"
echo "  Issues found: ${TOTAL_ISSUES}"
echo "  Report written to: $OUTPUT"
```

### Phase 6: Auto-Fix Mode (Optional)

If `--auto-fix true`, perform automatic fixes for clear-cut issues:

#### Fix 1: Remove Redundant Best Practices Sections

```bash
# For each redundant Best Practices section
jq -r '.issues[] | select(.type == "redundant_best_practices") | .files[] | "\(.file) \(.lines[0]) \(.lines[1])"' "$OUTPUT" | while read -r FILE START END; do
  # Remove lines START to END from FILE
  sed -i "${START},${END}d" "$BOOK_DIR/src/$FILE"
  echo "  Removed redundant Best Practices from $FILE"
done
```

#### Fix 2: Remove Best Practices from Reference Pages

Similar pattern - remove identified sections.

#### Fix 3: Consolidate Stub Navigation Files

```bash
# For each stub file
jq -r '.issues[] | select(.type == "stub_navigation_file") | .files[] | .file' "$OUTPUT" | while read -r STUB_FILE; do
  CHAPTER_DIR=$(dirname "$STUB_FILE")
  INDEX_FILE="$CHAPTER_DIR/index.md"

  # Append stub content to index.md
  echo "" >> "$INDEX_FILE"
  cat "$STUB_FILE" >> "$INDEX_FILE"

  # Remove stub file
  rm "$STUB_FILE"

  # Update SUMMARY.md to remove stub reference
  sed -i "/$(basename "$STUB_FILE")/d" "$BOOK_DIR/src/SUMMARY.md"

  echo "  Consolidated $STUB_FILE into $INDEX_FILE"
done
```

**Commit Auto-Fixes:**
```bash
if [ "$AUTO_FIX" = "true" ]; then
  git add "$BOOK_DIR/src"
  git commit -m "docs: holistic cleanup after drift detection

- Removed $REDUNDANT_COUNT redundant Best Practices sections
- Removed $REFERENCE_COUNT Best Practices from technical reference pages
- Consolidated $STUB_COUNT stub navigation files

Based on holistic validation report: $OUTPUT"

  echo "✓ Auto-fixes committed"
fi
```

### Phase 7: Summary Output

**If Auto-Fix Enabled:**
```
✓ Holistic Validation Complete
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Issues Found: 65
Auto-Fixed: 47

Fixes Applied:
  ✓ Removed 6 redundant Best Practices sections
  ✓ Removed 6 Best Practices from reference pages
  ✓ Consolidated 8 stub navigation files

Manual Review Required: 18 issues
  ⚠ 3 over-fragmented chapters (manual consolidation recommended)
  ⚠ 12 circular See Also references (need context-specific fixes)
  ⚠ 3 other structural issues

See detailed report: .prodigy/book-validation.json
```

**If Reporting Only:**
```
✓ Holistic Validation Complete
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Issues Found: 65

High Priority (12):
  • 6 redundant Best Practices sections
  • 6 Best Practices in technical reference pages

Medium Priority (35):
  • 3 over-fragmented chapters
  • 8 stub navigation files
  • 24 other structural issues

Low Priority (18):
  • 12 circular See Also references
  • 6 generic See Also lists

Recommendations:
  1. Run with --auto-fix to resolve 47 issues automatically
  2. Manually review over-fragmented chapters for consolidation
  3. Simplify circular See Also references

Detailed report: .prodigy/book-validation.json
```

### Success Criteria

- [ ] All chapters scanned and categorized
- [ ] All Best Practices sections identified and validated
- [ ] Redundancy detected across chapters
- [ ] Over-fragmentation detected
- [ ] Stub navigation files identified
- [ ] Circular references detected
- [ ] Validation report generated with severity levels
- [ ] Auto-fix mode works correctly (if enabled)
- [ ] mdbook build succeeds after auto-fixes

### Error Handling

**Book build fails:**
```
Error: Book build failed after auto-fixes
Run: cd book && mdbook build
Review errors and manually fix broken links.
```

**Invalid book structure:**
```
Error: Could not parse SUMMARY.md
Ensure SUMMARY.md exists and follows mdBook format.
```

**No issues found:**
```
✓ Book validation passed
No cross-cutting issues detected.
```
