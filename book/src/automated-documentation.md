# Automated Documentation with mdBook

This guide shows you how to set up automated, always-up-to-date documentation for any project using Prodigy's book workflow system. This same system maintains the documentation you're reading right now.

## Overview

The book workflow system:
- **Analyzes your codebase** to build a feature inventory
- **Detects documentation drift** by comparing docs to implementation
- **Updates documentation** automatically using Claude
- **Maintains consistency** across all chapters
- **Runs on any project** - just configure and go

The generalized commands work for any codebase: Rust, Python, JavaScript, etc.

## Prerequisites

1. **Install Prodigy**:
   ```bash
   cargo install prodigy
   ```

2. **Install mdBook**:
   ```bash
   cargo install mdbook
   ```

3. **Claude Code CLI** with valid API credentials

4. **Git repository** for your project

## Quick Start (30 Minutes)

### Step 1: Initialize Prodigy Commands

In your project directory:

```bash
# Initialize Prodigy and install book documentation commands
prodigy init
```

This creates `.claude/commands/` with the generalized book commands:
- `/prodigy-analyze-features-for-book` - Analyze codebase for feature inventory
- `/prodigy-analyze-book-chapter-drift` - Detect documentation drift per chapter
- `/prodigy-fix-book-drift` - Update chapters to fix drift
- `/prodigy-fix-book-build-errors` - Fix mdBook build errors

### Step 2: Initialize mdBook Structure

```bash
# Create book directory structure
mdbook init book --title "Your Project Documentation"

# Create workflow and config directories
mkdir -p workflows/data
mkdir -p .myproject  # Or .config, whatever you prefer
```

### Step 3: Create Project Configuration

Create `.myproject/book-config.json` (adjust paths and analysis targets for your project):

```json
{
  "project_name": "YourProject",
  "project_type": "cli_tool",
  "book_dir": "book",
  "book_src": "book/src",
  "book_build_dir": "book/book",
  "analysis_targets": [
    {
      "area": "cli_commands",
      "source_files": ["src/cli/", "src/commands/"],
      "feature_categories": ["commands", "arguments", "options"]
    },
    {
      "area": "core_features",
      "source_files": ["src/lib.rs", "src/core/"],
      "feature_categories": ["api", "public_functions", "exports"]
    },
    {
      "area": "configuration",
      "source_files": ["src/config/"],
      "feature_categories": ["config_options", "defaults", "validation"]
    }
  ],
  "chapter_file": "workflows/data/chapters.json",
  "custom_analysis": {
    "include_examples": true,
    "include_best_practices": true,
    "include_troubleshooting": true
  }
}
```

**Key Fields to Customize**:
- `project_name`: Your project's name (used in prompts)
- `project_type`: `cli_tool`, `library`, `web_service`, etc.
- `analysis_targets`: Areas of code to analyze for documentation
  - `area`: Logical grouping name
  - `source_files`: Paths to analyze (relative to project root)
  - `feature_categories`: Types of features to extract

### Step 4: Define Chapter Structure

Create `workflows/data/chapters.json`:

```json
{
  "chapters": [
    {
      "id": "getting-started",
      "title": "Getting Started",
      "file": "book/src/getting-started.md",
      "topics": ["Installation", "Quick start", "First steps"],
      "validation": "Check installation instructions and basic usage"
    },
    {
      "id": "user-guide",
      "title": "User Guide",
      "file": "book/src/user-guide.md",
      "topics": ["Core features", "Common workflows", "Examples"],
      "validation": "Verify all main features are documented"
    },
    {
      "id": "configuration",
      "title": "Configuration",
      "file": "book/src/configuration.md",
      "topics": ["Config files", "Options", "Defaults"],
      "validation": "Check config options match implementation"
    },
    {
      "id": "troubleshooting",
      "title": "Troubleshooting",
      "file": "book/src/troubleshooting.md",
      "topics": ["Common issues", "Debug mode", "FAQ"],
      "validation": "Ensure common issues are covered"
    }
  ]
}
```

**Chapter Definition Fields**:
- `id`: Unique identifier for the chapter
- `title`: Display title in the book
- `file`: Path to markdown file (relative to project root)
- `topics`: What should be covered in this chapter
- `validation`: What Claude should check for accuracy

