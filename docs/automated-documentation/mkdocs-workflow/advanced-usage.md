# Advanced Usage

This page covers advanced configuration options, integration with existing projects, troubleshooting, best practices, and complete workflow examples.

## Advanced Configuration

### Custom Project Configuration

Create a project-specific configuration file:

```json
// .prodigy/mkdocs-config.json
{
  "project_name": "Prodigy",
  "docs_dir": "book/src",
  "mkdocs_config": "mkdocs.yml",
  "theme": "material",
  "validation": {
    "require_source_attribution": true,
    "require_examples": true,
    "max_drift_severity": "medium"
  },
  "gap_detection": {
    "auto_create_stubs": true,
    "min_coverage_threshold": 0.8
  }
}
```

### Validation Thresholds

Configure validation strictness in the workflow:

```yaml
validate:
  threshold: 100  # 100% = strict, 80% = lenient
  on_incomplete:
    claude: "/prodigy-complete-mkdocs-fix --project $PROJECT_NAME --json '${item}' --gaps ${validation.gaps}"
    max_attempts: 3
    fail_workflow: false  # Continue even if can't reach 100%
```

### Error Handling

Configure how the workflow responds to failures:

```yaml
error_policy:
  on_item_failure: dlq       # Send failures to dead letter queue
  continue_on_failure: true  # Process remaining items
  max_failures: 2            # Stop after 2 failures
  error_collection: aggregate # Report all errors at end
```

## Using with Existing MkDocs Projects

### Migrating from Manual Documentation

1. **Initial Setup:**
   ```bash
   # Create chapter definitions
   prodigy run workflows/mkdocs-drift.yml
   ```

2. **Review Generated Content:**
   The workflow will detect your existing pages and only create stubs for missing ones.

3. **Iterative Improvement:**
   Run the workflow periodically to catch drift as your code evolves.

### Integrating with CI/CD

**GitHub Actions Example:**

```yaml
name: Update Documentation

on:
  schedule:
    - cron: '0 0 * * 0'  # Weekly on Sunday
  workflow_dispatch:

jobs:
  update-docs:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Prodigy
        run: cargo install prodigy

      - name: Run MkDocs Drift Detection
        run: prodigy run workflows/mkdocs-drift.yml --auto-merge

      - name: Create Pull Request
        uses: peter-evans/create-pull-request@v5
        with:
          title: "docs: automated MkDocs drift fixes"
          branch: docs/mkdocs-drift
```

## Troubleshooting

### Issue: Missing index.md

**Symptom:** 404 error on homepage when running `mkdocs serve`

**Solution:** The holistic validation should catch this automatically:
```bash
# The validation command will create it if auto-fix is enabled
/prodigy-validate-mkdocs-holistically --auto-fix true
```

Or create manually:
```bash
cat > ${DOCS_DIR}/index.md <<EOF
# Project Documentation
Welcome to the documentation.
EOF
```

### Issue: Orphaned Files

**Symptom:** Files exist in docs directory but not accessible through navigation

**Solution:** Check validation report:
```bash
cat .prodigy/mkdocs-analysis/validation.json | jq '.mkdocs_specific.orphaned_files'
```

Add files to `mkdocs.yml`:
```yaml
nav:
  - New Section:
    - Title: path/to/orphaned-file.md
```

### Issue: mkdocs build --strict Failures

**Symptom:** Workflow fails in reduce phase with build errors

**Solution:** The workflow automatically calls `/prodigy-fix-mkdocs-build-errors`:
```yaml
- shell: "mkdocs build --strict"
  on_failure:
    claude: "/prodigy-fix-mkdocs-build-errors --project $PROJECT_NAME"
```

If issues persist, manually check:
```bash
mkdocs build --strict 2>&1 | less
```

### Issue: Parallel Agents Overwhelming System

**Symptom:** System slowdown during map phase

**Solution:** Reduce parallelism:
```yaml
env:
  MAX_PARALLEL: "2"  # Reduce from 3 to 2
```

### Issue: Validation Threshold Too Strict

**Symptom:** Pages keep failing validation at 100% threshold

**Solution:** Lower threshold or allow incomplete:
```yaml
validate:
  threshold: 80  # Lower to 80%
  on_incomplete:
    fail_workflow: false  # Don't fail, just warn
```

## Best Practices

### 1. Run Regularly

Schedule periodic runs to catch drift early:
- **Weekly:** For active projects
- **Monthly:** For stable projects
- **After major changes:** When adding new features

### 2. Review Before Merging

