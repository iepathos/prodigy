## Documentation Versioning

> **Status: Planned Feature**
>
> Documentation versioning is currently in the design phase (Specifications 154, 155, 156 in draft status). This page documents the planned implementation for projects that need to serve multiple documentation versions.

For projects that need to serve multiple documentation versions (e.g., users on different software releases), Prodigy is designing a comprehensive versioned documentation system. This will allow users to select which version of the docs they want to view using a dropdown selector.

### Overview

The documentation versioning system consists of three integrated components:

1. **Version Selector UI** (Spec 154) - A dropdown component in the mdBook navigation that lets users switch between versions
2. **Versioned Deployment** (Spec 155) - GitHub Actions workflow that deploys each version to its own subdirectory
3. **Version-Aware Workflows** (Spec 156) - Enhanced book workflow that accepts a VERSION parameter for building version-specific documentation

**Planned Architecture:**
```
GitHub Repository
├── main branch (source code + docs)
├── Tags: v0.2.6, v0.2.5, v0.2.4, ...
└── gh-pages branch (deployed docs)
    ├── index.html → redirects to /latest/
    ├── versions.json
    ├── latest/ → copy of newest version
    ├── v0.2.6/ → built from v0.2.6 tag
    ├── v0.2.5/ → built from v0.2.5 tag
    └── v0.2.4/ → built from v0.2.4 tag
```

**Source**: specs/155-versioned-documentation-deployment.md:80-102

### Version Selector UI Component

**Design Overview** (Spec 154):

The version selector will be a lightweight JavaScript component that:
- Displays a dropdown in mdBook's navigation bar
- Fetches version metadata from `/versions.json` at the documentation root
- Preserves the current page path when switching versions (e.g., switching from `/v0.2.6/mapreduce/index.html` to `/v0.2.5/mapreduce/index.html`)
- Works across all mdBook themes with minimal configuration
- Gracefully degrades if `versions.json` is missing

**Integration Pattern:**
```toml
# book/book.toml
[output.html]
additional-css = ["theme/version-selector.css"]
additional-js = ["theme/version-selector.js"]
```

**Source**: specs/154-mdbook-version-selector-ui.md:110-116

**Key Features:**
- **Current Version Detection**: Automatically detects which version the user is viewing based on URL path pattern
- **Visual Indicators**: Highlights current version and marks latest version with "(Latest)" label
- **Fallback Handling**: If the current page doesn't exist in the target version, redirects to that version's index
- **Accessibility**: Keyboard navigable (Tab, Enter, Arrow keys) and screen reader compatible
- **Performance**: < 5KB combined JavaScript and CSS, no external dependencies

**Source**: specs/154-mdbook-version-selector-ui.md:36-53

### versions.json Schema

The version selector fetches version metadata from a central `versions.json` file:

```json
{
  "latest": "v0.2.6",
  "versions": [
    {
      "version": "v0.2.6",
      "path": "/v0.2.6/",
      "label": "v0.2.6 (Latest)",
      "released": "2025-01-15"
    },
    {
      "version": "v0.2.5",
      "path": "/v0.2.5/",
      "label": "v0.2.5",
      "released": "2025-01-10"
    }
  ]
}
```

**Field Descriptions:**
- `latest`: Version string of the newest release
- `version`: Semantic version tag (e.g., "v0.2.6")
- `path`: URL path to that version's documentation root
- `label`: Display text in dropdown (includes "(Latest)" for newest)
- `released`: ISO 8601 date of release (optional)

**Source**: specs/154-mdbook-version-selector-ui.md:118-137

### Versioned Deployment Workflow

**Design Overview** (Spec 155):

The deployment system will automatically build and deploy documentation when version tags are pushed:

**Workflow Triggers:**
```yaml
on:
  push:
    tags:
      - 'v*.*.*'  # Trigger on semver tags (e.g., v0.2.6)
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

**Source**: specs/155-versioned-documentation-deployment.md:108-124

**Deployment Process:**

1. **Tag-Triggered Build**: When you push a tag like `v0.2.6`, GitHub Actions automatically:
   - Checks out that specific tag
   - Runs the book workflow for that version
   - Builds documentation with mdBook
   - Deploys to `gh-pages:/v0.2.6/` directory

2. **versions.json Generation**: After deploying a version, a script scans the `gh-pages` branch to generate `versions.json` with all deployed versions

3. **Latest Pointer Update**: If the deployed version is the newest (highest semver), the `/latest/` directory is updated to point to it

4. **Root Redirect**: An `index.html` at the root redirects visitors to `/latest/`

**Key Features:**
- **Preserves Existing Versions**: Using `keep_files: true` ensures deploying v0.2.6 doesn't delete v0.2.5
- **Parallel Builds**: Manual rebuild workflow can rebuild multiple versions concurrently
- **Idempotent**: Rebuilding the same version produces identical output
- **Fail-Safe**: Build failures don't corrupt existing deployed versions

**Source**: specs/155-versioned-documentation-deployment.md:134-176

### Version-Aware Book Workflow

**Design Overview** (Spec 156):

The current `book-docs-drift.yml` workflow will be enhanced to accept a `VERSION` parameter:

**Workflow Configuration:**
```yaml
name: book-docs-drift-detection
mode: mapreduce

env:
  VERSION: "${VERSION:-latest}"  # Accept from caller or default to "latest"

  # Version-aware paths
  ANALYSIS_DIR: ".prodigy/book-analysis/${VERSION}"
  FEATURES_PATH: "${ANALYSIS_DIR}/features.json"
```

**Source**: specs/156-version-aware-book-workflow.md:77-100

**Key Enhancements:**

1. **VERSION Parameter**: The workflow will accept a `VERSION` environment variable (e.g., "v0.2.6", "latest")

2. **Version-Scoped Analysis**: Drift analysis results will be stored in version-specific directories:
   - `.prodigy/book-analysis/v0.2.6/`
   - `.prodigy/book-analysis/v0.2.5/`
   - `.prodigy/book-analysis/latest/`

3. **Version Validation**: Before processing, the workflow will validate the VERSION format (semver `vX.Y.Z` or "latest") and verify the tag exists

4. **Version in Documentation**: Generated documentation will include version metadata in headers or footers

**Claude Command Integration:**
```yaml
setup:
  - claude: "/prodigy-analyze-features-for-book --project $PROJECT_NAME --version $VERSION"

map:
  agent_template:
    - claude: "/prodigy-analyze-book-chapter-drift --project $PROJECT_NAME --json '${item}' --version $VERSION"
