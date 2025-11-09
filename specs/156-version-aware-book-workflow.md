---
number: 156
title: Version-Aware Book Workflow
category: foundation
priority: medium
status: draft
dependencies: [154, 155]
created: 2025-01-11
---

# Specification 156: Version-Aware Book Workflow

**Category**: foundation
**Priority**: medium
**Status**: draft
**Dependencies**: Spec 154 (mdBook Version Selector UI), Spec 155 (Versioned Documentation Deployment)

## Context

The current `book-docs-drift.yml` workflow analyzes the codebase at HEAD and generates documentation for the current state of the code. This works well for maintaining docs on the main branch, but doesn't support versioned documentation.

With Spec 154 (Version Selector UI) and Spec 155 (Versioned Deployment) in place, we can serve multiple doc versions. However, the workflow needs to be version-aware to:
- Accept a VERSION parameter specifying which version to document
- Analyze code at that specific version (git tag)
- Include version information in generated documentation
- Support both "latest" (main branch) and specific tagged versions

This enables the deployment workflow (Spec 155) to checkout a specific tag, run the book workflow for that version, and deploy to the corresponding subdirectory.

## Objective

Enhance the book-docs-drift workflow to:
- Accept an optional VERSION environment variable
- Operate correctly on tagged versions of the codebase
- Include version metadata in generated documentation
- Support both latest (unversioned) and specific version workflows
- Remain backward compatible for projects not using versioning

## Requirements

### Functional Requirements

- **VERSION Parameter**: Accept `VERSION` env var (e.g., "v0.2.6", "latest")
- **Version Detection**: Auto-detect version from git tag if not provided
- **Version in Docs**: Include version number in generated documentation pages
- **Tag-Based Analysis**: Analyze code at the specified git tag
- **Feature Inventory Versioning**: Include version in feature inventory file paths
- **Backward Compatibility**: Workflow works without VERSION parameter (defaults to "latest")
- **Version Validation**: Validate VERSION matches expected format (semver or "latest")
- **Error Handling**: Clear error if VERSION tag doesn't exist

### Non-Functional Requirements

- **Transparent**: Minimal changes to existing workflow structure
- **Reusable**: Works for any project using the book workflow system
- **Debuggable**: Clear logs showing which version is being processed
- **Idempotent**: Running workflow multiple times for same version produces same output

## Acceptance Criteria

- [ ] `book-docs-drift.yml` accepts `VERSION` env var
- [ ] If VERSION not set, defaults to "latest" (current behavior)
- [ ] Workflow includes VERSION in analysis directory paths (e.g., `.prodigy/book-analysis/v0.2.6/`)
- [ ] Generated docs include version number in footer or header
- [ ] Claude commands receive VERSION parameter
- [ ] Feature inventory includes version metadata
- [ ] Workflow validates VERSION format before starting
- [ ] Clear error message if VERSION tag doesn't exist
- [ ] Backward compatibility: existing workflows continue working unchanged
- [ ] Documentation updated with version parameter usage

## Technical Details

### Implementation Approach

**Workflow Enhancement Structure**:
```yaml
# book-docs-drift.yml
name: book-docs-drift-detection
mode: mapreduce

env:
  # Version configuration
  VERSION: "${VERSION:-latest}"  # Accept from caller or default to "latest"

  # Project configuration
  PROJECT_NAME: "Prodigy"
  PROJECT_CONFIG: ".prodigy/book-config.json"

  # Version-aware paths
  ANALYSIS_DIR: ".prodigy/book-analysis/${VERSION}"
  FEATURES_PATH: "${ANALYSIS_DIR}/features.json"

  # Book-specific settings
  BOOK_DIR: "book"
  CHAPTERS_FILE: "workflows/data/prodigy-chapters.json"

  # Workflow settings
  MAX_PARALLEL: "3"
```

**Key Changes**:
1. `VERSION` defaults to "latest" if not provided
2. `ANALYSIS_DIR` includes version: `.prodigy/book-analysis/v0.2.6/`
3. Feature inventory scoped to version: `features-v0.2.6.json`
4. Version passed to Claude commands for inclusion in docs

### Version Detection Logic

**Auto-detect from git tag** (optional enhancement):
```yaml
setup:
  - shell: |
      # If VERSION not set or is "latest", try to detect from git tag
      if [ "${VERSION}" = "latest" ] || [ -z "${VERSION}" ]; then
        DETECTED_VERSION=$(git describe --tags --exact-match 2>/dev/null || echo "latest")
        echo "VERSION=${DETECTED_VERSION}" >> $GITHUB_ENV
      fi
```

