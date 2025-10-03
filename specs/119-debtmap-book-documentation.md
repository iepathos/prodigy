---
number: 119
title: Add Book Documentation to Debtmap
category: documentation
priority: high
status: draft
dependencies: [118]
created: 2025-10-03
---

# Specification 119: Add Book Documentation to Debtmap

**Category**: documentation
**Priority**: high
**Status**: draft
**Dependencies**: [118]

## Context

Spec 118 generalized Prodigy's book documentation system to use parameter-based commands and configuration files. This proved the generalization works and created reusable infrastructure.

Debtmap is a Rust CLI tool for technical debt analysis and management. It currently lacks comprehensive documentation. This spec applies the generalized book documentation system (from Spec 118) to create automated documentation for Debtmap using mdBook and Prodigy workflows.

This serves as the proof-of-concept that the generalized system works across different projects with minimal customization.

## Objective

Set up automated book documentation for Debtmap by:
1. Creating mdBook structure for Debtmap documentation
2. Creating Debtmap-specific configuration and chapter definitions
3. Creating a Debtmap workflow instance using the generalized commands
4. Generating initial book content documenting Debtmap's features
5. Setting up GitHub workflow for automated deployment (optional)

## Requirements

### Functional Requirements

**FR1**: Create mdBook structure for Debtmap:
- Initialize mdBook in `../debtmap/book/`
- Create `book.toml` with Debtmap-specific metadata
- Create initial `SUMMARY.md` with chapter structure
- Create placeholder chapter files

**FR2**: Create Debtmap project configuration:
- Create `../debtmap/.debtmap/book-config.json` with Debtmap settings
- Define analysis targets for Debtmap codebase (CLI commands, rules, analysis types)
- Specify book paths and build configuration

**FR3**: Create Debtmap chapter definitions:
- Create `../debtmap/workflows/data/debtmap-chapters.json`
- Define chapters appropriate for Debtmap (e.g., "Getting Started", "Rules", "Analysis Types", "CLI Usage")
- Specify topics and validation focuses per chapter

**FR4**: Create Debtmap Claude commands:
- Create `/debtmap-analyze-features-for-book` (follows Prodigy pattern)
- Create `/debtmap-analyze-book-chapter-drift` (follows Prodigy pattern)
- Create `/debtmap-fix-book-drift` (follows Prodigy pattern)
- Commands use `debtmap-` prefix (required for Debtmap self-recognition)
- Implementation follows configuration-driven pattern from Spec 118

**FR5**: Create Debtmap workflow instance:
- Create `../debtmap/workflows/book-docs-drift.yml`
- Use Debtmap-specific commands
- Follow same pattern as Prodigy workflow

**FR6**: Generate initial documentation:
- Run workflow to analyze Debtmap codebase
- Generate feature inventory for Debtmap
- Create initial book content based on analysis
- Verify book builds successfully

### Non-Functional Requirements

**NFR1**: **Minimal Customization**: Should require <30 minutes to set up for someone familiar with the pattern

**NFR2**: **Consistency**: Documentation quality and structure comparable to Prodigy book

**NFR3**: **Maintainability**: Easy to update chapters and configuration as Debtmap evolves

**NFR4**: **Independence**: Debtmap book workflow runs independently from Prodigy

## Acceptance Criteria

- [ ] mdBook structure created in `../debtmap/book/`
- [ ] `../debtmap/book/book.toml` configured with Debtmap metadata
- [ ] `../debtmap/.debtmap/book-config.json` created with Debtmap configuration
- [ ] `../debtmap/workflows/data/debtmap-chapters.json` created with chapter definitions
- [ ] Debtmap Claude commands created in `../debtmap/.claude/commands/`:
  - [ ] `debtmap-analyze-features-for-book.md`
  - [ ] `debtmap-analyze-book-chapter-drift.md`
  - [ ] `debtmap-fix-book-drift.md`