### Step 5: Create Book Configuration

Edit `book/book.toml`:

```toml
[book]
title = "Your Project Documentation"
authors = ["Your Team"]
description = "Comprehensive guide to Your Project"
src = "src"
language = "en"

[build]
build-dir = "book"
create-missing = false

[output.html]
default-theme = "rust"
preferred-dark-theme = "navy"
smart-punctuation = true
git-repository-url = "https://github.com/yourorg/yourproject"
git-repository-icon = "fa-github"
edit-url-template = "https://github.com/yourorg/yourproject/edit/main/book/{path}"

[output.html.search]
enable = true
limit-results = 30
use-boolean-and = true
boost-title = 2

[output.html.fold]
enable = true
level = 1

[output.html.playground]
editable = false
copyable = true
line-numbers = true
```

### Step 6: Create Chapter Files

Create placeholder files for each chapter:

```bash
# Create initial chapters with basic structure
cat > book/src/getting-started.md <<EOF
# Getting Started

This chapter covers installation and initial setup.

## Installation

TODO: Add installation instructions

## Quick Start

TODO: Add quick start guide
EOF

# Repeat for other chapters...
```

Update `book/src/SUMMARY.md`:

```markdown
# Summary

[Introduction](intro.md)

# User Guide

- [Getting Started](getting-started.md)
- [User Guide](user-guide.md)
- [Configuration](configuration.md)

# Reference

- [Troubleshooting](troubleshooting.md)
```

### Step 7: Create the Workflow

Create `workflows/book-docs-drift.yml`:

```yaml
name: book-docs-drift-detection
mode: mapreduce

env:
  PROJECT_NAME: "YourProject"
  PROJECT_CONFIG: ".myproject/book-config.json"
  FEATURES_PATH: ".myproject/book-analysis/features.json"

setup:
  - shell: "mkdir -p .myproject/book-analysis"

  # Analyze codebase and build feature inventory
  - claude: "/prodigy-analyze-features-for-book --project $PROJECT_NAME --config $PROJECT_CONFIG"

map:
  input: "workflows/data/chapters.json"
  json_path: "$.chapters[*]"

  agent_template:
    # Analyze each chapter for drift
    - claude: "/prodigy-analyze-book-chapter-drift --project $PROJECT_NAME --json '${item}' --features $FEATURES_PATH"
      commit_required: true

  max_parallel: 3
  agent_timeout_secs: 900

reduce:
  # Aggregate all drift reports and fix issues
  - claude: "/prodigy-fix-book-drift --project $PROJECT_NAME --config $PROJECT_CONFIG"
    commit_required: true

  # Build the book
  - shell: "cd book && mdbook build"
    on_failure:
      claude: "/prodigy-fix-book-build-errors --project $PROJECT_NAME"

error_policy:
  on_item_failure: dlq
  continue_on_failure: true
  max_failures: 2

merge:
  commands:
    # Clean up temporary analysis files
    - shell: "rm -rf .myproject/book-analysis"
    - shell: "git add -A && git commit -m 'chore: remove temporary book analysis files' || true"

    # Final build verification
    - shell: "cd book && mdbook build"

    # Merge back to main branch
    - shell: "git fetch origin"
    - claude: "/merge-master"
    - claude: "/prodigy-merge-worktree ${merge.source_branch}"
```

**Workflow Sections**:

- **env**: Environment variables for project-specific configuration
- **setup**: Initialize analysis directory and build feature inventory
- **map**: Process each chapter in parallel to detect drift
- **reduce**: Aggregate results and update documentation
- **merge**: Cleanup and merge changes back to main branch

**Key Variables**:
- `PROJECT_NAME`: Used in prompts and context
- `PROJECT_CONFIG`: Path to your book-config.json
- `FEATURES_PATH`: Where feature inventory is stored

### Step 8: Run the Workflow

```bash
# Run the documentation workflow
prodigy run workflows/book-docs-drift.yml

# The workflow will:
# 1. Analyze your codebase for features
# 2. Check each chapter for documentation drift
# 3. Update chapters to match current implementation
# 4. Build the book to verify everything works
# 5. Merge changes back to your main branch
```

## Understanding the Workflow

### Phase 1: Setup - Feature Analysis

The setup phase analyzes your codebase and creates a feature inventory:

