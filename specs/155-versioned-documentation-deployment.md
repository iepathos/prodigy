---
number: 155
title: Versioned Documentation Deployment
category: foundation
priority: high
status: draft
dependencies: [154]
created: 2025-01-11
---

# Specification 155: Versioned Documentation Deployment

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: Spec 154 (mdBook Version Selector UI)

## Context

Prodigy currently deploys documentation from the main branch to GitHub Pages at the root path. Users viewing documentation always see the latest version, even if they're using an older release of the software.

Many projects (e.g., Rust, MkDocs, Docusaurus) serve multiple documentation versions simultaneously, deployed to subdirectories (e.g., `/v0.2.6/`, `/v0.2.5/`, `/latest/`). This allows users to access docs matching their installed version.

The version selector UI (Spec 154) provides the frontend component, but we need deployment infrastructure to:
- Build documentation for each git tag
- Deploy to version-specific subdirectories on GitHub Pages
- Maintain a `versions.json` manifest
- Update the `/latest/` symlink or copy

## Objective

Create a GitHub Actions workflow system that:
- Automatically builds and deploys versioned documentation when tags are pushed
- Deploys each version to its own subdirectory on the `gh-pages` branch
- Maintains a `versions.json` file with version metadata
- Supports manual rebuilds of all versions (for theme updates)
- Works for Prodigy and is reusable by other projects

## Requirements

### Functional Requirements

- **Tag-Triggered Build**: Automatically build docs when a version tag (e.g., `v0.2.6`) is pushed
- **Version-Specific Deployment**: Deploy to `/vX.Y.Z/` subdirectory on `gh-pages` branch
- **Latest Pointer**: Update `/latest/` to point to newest version
- **versions.json Generation**: Create or update `versions.json` with all deployed versions
- **Manual Rebuild**: Support manual workflow dispatch to rebuild specific or all versions
- **Parallel Builds**: Build multiple versions concurrently when rebuilding all
- **Keep Files**: Don't delete other versions when deploying a new one
- **Root Redirect**: Create root `index.html` redirecting to `/latest/`

### Non-Functional Requirements

- **Idempotent**: Rebuilding same version produces identical output
- **Fast**: Deploy single version in < 5 minutes
- **Reliable**: Fail gracefully on build errors, don't corrupt existing versions
- **Secure**: Use minimal permissions, no hard-coded secrets
- **Configurable**: Support different project structures via workflow inputs
- **Auditable**: Clear logs showing what was deployed where

## Acceptance Criteria

- [ ] GitHub Actions workflow (`deploy-docs-versioned.yml`) created
- [ ] Workflow triggers on tag push matching `v*.*.*` pattern
- [ ] Workflow supports manual dispatch with version input
- [ ] Each version deploys to `gh-pages:/vX.Y.Z/` directory
- [ ] `versions.json` generated with all deployed versions
- [ ] `/latest/` directory updated to newest version
- [ ] Root `index.html` redirects to `/latest/`
- [ ] Existing versions preserved when deploying new version
- [ ] Workflow supports rebuilding all versions from tags
- [ ] Failed builds don't corrupt `gh-pages` branch
- [ ] Clear commit messages on `gh-pages` branch show what was deployed
- [ ] Documentation for workflow configuration and customization

## Technical Details

### Implementation Approach

**Deployment Architecture**:
```
Git Repository
├── main branch (source code)
│   ├── book/src/*.md (docs source)
│   └── .github/workflows/deploy-docs-versioned.yml
│
├── Tag: v0.2.6
├── Tag: v0.2.5
├── Tag: v0.2.4
│
└── gh-pages branch (deployed docs)
    ├── index.html → redirects to /latest/
    ├── versions.json
    ├── latest/ → copy of v0.2.6
    ├── v0.2.6/
    │   ├── index.html
    │   └── ... (mdBook build output)
    ├── v0.2.5/
    │   └── ...
    └── v0.2.4/
        └── ...
```

### GitHub Actions Workflow Structure

**File**: `.github/workflows/deploy-docs-versioned.yml`