- [ ] `../debtmap/workflows/book-docs-drift.yml` created using Debtmap commands
- [ ] Workflow runs successfully from Debtmap directory
- [ ] Feature inventory generated at `../debtmap/.debtmap/book-analysis/features.json`
- [ ] Drift analysis completes for all Debtmap chapters
- [ ] Book builds successfully: `cd ../debtmap/book && mdbook build`
- [ ] Generated documentation accurately reflects Debtmap features
- [ ] Configuration-driven pattern from Spec 118 successfully reused

## Technical Details

### Implementation Approach

#### 1. Debtmap Book Structure

**Initial Chapter Structure** (based on Debtmap architecture):
```
book/src/
├── SUMMARY.md
├── intro.md
├── getting-started.md
├── cli-usage.md
├── rules.md
├── analysis-types.md
├── configuration.md
├── extending.md
└── troubleshooting.md
```

#### 2. Debtmap Book Configuration

**File**: `../debtmap/.debtmap/book-config.json`
```json
{
  "project_name": "Debtmap",
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
      "area": "rules",
      "source_files": ["src/rules/", "src/analysis/"],
      "feature_categories": ["rule_types", "rule_configuration", "custom_rules"]
    },
    {
      "area": "analysis",
      "source_files": ["src/analysis/", "src/engine/"],
      "feature_categories": ["analysis_types", "output_formats", "integrations"]
    },
    {
      "area": "configuration",
      "source_files": ["src/config/"],
      "feature_categories": ["config_file", "options", "defaults"]
    }
  ],
  "chapter_file": "workflows/data/debtmap-chapters.json",
  "custom_analysis": {
    "include_examples": true,
    "include_best_practices": true,
    "include_troubleshooting": true
  }
}
```

#### 3. Debtmap Chapter Definitions

**File**: `../debtmap/workflows/data/debtmap-chapters.json`
```json
{
  "chapters": [
    {
      "id": "getting-started",
      "title": "Getting Started",
      "file": "book/src/getting-started.md",
      "topics": ["Installation", "Basic usage", "First analysis"],
      "validation": "Check installation instructions and basic usage examples"
    },
    {
      "id": "cli-usage",
      "title": "CLI Usage",
      "file": "book/src/cli-usage.md",
      "topics": ["Commands", "Arguments", "Options", "Examples"],
      "validation": "Verify all CLI commands and options are documented"
    },
    {
      "id": "rules",
      "title": "Rules",
      "file": "book/src/rules.md",
      "topics": ["Built-in rules", "Rule configuration", "Custom rules"],
      "validation": "Check all rule types match implementation"
    },
    {
      "id": "analysis-types",
      "title": "Analysis Types",
      "file": "book/src/analysis-types.md",
      "topics": ["Static analysis", "Pattern matching", "Output formats"],
      "validation": "Verify analysis types and formats are current"
    },
    {
      "id": "configuration",
      "title": "Configuration",
      "file": "book/src/configuration.md",
      "topics": ["Config file", "Options", "Defaults"],
      "validation": "Check configuration options match config module"
    },
    {
      "id": "extending",
      "title": "Extending Debtmap",
      "file": "book/src/extending.md",
      "topics": ["Custom rules", "Plugins", "Integrations"],
      "validation": "Verify extension mechanisms are documented"
    },
    {
      "id": "troubleshooting",
      "title": "Troubleshooting",
      "file": "book/src/troubleshooting.md",
      "topics": ["Common issues", "Debug mode", "FAQ"],
      "validation": "Check common issues are covered"
    }
  ]
}
```

#### 4. Debtmap Workflow

