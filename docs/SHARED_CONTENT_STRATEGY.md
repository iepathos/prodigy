# Shared Content Strategy for mdbook and MkDocs

## Goal
Maintain a single source of truth for documentation content while supporting both mdbook and MkDocs Material output formats.

## Architecture

### Directory Structure
```
prodigy/
├── book/
│   ├── src/                    # PRIMARY SOURCE (single source of truth)
│   │   ├── SUMMARY.md         # mdbook navigation
│   │   ├── intro.md
│   │   ├── workflow-basics/
│   │   └── mapreduce/
│   └── book.toml              # mdbook config
│
├── docs/                       # Generated/symlinked for mkdocs
│   ├── index.md               # Symlink to book/src/intro.md
│   ├── workflow-basics/       # Symlink to book/src/workflow-basics/
│   └── mapreduce/             # Symlink to book/src/mapreduce/
│
├── mkdocs.yml                 # MkDocs config + navigation
│
├── workflows/
│   ├── book-docs-drift.yml    # Maintains book/src/ content
│   └── sync-mkdocs.yml        # Syncs book/src/ → docs/
│
└── .github/workflows/
    └── deploy-docs.yml        # Builds and deploys chosen format
```

### Content Rules

#### ✅ Use Standard Markdown (Compatible with Both)
- Standard headers (`#`, `##`, `###`)
- Lists (`-`, `*`, `1.`)
- Code blocks with language tags
- Tables
- Links (`[text](url)`)
- Images (`![alt](path)`)
- Basic blockquotes (`>`)

#### ⚠️ Avoid Format-Specific Features (or use sparingly)
- mdbook-specific: `{{#include}}`, `{{#playground}}`
- MkDocs-specific: `!!!` admonitions, `===` tabs
- If needed, use separate sections (see below)

#### 📝 Content Organization
- **Keep in `book/src/`** - This is the primary source
- **Use relative links** - Works in both systems
- **Standard markdown** - Maximum compatibility
- **Sections, not meta-pages** - "Best Practices" as H2, not separate file

### Sync Strategy

#### Option A: Symlinks (Unix/Mac/WSL)
```bash
#!/bin/bash
# scripts/setup-mkdocs-links.sh

mkdir -p docs

# Link intro.md as index.md (required by mkdocs)
ln -sf ../book/src/intro.md docs/index.md

# Link all chapter directories
for dir in book/src/*/; do
  dirname=$(basename "$dir")
  ln -sf "../book/src/$dirname" "docs/$dirname"
done

# Link individual files at root
ln -sf ../book/src/workflow-basics.md docs/workflow-basics.md
ln -sf ../book/src/commands.md docs/commands.md
# etc.
```

**Pros**: Real-time sync, zero duplication
**Cons**: Requires symlink support (not native Windows)

#### Option B: Build-Time Copy (Cross-Platform)
```bash
#!/bin/bash
# scripts/sync-mkdocs-content.sh

# Clean and recreate docs/
rm -rf docs/
mkdir -p docs

# Copy all content from book/src/
rsync -av --exclude='SUMMARY.md' book/src/ docs/

# Rename intro.md to index.md (mkdocs requirement)
if [ -f docs/intro.md ]; then
  mv docs/intro.md docs/index.md
fi
```

**Pros**: Works everywhere, clean separation
**Cons**: Need to sync before building, potential staleness

#### Option C: Prodigy Workflow (Showcase!)
```yaml
# workflows/sync-mkdocs.yml
name: sync-mkdocs-content
mode: standard

env:
  SOURCE_DIR: "book/src"
  TARGET_DIR: "docs"

steps:
  # Clean target
  - shell: "rm -rf ${TARGET_DIR} && mkdir -p ${TARGET_DIR}"

  # Copy content (exclude SUMMARY.md)
  - shell: "rsync -av --exclude='SUMMARY.md' ${SOURCE_DIR}/ ${TARGET_DIR}/"

  # Rename intro.md to index.md
  - shell: |
      if [ -f ${TARGET_DIR}/intro.md ]; then
        mv ${TARGET_DIR}/intro.md ${TARGET_DIR}/index.md
      fi

  # Update internal links if needed
  - claude: "/fix-mkdocs-links --dir ${TARGET_DIR}"

  # Commit changes
  - shell: "git add ${TARGET_DIR} && git commit -m 'docs: sync mkdocs content from book/src' || true"
```

