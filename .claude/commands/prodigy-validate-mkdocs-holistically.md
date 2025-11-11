# /prodigy-validate-mkdocs-holistically

Perform holistic validation of the entire mkdocs documentation after map phase completes. This validates cross-cutting concerns that individual subsection fixes cannot detect.

## Variables

- `--project <name>` - Project name (e.g., "Prodigy", "Debtmap")
- `--docs-dir <path>` - Path to mkdocs docs directory (default: "mkdocs")
- `--output <path>` - Path to write validation report (default: ".prodigy/mkdocs-validation.json")
- `--auto-fix <boolean>` - Automatically fix issues found (default: false)

## Execute

### Phase 1: Understand Context

You are performing **holistic validation** of the entire mkdocs documentation after the map phase has updated individual chapters/subsections. The map phase focuses on individual files and cannot detect:

1. **Redundancy across chapters** - Multiple files with overlapping Best Practices
2. **Structural inconsistencies** - Some chapters use subsections, others don't
3. **Navigation patterns** - Circular or redundant "See Also" links
4. **Content distribution** - Best Practices scattered vs centralized
5. **Chapter fragmentation** - Too many tiny subsections

**Your Goal**: Identify these cross-cutting issues and either fix them automatically or report them for manual review.

**CRITICAL IMPLEMENTATION REQUIREMENTS:**

1. **Use shell commands directly** - All scanning and auto-fix logic should use bash/sed/awk/grep
2. **Do NOT create Python scripts** - Execute commands inline, don't generate validate_docs.py or auto_fix.py
3. **Whitelist appropriately** - Root-level guides and chapter indexes (without dedicated best-practices.md) can have BP sections
4. **Better reference detection** - Use ratio of reference vs guide indicators, not absolute counts

### Phase 2: Extract Parameters

```bash
PROJECT_NAME="${project:?Error: --project is required}"
DOCS_DIR="${docs_dir:-mkdocs}"
OUTPUT="${output:-.prodigy/mkdocs-validation.json}"
AUTO_FIX="${auto_fix:-false}"
```

**Validate Inputs:**
```bash
if [ ! -d "$DOCS_DIR" ]; then
    echo "Error: MkDocs directory not found: $DOCS_DIR"
    exit 1
fi

if [ ! -f "mkdocs.yml" ]; then
    echo "Error: mkdocs.yml not found in project root"
    exit 1
fi
```

### Phase 3: Scan Documentation Structure

**Step 1: Build Chapter Inventory**

Scan `mkdocs.yml` to understand navigation structure:

```bash
# Extract navigation structure from mkdocs.yml
# MkDocs uses YAML nav structure like:
# nav:
#   - Home: index.md
#   - Getting Started: getting-started.md
#   - Advanced:
#       - Overview: advanced/index.md
#       - Features: advanced/features.md

yq eval '.nav' mkdocs.yml > /tmp/mkdocs-nav.txt
```

For each chapter, determine:
1. **Type**: `single-file` (e.g., `error-handling.md`) or `multi-subsection` (e.g., `environment/index.md`)
2. **Subsection count**: How many files under this chapter
3. **Has dedicated best-practices.md**: Check if `{chapter}/best-practices.md` exists
4. **Has dedicated troubleshooting.md**: Check if `{chapter}/troubleshooting.md` exists

**Step 2: Identify All Files with "Best Practices" Sections**

```bash
# Find all markdown files with Best Practices sections
find "$DOCS_DIR" -name "*.md" -type f -exec grep -l "^## Best Practices\|^### Best Practices" {} \; > /tmp/bp-files.txt
```

For each file:
1. **File path** relative to `$DOCS_DIR/`
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
    }
  ]
}
```

#### Anti-Pattern 2: Best Practices in Technical Reference Pages

**Issue**: Technical reference pages (syntax, configuration, API) have Best Practices sections.

**IMPORTANT: Whitelist root-level guides and chapter indexes**

**Detection Logic:**
```bash
# Identify technical reference pages
while read -r FILE; do
  BASENAME=$(basename "$FILE")
  RELATIVE_PATH="${FILE#$DOCS_DIR/}"

  # SKIP: Root-level guide files (appropriate for Best Practices)
  if [[ "$RELATIVE_PATH" == *.md ]] && [[ ! "$RELATIVE_PATH" =~ / ]]; then
    # Root-level files like error-handling.md, workflow-basics.md are guides
    continue
  fi

  # SKIP: Chapter index.md files (appropriate for Best Practices)
  if [[ "$BASENAME" == "index.md" ]]; then
    # Check if chapter has dedicated best-practices.md
    CHAPTER_DIR=$(dirname "$FILE")
    if [ ! -f "$CHAPTER_DIR/best-practices.md" ]; then
      # No dedicated file, index.md can have BP section
      continue
    fi
  fi

  # SKIP: Files explicitly marked as guides/tutorials
  if grep -qi "^# .*\(guide\|tutorial\|introduction\|overview\|getting started\)" "$FILE" | head -1; then
    continue
  fi

  # Check file content for reference page indicators
  REFERENCE_COUNT=$(grep -ci "syntax\|reference\|configuration\|fields\|options\|parameters\|properties\|attributes" "$FILE" | head -20)
  GUIDE_COUNT=$(grep -ci "tutorial\|guide\|walkthrough\|how to\|step-by-step" "$FILE" | head -20)

  # If reference indicators > guide indicators, it's likely a reference page
  if [ "$REFERENCE_COUNT" -gt "$((GUIDE_COUNT * 2))" ]; then
    echo "REFERENCE_PAGE: $FILE is technical reference with Best Practices section"
  fi