**Triggers**:
```yaml
on:
  push:
    tags:
      - 'v*.*.*'  # Trigger on semver tags
  workflow_dispatch:
    inputs:
      version:
        description: 'Version to deploy (tag name, "all", or "latest")'
        required: true
        default: 'latest'
      rebuild_all:
        description: 'Rebuild all versions'
        type: boolean
        default: false
```

**Jobs**:
1. **deploy-version**: Build and deploy specific version
2. **update-versions-json**: Update versions.json manifest
3. **update-latest**: Update /latest/ pointer
4. **rebuild-all-versions**: Rebuild all tagged versions (manual only)

### Workflow Logic

**Single Version Deployment**:
```yaml
deploy-version:
  runs-on: ubuntu-latest
  steps:
    - name: Determine version
      id: version
      run: |
        if [ "${{ github.event_name }}" = "push" ]; then
          VERSION="${{ github.ref_name }}"
        else
          VERSION="${{ github.event.inputs.version }}"
        fi
        echo "version=${VERSION}" >> $GITHUB_OUTPUT

    - name: Checkout tag
      uses: actions/checkout@v5
      with:
        ref: ${{ steps.version.outputs.version }}
        fetch-depth: 0

    - name: Setup Prodigy
      run: cargo install --path .

    - name: Setup mdBook
      uses: peaceiris/actions-mdbook@v2
      with:
        mdbook-version: 'latest'

    - name: Run book workflow
      run: prodigy run workflows/book-docs-drift.yml
      env:
        VERSION: ${{ steps.version.outputs.version }}

    - name: Deploy to GitHub Pages
      uses: peaceiris/actions-gh-pages@v4
      with:
        github_token: ${{ secrets.GITHUB_TOKEN }}
        publish_dir: ./book/book
        destination_dir: ${{ steps.version.outputs.version }}
        keep_files: true  # Don't delete other versions
        commit_message: "docs: deploy ${{ steps.version.outputs.version }}"
```

**Update versions.json**:
```yaml
update-versions-json:
  needs: deploy-version
  runs-on: ubuntu-latest
  steps:
    - name: Checkout gh-pages
      uses: actions/checkout@v5
      with:
        ref: gh-pages

    - name: Generate versions.json
      run: |
        ./scripts/generate-versions-json.sh > versions.json

    - name: Commit versions.json
      run: |
        git add versions.json
        git commit -m "docs: update versions.json"
        git push
```

**Update latest pointer**:
```yaml
update-latest:
  needs: deploy-version
  if: ${{ needs.deploy-version.outputs.is_latest == 'true' }}
  runs-on: ubuntu-latest
  steps:
    - name: Deploy to /latest/
      uses: peaceiris/actions-gh-pages@v4
      with:
        github_token: ${{ secrets.GITHUB_TOKEN }}
        publish_dir: ./book/book
        destination_dir: latest
        keep_files: true
        commit_message: "docs: update /latest/ to ${{ needs.deploy-version.outputs.version }}"
```

### versions.json Generation Script

**File**: `scripts/generate-versions-json.sh`

```bash
#!/bin/bash
# Generates versions.json by scanning gh-pages branch directories

set -e

# Find all version directories (matching vX.Y.Z pattern)
VERSIONS=$(find . -maxdepth 1 -type d -name 'v*.*.*' | sed 's|./||' | sort -V -r)

# Determine latest version (highest semver)
LATEST=$(echo "$VERSIONS" | head -1)

# Build JSON
echo "{"
echo "  \"latest\": \"$LATEST\","
echo "  \"versions\": ["

FIRST=true
for VERSION in $VERSIONS; do
  if [ "$FIRST" = true ]; then
    FIRST=false
  else
    echo ","
  fi

  LABEL="$VERSION"
  if [ "$VERSION" = "$LATEST" ]; then
    LABEL="$VERSION (Latest)"
  fi

  # Extract release date from git commit (if available)
  RELEASED=$(git log --format=%aI --max-count=1 --all -- "$VERSION" 2>/dev/null || echo "")

  echo "    {"
  echo "      \"version\": \"$VERSION\","
  echo "      \"path\": \"/$VERSION/\","
  echo "      \"label\": \"$LABEL\""
  [ -n "$RELEASED" ] && echo "      ,\"released\": \"$RELEASED\""
  echo -n "    }"
done

echo ""
echo "  ]"
echo "}"
```