**Pros**: Showcases Prodigy, automated, cross-platform
**Cons**: Extra workflow step

### Navigation Maintenance

#### mdbook Navigation (`book/src/SUMMARY.md`)
Maintained by `/prodigy-detect-documentation-gaps` and manual editing.

#### MkDocs Navigation (`mkdocs.yml`)
**Auto-generate from SUMMARY.md** using a conversion script:

```python
# scripts/generate-mkdocs-nav.py
import yaml
import re

def parse_summary(summary_path):
    """Parse SUMMARY.md and convert to mkdocs nav structure"""
    nav = []
    with open(summary_path) as f:
        for line in f:
            if match := re.match(r'^(\s*)- \[(.+?)\]\((.+?)\)', line):
                indent, title, path = match.groups()
                level = len(indent) // 2

                # Adjust path (intro.md → index.md)
                if path == 'intro.md':
                    path = 'index.md'

                # Build nav entry
                entry = {title: path}
                # TODO: Handle nesting based on level
                nav.append(entry)

    return nav

def update_mkdocs_config(nav):
    """Update mkdocs.yml with new navigation"""
    with open('mkdocs.yml', 'r') as f:
        config = yaml.safe_load(f)

    config['nav'] = nav

    with open('mkdocs.yml', 'w') as f:
        yaml.dump(config, f, default_flow_style=False, sort_keys=False)

if __name__ == '__main__':
    nav = parse_summary('book/src/SUMMARY.md')
    update_mkdocs_config(nav)
    print(f"Updated mkdocs.yml with {len(nav)} navigation entries")
```

Or use Prodigy:
```bash
prodigy run workflows/sync-mkdocs-nav.yml
```

### GitHub Actions Integration

```yaml
# .github/workflows/deploy-docs.yml
name: Deploy Documentation

on:
  push:
    branches: [main, master]
    paths:
      - 'book/**'
      - 'docs/**'
      - 'mkdocs.yml'
      - '.github/workflows/deploy-docs.yml'

env:
  # Toggle which format to deploy
  DEPLOY_FORMAT: mkdocs  # or 'mdbook' or 'both'

jobs:
  build-deploy:
    runs-on: ubuntu-latest
    permissions:
      contents: write
    steps:
      - uses: actions/checkout@v5

      # Sync content from book/src/ to docs/
      - name: Sync MkDocs content
        if: env.DEPLOY_FORMAT == 'mkdocs' || env.DEPLOY_FORMAT == 'both'
        run: |
          ./scripts/sync-mkdocs-content.sh

      # Build mdbook
      - name: Setup mdBook
        if: env.DEPLOY_FORMAT == 'mdbook' || env.DEPLOY_FORMAT == 'both'
        uses: peaceiris/actions-mdbook@v2
        with:
          mdbook-version: 'latest'

      - name: Build mdbook
        if: env.DEPLOY_FORMAT == 'mdbook' || env.DEPLOY_FORMAT == 'both'
        run: mdbook build book

      # Build mkdocs
      - name: Setup Python
        if: env.DEPLOY_FORMAT == 'mkdocs' || env.DEPLOY_FORMAT == 'both'
        uses: actions/setup-python@v5
        with:
          python-version: '3.x'

      - name: Install MkDocs Material
        if: env.DEPLOY_FORMAT == 'mkdocs' || env.DEPLOY_FORMAT == 'both'
        run: pip install mkdocs-material

      - name: Build MkDocs
        if: env.DEPLOY_FORMAT == 'mkdocs' || env.DEPLOY_FORMAT == 'both'
        run: mkdocs build

      # Deploy based on format
      - name: Deploy to GitHub Pages
        if: github.event_name == 'push'
        uses: peaceiris/actions-gh-pages@v4
        with:
          github_token: ${{ secrets.GITHUB_TOKEN }}
          publish_dir: ${{ env.DEPLOY_FORMAT == 'mdbook' && './book/book' || './site' }}
```