```yaml
setup:
  - shell: "mkdir -p .myproject/book-analysis"
  - claude: "/prodigy-analyze-features-for-book --project $PROJECT_NAME --config $PROJECT_CONFIG"
```

This generates `.myproject/book-analysis/features.json`:

```json
{
  "cli_commands": [
    {
      "name": "run",
      "description": "Execute a workflow",
      "arguments": ["workflow_file"],
      "options": ["--resume", "--dry-run"]
    }
  ],
  "api_functions": [
    {
      "name": "execute_workflow",
      "signature": "fn execute_workflow(config: Config) -> Result<()>",
      "purpose": "Main entry point for workflow execution"
    }
  ]
}
```

### Phase 2: Map - Chapter Drift Detection

Each chapter is processed in parallel to detect drift:

```yaml
map:
  input: "workflows/data/chapters.json"
  json_path: "$.chapters[*]"

  agent_template:
    - claude: "/prodigy-analyze-book-chapter-drift --project $PROJECT_NAME --json '${item}' --features $FEATURES_PATH"
      commit_required: true
```

For each chapter, Claude:
1. Reads the current chapter content
2. Compares it to the feature inventory
3. Identifies missing, outdated, or incorrect information
4. Creates a drift report

### Phase 3: Reduce - Fix Drift

The reduce phase aggregates all drift reports and updates chapters:

```yaml
reduce:
  - claude: "/prodigy-fix-book-drift --project $PROJECT_NAME --config $PROJECT_CONFIG"
    commit_required: true

  - shell: "cd book && mdbook build"
    on_failure:
      claude: "/prodigy-fix-book-build-errors --project $PROJECT_NAME"
```

Claude:
1. Reviews all drift reports
2. Updates chapters to fix issues
3. Ensures consistency across chapters
4. Verifies the book builds successfully

### Phase 4: Merge - Integration

The merge phase cleans up and integrates changes:

```yaml
merge:
  commands:
    - shell: "rm -rf .myproject/book-analysis"
    - shell: "git add -A && git commit -m 'chore: remove temporary book analysis files' || true"
    - shell: "cd book && mdbook build"
    - shell: "git fetch origin"
    - claude: "/merge-master"
    - claude: "/prodigy-merge-worktree ${merge.source_branch}"
```

## GitHub Actions Integration

### Automated Documentation Deployment

Create `.github/workflows/deploy-docs.yml`:

```yaml
name: Deploy Documentation

on:
  push:
    branches: [main, master]
    paths:
      - 'book/**'
      - 'workflows/book-docs-drift.yml'
  workflow_dispatch:  # Allow manual triggers

permissions:
  contents: write
  pages: write
  id-token: write

jobs:
  build-deploy:
    runs-on: ubuntu-latest
    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Setup mdBook
        uses: peaceiris/actions-mdbook@v2
        with:
          mdbook-version: 'latest'

      - name: Build book
        run: |
          cd book
          mdbook build

      - name: Deploy to GitHub Pages
        uses: peaceiris/actions-gh-pages@v4
        with:
          github_token: ${{ secrets.GITHUB_TOKEN }}
          publish_dir: ./book/book
          cname: docs.yourproject.com  # Optional: custom domain
```

### Periodic Documentation Updates

Create `.github/workflows/update-docs.yml`:

```yaml
name: Update Documentation

on:
  schedule:
    # Run weekly on Monday at 9 AM UTC
    - cron: '0 9 * * 1'
  workflow_dispatch:  # Allow manual triggers

jobs:
  update-docs:
    runs-on: ubuntu-latest
    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Setup Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable

      - name: Install Prodigy
        run: cargo install prodigy

      - name: Install mdBook
        uses: peaceiris/actions-mdbook@v2
        with:
          mdbook-version: 'latest'

      - name: Configure Claude API
        env:
          ANTHROPIC_API_KEY: ${{ secrets.ANTHROPIC_API_KEY }}
        run: |
          # Configure Claude Code CLI with API key
          echo "$ANTHROPIC_API_KEY" | claude-code auth login

      - name: Run documentation workflow
        run: |
          prodigy run workflows/book-docs-drift.yml

      - name: Create Pull Request
        uses: peter-evans/create-pull-request@v5
        with:
          token: ${{ secrets.GITHUB_TOKEN }}
          commit-message: 'docs: automated documentation update'
          title: 'Automated Documentation Update'
          body: |
            This PR was automatically created by the documentation workflow.

            Changes:
            - Updated documentation to match current codebase
            - Fixed any detected drift between docs and implementation

            Please review the changes before merging.
          branch: docs/automated-update
          delete-branch: true
```