### Root Index Redirect

**File**: `scripts/create-root-redirect.sh`

```bash
#!/bin/bash
# Creates index.html at root that redirects to /latest/

cat > index.html <<'EOF'
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta http-equiv="refresh" content="0; url=/latest/">
  <title>Redirecting to Documentation</title>
</head>
<body>
  <p>Redirecting to <a href="/latest/">latest documentation</a>...</p>
</body>
</html>
EOF
```

### Version Detection Logic

**Determine if deployed version is latest**:
```bash
#!/bin/bash
# Compare deployed version to all existing versions on gh-pages

DEPLOYED_VERSION="$1"
git fetch origin gh-pages
git checkout gh-pages

# Get all version directories
ALL_VERSIONS=$(find . -maxdepth 1 -type d -name 'v*.*.*' | sed 's|./||' | sort -V)

# Get highest version
LATEST=$(echo "$ALL_VERSIONS" | tail -1)

if [ "$DEPLOYED_VERSION" = "$LATEST" ]; then
  echo "is_latest=true" >> $GITHUB_OUTPUT
else
  echo "is_latest=false" >> $GITHUB_OUTPUT
fi
```

## Dependencies

**Prerequisites**:
- Spec 154 (mdBook Version Selector UI) - Provides frontend component
- Git tags following semver pattern (vX.Y.Z)
- GitHub Pages enabled for repository

**Affected Components**:
- `.github/workflows/deploy-docs.yml` - Existing deploy workflow (rename or deprecate)
- `book-docs-drift.yml` - May need VERSION env var support
- GitHub Pages configuration - Deploy from gh-pages branch

**External Dependencies**:
- `peaceiris/actions-mdbook@v2` - mdBook setup
- `peaceiris/actions-gh-pages@v4` - GitHub Pages deployment
- Standard GitHub Actions environment

**New Scripts**:
- `scripts/generate-versions-json.sh` - versions.json generator
- `scripts/create-root-redirect.sh` - Root redirect page
- `scripts/deploy-version.sh` - Helper for manual deployments

## Testing Strategy

### Unit Tests
- Test `generate-versions-json.sh` with mock directory structure
- Test version comparison logic (semver sorting)
- Test root redirect creation script

### Integration Tests
- **Manual Workflow Test**: Trigger workflow_dispatch with specific version
- **Tag Push Test**: Push a test tag, verify deployment to correct subdirectory
- **versions.json Test**: Verify versions.json updated with new version
- **Latest Update Test**: Verify /latest/ points to newest version
- **Preservation Test**: Deploy v0.2.6, then v0.2.5, verify both exist

### End-to-End Tests
- Full deployment flow: tag → build → deploy → verify
- Rebuild all versions: ensure all tags get rebuilt
- Theme update scenario: rebuild all versions with new theme
- Rollback scenario: redeploy older version as /latest/

### User Acceptance
- Navigate between versions using version selector (from Spec 154)
- Verify page paths preserved when switching versions
- Check all links work within each version
- Confirm search works within each version (mdBook search is version-scoped)

## Documentation Requirements

### Code Documentation
- Inline comments in workflow YAML explaining each step
- Comments in bash scripts for version detection logic
- Document workflow inputs and outputs

### User Documentation
- **Setup Guide**: How to configure workflow for new repository
- **Deployment Guide**: How to trigger manual deployments
- **Troubleshooting**: Common deployment issues and solutions
- **Customization**: How to modify workflow for different needs

### Repository-Specific Configuration
- **Template workflow**: Annotated example for other projects
- **Configuration variables**: Document required/optional inputs
- **Permissions**: Required GitHub permissions for workflow