### Workflow Integration

#### Primary Workflow: book-docs-drift.yml
Maintains `book/src/` content (existing workflow, no changes needed).

#### Secondary Workflow: sync-mkdocs.yml (New)
```yaml
name: sync-mkdocs
mode: standard

steps:
  # Sync content
  - shell: "./scripts/sync-mkdocs-content.sh"

  # Generate navigation
  - shell: "python scripts/generate-mkdocs-nav.py"

  # Validate mkdocs build
  - shell: "mkdocs build --strict"

  # Commit if changed
  - shell: |
      git add docs/ mkdocs.yml
      git commit -m "docs: sync mkdocs from book/src" || echo "No changes"
```

### Migration Steps

1. **Setup Symlinks or Sync Script**
   ```bash
   # Choose one:
   ./scripts/setup-mkdocs-links.sh      # Symlinks (Unix)
   ./scripts/sync-mkdocs-content.sh     # Copy (cross-platform)
   ```

2. **Generate Initial mkdocs.yml Navigation**
   ```bash
   python scripts/generate-mkdocs-nav.py
   ```

3. **Test Both Builds**
   ```bash
   mdbook build book    # Should work as before
   mkdocs build         # Should build from synced content
   ```

4. **Update GitHub Actions**
   - Add sync step
   - Add mkdocs build
   - Configure `DEPLOY_FORMAT` env var

5. **Document the Process**
   - Add to CONTRIBUTING.md
   - Explain sync strategy
   - Document how to switch formats

### Content Creation Workflow

When adding new documentation:

1. **Edit `book/src/`** - Primary source
2. **Update `book/src/SUMMARY.md`** - mdbook navigation
3. **Run sync** - `./scripts/sync-mkdocs-content.sh` (or let CI do it)
4. **Regenerate nav** - `python scripts/generate-mkdocs-nav.py` (or let CI do it)
5. **Test both builds**:
   ```bash
   mdbook build book
   mkdocs build
   ```
6. **Commit all changes**

### Handling Format-Specific Features

If you need format-specific features (e.g., Material admonitions), use HTML comments:

```markdown
# Standard content works in both

Regular markdown content here.

<!-- mkdocs-only
!!! note "MkDocs Users"
    This appears only in MkDocs builds.
-->

<!-- mdbook-only
> **Note for mdbook Users**
> This appears only in mdbook builds.
-->
```

Then use a preprocessor to strip the markers based on target format.

### Testing Strategy

```bash
# Test mdbook build
mdbook build book && mdbook serve book

# Test mkdocs build
./scripts/sync-mkdocs-content.sh
mkdocs build && mkdocs serve

# Test both navigation systems reference same files
diff <(grep -oP '\(.+?\)' book/src/SUMMARY.md | sort) \
     <(python -c "import yaml; y=yaml.safe_load(open('mkdocs.yml')); print(y['nav'])" | sort)
```

### Benefits of This Approach

1. **Single Source of Truth**: All content in `book/src/`
2. **Dual Output**: Both mdbook and MkDocs from same content
3. **Easy Switching**: Change one env var in GitHub Actions
4. **Showcases Prodigy**: Automated sync workflows
5. **Low Maintenance**: Edit once, builds both
6. **Migration Path**: Can transition between formats gradually

### Drawbacks and Mitigations

| Drawback | Mitigation |
|----------|-----------|
| Can't use format-specific features | Use HTML comments for conditional content |
| Need to sync before mkdocs build | Automate with scripts or CI |
| Symlinks don't work on Windows | Use sync script instead |
| Two navigation files to maintain | Auto-generate mkdocs nav from SUMMARY.md |

## Recommendation

**Phase 1**: Start with **Option B (Build-Time Copy)** + **Auto-generated nav**
- Most compatible (works on all platforms)
- Automated with scripts/CI
- Easy to understand and maintain

**Phase 2**: Consider **Option C (Prodigy Workflow)** to showcase capabilities
- Demonstrates Prodigy automation
- Provides content sync as a feature
- Good documentation example

**Phase 3**: Optimize based on usage
- If mostly using one format: make it primary
- If using both equally: consider format-specific content
- If migrating: use conversion workflow
