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

**FR1**: Refactor Claude commands to be parameter-based:
- Replace `/prodigy-analyze-features-for-book` with `/analyze-codebase-features --config <path>`
- Replace `/prodigy-analyze-book-chapter-drift` with `/analyze-chapter-drift --json <chapter> --features <path> --project <name>`
- Replace `/prodigy-fix-book-drift` with `/fix-documentation-drift --config <path> --drift-dir <path>`
- Commands accept project context as parameters rather than hardcoding "Prodigy"
- Use generic terminology that works for any project

**FR2**: Create Prodigy project configuration:
- Create `.prodigy/book-config.json` with Prodigy-specific settings
- Externalize book paths, analysis targets, and chapter file location
- Configuration validates on load with clear error messages

**FR3**: Update Prodigy workflow to use new commands:
- Update `workflows/book-docs-drift.yml` to define environment variables
- Update workflow to call generalized commands with parameters
- Rename `workflows/data/book-chapters.json` to `workflows/data/prodigy-chapters.json`
- Maintain identical workflow behavior and output

**FR4**: Ensure backward compatibility during transition:
- Keep old commands temporarily for comparison
- Verify new workflow produces identical results
- Test complete workflow end-to-end
- Remove old commands only after verification

### Non-Functional Requirements

**NFR1**: **Maintainability**: Commands should be easy to understand and modify for new projects

**NFR2**: **Reusability**: Minimal duplication between project-specific workflows

**NFR3**: **Flexibility**: Easy to add new projects without modifying core commands

**NFR4**: **Consistency**: Similar documentation quality across all projects

**NFR5**: **Performance**: Efficient analysis and update process for large codebases

## Acceptance Criteria

- [ ] New generalized Claude commands created in `.claude/commands/`
- [ ] Commands accept `--config`, `--project`, `--features`, and `--drift-dir` parameters
- [ ] `.prodigy/book-config.json` created with Prodigy's configuration
- [ ] `workflows/data/book-chapters.json` renamed to `workflows/data/prodigy-chapters.json`
- [ ] `workflows/book-docs-drift.yml` updated to use environment variables and new commands
- [ ] Workflow runs successfully and produces identical output to previous version
- [ ] Book builds successfully after drift fixes
- [ ] All Prodigy chapters analyzed and updated correctly
- [ ] Old Prodigy-specific commands can be removed (or marked deprecated)
- [ ] Commands are documented and ready for reuse by other projects (Spec 119)

## Technical Details

### Implementation Approach

#### 1. Command Generalization Strategy

**Current State**: Commands are Prodigy-specific
```bash
# Current: Hardcoded for Prodigy
/prodigy-analyze-features-for-book
/prodigy-analyze-book-chapter-drift
/prodigy-fix-book-drift
```

**Target State**: Project-agnostic with parameters
```bash
# Generalized with project context
/analyze-codebase-features --project $PROJECT_NAME --config $CONFIG_PATH
/analyze-chapter-drift --chapter-json "$CHAPTER" --features $FEATURES_PATH
/fix-documentation-drift --project $PROJECT_NAME --drift-dir $DRIFT_DIR
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

#### 3. Generalized Command Design

**Feature Analysis Command** (`/analyze-codebase-features`):
- Variables: `$PROJECT_CONFIG` (path to project config JSON)
- Reads project configuration to determine analysis targets
- Generates feature inventory at configured output path
- Supports different project types with custom analysis logic
- Outputs JSON compatible with drift detection

**Chapter Drift Command** (`/analyze-chapter-drift`):
- Variables: `$CHAPTER_JSON`, `$FEATURES_PATH`, `$PROJECT_NAME`
- Works with any chapter structure from chapter JSON
- Compares against feature inventory
- Generates standardized drift report
- Supports custom validation focuses per chapter

**Fix Drift Command** (`/fix-documentation-drift`):
- Variables: `$PROJECT_CONFIG`, `$DRIFT_DIR`
- Reads all drift reports from specified directory
- Updates chapters based on project configuration
- Preserves project-specific documentation style
- Validates mdBook build after changes

#### 4. Workflow Template Structure

**Template Workflow** (`workflows/templates/book-docs-drift-template.yml`):
```yaml
name: ${PROJECT_NAME}-book-docs-drift-detection
mode: mapreduce

setup:
  - shell: "mkdir -p ${ANALYSIS_DIR}"
  - claude: "/analyze-codebase-features --config ${PROJECT_CONFIG}"

map:
  input: "${CHAPTERS_FILE}"
  json_path: "$.chapters[*]"

  agent_template:
    - claude: "/analyze-chapter-drift --json '${item}' --features ${FEATURES_PATH} --project ${PROJECT_NAME}"
      commit_required: true

  max_parallel: 3
  agent_timeout_secs: 900