**Required Secrets**:
- `ANTHROPIC_API_KEY`: Your Claude API key (add in repository settings)

### Enable GitHub Pages

1. Go to repository Settings → Pages
2. Source: Deploy from a branch
3. Branch: `gh-pages` / `root`
4. Save

Your documentation will be available at: `https://yourorg.github.io/yourproject`

## Customization Examples

### For CLI Tools

Focus on commands and usage:

```json
{
  "analysis_targets": [
    {
      "area": "cli_commands",
      "source_files": ["src/cli/", "src/commands/"],
      "feature_categories": ["commands", "subcommands", "options", "arguments"]
    },
    {
      "area": "configuration",
      "source_files": ["src/config/"],
      "feature_categories": ["config_file", "environment_vars", "flags"]
    }
  ]
}
```

Chapter structure:
- Installation
- Quick Start
- Commands Reference
- Configuration
- Examples
- Troubleshooting

### For Libraries

Focus on API and usage patterns:

```json
{
  "analysis_targets": [
    {
      "area": "public_api",
      "source_files": ["src/lib.rs", "src/api/"],
      "feature_categories": ["functions", "types", "traits", "macros"]
    },
    {
      "area": "examples",
      "source_files": ["examples/"],
      "feature_categories": ["use_cases", "patterns", "integrations"]
    }
  ]
}
```

Chapter structure:
- Getting Started
- API Reference
- Core Concepts
- Advanced Usage
- Examples
- Migration Guides

### For Web Services

Focus on endpoints and integration:

```json
{
  "analysis_targets": [
    {
      "area": "api_endpoints",
      "source_files": ["src/routes/", "src/handlers/"],
      "feature_categories": ["endpoints", "methods", "parameters", "responses"]
    },
    {
      "area": "authentication",
      "source_files": ["src/auth/"],
      "feature_categories": ["auth_methods", "tokens", "permissions"]
    },
    {
      "area": "deployment",
      "source_files": ["deploy/", "docker/"],
      "feature_categories": ["docker", "kubernetes", "configuration"]
    }
  ]
}
```

Chapter structure:
- Overview
- Authentication
- API Reference
- Integration Guide
- Deployment
- Monitoring

## Best Practices

### 1. Start with Minimal Chapters

Don't try to document everything at once:

```json
{
  "chapters": [
    {"id": "intro", "title": "Introduction", ...},
    {"id": "quickstart", "title": "Quick Start", ...},
    {"id": "reference", "title": "Reference", ...}
  ]
}
```

Add more chapters as your project grows.

### 2. Focus Analysis Targets

Be specific about what to analyze:

```json
{
  "area": "cli_commands",
  "source_files": ["src/cli/commands/"],  // Specific path
  "feature_categories": ["commands", "options"]  // Specific categories
}
```

Overly broad targets create unfocused documentation.

### 3. Provide Chapter Context

Give Claude clear guidance on what each chapter should cover:

```json
{
  "id": "advanced",
  "title": "Advanced Features",
  "topics": ["Custom plugins", "Scripting", "Automation"],
  "validation": "Check that plugin API and scripting examples are up-to-date"
}
```

### 4. Review Initial Output

The first workflow run will:
- Identify what's missing
- Add current implementation details
- Create a baseline

Review and refine before committing.

### 5. Run Regularly

Documentation drift happens constantly:

```bash
# Run monthly or after major features
prodigy run workflows/book-docs-drift.yml

# Or set up GitHub Actions for automation
```

### 6. Use Validation Topics

Specify what Claude should validate:

```json
{
  "validation": "Check that all CLI commands in src/cli/commands/ are documented with current options and examples"
}
```

This ensures focused, accurate updates.

## Troubleshooting

### Issue: Feature Analysis Produces Empty Results

**Cause**: Analysis targets don't match your code structure

**Solution**: Check that `source_files` paths exist:
```bash
ls -la src/cli/  # Verify paths in analysis_targets
```

