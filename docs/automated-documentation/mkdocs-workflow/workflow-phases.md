# Workflow Phases

This page describes each phase of the MkDocs documentation workflow and provides a reference for all available workflow commands.

## Workflow Phases

### Setup Phase

**Step 1: Analyze Features**
```yaml
- claude: "/prodigy-analyze-features-for-mkdocs --project $PROJECT_NAME --config $PROJECT_CONFIG"
```

Scans your codebase to build a feature inventory including:
- Command types and syntax
- Configuration options
- Workflow features
- MapReduce capabilities

**Output:** `.prodigy/mkdocs-analysis/features.json`

**Step 2: Detect Gaps**
```yaml
- claude: "/prodigy-detect-mkdocs-gaps --project $PROJECT_NAME --config $PROJECT_CONFIG --features $FEATURES_PATH --chapters $CHAPTERS_FILE --docs-dir $DOCS_DIR"
```

Compares feature inventory against existing documentation to find:
- Missing pages for undocumented features
- Incomplete pages missing key information
- Structural gaps in navigation

**Outputs:**
- `.prodigy/mkdocs-analysis/gap-report.json`
- `.prodigy/mkdocs-analysis/flattened-items.json` (for map phase)
- New stub markdown files (if gaps found)

### Map Phase

Processes each documentation page in parallel:

**Step 1: Analyze Drift**
```yaml
- claude: "/prodigy-analyze-mkdocs-drift --project $PROJECT_NAME --json '${item}' --features $FEATURES_PATH"
  commit_required: true
```

Analyzes each page for:
- Outdated information
- Missing features
- Incorrect examples
- Broken references

**Output:** Drift analysis JSON with severity ratings

**Step 2: Fix Drift**
```yaml
- claude: "/prodigy-fix-mkdocs-drift --project $PROJECT_NAME --json '${item}'"
  commit_required: true
```

Fixes detected drift by:
- Updating documentation to match current implementation
- Adding source code references
- Fixing broken links
- Adding missing examples

**Step 3: Validate Fix**
```yaml
validate:
  claude: "/prodigy-validate-mkdocs-page --project $PROJECT_NAME --json '${item}' --output .prodigy/validation-result.json"
  result_file: ".prodigy/validation-result.json"
  threshold: 100
```

Ensures the fix meets quality standards:
- All required topics covered
- Examples are accurate
- Links are valid
- Source attribution present

### Reduce Phase

Aggregates results and performs holistic validation:

**Step 1: Build Documentation**
```yaml
- shell: "mkdocs build --strict"
  on_failure:
    claude: "/prodigy-fix-mkdocs-build-errors --project $PROJECT_NAME"
    commit_required: true
```

Runs `mkdocs build --strict` to catch:
- Broken internal links
- Missing files referenced in navigation
- Invalid markdown syntax
- Configuration errors

**Step 2: Holistic Validation**
```yaml
- claude: "/prodigy-validate-mkdocs-holistically --project $PROJECT_NAME --docs-dir $DOCS_DIR --output $ANALYSIS_DIR/validation.json --auto-fix true"
  commit_required: true
```

Performs cross-cutting validation:
- **Missing index.md** - Creates landing page if missing
- **Orphaned files** - Detects files not in navigation
- **Navigation completeness** - Ensures all files are accessible
- **Build validation** - Confirms mkdocs builds successfully
- **Content anti-patterns** - Detects redundant sections, circular references

**Step 3: Cleanup**
```yaml
- shell: "rm -rf ${ANALYSIS_DIR}/features.json ${ANALYSIS_DIR}/flattened-items.json ${ANALYSIS_DIR}/drift-*.json"
- shell: "git add -A && git commit -m 'chore: remove temporary mkdocs analysis files for ${PROJECT_NAME}' || true"
```

Removes temporary analysis files while preserving validation report.

## Workflow Commands Reference

### Setup Commands

**`/prodigy-analyze-features-for-mkdocs`**
- Scans codebase for features
- Outputs: `.prodigy/mkdocs-analysis/features.json`
- Reuses existing analysis if recent

**`/prodigy-detect-mkdocs-gaps`**
- Compares features against documentation
- Creates missing page stubs
- Outputs: gap report and flattened items for map phase

### Map Phase Commands

**`/prodigy-analyze-mkdocs-drift`**
- Analyzes single page for drift
- Compares against feature inventory
- Outputs: drift analysis JSON

**`/prodigy-fix-mkdocs-drift`**
- Fixes drift in single page
- Adds source attribution
- Updates examples and explanations

**`/prodigy-validate-mkdocs-page`**
- Validates page completeness
- Checks quality standards
- Returns quality score

**`/prodigy-complete-mkdocs-fix`**
- Iteratively improves page to meet threshold
- Addresses validation gaps
- Runs up to `max_attempts` times

### Reduce Phase Commands

**`/prodigy-fix-mkdocs-build-errors`**
- Fixes mkdocs build failures
- Repairs broken links
- Fixes navigation issues

**`/prodigy-validate-mkdocs-holistically`**
- Cross-cutting validation
- Checks navigation completeness
- Validates mkdocs build
- Detects content anti-patterns
- Auto-fixes with `--auto-fix true`