reduce:
  - claude: "/fix-documentation-drift --config ${PROJECT_CONFIG} --drift-dir ${DRIFT_DIR}"
    commit_required: true
  - shell: "cd ${BOOK_DIR} && mdbook build"
    on_failure:
      claude: "/fix-book-build-errors --project ${PROJECT_NAME}"

error_policy:
  on_item_failure: dlq
  continue_on_failure: true
  max_failures: 2
  error_collection: aggregate

merge:
  - shell: "rm -rf ${ANALYSIS_DIR}"
  - shell: "git add -A && git commit -m 'chore: remove temporary analysis files for ${PROJECT_NAME}' || true"
  - shell: "cd ${BOOK_DIR} && mdbook build"
  - shell: "git fetch origin"
  - claude: "/merge-master"
  - claude: "/prodigy-merge-worktree ${merge.source_branch}"
```

**Project-Specific Instance** (e.g., `workflows/prodigy-book-docs-drift.yml`):
```yaml
name: prodigy-book-docs-drift-detection
mode: mapreduce

env:
  PROJECT_NAME: "Prodigy"
  PROJECT_CONFIG: ".prodigy/book-config.json"
  CHAPTERS_FILE: "workflows/data/prodigy-chapters.json"
  ANALYSIS_DIR: ".prodigy/book-analysis"
  FEATURES_PATH: ".prodigy/book-analysis/features.json"
  DRIFT_DIR: ".prodigy/book-analysis"
  BOOK_DIR: "book"

setup:
  - shell: "mkdir -p $ANALYSIS_DIR"
  - claude: "/analyze-codebase-features --config $PROJECT_CONFIG"

# ... rest follows template pattern with environment variable substitution
```

### Architecture Changes

**New Files**:
- `.claude/commands/analyze-codebase-features.md` - Generalized feature analysis
- `.claude/commands/analyze-chapter-drift.md` - Generalized chapter drift detection
- `.claude/commands/fix-documentation-drift.md` - Generalized drift fixing
- `workflows/templates/book-docs-drift-template.yml` - Reusable workflow template
- `.prodigy/book-config.json` - Prodigy-specific configuration
- `workflows/data/prodigy-chapters.json` - Prodigy chapter definitions

**Modified Files**:
- `workflows/book-docs-drift.yml` - Use environment variables and generalized commands
- `workflows/data/book-chapters.json` - Rename to `workflows/data/prodigy-chapters.json`

**Files for Future Projects** (Spec 119):
- Commands in `.claude/commands/` are now reusable by other projects
- Workflow pattern can be copied and customized with project-specific env vars

**Deprecated Files** (remove after verification):
- `.claude/commands/prodigy-analyze-features-for-book.md` → replaced by generalized version
- `.claude/commands/prodigy-analyze-book-chapter-drift.md` → replaced by generalized version
- `.claude/commands/prodigy-fix-book-drift.md` → replaced by generalized version

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

**Phase 1: Create Generalized Commands**
1. Create `/analyze-codebase-features` based on `/prodigy-analyze-features-for-book`
2. Create `/analyze-chapter-drift` based on `/prodigy-analyze-book-chapter-drift`
3. Create `/fix-documentation-drift` based on `/prodigy-fix-book-drift`
4. Test commands work with Prodigy configuration

**Phase 2: Create Prodigy Configuration**
1. Create `.prodigy/book-config.json` with Prodigy's settings
2. Rename `workflows/data/book-chapters.json` → `workflows/data/prodigy-chapters.json`
3. Update chapter file path references

**Phase 3: Update Prodigy Workflow**
1. Add environment variables to `workflows/book-docs-drift.yml`
2. Update setup phase to call `/analyze-codebase-features --config $PROJECT_CONFIG`
3. Update map phase to call `/analyze-chapter-drift` with parameters
4. Update reduce phase to call `/fix-documentation-drift` with parameters
5. Test workflow end-to-end

**Phase 4: Verification and Cleanup**
1. Run workflow and compare output with previous runs
2. Verify book builds successfully
3. Verify all chapters updated correctly
4. Remove old Prodigy-specific commands (or mark deprecated)
5. Update documentation

### Backward Compatibility

**Temporary Compatibility**:
- Keep old Prodigy-specific commands until Prodigy migration complete
- Support both old and new workflow formats during transition
- Provide migration guide in PR description

**No Long-term Compatibility**:
- Old commands will be removed after migration
- Old workflow format will not be supported
- Configuration format may evolve

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
