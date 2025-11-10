# /validate-docs-structure

Validate consistency between mdbook (SUMMARY.md) and mkdocs (mkdocs.yml) documentation structures.

## Purpose

Ensure both documentation systems reference the same source files and maintain structural consistency without auto-modifying carefully crafted navigation structures.

## Validation Checks

### Phase 1: File Existence Validation

1. Parse all file references from `book/src/SUMMARY.md`:
   - Extract all `[Title](path.md)` references
   - Track section structure and nesting
   - Build complete list of mdbook-referenced files

2. Parse all file references from `mkdocs.yml`:
   - Extract all nav entries (simple and nested)
   - Handle Material theme tab structure
   - Build complete list of mkdocs-referenced files

3. Verify all referenced files exist in `book/src/`:
   - Check each file from both SUMMARY.md and mkdocs.yml
   - Report missing files with their source (mdbook/mkdocs)
   - Identify files that exist but aren't referenced in either system

### Phase 2: Link Consistency Validation

1. Check for broken internal links in markdown files:
   - Read all .md files in `book/src/`
   - Extract markdown links: `[text](path.md)` and `[text](path.md#anchor)`
   - Validate target files exist
   - Check anchor targets exist in target files (heading validation)

2. Identify link differences between systems:
   - Source code links (../../src/, ../../workflows/) should be plain text
   - Relative path issues (../../ vs ../)
   - Excluded file references (environment.md, workflow-basics.md)

### Phase 3: Structural Consistency

1. Compare page coverage:
   - Pages in SUMMARY.md but not mkdocs.yml
   - Pages in mkdocs.yml but not SUMMARY.md
   - Pages in neither (orphaned documentation)

2. Validate navigation structure:
   - Check for logical grouping differences
   - Identify pages that might belong in different sections
   - Suggest structural improvements based on content

### Phase 4: Build Validation

1. Test both documentation systems build successfully:
   - Run `mdbook build book` and check for errors/warnings
   - Run `mkdocs build --strict` and check for errors/warnings
   - Report any build issues with specific file references

## Output Format

Generate a structured validation report with:

### Summary Section
```
Documentation Structure Validation Report
=========================================
Generated: [timestamp]

Quick Stats:
- Total .md files in book/src/: X
- Files referenced in SUMMARY.md: X
- Files referenced in mkdocs.yml: X
- Missing file references: X
- Broken links found: X
- Build errors: X

Status: ✅ PASS / ⚠️  WARNINGS / ❌ FAIL
```

### Detailed Findings

**Missing Files:**
```
❌ Referenced but don't exist:
   - path/to/file.md (referenced in: SUMMARY.md line 42)
   - another/missing.md (referenced in: mkdocs.yml)

📄 Exist but not referenced:
   - book/src/orphaned-file.md (not in SUMMARY.md or mkdocs.yml)
```

**Broken Links:**
```
🔗 Broken internal links:
   - book/src/commands.md:537
     Link: [Environment Variables](./environment.md)
     Issue: environment.md is excluded from mkdocs build
     Suggestion: Use ./environment/index.md

   - book/src/advanced/index.md:289
     Link: [Variables](../workflow-basics/index.md#environment-variables)
     Issue: Anchor #environment-variables doesn't exist
     Suggestion: Use ../workflow-basics/environment-configuration.md
```

**Coverage Differences:**
```
📊 Pages in SUMMARY.md but not mkdocs.yml:
   - (none) ✅

📊 Pages in mkdocs.yml but not SUMMARY.md:
   - (none) ✅
```

**Build Validation:**
```
🏗️  mdbook build: ✅ PASS (0 errors, 0 warnings)
🏗️  mkdocs build: ✅ PASS (0 errors, 0 warnings)
```

**Structural Suggestions:**
```
💡 Recommendations:
   - Both systems are consistent ✅
   - All links validated ✅
   - No orphaned files ✅

   Optional improvements:
   - Consider adding intro.md references to README.md
   - Update SUMMARY.md with more detailed subsections matching mkdocs structure
```

## Implementation Guidelines

1. **Read files without modifying**:
   - Use Read tool to parse SUMMARY.md and mkdocs.yml
   - Use Glob to find all .md files in book/src/
   - Use Grep to search for link patterns

2. **Parse navigation structures**:
   - SUMMARY.md: Parse mdbook list syntax `- [Title](path.md)`
   - mkdocs.yml: Parse YAML nav section (handle nested structure)
   - Build normalized file path lists for comparison

3. **Validate file existence**:
   - Convert nav references to absolute paths
   - Check each file exists using file system
   - Track which system references which files

4. **Check link validity**:
   - Grep for markdown link patterns in all .md files
   - For each link, verify target exists
   - For anchor links, verify heading exists in target

5. **Build validation**:
   - Run `mdbook build book` and capture output
   - Run `mkdocs build --strict` and capture output
   - Parse for errors and warnings

6. **Generate report**:
   - Structured sections as shown above
   - Color-coded status (✅ ❌ ⚠️ 💡)
   - Actionable suggestions with file:line references

## Success Criteria

- All referenced files exist
- No broken internal links
- Both build systems pass
- Structural consistency maintained
- Clear, actionable report generated

## Notes

- This command is **read-only** - it never modifies files
- Designed to run before documentation changes are committed
- Can be integrated into CI/CD as a validation step
- Helps maintain dual mdbook/mkdocs compatibility

## Example Usage

```bash
claude /validate-docs-structure
```

Run before:
- Adding new documentation pages
- Reorganizing documentation structure
- Updating navigation in SUMMARY.md or mkdocs.yml
- Merging documentation PRs