done < /tmp/bp-files.txt
```

#### Anti-Pattern 3: Circular "See Also" References

**Issue**: Subsection A links to B, B links to A, creating circular navigation without hierarchy.

**Detection Logic:**
```bash
# Extract all "See Also" links from all files
find "$DOCS_DIR" -name "*.md" -type f | while read -r FILE; do
  # Find "See Also" section and extract links
  sed -n '/^## See Also/,/^##/p' "$FILE" | grep -oP '\[.*?\]\(\K[^\)]+' | while read -r LINK; do
    # Resolve relative link
    TARGET=$(cd "$(dirname "$FILE")" && realpath --relative-to="$DOCS_DIR" "$LINK" 2>/dev/null)
    echo "$FILE -> $TARGET"
  done
done > /tmp/see-also-graph.txt

# Detect circular references
# If A -> B and B -> A, report as circular
```

#### Anti-Pattern 4: Generic "See Also" Lists

**Issue**: Files list every other subsection in the chapter without explaining why.

**Detection Logic:**
```bash
# For each file with "See Also" section
find "$DOCS_DIR" -name "*.md" -type f | while read -r FILE; do
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

#### Anti-Pattern 5: Over-Fragmented Chapters

**Issue**: Chapters with too many subsections (>10) or subsections with minimal content (<100 lines).

**Detection Logic:**
```bash
# For each multi-subsection chapter
find "$DOCS_DIR" -type d -mindepth 1 | while read -r CHAPTER_DIR; do
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

#### Anti-Pattern 6: Stub Navigation Files

**Issue**: Files that are just navigation boilerplate (<50 lines, mostly links).

**Detection Logic:**
```bash
# Find small files
find "$DOCS_DIR" -name "*.md" -type f -exec sh -c 'wc -l "$1" | awk "$1 < 50 {print \$2}"' _ {} \; | while read -r FILE; do
  # Check if file is mostly links
  LINK_COUNT=$(grep -c '^\s*-\s*\[.*\](' "$FILE")
  LINE_COUNT=$(wc -l < "$FILE")

  # If more than 50% links, it's a navigation stub
  if [ "$((LINK_COUNT * 2))" -gt "$LINE_COUNT" ]; then
    echo "STUB_FILE: $FILE is only $LINE_COUNT lines with $LINK_COUNT links"
  fi
done
```

#### Anti-Pattern 7: Meta-Sections in Feature Chapters

**Issue**: "Best Practices" or "Common Patterns" files appear as subsections within feature-focused chapters (like "Advanced Features").

**Detection Logic:**
```bash
# Check mkdocs.yml for meta-sections under feature chapters
grep -A 20 "Advanced Features\|Advanced Topics" mkdocs.yml | while IFS= read -r LINE; do
  # Check if line is a meta-section under feature chapter
  if echo "$LINE" | grep -qi "Best Practices:\|Common Patterns:"; then
    # Extract file path
    FILE=$(echo "$LINE" | grep -oP ':\s*\K.*\.md')

    # Verify it's under a feature-focused chapter
    if [[ "$FILE" =~ ^advanced/ ]]; then
      echo "META_IN_FEATURES: $FILE is meta-section under feature chapter"
    fi
  fi
