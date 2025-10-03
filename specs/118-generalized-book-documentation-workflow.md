---
number: 118
title: Generalized Book Documentation Workflow
category: documentation
priority: high
status: draft
dependencies: []
created: 2025-10-03
---

# Specification 118: Generalized Book Documentation Workflow

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

This infrastructure is currently Prodigy-specific but should be generalized to work across multiple projects (e.g., Debtmap) with minimal configuration changes. The goal is to create reusable Claude commands and workflow patterns that can detect and fix documentation drift in any Rust project using mdBook.

## Objective

Create a generalized book documentation workflow system that:
1. Works across multiple projects without hardcoded project names
2. Uses project-agnostic Claude commands for analyzing features and fixing drift
3. Provides a template workflow that can be customized per project
4. Supports project-specific chapter structures and validation focuses
5. Enables consistent documentation quality across all projects in the monorepo

## Requirements

### Functional Requirements

**FR1**: Create project-agnostic Claude commands that:
- Accept project context as parameters rather than hardcoding project names
- Use generic terminology (e.g., "codebase features" not "Prodigy workflow features")
- Work with any Rust project structure
- Generate project-specific feature inventories

**FR2**: Implement parameterized workflow templates that:
- Accept project configuration through environment variables or YAML parameters
- Support custom chapter structures via external JSON files
- Allow project-specific validation criteria
- Enable customization of book build commands and paths

**FR3**: Generalize the analysis commands to:
- Detect features from any Rust codebase based on configuration
- Support different project architectures (CLI tools, libraries, applications)
- Generate appropriate feature inventories for different project types
- Allow customization of what features to analyze

**FR4**: Create reusable drift detection that:
- Compares any mdBook chapter against its codebase implementation
- Works with different chapter structures and topics
- Supports project-specific validation focuses
- Generates consistent drift reports across projects

**FR5**: Generalize the fix commands to:
- Update mdBook chapters based on drift reports from any project
- Maintain project-specific documentation style and tone
- Support different mdBook configurations
- Handle project-specific best practices

### Non-Functional Requirements

**NFR1**: **Maintainability**: Commands should be easy to understand and modify for new projects

**NFR2**: **Reusability**: Minimal duplication between project-specific workflows

**NFR3**: **Flexibility**: Easy to add new projects without modifying core commands

**NFR4**: **Consistency**: Similar documentation quality across all projects

**NFR5**: **Performance**: Efficient analysis and update process for large codebases

## Acceptance Criteria

- [ ] Claude commands use parameters instead of hardcoded project names
- [ ] Commands accept project-specific configuration (book paths, analysis targets, etc.)
- [ ] Workflow can be copied and customized for Debtmap with minimal changes
- [ ] Feature analysis adapts to different Rust project structures
- [ ] Chapter drift detection works across different documentation styles
- [ ] Fix commands preserve project-specific documentation characteristics
- [ ] Both Prodigy and Debtmap can use the same core Claude commands
- [ ] Documentation for using the system with new projects is clear
- [ ] Example configurations provided for different project types

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
- Rename `workflows/data/book-chapters.json` to `workflows/data/prodigy-chapters.json`

**Debtmap Files** (to be created):
- `../debtmap/.debtmap/book-config.json` - Debtmap configuration
- `../debtmap/workflows/data/debtmap-chapters.json` - Debtmap chapter definitions
- `../debtmap/workflows/book-docs-drift.yml` - Debtmap workflow instance
- `../debtmap/book/` - mdBook structure for Debtmap

**Deprecated Files**:
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

**Test 1: Prodigy Book Workflow**
- Run `prodigy run workflows/book-docs-drift.yml`
- Verify feature analysis generates `.prodigy/book-analysis/features.json`
- Verify drift detection creates drift reports for all chapters
- Verify fixes are applied and book builds successfully
- Verify merge workflow completes

**Test 2: Debtmap Book Workflow**
- Create Debtmap book structure with mdBook
- Create Debtmap book configuration and chapter definitions
- Run `prodigy run workflows/book-docs-drift.yml` from Debtmap directory
- Verify analysis and drift detection work for Debtmap codebase
- Verify book builds successfully

**Test 3: Cross-Project Command Reuse**
- Verify `/analyze-codebase-features` works with both Prodigy and Debtmap configs
- Verify `/analyze-chapter-drift` produces consistent drift reports
- Verify `/fix-documentation-drift` handles different documentation styles

**Test 4: Configuration Validation**
- Test with missing required config fields
- Test with invalid paths in configuration
- Test with malformed chapter JSON
- Verify helpful error messages

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
How to set up automated book documentation for any project using Prodigy workflows.

## Setup
1. Install mdBook
2. Create book structure: `mdbook init book`
3. Create project book configuration
4. Create chapter definitions
5. Create project workflow instance
6. Run workflow: `prodigy run workflows/book-docs-drift.yml`

## Configuration
- Project config structure and fields
- Chapter definition format
- Analysis target specification

## Customization
- Custom validation focuses
- Project-specific analysis targets
- Documentation style preservation

## Troubleshooting
- Common configuration errors
- Book build failures
- Drift detection issues
```

**Updated Documentation**:
- `README.md` - Add section on book documentation workflow
- `CLAUDE.md` - Document new Claude commands and their usage
- `book/src/` - Update Prodigy book with information about the generalized system

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
- Workflow variable names change (add `PROJECT_*` prefix)
- Command names change (remove `prodigy-` prefix)
- Chapter file path changes (add project name to filename)

### Migration Path

**For Prodigy**:
1. Create `.prodigy/book-config.json`
2. Rename `workflows/data/book-chapters.json` to `workflows/data/prodigy-chapters.json`
3. Update `workflows/book-docs-drift.yml` to add environment variables
4. Update workflow to use new command names
5. Test workflow execution
6. Remove old commands after verification

**For New Projects (Debtmap)**:
1. Run `mdbook init book` to create book structure
2. Create project book configuration at `.debtmap/book-config.json`
3. Create chapter definitions at `workflows/data/debtmap-chapters.json`
4. Copy workflow template to `workflows/book-docs-drift.yml`
5. Set environment variables for project
6. Create initial book content in `book/src/`
7. Run workflow to generate documentation
8. Set up GitHub workflow for deployment (optional)

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

**Adoption**:
- Both Prodigy and Debtmap use generalized workflow successfully
- Documentation stays in sync with codebase (drift detected and fixed within 1 week)
- New projects can be added in <30 minutes

**Quality**:
- Drift detection finds >90% of documentation/code mismatches
- Fixes maintain documentation quality without manual intervention
- Book builds succeed >95% of the time after fixes

**Maintainability**:
- Adding new chapter types requires only configuration changes
- Supporting new project types requires minimal command changes
- Developers can understand and modify configuration easily
