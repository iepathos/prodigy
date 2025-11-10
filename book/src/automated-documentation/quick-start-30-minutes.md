## Quick Start (30 Minutes)

Get Prodigy's automated documentation workflow running in your project in 30 minutes. This guide walks you through setting up and running your first documentation drift detection and fixing workflow.

### What You'll Accomplish

By the end of this guide, you'll have:
- Configured the automated documentation system for your project
- Run the workflow to detect and fix documentation drift
- Generated or updated an mdBook with accurate, code-grounded documentation
- Understood the three phases: setup, map, and reduce

### Prerequisites

Before starting, ensure you have:

1. **Rust and Cargo** - Install from [rustup.rs](https://rustup.rs/)
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

2. **Prodigy** - Install via cargo:
   ```bash
   cargo install prodigy
   ```

3. **Claude Code CLI** - Follow installation instructions at [Claude Code documentation](https://docs.anthropic.com/claude/docs/claude-code)

4. **mdBook** - For building documentation:
   ```bash
   cargo install mdbook
   ```

5. **Git Repository** - Your project should be in a git repository with existing or new documentation in a `book/` directory

### Step 1: Initialize Your Book Structure

If you don't already have an mdBook, create one:

```bash
# Create book directory and initialize
mkdir book
cd book
mdbook init

# This creates:
# book/
#   ├── book.toml
#   └── src/
#       ├── SUMMARY.md
#       └── chapter_1.md
```

**Source**: Standard mdBook initialization pattern

### Step 2: Create Book Configuration

Create `.prodigy/book-config.json` at your project root with your project details:

```json
{
  "project_name": "YourProject",
  "project_type": "cli_tool",
  "book_dir": "book",
  "book_src": "book/src",
  "book_build_dir": "book/book",
  "analysis_targets": [
    {
      "area": "workflow_basics",
      "source_files": [
        "src/config/workflow.rs",
        "src/workflow/executor.rs"
      ],
      "feature_categories": [
        "structure",
        "execution_model",
        "commit_tracking"
      ]
    }
  ],
  "chapter_file": "workflows/data/your-project-chapters.json",
  "custom_analysis": {
    "include_examples": true,
    "include_best_practices": true,
    "include_troubleshooting": true
  }
}
```

**Source**: Extracted from `.prodigy/book-config.json` structure (lines 1-219)

**Key Fields Explained**:
- `project_name` - Display name for your project
- `book_dir` - Root directory for your mdBook
- `analysis_targets` - Areas of your codebase to analyze for documentation
  - `area` - Logical grouping name
  - `source_files` - Files to analyze for this area
  - `feature_categories` - Types of features to extract
- `chapter_file` - Path to chapter definitions JSON

### Step 3: Define Chapter Structure

Create `workflows/data/your-project-chapters.json` to define your documentation structure:

```json
{
  "chapters": [
    {
      "id": "getting-started",
      "title": "Getting Started",
      "file": "getting-started.md",
      "topics": ["Installation", "Quick Start"],
      "feature_mapping": ["installation", "basic_usage"],
      "validation": "Check getting started guide matches current setup"
    },
    {
      "id": "advanced",
      "title": "Advanced Features",
      "file": "advanced/index.md",
      "topics": ["Configuration", "API Reference"],
      "feature_mapping": ["configuration", "api"],
      "validation": "Verify advanced features are documented"
    }
  ]
}
```

**Source**: Based on `workflows/data/prodigy-chapters.json` structure pattern

### Step 4: Create the Workflow File

Create `workflows/book-docs-drift.yml`:

```yaml
name: your-project-book-docs-drift-detection
mode: mapreduce

env:
  PROJECT_NAME: "YourProject"
  PROJECT_CONFIG: ".prodigy/book-config.json"
  FEATURES_PATH: ".prodigy/book-analysis/features.json"
  BOOK_DIR: "book"
  ANALYSIS_DIR: ".prodigy/book-analysis"
  CHAPTERS_FILE: "workflows/data/your-project-chapters.json"
  MAX_PARALLEL: "3"

setup:
  - shell: "mkdir -p $ANALYSIS_DIR"
  - claude: "/prodigy-analyze-features-for-book --project $PROJECT_NAME --config $PROJECT_CONFIG"
  - claude: "/prodigy-detect-documentation-gaps --project $PROJECT_NAME --config $PROJECT_CONFIG --features $FEATURES_PATH --chapters $CHAPTERS_FILE --book-dir $BOOK_DIR"

map:
  input: "${ANALYSIS_DIR}/flattened-items.json"
  json_path: "$[*]"

  agent_template:
    - claude: "/prodigy-analyze-subsection-drift --project $PROJECT_NAME --json '${item}' --features $FEATURES_PATH"
      commit_required: true

    - claude: "/prodigy-fix-subsection-drift --project $PROJECT_NAME --json '${item}'"
      commit_required: true
      validate:
        claude: "/prodigy-validate-doc-fix --project $PROJECT_NAME --json '${item}' --output .prodigy/validation-result.json"
        result_file: ".prodigy/validation-result.json"
        threshold: 100
        on_incomplete:
          claude: "/prodigy-complete-doc-fix --project $PROJECT_NAME --json '${item}' --gaps ${validation.gaps}"
          max_attempts: 3
          fail_workflow: false
          commit_required: true

  max_parallel: ${MAX_PARALLEL}

reduce:
  - shell: "cd book && mdbook build"
    on_failure:
      claude: "/prodigy-fix-book-build-errors --project $PROJECT_NAME"
      commit_required: true

  - shell: "rm -rf ${ANALYSIS_DIR}"
  - shell: "git add -A && git commit -m 'chore: remove temporary book analysis files' || true"

error_policy:
  on_item_failure: dlq
  continue_on_failure: true
  max_failures: 2
  error_collection: aggregate

merge:
  commands:
    - shell: "git fetch origin"
    - claude: "/prodigy-merge-master --project ${PROJECT_NAME}"
    - claude: "/prodigy-merge-worktree ${merge.source_branch} ${merge.target_branch}"
```

**Source**: Adapted from `workflows/book-docs-drift.yml` (lines 1-101)

### Step 5: Run the Workflow

Execute the documentation workflow:

```bash
# Run with verbose output to see progress
prodigy run workflows/book-docs-drift.yml -v
```

**What Happens During Execution**:

**Setup Phase** (runs in parent worktree):
```
✓ Creating analysis directory
✓ Analyzing codebase features → .prodigy/book-analysis/features.json
✓ Detecting documentation gaps → .prodigy/book-analysis/flattened-items.json
```

**Map Phase** (parallel agents, each in isolated worktree):
```
Agent 1: Analyzing chapter 'Getting Started'
  ✓ Drift analysis complete → drift report
  ✓ Fixing outdated examples
  ✓ Validation: 100% complete
  ✓ Merged to parent worktree

Agent 2: Analyzing chapter 'Advanced Features'
  ✓ Drift analysis complete → drift report
  ✓ Adding missing configuration docs
  ✓ Validation: 100% complete
  ✓ Merged to parent worktree
```

**Reduce Phase** (runs in parent worktree):
```
✓ Building book with mdbook
✓ Cleaning up temporary files
✓ Ready to merge to master
```

**Source**: Execution flow based on MapReduce workflow phases in `workflows/book-docs-drift.yml` and features.json analysis

### Step 6: Review and Merge Changes

After the workflow completes, you'll see:

```
Workflow completed successfully!

Merge session-abc123 to master? [Y/n]
```

Before merging, review the changes:

```bash
# Check what was modified
cd ~/.prodigy/worktrees/your-project/session-abc123/
git log --oneline
git diff master

# Review specific chapter changes
git show HEAD:book/src/getting-started.md
```

Type `Y` to merge changes back to your original branch.

### Step 7: Verify the Updated Documentation

After merging, view your updated documentation:

```bash
# Build and serve locally
cd book
mdbook serve --open

# Your browser opens to http://localhost:3000
# Navigate through chapters to see updated content
```

**Expected Results**:
- ✓ All code examples reference actual source files
- ✓ Configuration examples match your codebase types
- ✓ API documentation reflects current function signatures
- ✓ Outdated sections updated or removed
- ✓ Cross-references between chapters are valid

### Understanding the Workflow Phases

The automated documentation workflow uses Prodigy's MapReduce pattern with three phases:

#### Setup Phase
**Purpose**: Analyze your codebase and prepare work items

**Commands**:
1. `mkdir -p $ANALYSIS_DIR` - Create temporary analysis directory
2. `/prodigy-analyze-features-for-book` - Extract features from source files into features.json
3. `/prodigy-detect-documentation-gaps` - Compare features to chapters, create flattened-items.json

**Output**: JSON file with list of chapters/subsections to process

**Source**: Setup phase from `workflows/book-docs-drift.yml:24-34`

#### Map Phase
**Purpose**: Process each chapter/subsection in parallel to detect and fix drift

**For each chapter**:
1. `/prodigy-analyze-subsection-drift` - Compare chapter to codebase, identify outdated/missing content
2. `/prodigy-fix-subsection-drift` - Update markdown file with accurate, grounded examples
3. Validation - Ensure documentation meets quality standards (100% threshold)
4. Gap filling - If validation fails, run completion attempts (max 3)

**Parallelism**: Configured via `max_parallel: 3` - three chapters processed simultaneously

**Isolation**: Each chapter processed in its own git worktree, merged back to parent automatically

**Source**: Map phase from `workflows/book-docs-drift.yml:37-59`

#### Reduce Phase
**Purpose**: Validate the complete book and clean up

**Commands**:
1. `mdbook build` - Compile the book to catch broken links or formatting errors
2. `/prodigy-fix-book-build-errors` - Fix any build errors (only runs if build fails)
3. Cleanup temporary analysis files

**Source**: Reduce phase from `workflows/book-docs-drift.yml:62-69`

### Customization Tips

**Adjust Parallelism**:
```yaml
env:
  MAX_PARALLEL: "5"  # Process 5 chapters at once (default: 3)
```

**Focus on Specific Areas**:
Edit `.prodigy/book-config.json` to analyze only certain parts of your codebase:
```json
{
  "analysis_targets": [
    {
      "area": "api",
      "source_files": ["src/api/**/*.rs"],
      "feature_categories": ["endpoints", "authentication"]
    }
  ]
}
```

**Change Validation Threshold**:
```yaml
validate:
  threshold: 95  # Allow 95% instead of 100%
```

### Troubleshooting

**Issue**: "Claude command not found"
- **Solution**: Ensure Claude Code CLI is installed and in your PATH
- Verify: `claude --version`

**Issue**: "features.json not generated"
- **Cause**: Setup phase failed to analyze codebase
- **Solution**: Check that `source_files` in book-config.json exist and are valid
- Debug: Run with `-vv` for detailed logs

**Issue**: "mdbook build fails with broken links"
- **Cause**: Cross-references to non-existent chapters
- **Solution**: The workflow automatically fixes this in reduce phase
- Manual fix: Check `book/src/SUMMARY.md` for invalid links

**Issue**: "Validation threshold not met"
- **Cause**: Documentation doesn't meet 100% quality standard
- **Solution**: The `on_incomplete` handler attempts to complete gaps (max 3 attempts)
- If still incomplete: Review `.prodigy/validation-result.json` for details

**Source**: Common issues from features.json troubleshooting section (lines 802-885)

### Next Steps

Now that you have automated documentation working, explore:

- **[Understanding the Workflow](./understanding-the-workflow.md)** - Deep dive into how the workflow operates
- **[Customization Examples](./customization-examples.md)** - Adapt the workflow for your project's needs
- **[GitHub Actions Integration](./github-actions-integration.md)** - Automate documentation updates on every commit
- **[Best Practices](./best-practices.md)** - Guidelines for maintaining high-quality documentation
- **[Troubleshooting](./troubleshooting.md)** - Solutions to common problems

### What You've Learned

✓ How to configure automated documentation for any project
✓ The structure of a MapReduce documentation workflow
✓ How setup/map/reduce phases process your codebase
✓ How to review and merge documentation updates
✓ Basic troubleshooting techniques

### Time Investment Breakdown

- Prerequisites setup: 5-10 minutes (one-time)
- Configuration files: 10 minutes
- First workflow run: 5-10 minutes (depending on project size)
- Review and merge: 5 minutes

**Total: 25-35 minutes** for your first complete run
