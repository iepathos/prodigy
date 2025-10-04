# Book Documentation Workflow

## Overview

Prodigy provides an automated book documentation system that keeps your mdBook documentation in sync with your codebase. The system uses Prodigy MapReduce workflows to:

- **Analyze your codebase** to extract current features and capabilities
- **Detect drift** between documentation and implementation
- **Automatically fix** outdated or missing documentation
- **Validate** that the book builds successfully

This workflow is **project-agnostic** and can be used with any Rust project that uses mdBook for documentation. The same core Claude commands and workflow patterns work across multiple projects (e.g., Prodigy, Debtmap) with only configuration changes.

## Setup

### 1. Install mdBook

```bash
cargo install mdbook
```

### 2. Initialize Book Structure

From your project root:

```bash
mdbook init book
```

This creates:
- `book/book.toml` - mdBook configuration
- `book/src/SUMMARY.md` - Table of contents
- `book/src/` - Markdown source files

### 3. Create Project Book Configuration

Create a project-specific configuration file (e.g., `.prodigy/book-config.json` or `.debtmap/book-config.json`):

```json
{
  "project_name": "YourProject",
  "project_type": "cli_tool",
  "book_dir": "book",
  "book_src": "book/src",
  "book_build_dir": "book/book",
  "analysis_targets": [
    {
      "area": "core_features",
      "source_files": ["src/main.rs", "src/lib.rs"],
      "feature_categories": ["structure", "capabilities", "configuration"]
    }
  ],
  "chapter_file": "workflows/data/yourproject-chapters.json",
  "custom_analysis": {
    "include_examples": true,
    "include_best_practices": true,
    "include_troubleshooting": true
  }
}
```

**Project Types**:
- `cli_tool` - Command-line applications
- `library` - Rust libraries
- `application` - General applications
- `workflow_orchestrator` - Workflow systems like Prodigy

### 4. Create Chapter Definitions

Create a chapters file referenced in your config (e.g., `workflows/data/yourproject-chapters.json`):

```json
{
  "chapters": [
    {
      "id": "getting-started",
      "title": "Getting Started",
      "file": "book/src/getting-started.md",
      "topics": ["Installation", "Quick Start", "Basic Usage"],
      "validation": "Ensure installation steps are current and examples work",
      "source_references": ["README.md", "src/main.rs"]
    },
    {
      "id": "configuration",
      "title": "Configuration",
      "file": "book/src/configuration.md",
      "topics": ["Config File", "Environment Variables", "CLI Flags"],
      "validation": "Verify all configuration options are documented",
      "source_references": ["src/config/*.rs"]
    }
  ]
}
```

### 5. Create Project Workflow

Create `workflows/book-docs-drift.yml` based on the template pattern:

```yaml
name: yourproject-book-docs-drift-detection
mode: mapreduce

env:
  PROJECT_NAME: "YourProject"
  PROJECT_CONFIG: ".yourproject/book-config.json"
  CHAPTERS_FILE: "workflows/data/yourproject-chapters.json"
  ANALYSIS_DIR: ".yourproject/book-analysis"
  FEATURES_PATH: ".yourproject/book-analysis/features.json"
  DRIFT_DIR: ".yourproject/book-analysis"
  BOOK_DIR: "book"

setup:
  - shell: "mkdir -p $ANALYSIS_DIR"
  - claude: "/analyze-codebase-features --config $PROJECT_CONFIG"

map:
  input: "$CHAPTERS_FILE"
  json_path: "$.chapters[*]"

  agent_template:
    - claude: "/analyze-chapter-drift --json '${item}' --features $FEATURES_PATH --project $PROJECT_NAME"
      commit_required: true

  max_parallel: 3

reduce:
  - claude: "/fix-documentation-drift --config $PROJECT_CONFIG --drift-dir $DRIFT_DIR"
    commit_required: true
  - shell: "cd $BOOK_DIR && mdbook build"
    on_failure:
      claude: "/fix-book-build-errors --project $PROJECT_NAME"

error_policy:
  on_item_failure: dlq
  continue_on_failure: true
  max_failures: 2

merge:
  - shell: "rm -rf $ANALYSIS_DIR"
  - shell: "git add -A && git commit -m 'chore: remove temporary analysis files for $PROJECT_NAME' || true"
  - shell: "cd $BOOK_DIR && mdbook build"
  - shell: "git fetch origin"
  - claude: "/merge-master"
  - claude: "/prodigy-merge-worktree ${merge.source_branch}"
```

