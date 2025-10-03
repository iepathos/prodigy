---
number: 118
title: Generalize Prodigy Book Documentation Commands
category: documentation
priority: high
status: draft
dependencies: []
created: 2025-10-03
---

# Specification 118: Generalize Prodigy Book Documentation Commands

**Category**: documentation
**Priority**: high
**Status**: draft
**Dependencies**: None

## Context

Prodigy has successfully implemented an automated book documentation system using:
- mdBook for generating static documentation sites
- Prodigy MapReduce workflow (`workflows/book-docs-drift.yml`) for drift detection and fixes
- Specialized Claude commands for analyzing and updating documentation
- GitHub workflow for automated deployment to GitHub Pages

The current implementation uses Prodigy-specific commands (`/prodigy-analyze-features-for-book`, etc.) with hardcoded project assumptions. To enable reuse across other projects (see Spec 119 for Debtmap), these commands need to be generalized to accept project configuration as parameters.

This spec focuses on **refactoring the Prodigy implementation only** to use generalized, parameter-based commands while maintaining identical functionality. This proves the generalization works before applying it to other projects.

## Objective

Refactor Prodigy's book documentation workflow to:
1. Use project-agnostic Claude commands that accept configuration parameters
2. Externalize project-specific details into JSON configuration files
3. Maintain identical functionality and output for Prodigy's existing book workflow
4. Create reusable command templates that other projects can use
5. Verify the refactored system works correctly with Prodigy before expanding to other projects

## Requirements

### Functional Requirements

**FR1**: Refactor Claude commands to be configuration-driven:
- Keep `/prodigy-analyze-features-for-book`, `/prodigy-analyze-book-chapter-drift`, `/prodigy-fix-book-drift` names
- Make implementations read from `.prodigy/book-config.json` instead of hardcoding paths/settings
- Generalize internal logic to work with any project structure via configuration
- Extract reusable patterns that Debtmap commands (Spec 119) can follow

**FR2**: Create Prodigy project configuration:
- Create `.prodigy/book-config.json` with Prodigy-specific settings
- Externalize book paths, analysis targets, and chapter file location
- Configuration validates on load with clear error messages

**FR3**: Update Prodigy workflow to use new commands:
- Update `workflows/book-docs-drift.yml` to define environment variables
- Update workflow to call generalized commands with parameters
- Rename `workflows/data/book-chapters.json` to `workflows/data/prodigy-chapters.json`
- Maintain identical workflow behavior and output

