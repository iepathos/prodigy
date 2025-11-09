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

### GitHub Pages Versioning Architecture

Understanding how GitHub Pages serves multiple versions is critical for implementing this spec correctly.

#### Two-Branch Architecture

**Branch 1: Source Branch (main/master)**
```
main branch (your source code):
├── src/                    # Rust/source code
├── book/src/               # Markdown documentation source
├── workflows/
│   └── book-docs-drift.yml
└── .github/workflows/
    └── deploy-docs-versioned.yml

Git tags on main:
- v0.2.4
- v0.2.5
- v0.2.6
```

**Branch 2: gh-pages Branch (built output)**
```
gh-pages branch (generated HTML, managed by peaceiris action):
├── v0.2.4/
│   ├── index.html
│   ├── intro.html
│   └── ... (complete mdBook build)
├── v0.2.5/
│   └── ... (complete mdBook build)
├── v0.2.6/
│   └── ... (complete mdBook build)
└── latest/
    └── ... (copy of newest version)

No tags on gh-pages, just directories!
```

**GitHub Pages Configuration:**
- Repository Settings → Pages → Source: "Deploy from a branch"
- Branch: `gh-pages`, Directory: `/` (root)

#### Complete Deployment Flow

**Step 1: Developer pushes tag**
```bash
git tag v0.2.6
git push origin v0.2.6
```

**Step 2: GitHub Actions workflow triggers**
```yaml
on:
  push:
    tags:
      - 'v*.*.*'  # Matches v0.2.6
```

**Step 3: Checkout tag on GitHub Actions runner**
```yaml
- name: Checkout tag
  uses: actions/checkout@v5
  with:
    ref: v0.2.6  # Repository now at v0.2.6 state
```

**Step 4: Build docs from tagged version**
```yaml
- name: Run book workflow
  run: prodigy run workflows/book-docs-drift.yml
  env:
    VERSION: v0.2.6

# This executes:
# 1. Prodigy creates worktree from v0.2.6 (already checked out)
# 2. Claude analyzes v0.2.6 code in worktree
# 3. Claude generates docs with "v0.2.6" labels
# 4. mdbook build creates HTML in book/book/
```

**Step 5: Deploy to gh-pages with peaceiris action**
```yaml
- name: Deploy to GitHub Pages
  uses: peaceiris/actions-gh-pages@v4
  with:
    github_token: ${{ secrets.GITHUB_TOKEN }}
    publish_dir: ./book/book       # What to deploy
    destination_dir: v0.2.6        # Where to put it
    keep_files: true               # CRITICAL: preserve other versions
```

**What peaceiris/actions-gh-pages does:**
```bash
# 1. Switches to gh-pages branch
git checkout gh-pages

# 2. Creates/updates version directory
rm -rf v0.2.6/               # Remove old v0.2.6 if exists
mkdir -p v0.2.6/             # Create directory
cp -r ./book/book/* v0.2.6/  # Copy all built HTML/CSS/JS

# 3. Commits changes
git add v0.2.6/
git commit -m "docs: deploy v0.2.6"

# 4. Pushes to GitHub
git push origin gh-pages
```

**Step 6: GitHub Pages automatically serves updated content**

Within minutes, GitHub Pages serves:
- `https://yourorg.github.io/yourproject/v0.2.6/` (new version)
- `https://yourorg.github.io/yourproject/v0.2.5/` (still there!)
- `https://yourorg.github.io/yourproject/v0.2.4/` (still there!)

#### The Critical `keep_files: true` Parameter

**Without `keep_files: true` (DANGEROUS - default behavior):**
```yaml
keep_files: false  # or omitted

# What happens:
1. Checkout gh-pages
2. DELETE EVERYTHING on gh-pages branch  ← WIPES ALL VERSIONS!
3. Add new v0.2.6/ directory
4. Commit and push

# Result: gh-pages branch only contains v0.2.6/
# You LOSE v0.2.5, v0.2.4, and all other versions!
```

**With `keep_files: true` (SAFE - required for versioning):**
```yaml
keep_files: true

# What happens:
1. Checkout gh-pages
2. Keep all existing directories (v0.2.5/, v0.2.4/, etc.)
3. Update/add v0.2.6/ directory
4. Commit and push

# Result: gh-pages branch contains ALL versions
# v0.2.6/, v0.2.5/, v0.2.4/ all coexist!
```

#### URL Structure and File Mapping

**gh-pages branch structure:**
```
gh-pages/
├── index.html                    → yourorg.github.io/project/
├── versions.json                 → yourorg.github.io/project/versions.json
├── v0.2.6/
│   ├── index.html               → yourorg.github.io/project/v0.2.6/
│   ├── intro.html               → yourorg.github.io/project/v0.2.6/intro.html
│   └── getting-started.html     → yourorg.github.io/project/v0.2.6/getting-started.html
├── v0.2.5/
│   └── index.html               → yourorg.github.io/project/v0.2.5/
└── latest/
    └── index.html               → yourorg.github.io/project/latest/
```

**GitHub Pages serves the gh-pages branch as a static file server:**
- Path on gh-pages branch directly maps to URL path
- No special GitHub Pages features needed
- Works exactly like serving from any static file host