**File**: `../debtmap/workflows/book-docs-drift.yml`
```yaml
name: debtmap-book-docs-drift-detection
mode: mapreduce

env:
  PROJECT_NAME: "Debtmap"
  PROJECT_CONFIG: ".debtmap/book-config.json"
  CHAPTERS_FILE: "workflows/data/debtmap-chapters.json"
  ANALYSIS_DIR: ".debtmap/book-analysis"
  FEATURES_PATH: ".debtmap/book-analysis/features.json"
  DRIFT_DIR: ".debtmap/book-analysis"
  BOOK_DIR: "book"

setup:
  - shell: "mkdir -p .debtmap/book-analysis"

  # Debtmap command reads .debtmap/book-config.json for configuration
  - claude: "/debtmap-analyze-features-for-book"

map:
  input: "workflows/data/debtmap-chapters.json"
  json_path: "$.chapters[*]"

  agent_template:
    # Debtmap command receives chapter via ${item}, reads config for rest
    - claude: "/debtmap-analyze-book-chapter-drift --json '${item}'"
      commit_required: true

  max_parallel: 3
  agent_timeout_secs: 900

reduce:
  # Debtmap command reads config to find drift reports and chapters
  - claude: "/debtmap-fix-book-drift"
    commit_required: true

  - shell: "cd book && mdbook build"
    on_failure:
      claude: "/debtmap-fix-book-build-errors"

error_policy:
  on_item_failure: dlq
  continue_on_failure: true
  max_failures: 2
  error_collection: aggregate

merge:
  - shell: "rm -rf $ANALYSIS_DIR"
  - shell: "git add -A && git commit -m 'chore: remove temporary analysis files for $PROJECT_NAME' || true"
  - shell: "cd $BOOK_DIR && mdbook build"
  - shell: "git fetch origin"
  - claude: "/merge-master"
  - claude: "/prodigy-merge-worktree ${merge.source_branch}"
```

#### 5. Debtmap book.toml

**File**: `../debtmap/book/book.toml`
```toml
[book]
title = "Debtmap Documentation"
authors = ["Debtmap Contributors"]
description = "Technical debt analysis and management for Rust projects"
src = "src"
language = "en"

[build]
build-dir = "book"
create-missing = false

[output.html]
default-theme = "rust"
preferred-dark-theme = "navy"
smart-punctuation = true
mathjax-support = false
copy-fonts = true
no-section-label = false
git-repository-url = "https://github.com/gbaker-prodigy/debtmap"
git-repository-icon = "fa-github"
edit-url-template = "https://github.com/gbaker-prodigy/debtmap/edit/main/book/{path}"

[output.html.search]
enable = true
limit-results = 30
teaser-word-count = 30
use-boolean-and = true
boost-title = 2
boost-hierarchy = 1
boost-paragraph = 1
expand = true
heading-split-level = 3

[output.html.fold]
enable = true
level = 1

[output.html.playground]
editable = false
copyable = true
copy-js = true
line-numbers = true
```

### Architecture Changes

**New Files in Debtmap**:
- `../debtmap/book/book.toml` - mdBook configuration
- `../debtmap/book/src/SUMMARY.md` - Chapter structure
- `../debtmap/book/src/*.md` - Chapter files (7 chapters)
- `../debtmap/.debtmap/book-config.json` - Project configuration
- `../debtmap/workflows/data/debtmap-chapters.json` - Chapter definitions
- `../debtmap/workflows/book-docs-drift.yml` - Workflow instance
- `../debtmap/.claude/commands/debtmap-analyze-features-for-book.md` - Feature analysis command
- `../debtmap/.claude/commands/debtmap-analyze-book-chapter-drift.md` - Drift detection command
- `../debtmap/.claude/commands/debtmap-fix-book-drift.md` - Drift fixing command
- `../debtmap/.claude/commands/debtmap-fix-book-build-errors.md` - Error handling command (optional)

**New Files in Debtmap (optional)**:
- `../debtmap/.github/workflows/deploy-docs.yml` - GitHub Pages deployment

**No Changes in Prodigy**:
- Prodigy commands remain unchanged
- Pattern is reused, not the commands themselves

### Debtmap Command Creation

Debtmap creates its own commands following the configuration-driven pattern from Spec 118:

**`/debtmap-analyze-features-for-book`**:
- Reads `.debtmap/book-config.json` for analysis targets
- Generates feature inventory at `.debtmap/book-analysis/features.json`
- Implementation mirrors `/prodigy-analyze-features-for-book` but uses Debtmap config

**`/debtmap-analyze-book-chapter-drift`**:
- Receives chapter JSON via `--json` flag
- Reads `.debtmap/book-analysis/features.json` for ground truth
- Generates drift reports in `.debtmap/book-analysis/`
- Implementation mirrors `/prodigy-analyze-book-chapter-drift` but uses Debtmap paths

**`/debtmap-fix-book-drift`**:
- Reads drift reports from `.debtmap/book-analysis/drift-*.json`
- Updates chapters in `book/src/`
- Implementation mirrors `/prodigy-fix-book-drift` but uses Debtmap config

This proves the configuration-driven pattern is reusable across projects.

## Dependencies

**Prerequisites**:
- Spec 118 completed (generalized commands available)
- mdBook installed (or available via `cargo install mdbook`)
- Debtmap codebase accessible at `../debtmap/`

**Affected Components**:
- None in Prodigy - all changes are in Debtmap project
- Debtmap gets new book infrastructure

## Testing Strategy

### Integration Tests

**Test 1: Debtmap Book Setup**
- Run `mdbook init book` in Debtmap directory
- Create all configuration files
- Verify directory structure is correct
- Verify `book.toml` is valid

**Test 2: Debtmap Workflow Execution**
- Run `prodigy run workflows/book-docs-drift.yml` from Debtmap directory
- Verify setup phase creates `.debtmap/book-analysis/`
- Verify feature analysis generates `features.json` with Debtmap features
- Verify map phase processes all Debtmap chapters
- Verify reduce phase updates chapter files
- Verify book builds successfully

**Test 3: Generated Content Quality**
- Review generated `features.json` - should contain Debtmap-specific features
- Review drift reports - should identify Debtmap documentation needs
- Review updated chapters - should accurately document Debtmap
- Verify examples use Debtmap commands and syntax

**Test 4: Command Pattern Reuse**
- Verify Debtmap commands follow same pattern as Prodigy commands
- Verify configuration-driven approach works for Debtmap
- Verify error messages reference "Debtmap" not "Prodigy"
- Verify `debtmap-` prefix is recognized by Debtmap build process

### User Acceptance

**Acceptance 1: Setup Time**
- Developer can set up Debtmap book in <30 minutes
- Configuration is straightforward
- Workflow "just works" after configuration

**Acceptance 2: Documentation Quality**
- Generated documentation accurately reflects Debtmap features
- Examples are valid and work
- Documentation style is consistent and clear
- No Prodigy-specific content appears in Debtmap docs

## Documentation Requirements

### Code Documentation
- Document Debtmap-specific configuration in `book-config.json`
- Add comments to Debtmap chapter definitions explaining structure

### User Documentation

**Update**: `docs/book-documentation-workflow.md`
Add section:
```markdown
## Debtmap Example

Debtmap successfully uses the generalized book documentation system:

### Configuration
- Config: `.debtmap/book-config.json`
- Chapters: `workflows/data/debtmap-chapters.json`
- Workflow: `workflows/book-docs-drift.yml`

### Running
```bash
cd ../debtmap
prodigy run workflows/book-docs-drift.yml
```

### Customization
Debtmap defines 7 chapters focused on CLI usage, rules, and analysis.
Configuration specifies Debtmap-specific analysis targets.
```

**New Documentation** (in Debtmap):
- `../debtmap/README.md` - Add link to generated book documentation
- `../debtmap/book/src/intro.md` - Introduction to Debtmap

## Implementation Notes

### Setup Sequence

**Step 1: Initialize mdBook**
```bash
cd ../debtmap
mdbook init book --title "Debtmap Documentation"
```