done
```

### Phase 5: Generate Holistic Validation Report

**Compile All Findings:**

```json
{
  "validation_timestamp": "2025-01-10T15:30:00Z",
  "project": "$PROJECT_NAME",
  "docs_dir": "$DOCS_DIR",
  "total_files": 147,
  "total_chapters": 15,
  "issues_found": [
    {/* Anti-Pattern 1 findings */},
    {/* Anti-Pattern 2 findings */},
    {/* Anti-Pattern 3 findings */},
    {/* Anti-Pattern 4 findings */},
    {/* Anti-Pattern 5 findings */},
    {/* Anti-Pattern 6 findings */},
    {/* Anti-Pattern 7 findings */}
  ],
  "summary": {
    "redundant_best_practices": 6,
    "best_practices_in_reference": 6,
    "circular_see_also": 12,
    "generic_see_also": 30,
    "over_fragmented_chapters": 3,
    "stub_navigation_files": 8,
    "meta_sections_in_feature_chapters": 2
  },
  "recommendations": [
    "Remove 6 redundant Best Practices sections",
    "Remove 6 Best Practices sections from technical reference pages",
    "Consolidate 3 over-fragmented chapters",
    "Merge 8 stub navigation files into chapter indexes",
    "Remove 2 meta-sections from feature chapters"
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

If `--auto-fix true`, perform automatic fixes for clear-cut issues.

**IMPORTANT: Use direct shell commands, NOT Python scripts.**

#### Fix 1: Remove Redundant Best Practices Sections

```bash
# For each redundant Best Practices section
jq -r '.issues[] | select(.type == "redundant_best_practices") | .files[] | "\(.file) \(.lines[0]) \(.lines[1])"' "$OUTPUT" | while read -r FILE START END; do
  FULL_PATH="$DOCS_DIR/$FILE"

  # Backup file before editing
  cp "$FULL_PATH" "$FULL_PATH.bak"

  # Remove lines START to END (inclusive)
  sed -i.tmp "${START},${END}d" "$FULL_PATH"
  rm "$FULL_PATH.tmp" 2>/dev/null || true

  echo "  ✓ Removed redundant Best Practices from $FILE (lines $START-$END)"
done
```

#### Fix 2: Remove Best Practices from Reference Pages

```bash
# For each Best Practices section in reference pages
jq -r '.issues[] | select(.type == "best_practices_in_reference") | .files[] | "\(.file) \(.lines[0]) \(.lines[1])"' "$OUTPUT" | while read -r FILE START END; do
  FULL_PATH="$DOCS_DIR/$FILE"

  # Skip if already processed by redundant_best_practices
  if [ ! -f "$FULL_PATH.bak" ]; then
    cp "$FULL_PATH" "$FULL_PATH.bak"
    sed -i.tmp "${START},${END}d" "$FULL_PATH"
    rm "$FULL_PATH.tmp" 2>/dev/null || true
    echo "  ✓ Removed Best Practices from reference page $FILE (lines $START-$END)"
  fi
done
```

#### Fix 3: Consolidate Stub Navigation Files

```bash
# For each stub navigation file
jq -r '.issues[] | select(.type == "stub_navigation_file") | .files[] | .file' "$OUTPUT" | while read -r STUB_FILE; do
  STUB_PATH="$DOCS_DIR/$STUB_FILE"
  CHAPTER_DIR=$(dirname "$STUB_PATH")
  INDEX_FILE="$CHAPTER_DIR/index.md"

  if [ ! -f "$INDEX_FILE" ]; then
    echo "  ⚠ Warning: No index.md found for $STUB_FILE, skipping"
    continue
  fi

  # Backup index before appending
  cp "$INDEX_FILE" "$INDEX_FILE.bak"

  # Append stub content to index.md with separator
  echo "" >> "$INDEX_FILE"
  echo "---" >> "$INDEX_FILE"
  echo "" >> "$INDEX_FILE"
  cat "$STUB_PATH" >> "$INDEX_FILE"

  # Remove stub file
  rm "$STUB_PATH"

  # Update mkdocs.yml to remove stub reference
  STUB_BASENAME=$(basename "$STUB_FILE")
  sed -i.tmp "/- .*: $STUB_FILE/d" mkdocs.yml
  rm mkdocs.yml.tmp 2>/dev/null || true

  echo "  ✓ Consolidated $STUB_FILE into index.md"
done
```

#### Fix 4: Remove Meta-Sections from Feature Chapters

```bash
# For each meta-section in feature chapters
jq -r '.issues[] | select(.type == "meta_sections_in_feature_chapters") | .files[] | .file' "$OUTPUT" | while read -r META_FILE; do
  META_PATH="$DOCS_DIR/$META_FILE"
  META_BASENAME=$(basename "$META_FILE")

  # Remove the file
  if [ -f "$META_PATH" ]; then
    rm "$META_PATH"
    echo "  ✓ Removed meta-section $META_FILE from feature chapter"
  fi

  # Remove from mkdocs.yml
  sed -i.tmp "/- .*: $META_FILE/d" mkdocs.yml
  rm mkdocs.yml.tmp 2>/dev/null || true

  echo "  ✓ Updated mkdocs.yml to remove $META_BASENAME"
done
```

**Cleanup Backups:**
```bash
# Remove backup files after successful fixes
find "$DOCS_DIR" -name "*.bak" -delete
```

**Commit Auto-Fixes:**
```bash
if [ "$AUTO_FIX" = "true" ]; then
  git add "$DOCS_DIR" mkdocs.yml
  git commit -m "docs: holistic cleanup after drift detection (mkdocs)

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
✓ Holistic Validation Complete (MkDocs)
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

See detailed report: .prodigy/mkdocs-validation.json
```

**If Reporting Only:**
```
✓ Holistic Validation Complete (MkDocs)
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

Detailed report: .prodigy/mkdocs-validation.json
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
- [ ] mkdocs build succeeds after auto-fixes

### Error Handling

**MkDocs build fails:**
```
Error: MkDocs build failed after auto-fixes
Run: mkdocs build --strict
Review errors and manually fix broken links.
```

**Invalid mkdocs structure:**
```
Error: Could not parse mkdocs.yml
Ensure mkdocs.yml exists and follows MkDocs YAML format.
```

**No issues found:**
```
✓ MkDocs validation passed
No cross-cutting issues detected.
```
