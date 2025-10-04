---
number: 119
title: Add Book Documentation to Debtmap
category: documentation
priority: high
status: draft
dependencies: [118, 120]
created: 2025-10-03
---

# Specification 119: Add Book Documentation to Debtmap

**Category**: documentation
**Priority**: high
**Status**: draft
**Dependencies**: [118, 120]

## Context

Spec 118 generalized Prodigy's book documentation commands to accept parameters (`--project`, `--config`, etc.) so they work for ANY codebase. The same `prodigy-` prefixed commands are now used by all projects.

Debtmap is a Rust CLI tool for technical debt analysis and management. It currently lacks comprehensive documentation. This spec applies the generalized book documentation commands to Debtmap by creating Debtmap-specific configuration and using the same `prodigy-` commands with different parameters.

This proves the generalization works: identical commands, different configuration.

## Objective

Set up automated book documentation for Debtmap by:
1. Creating mdBook structure for Debtmap documentation
2. Creating Debtmap-specific configuration and chapter definitions
3. Creating Debtmap workflow that uses the SAME `prodigy-` commands with Debtmap parameters
4. Generating initial book content documenting Debtmap's features
5. Setting up GitHub workflow for automated deployment (optional)
6. Proving the same commands work for multiple projects via configuration

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

**FR4**: Create Debtmap workflow instance:
- Create `../debtmap/workflows/book-docs-drift.yml`
- Use the SAME `prodigy-` commands from Spec 118
- Set Debtmap-specific environment variables (PROJECT_NAME, PROJECT_CONFIG, etc.)
- Pass Debtmap parameters to the generalized commands
- Follow same workflow pattern as Prodigy

**FR5**: Generate initial documentation:
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
- [ ] `../debtmap/workflows/book-docs-drift.yml` created using SAME `prodigy-` commands
- [ ] Workflow sets Debtmap environment variables (PROJECT_NAME="Debtmap", etc.)
- [ ] Workflow runs successfully from Debtmap directory
- [ ] Feature inventory generated at `../debtmap/.debtmap/book-analysis/features.json`
- [ ] Drift analysis completes for all Debtmap chapters
- [ ] Book builds successfully: `cd ../debtmap/book && mdbook build`
- [ ] Generated documentation accurately reflects Debtmap features
- [ ] Same commands work for both Prodigy and Debtmap (proves generalization)

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
  FEATURES_PATH: ".debtmap/book-analysis/features.json"

setup:
  - shell: "mkdir -p .debtmap/book-analysis"

  # Same prodigy- command as Prodigy workflow, different parameters
  - claude: "/prodigy-analyze-features-for-book --project $PROJECT_NAME --config $PROJECT_CONFIG"

map:
  input: "workflows/data/debtmap-chapters.json"
  json_path: "$.chapters[*]"

  agent_template:
    # Same prodigy- command, Debtmap parameters
    - claude: "/prodigy-analyze-book-chapter-drift --project $PROJECT_NAME --json '${item}' --features $FEATURES_PATH"
      commit_required: true

  max_parallel: 3
  agent_timeout_secs: 900

reduce:
  # Same prodigy- command, Debtmap configuration
  - claude: "/prodigy-fix-book-drift --project $PROJECT_NAME --config $PROJECT_CONFIG"
    commit_required: true

  - shell: "cd book && mdbook build"
    on_failure:
      claude: "/prodigy-fix-book-build-errors --project $PROJECT_NAME"

error_policy:
  on_item_failure: dlq
  continue_on_failure: true
  max_failures: 2
  error_collection: aggregate

merge:
  - shell: "rm -rf .debtmap/book-analysis"
  - shell: "git add -A && git commit -m 'chore: remove temporary book analysis files for Debtmap' || true"
  - shell: "cd book && mdbook build"
  - shell: "git fetch origin"
  - claude: "/merge-master"
  - claude: "/prodigy-merge-worktree ${merge.source_branch}"
```

**Key Points**:
- **Identical commands** to Prodigy workflow (`/prodigy-analyze-features-for-book`, etc.)
- **Different environment variables** (PROJECT_NAME="Debtmap", PROJECT_CONFIG=".debtmap/book-config.json")
- **Different chapter file** (debtmap-chapters.json instead of prodigy-chapters.json)
- **Same workflow structure** - easy to copy and adapt from Prodigy

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
- `../debtmap/.debtmap/book-config.json` - Debtmap project configuration
- `../debtmap/workflows/data/debtmap-chapters.json` - Debtmap chapter definitions
- `../debtmap/workflows/book-docs-drift.yml` - Debtmap workflow (uses same `prodigy-` commands)

**New Files in Debtmap (optional)**:
- `../debtmap/.github/workflows/deploy-docs.yml` - GitHub Pages deployment

**No New Commands**:
- Debtmap uses existing `prodigy-` commands from Prodigy repo
- Commands are generalized and work for any project via parameters
- Zero code duplication

### Command Reuse

Debtmap uses the **EXACT SAME commands** as Prodigy (from Spec 118):

**`/prodigy-analyze-features-for-book`**:
- Debtmap calls with: `--project Debtmap --config .debtmap/book-config.json`
- Prodigy calls with: `--project Prodigy --config .prodigy/book-config.json`
- Same command, different parameters

**`/prodigy-analyze-book-chapter-drift`**:
- Debtmap calls with: `--project Debtmap --json '${item}' --features .debtmap/book-analysis/features.json`
- Prodigy calls with: `--project Prodigy --json '${item}' --features .prodigy/book-analysis/features.json`
- Same command, different parameters

**`/prodigy-fix-book-drift`**:
- Debtmap calls with: `--project Debtmap --config .debtmap/book-config.json`
- Prodigy calls with: `--project Prodigy --config .prodigy/book-config.json`
- Same command, different parameters

This proves the generalization works: **zero command duplication**, only configuration differences.

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

**Test 4: Command Reusability**
- Verify same `prodigy-` commands used by Debtmap workflow
- Verify commands accept parameters correctly
- Verify error messages reference "Debtmap" (from --project parameter)
- Verify no command duplication between projects

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

**Step 3: Create Workflow**
- Copy `workflows/book-docs-drift.yml` from Prodigy
- Change environment variables to Debtmap values:
  - `PROJECT_NAME: "Debtmap"`
  - `PROJECT_CONFIG: ".debtmap/book-config.json"`
  - `FEATURES_PATH: ".debtmap/book-analysis/features.json"`
- Update input path to `workflows/data/debtmap-chapters.json`
- Keep same `prodigy-` commands (they accept parameters)

**Step 4: Create Initial Chapters**
- Create placeholder files in `book/src/`
- Add basic structure and headings
- Let workflow fill in details based on codebase analysis

**Step 5: Run Workflow**
```bash
cd ../debtmap
prodigy run workflows/book-docs-drift.yml
```

**Step 6: Review and Refine**
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

**Command Reusability**:
- Same `prodigy-` commands work for both Prodigy and Debtmap
- Setup time <30 minutes (no command creation needed)
- Zero command duplication between projects
- Debtmap book quality comparable to Prodigy book

**Independence**:
- Debtmap workflow runs independently
- Debtmap has its own configuration and chapters
- No coupling between Prodigy and Debtmap books