**FR4**: Maintain command naming convention:
- Keep `prodigy-` prefix on all Prodigy commands (required for Prodigy's self-recognition)
- Make command implementations configuration-driven rather than changing names
- Create pattern that Debtmap can follow with `debtmap-` prefix

### Non-Functional Requirements

**NFR1**: **Maintainability**: Commands should be easy to understand and modify for new projects

**NFR2**: **Reusability**: Minimal duplication between project-specific workflows

**NFR3**: **Flexibility**: Easy to add new projects without modifying core commands

**NFR4**: **Consistency**: Similar documentation quality across all projects

**NFR5**: **Performance**: Efficient analysis and update process for large codebases

## Acceptance Criteria

- [ ] `.prodigy/book-config.json` created with Prodigy's configuration
- [ ] `workflows/data/book-chapters.json` renamed to `workflows/data/prodigy-chapters.json`
- [ ] Prodigy commands refactored to read from configuration files
- [ ] Commands keep `prodigy-` prefix (required for Prodigy self-recognition)
- [ ] Command implementations are generic and configuration-driven internally
- [ ] `workflows/book-docs-drift.yml` updated to use configuration approach
- [ ] Workflow runs successfully and produces identical output to previous version
- [ ] Book builds successfully after drift fixes
- [ ] All Prodigy chapters analyzed and updated correctly
- [ ] Command pattern documented for Debtmap to follow (Spec 119)

## Technical Details

### Implementation Approach

#### 1. Command Refactoring Strategy

**Current State**: Commands have hardcoded paths and Prodigy-specific assumptions
```bash
# Current: Hardcoded paths like "book/src/", ".prodigy/book-analysis/"
/prodigy-analyze-features-for-book
/prodigy-analyze-book-chapter-drift
/prodigy-fix-book-drift
```

**Target State**: Same command names, but configuration-driven
```bash
# Refactored: Read from .prodigy/book-config.json
/prodigy-analyze-features-for-book    # Reads config for paths, analysis targets
/prodigy-analyze-book-chapter-drift   # Reads config for chapter structure
/prodigy-fix-book-drift               # Reads config for output paths

# Commands keep prodigy- prefix (required for Prodigy's self-recognition)
# But internal implementation is generic and driven by configuration
```

#### 2. Configuration Structure

**Project Configuration File** (e.g., `.prodigy/book-config.json` or `.debtmap/book-config.json`):
```json
{
  "project_name": "Prodigy",
  "project_type": "cli_tool",
  "book_dir": "book",
  "book_src": "book/src",
  "book_build_dir": "book/book",
  "analysis_targets": [
    {
      "area": "workflow_basics",
      "source_files": ["src/config/workflow.rs", "src/cook/workflow/executor.rs"],
      "feature_categories": ["structure", "execution_model", "commit_tracking"]
    },
    {
      "area": "mapreduce",
      "source_files": ["src/config/mapreduce.rs", "src/cook/execution/mapreduce/"],
      "feature_categories": ["phases", "capabilities", "configuration"]
    }
  ],
  "chapter_file": "workflows/data/book-chapters.json",
  "custom_analysis": {
    "include_examples": true,
    "include_best_practices": true,
    "include_troubleshooting": true
  }
}
```

**Chapter Definition** (project-specific, e.g., `workflows/data/{project}-chapters.json`):
```json
{
  "chapters": [
    {
      "id": "chapter-id",
      "title": "Chapter Title",
      "file": "book/src/chapter.md",
      "topics": ["Topic 1", "Topic 2"],
      "validation": "Validation focus for this chapter",
      "source_references": ["src/module/*.rs"]
    }
  ]
}
```

#### 3. Configuration-Driven Command Design

**Feature Analysis Command** (`/prodigy-analyze-features-for-book`):
- Reads `.prodigy/book-config.json` to determine analysis targets
- Generates feature inventory at configured output path (`.prodigy/book-analysis/features.json`)
- Analysis targets specified in config, not hardcoded
- Outputs JSON compatible with drift detection

**Chapter Drift Command** (`/prodigy-analyze-book-chapter-drift`):
- Receives chapter JSON via `--json` flag (passed from workflow)
- Reads `.prodigy/book-analysis/features.json` for ground truth
- Chapter structure comes from `workflows/data/prodigy-chapters.json`
- Generates drift report in `.prodigy/book-analysis/`
- Supports custom validation focuses per chapter

**Fix Drift Command** (`/prodigy-fix-book-drift`):
- Reads all drift reports from `.prodigy/book-analysis/drift-*.json`
- Updates chapters in `book/src/` based on drift analysis
- Book paths come from configuration
- Validates mdBook build after changes

#### 4. Refactored Workflow Structure

**Updated Workflow** (`workflows/book-docs-drift.yml`):
```yaml
name: prodigy-book-docs-drift-detection
mode: mapreduce

setup:
  - shell: "mkdir -p .prodigy/book-analysis"

  # Command reads .prodigy/book-config.json for configuration
  - claude: "/prodigy-analyze-features-for-book"

map:
  input: "workflows/data/prodigy-chapters.json"  # Renamed from book-chapters.json
  json_path: "$.chapters[*]"

  agent_template:
    # Command receives chapter via ${item}, reads config for rest
    - claude: "/prodigy-analyze-book-chapter-drift --json '${item}'"
      commit_required: true

  max_parallel: 3
  agent_timeout_secs: 900

reduce:
  # Command reads config to find drift reports and chapters
  - claude: "/prodigy-fix-book-drift"
    commit_required: true

  - shell: "cd book && mdbook build"
    on_failure:
      claude: "/prodigy-fix-book-build-errors"

error_policy:
  on_item_failure: dlq
  continue_on_failure: true
  max_failures: 2
  error_collection: aggregate

merge:
  - shell: "rm -rf .prodigy/book-analysis"
  - shell: "git add -A && git commit -m 'chore: remove temporary book analysis files' || true"
  - shell: "cd book && mdbook build"
  - shell: "git fetch origin"
  - claude: "/merge-master"
  - claude: "/prodigy-merge-worktree ${merge.source_branch}"
```

**Key Changes**:
- Commands keep `prodigy-` prefix
- Configuration paths come from `.prodigy/book-config.json`
- Chapter file renamed to `prodigy-chapters.json` for clarity
- Commands are simpler (no need to pass config paths explicitly)

### Architecture Changes

**New Files**:
- `.prodigy/book-config.json` - Prodigy-specific configuration
- `workflows/data/prodigy-chapters.json` - Renamed from `book-chapters.json`

**Modified Files**:
- `.claude/commands/prodigy-analyze-features-for-book.md` - Refactored to read from config
- `.claude/commands/prodigy-analyze-book-chapter-drift.md` - Refactored to read from config
- `.claude/commands/prodigy-fix-book-drift.md` - Refactored to read from config
- `workflows/book-docs-drift.yml` - Updated to use configuration-driven commands

**Pattern for Future Projects** (Spec 119):
- Debtmap will create similar files with `debtmap-` prefix
- Command implementations can follow same configuration-driven pattern
- Workflow structure can be copied and adapted

### Data Structures

**Project Configuration Schema**:
```rust
pub struct BookProjectConfig {
    pub project_name: String,
    pub project_type: ProjectType,
    pub book_dir: PathBuf,
    pub book_src: PathBuf,
    pub book_build_dir: PathBuf,
    pub analysis_targets: Vec<AnalysisTarget>,
    pub chapter_file: PathBuf,
    pub custom_analysis: CustomAnalysisConfig,
}

pub enum ProjectType {
    CliTool,
    Library,
    Application,
    WorkflowOrchestrator,
}

pub struct AnalysisTarget {
    pub area: String,
    pub source_files: Vec<String>,
    pub feature_categories: Vec<String>,
}

pub struct CustomAnalysisConfig {
    pub include_examples: bool,
    pub include_best_practices: bool,
    pub include_troubleshooting: bool,
}
```

### APIs and Interfaces

**Command Interface Pattern**:
All generalized commands follow this parameter pattern:
```bash
# Required parameters
--config <path>          # Path to project book configuration JSON
--project <name>         # Project name for output and messaging

# Context-specific parameters
--features <path>        # Path to features.json (for drift detection)
--drift-dir <path>       # Directory containing drift reports (for fixes)
--chapter-json <json>    # Chapter definition JSON (for chapter analysis)
```

**Environment Variable Contract**:
Workflows must define these variables:
- `$PROJECT_NAME` - Display name of project
- `$PROJECT_CONFIG` - Path to book configuration
- `$CHAPTERS_FILE` - Path to chapters definition JSON
- `$ANALYSIS_DIR` - Directory for temporary analysis files
- `$FEATURES_PATH` - Path to generated features.json
- `$DRIFT_DIR` - Directory for drift reports
- `$BOOK_DIR` - Root directory of mdBook

## Dependencies

**Prerequisites**: None - this is a new capability

**Affected Components**:
- `.claude/commands/prodigy-*-book-*.md` - Will be replaced by generalized versions
- `workflows/book-docs-drift.yml` - Will be updated to use new commands
- GitHub workflow `.github/workflows/deploy-docs.yml` - May need project-specific variants

**External Dependencies**:
- mdBook (already in use)
- jq (for JSON processing in shell commands)
- Project-specific book configurations

## Testing Strategy

### Unit Tests
Not applicable - this is a documentation automation feature implemented in Claude commands and YAML workflows.

### Integration Tests

**Test 1: Refactored Prodigy Workflow**
- Run `prodigy run workflows/book-docs-drift.yml` with new commands
- Verify feature analysis generates `.prodigy/book-analysis/features.json`
- Verify drift detection creates drift reports for all 9 chapters
- Verify fixes are applied and book builds successfully
- Verify merge workflow completes
- Compare output with previous workflow run (should be identical)

**Test 2: Configuration Validation**
- Test with missing required config fields
- Test with invalid paths in `.prodigy/book-config.json`
- Test with malformed chapter JSON in `workflows/data/prodigy-chapters.json`
- Verify helpful error messages

**Test 3: Command Parameter Validation**
- Test commands with missing required parameters
- Test commands with invalid file paths
- Verify error messages guide user to fix issues

### Performance Tests

**Test 1: Large Codebase Analysis**
- Time feature analysis on Prodigy codebase (~50K LOC)
- Verify analysis completes in reasonable time (<5 minutes)
- Check memory usage stays within bounds

**Test 2: Parallel Chapter Processing**
- Verify map phase processes chapters in parallel (max_parallel: 3)
- Check total workflow time vs sequential processing
- Verify no resource contention issues

### User Acceptance

**Acceptance 1: Developer Experience**
- Developer can add book documentation to new project in <30 minutes
- Configuration is intuitive and well-documented
- Error messages are actionable
- Documentation generation "just works"

**Acceptance 2: Documentation Quality**
- Generated drift reports accurately identify issues
- Fixes maintain documentation quality and style
- Examples in documentation are valid and work
- Cross-references between chapters remain intact

## Documentation Requirements

### Code Documentation
- Document all Claude command parameters and expected inputs
- Include examples in command markdown files
- Explain configuration schema with examples

### User Documentation

**New Documentation**:

**`docs/book-documentation-workflow.md`**:
```markdown
# Book Documentation Workflow

## Overview
Automated book documentation system using Prodigy MapReduce workflows.

## Prodigy Usage
The Prodigy book workflow has been refactored to use generalized commands:
- Configuration: `.prodigy/book-config.json`
- Chapters: `workflows/data/prodigy-chapters.json`
- Workflow: `workflows/book-docs-drift.yml`

## Commands
- `/analyze-codebase-features --config <path>` - Analyze codebase for features
- `/analyze-chapter-drift --json <chapter> --features <path> --project <name>` - Detect drift
- `/fix-documentation-drift --config <path> --drift-dir <path>` - Fix drift

## Configuration Schema
- Project config structure and required fields
- Chapter definition format
- Analysis target specification

## Reusing for Other Projects
See Spec 119 for applying this system to Debtmap or other projects.
```

**Updated Documentation**:
- `CLAUDE.md` - Document new generalized Claude commands and parameters
- `book/src/` - Update Prodigy book if needed to reference new command names

### Architecture Updates

Update `ARCHITECTURE.md` to document:
- Generalized documentation workflow architecture
- Configuration-driven command design
- Project-agnostic pattern usage
- Extension points for new projects

## Implementation Notes

### Generalization Principles

1. **Configuration Over Convention**: Use configuration files to specify project-specific details rather than hardcoding in commands

2. **Parameters Over Hardcoding**: Pass context through command parameters rather than embedding in command logic

3. **Templates Over Duplication**: Create workflow templates that can be instantiated per-project

4. **Descriptive Over Prescriptive**: Commands describe what to analyze/fix based on configuration rather than prescribing specific Prodigy features

5. **Validation**: Validate configurations early and provide clear error messages

### Command Migration Strategy

**Phase 1**: Create generalized versions alongside existing commands
- `/analyze-codebase-features` alongside `/prodigy-analyze-features-for-book`
- Test with Prodigy configuration
- Verify outputs match

**Phase 2**: Update Prodigy workflow to use new commands
- Add environment variables
- Switch to generalized commands
- Test end-to-end

**Phase 3**: Apply to Debtmap
- Create Debtmap configurations
- Create Debtmap workflow instance
- Test independently

**Phase 4**: Deprecate old commands
- Remove `/prodigy-analyze-features-for-book`
- Remove `/prodigy-analyze-book-chapter-drift`
- Remove `/prodigy-fix-book-drift`

### Common Pitfalls

**Pitfall 1**: Over-generalization
- **Risk**: Making commands too abstract and hard to use
- **Mitigation**: Keep common cases simple, support customization through config

**Pitfall 2**: Configuration complexity
- **Risk**: Configuration becomes as complex as writing code
- **Mitigation**: Provide good defaults, clear examples, validation

**Pitfall 3**: Breaking existing workflows
- **Risk**: Changes break Prodigy book workflow
- **Mitigation**: Test thoroughly, migrate incrementally, keep old commands until proven

**Pitfall 4**: Project-specific assumptions
- **Risk**: Hidden assumptions about Prodigy structure leak into "general" commands
- **Mitigation**: Test with Debtmap early, review for hardcoded assumptions

## Migration and Compatibility

### Breaking Changes
- Workflow variable names change (add `PROJECT_*` prefix for clarity)
- Command names change (remove `prodigy-` prefix to make them reusable)
- Chapter file renamed: `book-chapters.json` → `prodigy-chapters.json`
- Commands now require parameters: `--config`, `--project`, etc.

### Migration Path

**Phase 1: Create Prodigy Configuration**
1. Create `.prodigy/book-config.json` with Prodigy's paths and analysis targets
2. Rename `workflows/data/book-chapters.json` → `workflows/data/prodigy-chapters.json`
3. Validate configuration structure

**Phase 2: Refactor Prodigy Commands**
1. Update `/prodigy-analyze-features-for-book` to read from `.prodigy/book-config.json`
2. Update `/prodigy-analyze-book-chapter-drift` to use configuration
3. Update `/prodigy-fix-book-drift` to use configuration
4. Keep command names unchanged (required for Prodigy self-recognition)
5. Test commands individually with configuration

**Phase 3: Update Prodigy Workflow**
1. Update `workflows/book-docs-drift.yml` to reference `prodigy-chapters.json`
2. Simplify command invocations (config is implicit)
3. Test workflow end-to-end

**Phase 4: Verification**
1. Run workflow and compare output with previous runs
2. Verify book builds successfully
3. Verify all chapters updated correctly
4. Document configuration-driven pattern for Debtmap

### Backward Compatibility

**Backward Compatibility**:
- Command names unchanged (`prodigy-` prefix maintained)
- Workflow structure largely unchanged
- Existing workflow files will need chapter path update
- Configuration is additive (new `.prodigy/book-config.json` file)

**Breaking Changes**:
- `workflows/data/book-chapters.json` renamed to `prodigy-chapters.json`
- Commands now require `.prodigy/book-config.json` to exist
- Hardcoded paths removed from command implementations

## Success Metrics

**Functionality**:
- Refactored Prodigy workflow produces identical results to original
- All 9 Prodigy chapters analyzed and fixed correctly
- Book builds successfully after drift fixes
- Workflow completes without errors

**Quality**:
- Commands are parameter-based and accept configuration
- Configuration schema is clear and well-documented
- Error messages are helpful and actionable
- Code is ready for reuse by other projects (Spec 119)

**Maintainability**:
- Commands use generic terminology (no "Prodigy" hardcoding)
- Configuration externalizes all project-specific details
- Pattern is clear enough for other projects to follow
- Documentation explains how to reuse for new projects