### 6. Run the Workflow

```bash
prodigy run workflows/book-docs-drift.yml
```

The workflow will:
1. Analyze your codebase features
2. Check each chapter for drift
3. Fix any detected drift
4. Build the book
5. Merge changes back to your main branch

## Configuration

### Project Configuration Structure

**Required Fields**:
- `project_name` - Display name for your project
- `project_type` - Type of project (see Setup step 3)
- `book_dir` - Root directory of mdBook (typically `book`)
- `book_src` - Source directory for markdown files (typically `book/src`)
- `book_build_dir` - Output directory for built book (typically `book/book`)
- `analysis_targets` - Array of areas to analyze
- `chapter_file` - Path to chapter definitions JSON

**Optional Fields**:
- `custom_analysis` - Customize what to include in feature analysis

### Chapter Definition Format

Each chapter definition includes:

- `id` - Unique identifier for the chapter
- `title` - Chapter title for display
- `file` - Path to the markdown file
- `topics` - Array of topics covered in the chapter
- `validation` - What to validate when checking for drift
- `source_references` - Code files that should be reflected in this chapter

**Example Chapter**:
```json
{
  "id": "advanced-features",
  "title": "Advanced Features",
  "file": "book/src/advanced.md",
  "topics": ["Plugins", "Extensions", "Customization"],
  "validation": "Ensure plugin API is documented and examples are current",
  "source_references": ["src/plugins/*.rs", "src/extensions/*.rs"]
}
```

### Analysis Target Specification

Analysis targets tell the system which code to analyze for features:

```json
{
  "area": "workflow_execution",
  "source_files": [
    "src/workflow/executor.rs",
    "src/workflow/runtime.rs"
  ],
  "feature_categories": [
    "execution_model",
    "parallelism",
    "error_handling"
  ]
}
```

**Fields**:
- `area` - Descriptive name for this area of the codebase
- `source_files` - Files or glob patterns to analyze
- `feature_categories` - What aspects to focus on

**Feature Categories**:
- `structure` - Data structures and types
- `capabilities` - What the code can do
- `configuration` - Configuration options
- `execution_model` - How things run
- `error_handling` - Error handling approaches
- `parallelism` - Concurrent execution features
- `integration` - Integration points

## Customization

### Custom Validation Focuses

Tailor drift detection for each chapter by setting specific validation criteria:

**Example - Tutorial Chapter**:
```json
{
  "id": "tutorial",
  "validation": "Verify all code examples compile and run, check CLI output matches examples"
}
```

**Example - API Reference**:
```json
{
  "id": "api-reference",
  "validation": "Ensure all public functions are documented with correct signatures and return types"
}
```

**Example - Architecture Guide**:
```json
{
  "id": "architecture",
  "validation": "Verify architecture diagrams reflect current module structure and dependencies"
}
```

### Project-Specific Analysis Targets

Customize feature analysis for different project architectures:

**CLI Tool**:
```json
{
  "analysis_targets": [
    {
      "area": "commands",
      "source_files": ["src/commands/*.rs"],
      "feature_categories": ["structure", "capabilities"]
    },
    {
      "area": "cli_interface",
      "source_files": ["src/cli.rs"],
      "feature_categories": ["configuration", "flags", "arguments"]
    }
  ]
}
```

**Library**:
```json
{
  "analysis_targets": [
    {
      "area": "public_api",
      "source_files": ["src/lib.rs", "src/api/*.rs"],
      "feature_categories": ["structure", "capabilities", "integration"]
    },
    {
      "area": "examples",
      "source_files": ["examples/*.rs"],
      "feature_categories": ["usage_patterns"]
    }
  ]
}
```

