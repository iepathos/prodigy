# Dual Documentation Quick Start (mdbook + MkDocs)

Prodigy now supports **both mdbook and MkDocs Material** from a single source!

## Architecture

```
book/src/          # 📁 Single source of truth (all markdown files)
├── SUMMARY.md     # mdbook navigation
├── index.md       # Home page (both formats)
└── chapters/      # Content

book.toml          # mdbook config → points to book/src/
mkdocs.yml         # mkdocs config → points to book/src/

book/book/         # mdbook output
site/              # mkdocs output
```

**No duplication, no sync, no symlinks!** Both tools read the same files.

## Quick Commands

### Test Locally

```bash
# Install MkDocs Material
pip install -r requirements.txt

# Preview mdbook
mdbook build book && mdbook serve book
# → http://localhost:3000

# Preview MkDocs
mkdocs serve
# → http://localhost:8000
```

### Sync Navigation

```bash
# Auto-sync SUMMARY.md → mkdocs.yml nav
python scripts/mdbook-mkdocs-sync.py --rename-files

# Or just preview what would change
python scripts/mdbook-mkdocs-sync.py --dry-run
```

### Switch Deployment Format

Edit `.github/workflows/deploy-docs.yml`:

```yaml
env:
  DEPLOY_FORMAT: mkdocs  # or 'mdbook'
```

That's it! Push and GitHub Actions handles the rest.

## Editing Workflow

1. **Edit markdown in `book/src/`** (single source)
2. **Update `book/src/SUMMARY.md`** (mdbook nav)
3. **Sync mkdocs nav**:
   ```bash
   python scripts/mdbook-mkdocs-sync.py --rename-files
   ```
4. **Test both formats**:
   ```bash
   mdbook build book
   mkdocs build
   ```
5. **Commit and push**

## What Was Set Up

### Files Created
- ✅ `mkdocs.yml` - MkDocs configuration (points to `book/src/`)
- ✅ `scripts/mdbook-mkdocs-sync.py` - Navigation sync tool
- ✅ `requirements.txt` - Python dependencies
- ✅ Updated `.github/workflows/deploy-docs.yml` - Deploy either format

### Documentation Created
- `docs/SINGLE_SOURCE_SETUP.md` - Complete setup guide
- `docs/MDBOOK_MKDOCS_SYNC_STANDALONE.md` - How to make tool standalone
- `docs/SYNC_TOOL_DECISION.md` - Keep in Prodigy vs extract
- `docs/SHARED_CONTENT_STRATEGY.md` - Content sharing strategies
- `docs/MKDOCS_MIGRATION_PLAN.md` - Original migration planning

## Key Features

### Navigation Sync Tool (`mdbook-mkdocs-sync.py`)
- ✅ Parses `SUMMARY.md` → generates `mkdocs.yml` nav
- ✅ Handles `intro.md` → `index.md` automatically
- ✅ Supports dry-run mode
- ✅ CLI interface with options
- ✅ Integrated into GitHub Actions
- ✅ **Could be standalone tool** (see docs/MDBOOK_MKDOCS_SYNC_STANDALONE.md)

### Single Source
- ✅ All content in `book/src/`
- ✅ Both configs point to same directory
- ✅ No duplication, no sync scripts
- ✅ Edit once, build both

### Easy Switching
- ✅ Change one env var in GitHub Actions
- ✅ Both formats always available locally
- ✅ Same content, different presentation

## What You Need to Do

### Option A: Use MkDocs Now (Recommended)
```bash
# 1. Sync navigation (handles intro.md → index.md)
python scripts/mdbook-mkdocs-sync.py --rename-files

# 2. Test MkDocs build
pip install -r requirements.txt
mkdocs build
mkdocs serve

# 3. Commit changes
git add .
git commit -m "feat: add MkDocs Material support"
git push

# 4. Verify GitHub Actions uses MkDocs
# Already set to DEPLOY_FORMAT: mkdocs in workflow
```

### Option B: Keep mdbook for Now
```bash
# 1. Change deployment format
# Edit .github/workflows/deploy-docs.yml:
#   DEPLOY_FORMAT: mdbook

# 2. Push changes
git add .
git commit -m "docs: add mkdocs support (keep mdbook deployment)"
git push

# MkDocs available locally, mdbook deployed to GitHub Pages
```

## Testing

### Test Navigation Sync
```bash
# See what would change
python scripts/mdbook-mkdocs-sync.py --dry-run

# Actually sync
python scripts/mdbook-mkdocs-sync.py --rename-files

# Verify mkdocs.yml updated
git diff mkdocs.yml
```

### Test Both Builds
```bash
# mdbook
mdbook build book
# Check: book/book/index.html

# MkDocs
mkdocs build
# Check: site/index.html
```

### Compare Outputs
```bash
# Serve both locally
mdbook serve book &  # Port 3000
mkdocs serve &       # Port 8000

# Visit both:
# http://localhost:3000 (mdbook)
# http://localhost:8000 (MkDocs)
```

## Troubleshooting

### "No module named yaml"
```bash
pip install pyyaml
```

### "mkdocs.yml not found"
```bash
# Already created at project root
ls mkdocs.yml
```

### Navigation out of sync
```bash
# Re-run sync tool
python scripts/mdbook-mkdocs-sync.py --rename-files
```

### Build errors
```bash
# mdbook
mdbook build book

# MkDocs (strict mode shows all errors)
mkdocs build --strict
```

## Next Steps

1. **Choose format** for GitHub Pages:
   - Edit `.github/workflows/deploy-docs.yml`
   - Change `DEPLOY_FORMAT` env var
   - Push to deploy

2. **Test locally**:
   - Run both `mdbook serve` and `mkdocs serve`
   - Compare UX, features, appearance
   - Decide which you prefer for users

3. **Document choice**:
   - Update main README.md
   - Link to live documentation
   - Explain why you chose that format

4. **Consider tool extraction**:
   - See `docs/MDBOOK_MKDOCS_SYNC_STANDALONE.md`
   - Could publish `mdbook-mkdocs-sync` to PyPI
   - Useful for other projects

## Benefits

✅ **Flexibility**: Switch formats anytime with one line change
✅ **No Lock-in**: Keep both options available
✅ **Single Source**: Edit once, builds both
✅ **Showcases Prodigy**: Demonstrates workflow automation
✅ **Developer Choice**: Users can pick preferred format
✅ **Easy Migration**: Test MkDocs without losing mdbook

## Summary

You now have:
- ✅ Single source documentation (`book/src/`)
- ✅ Both mdbook and MkDocs support
- ✅ Automatic navigation sync
- ✅ GitHub Actions deployment
- ✅ Easy format switching
- ✅ No duplication or complex sync

**Just run:**
```bash
python scripts/mdbook-mkdocs-sync.py --rename-files
mkdocs serve
```

And you're ready to go! 🚀

## Resources

- **Setup Guide**: `docs/SINGLE_SOURCE_SETUP.md`
- **Tool Strategy**: `docs/SYNC_TOOL_DECISION.md`
- **Standalone Tool Plan**: `docs/MDBOOK_MKDOCS_SYNC_STANDALONE.md`
- [MkDocs Material Docs](https://squidfunk.github.io/mkdocs-material/)
- [mdbook Docs](https://rust-lang.github.io/mdBook/)
