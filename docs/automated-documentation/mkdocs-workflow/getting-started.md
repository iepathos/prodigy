# Getting Started

This guide covers the basics of running the MkDocs documentation workflow, including quick start instructions and configuration options.

## Overview

This workflow is designed for projects using **MkDocs Material** as their documentation system. It provides the same capabilities as the mdbook workflow but targets MkDocs-specific features and structure.

**Key Features:**
- Automatic gap detection for undocumented features
- Drift analysis comparing docs against source code
- Intelligent fixes with source attribution
- MkDocs build validation with `--strict` mode
- Navigation completeness checking
- Broken link detection

## Quick Start

### 1. Run the Workflow

```bash
prodigy run workflows/mkdocs-drift.yml
```

The workflow will:
1. Analyze your codebase for features
2. Detect documentation gaps
3. Create missing documentation pages
4. Analyze existing pages for drift
5. Fix drift with source references
6. Validate MkDocs build
7. Generate validation report

### 2. Review Generated Documentation

After completion, check:
- **Generated pages:** New markdown files in your docs directory
- **Validation report:** `.prodigy/mkdocs-analysis/validation.json`
- **Gap report:** `.prodigy/mkdocs-analysis/gap-report.json`

### 3. Merge Changes

The workflow runs in an isolated git worktree. When complete, you'll be prompted:

```
Merge session-abc123 to mkdocs? [y/N]
```

Review the changes and merge when satisfied.

## Configuration Options

### Environment Variables

All configuration is done through environment variables in the workflow YAML:

```yaml
env:
  # Project Configuration
  PROJECT_NAME: "Prodigy"              # Your project name
  PROJECT_CONFIG: ".prodigy/mkdocs-config.json"  # MkDocs-specific config
  FEATURES_PATH: ".prodigy/mkdocs-analysis/features.json"  # Feature inventory

  # MkDocs-Specific Settings
  DOCS_DIR: "book/src"                 # Documentation source directory
  MKDOCS_CONFIG: "mkdocs.yml"          # MkDocs configuration file
  ANALYSIS_DIR: ".prodigy/mkdocs-analysis"  # Analysis output directory
  CHAPTERS_FILE: "workflows/data/prodigy-chapters.json"  # Chapter definitions

  # Workflow Settings
  MAX_PARALLEL: "3"                    # Number of parallel agents
```

### Configuring Documentation Directory

The workflow supports flexible documentation directory configuration through the `DOCS_DIR` variable:

#### Option 1: Separate MkDocs Directory (Default)

```yaml
env:
  DOCS_DIR: "docs"
  CHAPTERS_FILE: "workflows/data/mkdocs-chapters.json"
```

**Use this when:**
- You want MkDocs-specific documentation separate from mdbook
- You need a curated subset of documentation for MkDocs
- You're testing both documentation systems

**Structure:**
```
docs/
├── index.md
├── workflow-basics/
│   ├── variables.md
│   └── environment.md
└── mapreduce/
    └── overview.md
mkdocs.yml (docs_dir: docs)
```

#### Option 2: Shared Source with mdbook

```yaml
env:
  DOCS_DIR: "book/src"
  CHAPTERS_FILE: "workflows/data/prodigy-chapters.json"
```

**Use this when:**
- You want a single source of truth for both mdbook and MkDocs
- You're migrating from mdbook to MkDocs
- You want complete documentation in both formats

**Structure:**
```
book/src/
├── index.md
├── SUMMARY.md (for mdbook)
├── workflow-basics/
│   ├── index.md
│   └── *.md
└── mapreduce/
    ├── index.md
    └── *.md
mkdocs.yml (docs_dir: book/src, exclude: SUMMARY.md)
```

**Important:** When using `book/src`, update `mkdocs.yml`:

```yaml
docs_dir: book/src
exclude_docs: |
  SUMMARY.md
```

### Chapter Definitions

Chapter definitions are stored in JSON files that define the documentation structure:

**For separate MkDocs docs:**
```json
// workflows/data/mkdocs-chapters.json
[
  {
    "id": "workflow-basics",
    "title": "Workflow Basics",
    "pages": [
      {"id": "variables", "title": "Variables"},
      {"id": "environment", "title": "Environment"}
    ]
  }
]
```

**For shared book/src:**
```json
// workflows/data/prodigy-chapters.json
// (More comprehensive structure matching mdbook SUMMARY.md)
```

### Parallelism Configuration

Control how many documentation pages are processed simultaneously:

```yaml
env:
  MAX_PARALLEL: "3"  # Process 3 pages at once

map:
  max_parallel: ${MAX_PARALLEL}
```

**Guidelines:**
- `1-3`: Conservative, good for development
- `4-6`: Balanced, recommended for most projects
- `7-10`: Aggressive, faster but higher resource usage
