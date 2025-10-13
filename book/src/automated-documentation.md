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

4. **Git** - Version control system (git 2.25+ recommended) and an initialized git repository for your project

   ```bash
   # Verify git is installed
   git --version

   # Initialize a repository if needed
   git init
   ```

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
  ANALYSIS_DIR: ".myproject/book-analysis"

setup:
  - shell: "mkdir -p ${ANALYSIS_DIR}"

  # Analyze codebase and build feature inventory
  - claude: "/prodigy-analyze-features-for-book --project $PROJECT_NAME --config $PROJECT_CONFIG"

map:
  input: "workflows/data/chapters.json"
  json_path: "$.chapters[*]"

  agent_template:
    # Step 1: Analyze the chapter for drift
    - claude: "/prodigy-analyze-book-chapter-drift --project $PROJECT_NAME --json '${item}' --features $FEATURES_PATH"
      commit_required: true

    # Step 2: Fix the drift in this chapter
    - claude: "/prodigy-fix-chapter-drift --project $PROJECT_NAME --chapter-id ${item.id}"
      commit_required: true

  max_parallel: 3
  agent_timeout_secs: 900  # 15-minute timeout per agent

reduce:
  # Rebuild the book to ensure all chapters compile together
  - shell: "(cd book && mdbook build)"
    on_failure:
      # Only needed if there are build errors (broken links, etc)
      claude: "/prodigy-fix-book-build-errors --project $PROJECT_NAME"
      commit_required: true

error_policy:
  on_item_failure: dlq
  continue_on_failure: true
  max_failures: 2
  error_collection: aggregate

# Configuration parameters:
# - max_parallel: 3 chapters processed concurrently
# - agent_timeout_secs: 900 sets a 15-minute timeout per agent (900 seconds = 15 minutes)
#   This prevents any single chapter from hanging the entire workflow
#   Adjust this value based on your expected chapter processing time
#
# Error Policy fields:
# - on_item_failure: dlq - Failed chapters are sent to the Dead Letter Queue for manual review and retry
# - continue_on_failure: true - Workflow continues processing other chapters even if some fail
# - max_failures: 2 - Stop the entire workflow if more than 2 chapters fail (prevents cascading failures)
# - error_collection: aggregate - Collect all errors and report them together at the end

merge:
  commands:
    # Step 1: Clean up temporary analysis files
    - shell: "rm -rf ${ANALYSIS_DIR}"
    # The '|| true' prevents the merge phase from failing if there are no changes to commit
    # (e.g., if cleanup didn't modify any tracked files). This is a safety pattern for optional cleanup steps.
    - shell: "git add -A && git commit -m 'chore: remove temporary book analysis files for ${PROJECT_NAME}' || true"

    # Step 2: Validate book builds successfully
    - shell: "(cd book && mdbook build)"

    # Step 3: Fetch latest changes and merge master into worktree
    - shell: "git fetch origin"
    - claude: "/prodigy-merge-master --project ${PROJECT_NAME}"

    # Step 4: Merge worktree back to master
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
# 1. Setup: Analyze your codebase for features (creates feature inventory)
# 2. Map: For each chapter in parallel:
#    a. Analyze chapter for drift (creates drift report)
#    b. Fix the chapter based on drift report
# 3. Reduce: Build the complete book to verify all chapters work together
# 4. Merge: Clean up temp files and merge changes back to main branch
```

## Understanding the Workflow

> **Note**: All workflow phases (setup, map, reduce, merge) execute in an isolated git worktree, ensuring the main repository remains untouched during execution. This isolation is a key feature of Prodigy's MapReduce implementation (Spec 127).

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

### Phase 2: Map - Chapter Drift Detection and Fixing

**Execution Model**: The map phase processes chapters with controlled parallelism (max_parallel: 3 chapters at a time). For each chapter, two steps execute sequentially in the same isolated agent worktree:

1. **Analyze** - Detect drift and create a drift report
2. **Fix** - Read the drift report and update the chapter

Each agent runs in its own isolated git worktree, allowing multiple chapters to be processed concurrently without interference. This sequential execution within each agent ensures the drift report from step 1 is available to step 2. Meanwhile, multiple chapters are processed in parallel across different agent worktrees.

```yaml
map:
  input: "workflows/data/chapters.json"
  json_path: "$.chapters[*]"

  agent_template:
    # Step 1: Analyze the chapter for drift
    - claude: "/prodigy-analyze-book-chapter-drift --project $PROJECT_NAME --json '${item}' --features $FEATURES_PATH"
      commit_required: true

    # Step 2: Fix the drift in this chapter
    - claude: "/prodigy-fix-chapter-drift --project $PROJECT_NAME --chapter-id ${item.id}"
      commit_required: true