**Application**:
```json
{
  "analysis_targets": [
    {
      "area": "core_logic",
      "source_files": ["src/core/*.rs"],
      "feature_categories": ["structure", "execution_model"]
    },
    {
      "area": "integrations",
      "source_files": ["src/integrations/*.rs"],
      "feature_categories": ["integration", "configuration"]
    }
  ]
}
```

### Documentation Style Preservation

The fix command preserves your documentation style by:

1. **Maintaining tone** - Keeps existing writing style (formal, casual, technical)
2. **Preserving structure** - Maintains your heading hierarchy and organization
3. **Keeping examples** - Updates examples without changing format
4. **Retaining custom sections** - Preserves tips, warnings, notes

**Tips for Style Consistency**:
- Define style guidelines in `book/src/style-guide.md`
- Use consistent terminology across chapters
- Include style notes in chapter validation fields
- Review fixes before merging to ensure style matches

## Troubleshooting

### Common Configuration Errors

**Error: "Config file not found"**
```
Solution: Ensure the path in your workflow's PROJECT_CONFIG variable
points to an existing file. Check for typos in the path.
```

**Error: "Invalid JSON in config file"**
```
Solution: Validate your JSON using `jq . < .yourproject/book-config.json`.
Common issues: trailing commas, missing quotes, unescaped characters.
```

**Error: "Chapter file not found"**
```
Solution: Verify the chapter_file path in your project config matches
the actual location. The path should be relative to the project root.
```

**Error: "Source files do not exist"**
```
Solution: Check that source_files in analysis_targets point to real files.
Use glob patterns carefully and verify paths are relative to project root.
```

### Book Build Failures

**Error: "mdbook: command not found"**
```
Solution: Install mdBook with `cargo install mdbook`.
Verify it's in your PATH with `which mdbook`.
```

**Error: "SUMMARY.md references non-existent file"**
```
Solution: Ensure all files in book/src/SUMMARY.md exist.
Create missing files or update SUMMARY.md to remove references.
```

**Error: "Invalid markdown syntax"**
```
Solution: Run `mdbook build book` manually to see detailed error.
Common issues: unclosed code blocks, invalid table syntax, broken links.
Check the specific file and line number in the error message.
```

**Error: "Build succeeds but chapters look wrong"**
```
Solution: Clear the build cache with `rm -rf book/book` and rebuild.
Check book.toml for correct configuration.
Verify SUMMARY.md has correct chapter ordering.
```

### Drift Detection Issues

**Issue: "No drift detected but docs are clearly out of date"**
```
Possible causes:
1. Analysis targets don't include the relevant code
   - Add source files to analysis_targets in config

2. Validation focus is too narrow
   - Broaden the validation field in chapter definition

3. Feature categories don't match what changed
   - Add relevant categories to analysis_targets

4. Code changes aren't in tracked files
   - Update source_references in chapter definition
```

**Issue: "False positives - drift detected when docs are fine"**
```
Possible causes:
1. Validation is too strict
   - Adjust validation field to focus on actual requirements

2. Analysis includes internal implementation details
   - Limit source_files to public API files

3. Documentation uses different terminology
   - Align terminology between code and docs, or adjust validation
```

**Issue: "Drift reports are empty or generic"**
```
Solutions:
1. Check that features.json was generated correctly
   - Look in $ANALYSIS_DIR/features.json
   - Verify it contains relevant feature data

2. Ensure chapter topics match feature categories
   - Align topics in chapter definition with analysis targets

3. Verify source_references point to changing code
   - Update source_references to include relevant files
```

**Issue: "Drift fixes don't compile or work"**
```
Solutions:
1. Run tests after fixes to catch issues early
   - Add test step to reduce phase

2. Review drift reports before auto-fixing
   - Use manual review for critical documentation

3. Improve validation specificity
   - Add detailed validation criteria to chapters

4. Test examples independently
   - Create test harness for doc examples
```