### Version Validation

**Validate VERSION format before processing**:
```yaml
setup:
  - shell: |
      # Validate VERSION format (semver vX.Y.Z or "latest")
      if [ "${VERSION}" != "latest" ]; then
        if ! echo "${VERSION}" | grep -qE '^v[0-9]+\.[0-9]+\.[0-9]+$'; then
          echo "Error: VERSION must be semver (vX.Y.Z) or 'latest'"
          exit 1
        fi

        # Verify tag exists
        if ! git rev-parse "${VERSION}" >/dev/null 2>&1; then
          echo "Error: Tag ${VERSION} does not exist"
          exit 1
        fi
      fi
```

### Claude Command Updates

**Pass VERSION to Claude commands**:
```yaml
setup:
  - claude: "/prodigy-analyze-features-for-book --project $PROJECT_NAME --config $PROJECT_CONFIG --version $VERSION"

map:
  agent_template:
    - claude: "/prodigy-analyze-book-chapter-drift --project $PROJECT_NAME --json '${item}' --features $FEATURES_PATH --version $VERSION"
    - claude: "/prodigy-fix-chapter-drift --project $PROJECT_NAME --chapter-id ${item.id} --version $VERSION"
```

**Claude commands use VERSION**:
- Include version in generated doc headers
- Tag drift reports with version
- Reference version-specific code examples

### Version in Generated Documentation

**Add version to mdBook configuration** (optional):
```toml
# book/book.toml
[book]
title = "Prodigy Documentation"
description = "AI-powered workflow orchestration for development teams"

[output.html]
# Version injected via preprocessor or theme
additional-css = ["theme/version-badge.css"]
```

**Version badge/footer**:
```html
<!-- Injected into each page -->
<div class="version-badge">
  Documentation for version v0.2.6
</div>
```

### Feature Inventory Versioning

**Version-scoped feature inventory**:
```json
{
  "version": "v0.2.6",
  "generated_at": "2025-01-15T10:30:00Z",
  "features": {
    "cli_commands": [...],
    "api_functions": [...]
  }
}
```

**Benefit**: Historical record of features at each version, useful for:
- Documenting when features were added/removed
- Generating changelogs
- Comparing feature sets across versions

## Dependencies

**Prerequisites**:
- Spec 154 (mdBook Version Selector UI) - Provides version switching frontend
- Spec 155 (Versioned Documentation Deployment) - Orchestrates version-specific builds

**Affected Components**:
- `workflows/book-docs-drift.yml` - Enhanced with VERSION support
- `.claude/commands/prodigy-analyze-features-for-book.md` - Accept --version flag
- `.claude/commands/prodigy-analyze-book-chapter-drift.md` - Accept --version flag
- `.claude/commands/prodigy-fix-chapter-drift.md` - Accept --version flag
- `.prodigy/book-config.json` - Optional version-specific config

**External Dependencies**: None (all changes internal)

## Testing Strategy

### Unit Tests
- Test VERSION defaulting to "latest"
- Test VERSION validation regex
- Test analysis directory path construction with VERSION
- Test version metadata injection into feature inventory

### Integration Tests
- **Latest Version Test**: Run workflow without VERSION, verify "latest" used
- **Specific Version Test**: Run workflow with VERSION=v0.2.6, verify correct tag analyzed
- **Invalid Version Test**: Run with invalid VERSION, verify error
- **Missing Tag Test**: Run with non-existent tag, verify error
- **Backward Compatibility Test**: Run existing workflow configs, verify no breakage

### End-to-End Tests
- Deploy v0.2.6 docs using versioned workflow
- Deploy v0.2.5 docs using versioned workflow
- Verify both exist with correct content
- Switch between versions using UI, verify correct docs displayed

### User Acceptance
- Manual review of generated docs for correct version
- Verify version badge/footer displays correctly
- Confirm feature examples match tagged version
- Validate drift detection works on historical versions

## Documentation Requirements

### Code Documentation
- Comment VERSION parameter usage in workflow YAML
- Document Claude command --version flag
- Explain version-scoped analysis directories