### Architecture Updates
- Update `automated-documentation.md` with versioning deployment section
- Add diagram showing deployment flow
- Document version retention policy (keep last N versions)

## Implementation Notes

### GitHub Pages Limitations
- **gh-pages branch size**: Monitor total size, GitHub Pages has 1GB soft limit
- **Build time**: GitHub Actions has 6-hour timeout (unlikely to hit)
- **Concurrent deployments**: peaceiris/actions-gh-pages handles concurrency

### Version Retention Strategy
- **Keep all major versions**: v1.0.0, v2.0.0, v3.0.0
- **Keep last 3 minor versions** per major: v2.3.0, v2.2.0, v2.1.0
- **Keep last 5 patch versions** per minor: v2.3.5, v2.3.4, ...
- **Cleanup old versions**: Manual or automated script

Example retention script:
```bash
# scripts/cleanup-old-versions.sh
# Keep last 5 versions, delete older ones
```

### Workflow Performance Optimization
- **Parallel builds**: Use matrix strategy for rebuilding all versions
- **Caching**: Cache Cargo build, mdBook binary
- **Conditional steps**: Skip unnecessary steps (e.g., don't update /latest/ for old versions)

### Security Considerations
- **Minimal permissions**: Workflow only needs `contents: write` for gh-pages
- **No secrets required**: Uses GitHub-provided `GITHUB_TOKEN`
- **Input validation**: Validate version tag format before checkout

### Rollback Procedure
If deployment fails or deploys broken docs:
1. Manually delete version directory from gh-pages: `git rm -r vX.Y.Z/`
2. Revert versions.json: `git revert <commit>`
3. Optionally redeploy previous version as /latest/

## Migration and Compatibility

### Backward Compatibility
- Existing non-versioned deployment continues to work
- Users can gradually migrate to versioned deployment
- Both workflows can coexist temporarily

### Migration Path
1. **Phase 1**: Add `deploy-docs-versioned.yml`, keep `deploy-docs.yml`
2. **Phase 2**: Deploy current version to `/v{current}/` and `/latest/`
3. **Phase 3**: Deploy historical versions (last 3-5 tags)
4. **Phase 4**: Add version selector UI (Spec 154)
5. **Phase 5**: Deprecate `deploy-docs.yml`, use only versioned workflow

### Configuration for Other Projects

**Minimal configuration** for other repos:
```yaml
# .github/workflows/deploy-docs-versioned.yml
# Copy template and customize these variables:
env:
  BOOK_DIR: "book"              # Path to mdBook directory
  WORKFLOW_FILE: "workflows/book-docs-drift.yml"  # Optional: run workflow
  SKIP_WORKFLOW: false          # Set true to skip Prodigy workflow
```

## File Locations

**GitHub Actions Workflow**:
- `.github/workflows/deploy-docs-versioned.yml` - Main deployment workflow
- `.github/workflows/rebuild-all-docs.yml` - Helper for rebuilding all versions (optional)

**Scripts**:
- `scripts/generate-versions-json.sh` - versions.json generator
- `scripts/create-root-redirect.sh` - Root index.html creator
- `scripts/cleanup-old-versions.sh` - Version retention script
- `scripts/deploy-version.sh` - Manual deployment helper

**Documentation**:
- `book/src/versioning-deployment.md` - Deployment guide (new chapter)
- `book/src/automated-documentation.md` - Updated with versioning section
- `.github/workflows/README.md` - Workflow documentation

**Templates**:
- `templates/workflows/deploy-docs-versioned.yml` - Template for other repos
- `templates/scripts/` - Template scripts for other repos

## Success Metrics

- Successful deployment of 3+ versions to Prodigy's GitHub Pages
- versions.json correctly lists all deployed versions
- Version selector UI (Spec 154) successfully switches between versions
- /latest/ always points to newest version
- Workflow completes in < 5 minutes for single version
- Zero deployment failures in first month
- Documentation is clear enough for external projects to adopt
- At least one external project successfully deploys versioned docs