### Performance Issues

**Issue: "Workflow takes too long"**
```
Optimizations:
1. Increase max_parallel for chapter analysis
   - Try max_parallel: 5 (default is 3)

2. Reduce scope of analysis_targets
   - Focus on most important code areas
   - Split into separate workflows if needed

3. Filter chapters
   - Only process chapters that changed
   - Use git diff to identify relevant chapters
```

**Issue: "High memory usage"**
```
Solutions:
1. Reduce max_parallel
   - Lower parallelism trades speed for memory

2. Split large analysis targets
   - Break into smaller, focused targets

3. Limit feature_categories per target
   - Reduce categories to most relevant ones
```

### Merge Issues

**Issue: "Merge conflicts in documentation"**
```
Solutions:
1. Pull latest changes before running workflow
   - Add "git fetch origin" and "git merge" to setup phase

2. Coordinate with team on doc changes
   - Use feature branches for large doc updates

3. Review and resolve conflicts manually
   - Workflow will pause for manual resolution
```

**Issue: "Merge rejected by CI"**
```
Solutions:
1. Ensure book builds in merge phase
   - Add mdbook build step to merge commands

2. Run tests as part of merge validation
   - Add test step to verify doc examples

3. Check that all temporary files are cleaned up
   - Verify cleanup commands in merge phase work correctly
```

## Advanced Usage

### Multiple Books Per Project

If your project has multiple books (e.g., user guide and API reference):

1. Create separate configurations:
   - `.yourproject/user-guide-config.json`
   - `.yourproject/api-reference-config.json`

2. Create separate workflows:
   - `workflows/user-guide-drift.yml`
   - `workflows/api-reference-drift.yml`

3. Run independently:
   ```bash
   prodigy run workflows/user-guide-drift.yml
   prodigy run workflows/api-reference-drift.yml
   ```

### Incremental Updates

To update only specific chapters:

```yaml
map:
  filter: "item.id == 'getting-started' || item.id == 'tutorial'"
```

### Continuous Integration

Add to your CI pipeline:

```yaml
# .github/workflows/docs.yml
- name: Check Documentation Drift
  run: |
    prodigy run workflows/book-docs-drift.yml

- name: Deploy Book
  run: |
    cd book && mdbook build
    # Deploy book/book/* to hosting
```

### Custom Analysis Commands

Create project-specific analysis commands by extending the base commands:

```markdown
# .claude/commands/custom-feature-analysis.md

1. Run base analysis: `/analyze-codebase-features --config $1`
2. Add custom analysis for your domain
3. Merge results into features.json
```

## Best Practices

1. **Keep chapters focused** - One topic per chapter for easier drift detection
2. **Update regularly** - Run workflow weekly or before releases
3. **Review auto-fixes** - Check drift fixes before merging
4. **Test examples** - Ensure code examples compile and work
5. **Version your config** - Track config changes in git
6. **Document style** - Maintain style guide for consistency
7. **Start small** - Begin with a few chapters and expand
8. **Monitor drift patterns** - Track which areas drift most often
9. **Coordinate with team** - Align on documentation updates
10. **Automate deployment** - Deploy book automatically after updates

## Getting Help

For issues with:
- **mdBook setup**: See [mdBook Guide](https://rust-lang.github.io/mdBook/)
- **Workflow syntax**: See `docs/workflow-syntax.md`
- **Claude commands**: Check `.claude/commands/` for command docs
- **Prodigy issues**: File issue at https://github.com/yourusername/prodigy/issues

## Example Projects

### Prodigy

See Prodigy's own book documentation setup:
- Config: `.prodigy/book-config.json`
- Chapters: `workflows/data/prodigy-chapters.json`
- Workflow: `workflows/book-docs-drift.yml`
- Book: `book/src/`

### Debtmap

See Debtmap's book documentation setup:
- Config: `.debtmap/book-config.json`
- Chapters: `workflows/data/debtmap-chapters.json`
- Workflow: `workflows/book-docs-drift.yml`
- Book: `book/src/`