```

**Detailed Breakdown**:

**Step 1: Analyze** - For each chapter, Claude:
1. Reads the current chapter content
2. Compares it to the feature inventory
3. Identifies missing, outdated, or incorrect information
4. Creates a drift report (`.prodigy/book-analysis/drift-{chapter-id}.json`)

**Step 2: Fix** - Then immediately for the same chapter, Claude:
1. Reads the drift report created in step 1
2. Updates the chapter file to fix all identified issues
3. Commits the fixes to the worktree

Both steps run sequentially for each chapter, and chapters are processed in parallel.

**Why commit_required: true is Critical**

Each map agent runs in its own isolated git worktree. The `commit_required: true` flag ensures the drift report is committed to git in that worktree. This is critical because without the commit, the drift report file created by step 1 would not be accessible to step 2, even though they run sequentially in the same agent worktree.

### Phase 3: Reduce - Validate Book Build

The reduce phase validates that all updated chapters build successfully together:

```yaml
reduce:
  # Rebuild the book to ensure all chapters compile together
  - shell: "(cd book && mdbook build)"
    on_failure:
      # Only needed if there are build errors (broken links, etc)
      claude: "/prodigy-fix-book-build-errors --project $PROJECT_NAME"
      commit_required: true
```

Since chapter fixes happen in the map phase, the reduce phase focuses on:
1. Building the complete book with all updated chapters
2. Detecting any build errors (broken cross-references, invalid links, etc.)
3. Fixing build errors if they occur (via Claude command)

This ensures that all chapters work together correctly after parallel updates.

## Automatic Gap Detection

The book workflow system includes automatic gap detection to identify missing or incomplete documentation. This ensures your documentation coverage is comprehensive and catches areas that need attention.

### How It Works

Gap detection runs as part of the documentation workflow and analyzes your documentation against your codebase to identify:

1. **Missing Documentation**: Features, APIs, or commands in the code that aren't documented
2. **Incomplete Coverage**: Documentation sections that exist but don't cover all aspects
3. **Structural Issues**: Missing chapters or sections that should exist based on your project structure

The gap detection process:

1. **Feature Inventory Analysis**: The setup phase creates a complete inventory of your codebase features
2. **Documentation Coverage Analysis**: Each chapter is analyzed to determine what features it documents
3. **Gap Identification**: Missing features are identified by comparing documentation coverage to the feature inventory
4. **Prioritization**: Gaps are assigned severity levels (critical, high, medium, low) based on importance
5. **Reporting**: Gaps are saved to `.prodigy/book-analysis/gaps-report.json` with details for each issue

### Gap Severity Levels

Gaps are categorized by severity to help prioritize documentation work:

- **Critical**: Core functionality or main features that are completely undocumented
  - Example: Primary CLI commands with no usage documentation
  - Example: Public API functions that are exported but not documented

- **High**: Important features or commonly-used functionality with missing documentation
  - Example: Configuration options that affect behavior but aren't documented
  - Example: Command-line flags that are widely used but lack examples

- **Medium**: Secondary features or less critical areas with incomplete coverage
  - Example: Advanced features that are documented but lack detailed examples
  - Example: Edge cases or error handling that isn't fully explained

- **Low**: Minor gaps or enhancements that would improve documentation quality
  - Example: Missing troubleshooting tips for uncommon issues
  - Example: Additional examples that would be helpful but aren't essential

### Gap Report Format

The gap detection system generates a detailed report at `.prodigy/book-analysis/gaps-report.json`:

```json
{
  "timestamp": "2025-01-15T10:30:00Z",
  "project_name": "YourProject",
  "total_gaps": 5,
  "gaps_by_severity": {
    "critical": 1,
    "high": 2,
    "medium": 1,
    "low": 1
  },
  "gaps": [
    {
      "id": "missing-cli-command-docs",
      "severity": "critical",
      "type": "missing_documentation",
      "description": "CLI command 'process' is not documented",
      "location": "book/src/user-guide.md",
      "affected_feature": {
        "name": "process",
        "type": "cli_command",
        "source": "src/cli/commands/process.rs"
      },
      "suggested_fix": "Add section documenting the 'process' command with usage examples and options",
      "detected_at": "2025-01-15T10:30:00Z"
    }
  ]
}
```

### Customization

You can customize gap detection behavior in your `book-config.json`:

```json
{
  "project_name": "YourProject",
  "gap_detection": {
    "enabled": true,
    "severity_rules": {
      "undocumented_public_api": "critical",
      "undocumented_cli_command": "critical",
      "missing_examples": "medium",
      "incomplete_troubleshooting": "low"
    },
    "ignore_patterns": [
      "internal_*",
      "test_helpers",
      "deprecated_*"
    ],
    "required_sections": [
      "Installation",
      "Quick Start",
      "Configuration",
      "Troubleshooting"
    ]
  }
}
```

**Configuration Options**:

- `enabled`: Toggle gap detection on/off (default: true)
- `severity_rules`: Custom rules for assigning severity levels to different gap types
- `ignore_patterns`: Feature name patterns to exclude from gap detection
- `required_sections`: Section names that must exist in documentation

### Manual Review Recommendations

While gap detection is automatic, manual review is recommended for:

1. **Critical and High Severity Gaps**: Review these immediately as they indicate missing core documentation
2. **New Features**: When adding new features to your codebase, check the gap report to ensure they're documented
3. **After Major Refactoring**: Restructuring code may create new gaps or invalidate existing documentation
4. **Before Releases**: Run gap detection before major releases to ensure complete documentation coverage
5. **Severity Accuracy**: Verify that automatically assigned severity levels match your project's priorities

**Best Practices for Manual Review**:

- Run gap detection regularly (weekly or after significant code changes)
- Address critical gaps before merging feature branches
- Use gap reports to plan documentation work in sprints
- Keep the gaps report file in version control to track progress
- Review ignored patterns periodically to ensure they're still relevant

**Analyzing the Gaps Report:**

Use these commands to query and analyze the gaps report:

```bash
# View all detected gaps
cat .prodigy/book-analysis/gaps-report.json | jq '.gaps'