```

**Source**: specs/156-version-aware-book-workflow.md:143-153

**Backward Compatibility**: If `VERSION` is not provided, the workflow defaults to "latest" and maintains current behavior.

### Setup Instructions (Planned)

When implemented, setting up versioned documentation will involve:

1. **Add Version Selector Theme Files:**
   ```bash
   # Copy version selector components to your mdBook theme
   cp version-selector.js book/theme/
   cp version-selector.css book/theme/
   ```

2. **Update book.toml Configuration:**
   ```toml
   [output.html]
   additional-css = ["theme/version-selector.css"]
   additional-js = ["theme/version-selector.js"]
   ```

3. **Add Deployment Workflow:**
   ```bash
   # Copy the versioned deployment workflow
   cp templates/workflows/deploy-docs-versioned.yml .github/workflows/
   ```

4. **Create Initial versions.json:**
   ```json
   {
     "latest": "v0.2.6",
     "versions": [
       {
         "version": "v0.2.6",
         "path": "/v0.2.6/",
         "label": "v0.2.6 (Latest)",
         "released": "2025-01-15"
       }
     ]
   }
   ```

5. **Deploy and Test:**
   - Push a version tag to trigger deployment
   - Verify version selector appears in navigation
   - Test switching between versions

**Source**: specs/154-mdbook-version-selector-ui.md:163-175

### Version Retention Strategy (Planned)

The design includes a version retention policy to manage storage on GitHub Pages:

**Retention Rules:**
- **Keep all major versions**: v1.0.0, v2.0.0, v3.0.0
- **Keep last 3 minor versions per major**: v2.3.0, v2.2.0, v2.1.0
- **Keep last 5 patch versions per minor**: v2.3.5, v2.3.4, v2.3.3, v2.3.2, v2.3.1

**Cleanup Process:**
A planned cleanup script will identify and remove old versions:
```bash
# scripts/cleanup-old-versions.sh (planned)
# Removes versions not matching retention policy
# Regenerates versions.json after cleanup
```

**Considerations:**
- GitHub Pages has a 1GB soft limit
- Each documentation version is typically 5-10MB
- Retention policy allows ~50-100 versions before cleanup needed

**Source**: specs/155-versioned-documentation-deployment.md:392-402

### Manual Deployment (Planned)

The deployment workflow will support manual triggering for specific use cases:

**Rebuild Specific Version:**
```bash
# Trigger workflow manually from GitHub UI
# Set version: v0.2.5
# Or use GitHub CLI:
gh workflow run deploy-docs-versioned.yml -f version=v0.2.5
```

**Rebuild All Versions:**
```bash
# Useful after theme updates or global doc changes
gh workflow run deploy-docs-versioned.yml -f rebuild_all=true
```

**Use Cases for Manual Deployment:**
- Updating documentation theme across all versions
- Fixing critical documentation errors in historical versions
- Regenerating `versions.json` after manual gh-pages branch cleanup
- Testing deployment workflow changes

**Source**: specs/155-versioned-documentation-deployment.md:436-444

### Integration with Automated Documentation

The versioning system will integrate with Prodigy's existing automated documentation workflow:

**Workflow Integration:**
```yaml
# .github/workflows/deploy-docs-versioned.yml
steps:
  - name: Run book workflow for version
    run: prodigy run workflows/book-docs-drift.yml
    env:
      VERSION: ${{ steps.version.outputs.version }}
```

When a version tag is pushed, the deployment workflow will:
1. Check out the tagged code
2. Run the version-aware book workflow to analyze features at that version
3. Build documentation matching that version's implementation
4. Deploy to the version-specific subdirectory

This ensures documentation always matches the code at each version.

**Source**: specs/155-versioned-documentation-deployment.md:155-167, specs/156-version-aware-book-workflow.md:143-153

### Testing Versioned Documentation Locally

**Planned Testing Workflow:**

1. **Build Multiple Versions Locally:**
   ```bash
   # Checkout and build v0.2.6
   git checkout v0.2.6
   prodigy run workflows/book-docs-drift.yml
   mdbook build book
   mv book/book build/v0.2.6

   # Checkout and build v0.2.5
   git checkout v0.2.5
   prodigy run workflows/book-docs-drift.yml
   mdbook build book
   mv book/book build/v0.2.5
   ```

2. **Create Test versions.json:**
   ```bash
   cat > build/versions.json <<EOF
   {
     "latest": "v0.2.6",
     "versions": [
       {"version": "v0.2.6", "path": "/v0.2.6/", "label": "v0.2.6 (Latest)"},
       {"version": "v0.2.5", "path": "/v0.2.5/", "label": "v0.2.5"}
     ]
   }
   EOF
   ```

3. **Serve Locally:**
   ```bash
   cd build
   python -m http.server 8000
   # Visit http://localhost:8000/v0.2.6/
   ```

4. **Test Version Selector:**
   - Verify dropdown appears in navigation
   - Switch between versions
   - Confirm page paths are preserved
   - Test fallback when page doesn't exist in older version

### Browser Compatibility (Planned)

The version selector will be designed to work across modern browsers:

**Supported Browsers:**
- Chrome, Firefox, Safari, Edge (latest versions)
- Mobile browsers (iOS Safari, Chrome Mobile)

**Technology Choices:**
- Uses `fetch()` API (ES6, widely supported)
- Semantic HTML (`<select>` element)
- CSS Grid/Flexbox for layout
- No external dependencies (no jQuery)

**Graceful Degradation:**
- If `fetch()` unavailable (very old browsers), selector won't render but docs remain accessible
- If `versions.json` missing, component silently skips rendering (no errors shown)

**Source**: specs/154-mdbook-version-selector-ui.md:233-247