### User Documentation
- **Update automated-documentation.md**: Add "Versioned Workflows" section
- **Setup guide**: How to enable versioning in book workflow
- **Usage examples**: Running workflow for specific versions
- **Troubleshooting**: Common version-related issues

### Configuration Examples

**For versioned docs**:
```yaml
# Run workflow for specific version
export VERSION=v0.2.6
prodigy run workflows/book-docs-drift.yml
```

**For latest docs** (default):
```yaml
# No VERSION needed, defaults to "latest"
prodigy run workflows/book-docs-drift.yml
```

## Implementation Notes

### Git Worktree Considerations

The workflow runs in a git worktree (Spec 127). For versioned docs:
- Deployment workflow (Spec 155) checks out the tag
- Prodigy creates worktree from that checked-out state
- Workflow analyzes code at the tag, not HEAD
- All git operations are scoped to the tag

**Important**: Workflow doesn't need to checkout tag itself - deployment workflow handles this.

### Version in Drift Detection

When analyzing drift for historical versions:
- Compare docs to code **at that version**, not current main
- Drift reports reference version-specific features
- Recommendations consider version-specific APIs
- Examples use syntax available in that version

### Claude Command Enhancements

**Example: Include version in chapter updates**:
```markdown
# Getting Started

> **Version**: v0.2.6

To install Prodigy v0.2.6:
\`\`\`bash
cargo install prodigy@0.2.6
\`\`\`
```

**Version-aware examples**:
- If analyzing v0.2.5, don't include features added in v0.2.6
- Use API signatures from tagged version, not latest

### Cleanup Strategy

**Version-scoped analysis directories**:
```
.prodigy/book-analysis/
├── latest/
│   ├── features.json
│   └── drift-*.json
├── v0.2.6/
│   ├── features.json
│   └── drift-*.json
└── v0.2.5/
    ├── features.json
    └── drift-*.json
```

**Cleanup options**:
1. Keep version-specific analysis for historical record
2. Clean up after merge (current behavior)
3. Optional retention policy (keep last N versions)

## Migration and Compatibility

### Backward Compatibility

**Existing workflows continue working**:
- If VERSION not set, defaults to "latest"
- Existing env var structure unchanged
- No breaking changes to workflow interface

**Gradual Adoption**:
1. Projects can adopt versioning incrementally
2. Start with adding VERSION parameter support
3. Later enable deployment workflow (Spec 155)
4. Finally add version selector UI (Spec 154)

### Configuration for Other Projects

**Minimal changes needed**:
```yaml
# Existing workflow
env:
  PROJECT_NAME: "MyProject"
  # ... other vars

# Add versioning support
env:
  VERSION: "${VERSION:-latest}"  # Just add this line
  PROJECT_NAME: "MyProject"
  ANALYSIS_DIR: ".myproject/book-analysis/${VERSION}"  # Make version-aware
  # ... other vars
```

### Migration Checklist

For existing book workflow users adopting versioning:
- [ ] Add `VERSION: "${VERSION:-latest}"` to env block
- [ ] Update `ANALYSIS_DIR` to include `${VERSION}`
- [ ] Update Claude command calls to pass `--version $VERSION`
- [ ] Test with VERSION unset (should default to "latest")
- [ ] Test with VERSION=v1.0.0 (should work with tag)
- [ ] Deploy using Spec 155 workflow

## File Locations

**Workflow**:
- `workflows/book-docs-drift.yml` - Enhanced with VERSION support

**Claude Commands** (updated):
- `.claude/commands/prodigy-analyze-features-for-book.md` - Add --version flag
- `.claude/commands/prodigy-analyze-book-chapter-drift.md` - Add --version flag
- `.claude/commands/prodigy-fix-chapter-drift.md` - Add --version flag

**Documentation**:
- `book/src/automated-documentation.md` - Add versioning section
- `book/src/versioning-workflows.md` - New chapter on version workflows

**Examples**:
- `examples/versioned-workflow/` - Example configuration
- `examples/versioned-workflow/book-docs-drift.yml` - Template workflow

## Success Metrics

- Workflow successfully processes docs for v0.2.6, v0.2.5, v0.2.4
- Generated docs correctly show version in badge/footer
- Feature inventory scoped to version shows correct features for each release
- Backward compatibility: existing non-versioned workflows unchanged
- Zero regression in non-versioned workflow usage
- Documentation clear enough for external projects to adopt
- Integration with Spec 155 deployment workflow successful