# Filter to show only critical gaps
cat .prodigy/book-analysis/gaps-report.json | jq '.gaps[] | select(.severity == "critical")'

# Count gaps by severity level
cat .prodigy/book-analysis/gaps-report.json | jq '.gaps_by_severity'

# Get gap details for a specific chapter
cat .prodigy/book-analysis/gaps-report.json | jq '.gaps[] | select(.location | contains("user-guide.md"))'

# List all affected features
cat .prodigy/book-analysis/gaps-report.json | jq '.gaps[].affected_feature.name'
```

### Integration with Drift Detection

Gap detection complements drift detection in the book workflow:

- **Drift Detection**: Identifies documentation that's outdated or incorrect compared to current implementation
- **Gap Detection**: Identifies missing documentation that should exist but doesn't

Together, these systems ensure your documentation is both accurate and complete:

1. **Drift Detection** keeps existing documentation synchronized with code changes
2. **Gap Detection** identifies areas where documentation is missing entirely
3. Both run as part of the same workflow for comprehensive documentation quality

### Phase 4: Merge - Integration

The merge phase cleans up and integrates changes:

```yaml
merge:
  commands:
    # Step 1: Clean up temporary analysis files
    - shell: "rm -rf ${ANALYSIS_DIR}"
    # The '|| true' prevents the merge phase from failing if there are no changes to commit
    # (e.g., if cleanup didn't modify any tracked files). This is a safety pattern for optional cleanup steps.
    - shell: "git add -A && git commit -m 'chore: remove temporary book analysis files for ${PROJECT_NAME}' || true"

    # Step 2: Validate book builds successfully
    - shell: "(cd book && mdbook build)"

    # Step 3: Fetch latest changes and merge master into worktree
    - shell: "git fetch origin"
    - claude: "/prodigy-merge-master --project ${PROJECT_NAME}"

    # Step 4: Merge worktree back to master
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
      - '.github/workflows/deploy-docs.yml'
  pull_request:
    branches: [main, master]
    paths:
      - 'book/**'
      - '.github/workflows/deploy-docs.yml'

jobs:
  build-deploy:
    runs-on: ubuntu-latest
    permissions:
      contents: write  # Required to push to gh-pages branch
    steps:
      - name: Checkout repository
        uses: actions/checkout@v5

      - name: Setup mdBook
        uses: peaceiris/actions-mdbook@v2
        with:
          mdbook-version: 'latest'

      - name: Build book
        run: mdbook build book

      - name: Deploy to GitHub Pages
        if: github.event_name == 'push' && (github.ref == 'refs/heads/main' || github.ref == 'refs/heads/master')
        uses: peaceiris/actions-gh-pages@v4
        with:
          github_token: ${{ secrets.GITHUB_TOKEN }}
          publish_dir: ./book/book
```

### Periodic Documentation Updates

> **Note**: Automated documentation updates in CI/CD are not yet fully supported. Claude Code CLI installation and authentication in GitHub Actions is still in development.
>
> For now, run the book workflow manually:
> ```bash
> prodigy run workflows/book-docs-drift.yml
> ```
>
> When CI support is added, Prodigy's json_log_location tracking (Spec 121) will enable debugging Claude commands in CI by capturing detailed JSON logs for each command execution. This will make it easy to troubleshoot documentation updates that fail in CI environments.
>
> Watch the Prodigy and Claude Code documentation for updates on CI integration.

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

### Issue: Some Chapters Failed to Update

**Cause**: Chapter processing timeout, Claude error, or validation failure

**Solution**: Use the Dead Letter Queue (DLQ) to retry failed chapters:

```bash
# View failed chapters
prodigy dlq show <job_id>

# Retry all failed chapters
prodigy dlq retry <job_id>

# Retry with custom parallelism
prodigy dlq retry <job_id> --max-parallel 2

# Dry run to see what would be retried
prodigy dlq retry <job_id> --dry-run
```

The DLQ preserves all context from the original failure, making it safe to retry after fixing any underlying issues. Failed items in the DLQ include the `json_log_location` field pointing to detailed Claude execution logs. Use this to debug exactly what went wrong during chapter processing.

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