Always review generated documentation:
```bash
# Check what changed
cd ~/.prodigy/worktrees/prodigy/session-abc123/
git log --oneline
git diff master
```

### 3. Maintain Chapter Definitions

Keep your chapter definitions file updated:
```json
// Add new sections as your project grows
{
  "id": "new-feature",
  "title": "New Feature",
  "pages": [...]
}
```

### 4. Use Auto-Fix Judiciously

Enable auto-fix for clear-cut issues:
```yaml
- claude: "/prodigy-validate-mkdocs-holistically --auto-fix true"
```

But review auto-fixes before merging!

### 5. Version Your Validation Reports

Keep validation reports in git for tracking:
```bash
git add .prodigy/mkdocs-analysis/validation.json
git commit -m "docs: validation report for mkdocs drift run"
```

## Examples

### Example 1: Full Documentation from Scratch

```yaml
name: prodigy-mkdocs-full-build
mode: mapreduce

env:
  PROJECT_NAME: "MyProject"
  DOCS_DIR: "docs"
  CHAPTERS_FILE: "workflows/data/mkdocs-chapters.json"
  MAX_PARALLEL: "5"

setup:
  - shell: "mkdir -p .prodigy/mkdocs-analysis"
  - claude: "/prodigy-analyze-features-for-mkdocs --project $PROJECT_NAME"
  - claude: "/prodigy-detect-mkdocs-gaps --project $PROJECT_NAME --docs-dir $DOCS_DIR --chapters $CHAPTERS_FILE"

map:
  input: ".prodigy/mkdocs-analysis/flattened-items.json"
  json_path: "$[*]"
  agent_template:
    - claude: "/prodigy-analyze-mkdocs-drift --project $PROJECT_NAME --json '${item}'"
    - claude: "/prodigy-fix-mkdocs-drift --project $PROJECT_NAME --json '${item}'"
  max_parallel: ${MAX_PARALLEL}

reduce:
  - shell: "mkdocs build --strict"
  - claude: "/prodigy-validate-mkdocs-holistically --project $PROJECT_NAME --docs-dir $DOCS_DIR --auto-fix true"
```

### Example 2: Drift Detection Only (No Gaps)

```yaml
name: prodigy-mkdocs-drift-only
mode: mapreduce

env:
  PROJECT_NAME: "MyProject"
  DOCS_DIR: "book/src"
  MAX_PARALLEL: "3"

setup:
  - claude: "/prodigy-analyze-features-for-mkdocs --project $PROJECT_NAME"
  # Skip gap detection - just process existing pages

map:
  # Manually specify pages instead of using flattened-items.json
  input:
    list:
      - {file: "book/src/index.md", title: "Home"}
      - {file: "book/src/guide.md", title: "Guide"}

  agent_template:
    - claude: "/prodigy-analyze-mkdocs-drift --project $PROJECT_NAME --json '${item}'"
    - claude: "/prodigy-fix-mkdocs-drift --project $PROJECT_NAME --json '${item}'"
  max_parallel: ${MAX_PARALLEL}

reduce:
  - shell: "mkdocs build --strict"
  - claude: "/prodigy-validate-mkdocs-holistically --project $PROJECT_NAME --docs-dir $DOCS_DIR"
```

### Example 3: Shared Source with mdbook

```yaml
name: prodigy-mkdocs-shared-source
mode: mapreduce

env:
  PROJECT_NAME: "MyProject"
  DOCS_DIR: "book/src"  # Shared with mdbook
  CHAPTERS_FILE: "workflows/data/prodigy-chapters.json"  # Use mdbook chapters
  MAX_PARALLEL: "4"

setup:
  - claude: "/prodigy-analyze-features-for-mkdocs --project $PROJECT_NAME"
  - claude: "/prodigy-detect-mkdocs-gaps --project $PROJECT_NAME --docs-dir $DOCS_DIR --chapters $CHAPTERS_FILE"

map:
  input: ".prodigy/mkdocs-analysis/flattened-items.json"
  json_path: "$[*]"
  agent_template:
    - claude: "/prodigy-analyze-mkdocs-drift --project $PROJECT_NAME --json '${item}'"
    - claude: "/prodigy-fix-mkdocs-drift --project $PROJECT_NAME --json '${item}'"
  max_parallel: ${MAX_PARALLEL}

reduce:
  # Validate both mdbook and mkdocs builds
  - shell: "cd book && mdbook build"
  - shell: "mkdocs build --strict"
  - claude: "/prodigy-validate-mkdocs-holistically --project $PROJECT_NAME --docs-dir $DOCS_DIR --auto-fix true"
```