#### Version Selector Integration

With the version selector UI (Spec 154), users can switch between versions:

**User visits:** `yourorg.github.io/project/v0.2.6/getting-started.html`

**Version selector JavaScript detects:**
```javascript
// Parse URL: /v0.2.6/getting-started.html
const currentVersion = "v0.2.6";
const currentPage = "getting-started.html";

// Fetch versions.json from root
fetch('/versions.json')
  .then(data => {
    // Build dropdown with all versions
    buildDropdown(data.versions, currentVersion);
  });

// When user selects v0.2.7:
function switchVersion(newVersion) {
  // Navigate to: /v0.2.7/getting-started.html
  window.location.href = `/${newVersion}/${currentPage}`;
}
```

**Same domain, different subdirectories** - version selector works perfectly.

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

Claude commands must be updated to accept and utilize the `--version` flag. Each command should:

1. **Accept the --version parameter**
2. **Include version in generated content**
3. **Add version metadata to outputs**
4. **Generate version-appropriate documentation**

#### Command Modification Requirements

**For `/prodigy-analyze-features-for-book`:**

Add to the command prompt:
```markdown
## Version Parameter

This command accepts an optional `--version` flag to generate version-specific feature inventory.

**Usage:**
\`\`\`bash
/prodigy-analyze-features-for-book --project Prodigy --config .prodigy/book-config.json --version v0.2.6
\`\`\`

**Behavior:**

1. **Version Metadata**: Add version field to feature inventory JSON
2. **Output Path**: Save to version-scoped directory (`.prodigy/book-analysis/v0.2.6/features.json`)
3. **Historical Accuracy**: Document features as they exist in the current worktree (which corresponds to the checked-out version)

**Example Output:**
\`\`\`json
{
  "version": "v0.2.6",
  "generated_at": "2025-01-15T10:30:00Z",
  "commit": "abc123def456",
  "features": {
    "cli_commands": [...],
    "api_functions": [...]
  }
}
\`\`\`

**Important:** The code being analyzed is already at the correct version because the deployment workflow (Spec 155) checks out the tag before running this workflow. The --version flag is primarily for labeling and organizing outputs, not for determining which code to analyze.
```

**For `/prodigy-analyze-book-chapter-drift` and `/prodigy-fix-chapter-drift`:**

Add to the command prompts:
```markdown
## Version Parameter

This command accepts an optional `--version` flag to generate version-specific documentation.

**Usage:**
\`\`\`bash
/prodigy-fix-chapter-drift --project Prodigy --chapter-id getting-started --version v0.2.6
\`\`\`

**Behavior:**

1. **Version Badge**: Include version indicator at the top of each chapter
2. **Version-Specific Instructions**: Reference the specific version in installation and usage examples
3. **Accurate Feature Documentation**: Document only features available in this version (the code in the worktree)

**Generated Content Example:**

\`\`\`markdown
# Getting Started

<div class="version-info">
📘 Documentation for Prodigy v0.2.6
</div>

## Installation

To install Prodigy v0.2.6:

\`\`\`bash
cargo install prodigy@0.2.6
\`\`\`

Or download from the [v0.2.6 release](https://github.com/yourorg/prodigy/releases/tag/v0.2.6).

## Quick Start

...
\`\`\`

**Version-Specific Considerations:**

- Use API signatures and syntax from this version
- Include version number in code examples and installation instructions
- Don't document features added in later versions
- If a feature was removed in later versions, document it as it exists in this version
- Reference version-specific release notes or changelogs when relevant
```

#### How Versioning Actually Works

**Critical Understanding:**

The deployment workflow (Spec 155) follows this sequence:

```
1. GitHub Actions checks out tag v0.2.6
   ↓
2. Repository is now at v0.2.6 state
   ↓
3. Prodigy creates worktree from current state (v0.2.6)
   ↓
4. Claude analyzes code in worktree (v0.2.6 code)
   ↓
5. --version v0.2.6 tells Claude to label docs with v0.2.6
   ↓
6. mdbook build creates HTML
   ↓
7. Deploy to gh-pages:/v0.2.6/ subdirectory
```

**The VERSION parameter serves two purposes:**

1. **Metadata/Labeling**: Include "v0.2.6" in generated docs, version badges, installation instructions
2. **Output Organization**: Save analysis to `.prodigy/book-analysis/v0.2.6/`

**It does NOT:**
- ❌ Control which code is analyzed (that's already at the tag)
- ❌ Checkout a different version (deployment workflow handles that)
- ❌ Determine feature availability (features are determined by reading the worktree code)

**Example: Include version in chapter updates**:
```markdown
# Getting Started

<div class="version-info">
📘 Documentation for Prodigy v0.2.6
</div>

## Installation

To install Prodigy v0.2.6:
\`\`\`bash
cargo install prodigy@0.2.6
\`\`\`

Download: [v0.2.6 Release](https://github.com/yourorg/prodigy/releases/tag/v0.2.6)
```

**Version-aware examples**:
- If analyzing v0.2.5, don't include features added in v0.2.6 (Claude won't see them in the code anyway)
- Use API signatures from tagged version (what's in the worktree)
- Reference version-specific release URLs and package versions

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