**Step 2: Create Configuration**
- Create `.debtmap/book-config.json` with Debtmap analysis targets
- Create `workflows/data/debtmap-chapters.json` with chapter structure
- Update `book/book.toml` with Debtmap metadata

**Step 3: Create Debtmap Commands**
- Create `.claude/commands/debtmap-analyze-features-for-book.md`
  - Model after Prodigy's version but read from `.debtmap/book-config.json`
- Create `.claude/commands/debtmap-analyze-book-chapter-drift.md`
  - Model after Prodigy's version but use Debtmap paths
- Create `.claude/commands/debtmap-fix-book-drift.md`
  - Model after Prodigy's version but use Debtmap configuration

**Step 4: Create Workflow**
- Create `workflows/book-docs-drift.yml` based on Prodigy pattern
- Use Debtmap-specific commands (`debtmap-` prefix)
- Reference `debtmap-chapters.json` in map phase

**Step 5: Create Initial Chapters**
- Create placeholder files in `book/src/`
- Add basic structure and headings
- Let workflow fill in details based on codebase analysis

**Step 6: Run Workflow**
```bash
cd ../debtmap
prodigy run workflows/book-docs-drift.yml
```

**Step 7: Review and Refine**
- Review generated documentation
- Make manual adjustments if needed
- Commit results

### Debtmap-Specific Considerations

**Analysis Targets**: Focus on:
- CLI commands and options
- Rule types and configuration
- Analysis engine capabilities
- Output formats
- Configuration options

**Chapter Structure**: Optimized for:
- New users (getting started)
- Reference (CLI usage, rules)
- Advanced users (extending, custom rules)
- Troubleshooting

**Examples**: Should show:
- Real Debtmap commands
- Typical `.debtmap.toml` configurations
- Common analysis workflows
- Integration examples

### Common Pitfalls

**Pitfall 1**: Copying Prodigy chapter structure
- **Risk**: Debtmap chapters should reflect Debtmap's architecture, not Prodigy's
- **Mitigation**: Design chapter structure based on Debtmap's actual features

**Pitfall 2**: Analysis targets too broad
- **Risk**: Feature inventory becomes overwhelming or unfocused
- **Mitigation**: Focus on user-facing features (CLI, rules, config)

**Pitfall 3**: Insufficient initial content
- **Risk**: Drift analysis has nothing to compare against
- **Mitigation**: Create minimal chapter outlines before running workflow

## Migration and Compatibility

### Breaking Changes
None - this is new functionality for Debtmap

### Deployment

**Optional GitHub Pages Setup**:
If deploying to GitHub Pages, create `../debtmap/.github/workflows/deploy-docs.yml`:
```yaml
name: Deploy Documentation

on:
  push:
    branches: [main, master]
    paths:
      - 'book/**'
  pull_request:
    branches: [main, master]
    paths:
      - 'book/**'

jobs:
  build-deploy:
    runs-on: ubuntu-latest
    permissions:
      contents: write
    steps:
      - uses: actions/checkout@v4
      - uses: peaceiris/actions-mdbook@v2
        with:
          mdbook-version: 'latest'
      - name: Build book
        run: mdbook build book
      - name: Deploy to GitHub Pages
        if: github.event_name == 'push'
        uses: peaceiris/actions-gh-pages@v4
        with:
          github_token: ${{ secrets.GITHUB_TOKEN }}
          publish_dir: ./book/book
```

## Success Metrics

**Functionality**:
- Debtmap workflow runs successfully without errors
- All Debtmap chapters analyzed and populated
- Book builds successfully
- Generated content accurately reflects Debtmap

**Pattern Reusability**:
- Configuration-driven pattern successfully applied to Debtmap
- Setup time <30 minutes (including command creation)
- Commands follow same structure as Prodigy (proving pattern works)
- Debtmap book quality comparable to Prodigy book

**Independence**:
- Debtmap workflow runs independently
- Debtmap has its own configuration and chapters
- No coupling between Prodigy and Debtmap books