Adjust paths in `book-config.json` to match your actual structure.

### Issue: Chapters Not Updating

**Cause**: Chapter files don't exist or paths are wrong

**Solution**: Verify chapter files exist:
```bash
# Check all chapters listed in chapters.json exist
cat workflows/data/chapters.json | jq -r '.chapters[].file' | xargs ls -la
```

### Issue: mdBook Build Fails

**Cause**: SUMMARY.md doesn't match chapter files

**Solution**: Ensure all chapters in `SUMMARY.md` have corresponding files:
```bash
cd book && mdbook build
```

Fix any missing files or broken links.

### Issue: Workflow Takes Too Long

**Cause**: Too many chapters or overly broad analysis

**Solution**:
1. Reduce `max_parallel` in map phase (default: 3)
2. Split large chapters into smaller ones
3. Narrow `analysis_targets` to essential code paths

### Issue: Documentation Quality Issues

**Cause**: Insufficient initial content or unclear validation

**Solution**:
1. Create better chapter outlines before running workflow
2. Add more specific `validation` criteria in chapters.json
3. Review and manually refine after first run

## Advanced Configuration

### Custom Analysis Functions

You can extend the analysis by providing custom analysis functions in your config:

```json
{
  "custom_analysis": {
    "include_examples": true,
    "include_best_practices": true,
    "include_troubleshooting": true,
    "analyze_dependencies": true,
    "extract_code_comments": true,
    "include_performance_notes": true
  }
}
```

### Multi-Language Projects

For projects with multiple languages:

```json
{
  "analysis_targets": [
    {
      "area": "rust_backend",
      "source_files": ["src/"],
      "feature_categories": ["api", "services"],
      "language": "rust"
    },
    {
      "area": "typescript_frontend",
      "source_files": ["web/src/"],
      "feature_categories": ["components", "hooks"],
      "language": "typescript"
    }
  ]
}
```

### Chapter Dependencies

Some chapters may depend on others:

```json
{
  "chapters": [
    {
      "id": "basics",
      "title": "Basic Usage",
      "dependencies": []
    },
    {
      "id": "advanced",
      "title": "Advanced Usage",
      "dependencies": ["basics"],
      "validation": "Ensure examples build on concepts from Basic Usage chapter"
    }
  ]
}
```

## Real-World Example: Prodigy's Own Documentation

This documentation you're reading is maintained by the same workflow described here. You can examine the configuration:

**Configuration**: `.prodigy/book-config.json`
```json
{
  "project_name": "Prodigy",
  "project_type": "cli_tool",
  "analysis_targets": [
    {
      "area": "workflow_execution",
      "source_files": ["src/workflow/", "src/orchestrator/"],
      "feature_categories": ["workflow_types", "execution_modes", "lifecycle"]
    },
    {
      "area": "mapreduce",
      "source_files": ["src/mapreduce/"],
      "feature_categories": ["map_phase", "reduce_phase", "parallelism"]
    }
  ]
}
```

**Chapters**: `workflows/data/prodigy-chapters.json`
```json
{
  "chapters": [
    {
      "id": "workflow-basics",
      "title": "Workflow Basics",
      "file": "book/src/workflow-basics.md",
      "topics": ["Standard workflows", "Basic structure"],
      "validation": "Check workflow syntax matches current implementation"
    }
  ]
}
```

**Workflow**: `workflows/book-docs-drift.yml`

Study these files for a complete working example.

## Next Steps

1. **Set up the basics**: Follow the Quick Start to get a minimal book running
2. **Customize for your project**: Adjust analysis targets and chapters
3. **Run the workflow**: Generate your first automated update
4. **Refine iteratively**: Review output and improve configuration
5. **Automate**: Set up GitHub Actions for continuous documentation
6. **Extend**: Add more chapters as your project grows

## Benefits

This approach provides:

- ✅ **Always up-to-date documentation** - Runs automatically to detect drift
- ✅ **Consistent quality** - Same analysis across all chapters
- ✅ **Reduced maintenance** - Less manual documentation work
- ✅ **Accurate examples** - Extracted from actual code
- ✅ **Version control** - All changes tracked in git
- ✅ **Easy to customize** - Configuration-based, works for any project

The same commands that maintain Prodigy's documentation can maintain yours.
