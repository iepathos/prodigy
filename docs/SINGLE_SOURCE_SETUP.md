# Single Source Documentation Setup

Prodigy uses a **single source directory** (`book/src/`) for both mdbook and MkDocs Material documentation.

## Architecture

```
prodigy/
├── book.toml           # mdbook config (points to book/src)
├── mkdocs.yml          # mkdocs config (points to book/src via docs_dir)
├── book/
│   ├── book.toml       # (duplicate for convenience when in book/ dir)
│   ├── src/            # 📁 SINGLE SOURCE OF TRUTH
│   │   ├── SUMMARY.md  # mdbook navigation
│   │   ├── index.md    # Home page (both formats)
│   │   ├── workflow-basics/
│   │   └── mapreduce/
│   └── book/           # mdbook build output
└── site/               # mkdocs build output
```

## Key Points

✅ **Single Source**: All markdown files live in `book/src/`
✅ **No Duplication**: No sync, copy, or symlinks needed
✅ **Both Configs Point Here**:
   - `book.toml` has `src = "src"`
   - `mkdocs.yml` has `docs_dir: book/src`

✅ **Different Outputs**:
   - mdbook builds to `book/book/`
   - mkdocs builds to `site/`

## Building Locally

### Build mdbook
```bash
cd book
mdbook build
mdbook serve  # Preview at http://localhost:3000
```

Or from project root:
```bash
mdbook build book
```

### Build MkDocs
```bash
# From project root
mkdocs build
mkdocs serve  # Preview at http://localhost:8000
```

## Switching Between Formats

Just change one line in `.github/workflows/deploy-docs.yml`:

```yaml
env:
  DEPLOY_FORMAT: mkdocs  # or 'mdbook'
```

That's it! Both use the same source files.

## Navigation Management

### mdbook Navigation
Manually maintained in `book/src/SUMMARY.md`:

```markdown
# Summary

- [Home](index.md)
- [Workflow Basics](workflow-basics.md)
  - [Command Types](workflow-basics/command-types.md)
```

### MkDocs Navigation
Defined in `mkdocs.yml` under the `nav:` section:

```yaml
nav:
  - Home: index.md
  - Workflow Basics:
      - workflow-basics.md
      - Command Types: workflow-basics/command-types.md
```

### Auto-Sync Navigation (Optional)

To automatically generate mkdocs nav from SUMMARY.md:

```bash
python scripts/sync-mkdocs-nav.py
```

This reads `book/src/SUMMARY.md` and updates the `nav:` section in `mkdocs.yml`.

## Content Guidelines

### ✅ Use Standard Markdown (Works in Both)

- Headers: `#`, `##`, `###`
- Lists: `-`, `*`, `1.`
- Code blocks: ` ```rust `
- Tables
- Links: `[text](url)`
- Images: `![alt](path)`
- Blockquotes: `>`

### ⚠️ Format-Specific Features (Use Sparingly)

**MkDocs Material offers:**
```markdown
!!! note "Information"
    This is an admonition box (Material only)

=== "Tab 1"
    Content
=== "Tab 2"
    More content
```

**mdbook offers:**
```markdown
{{#include file.md}}
```

**Recommendation**: Stick to standard markdown for maximum compatibility.

## File Requirements

### Home Page
Both formats need a home page:
- **mdbook**: First file in SUMMARY.md (can be any name)
- **mkdocs**: **Must** be `index.md`

**Solution**: Use `index.md` for both (mdbook doesn't care about the filename).

### Directory Indexes
For chapters with subsections:

```
workflow-basics/
├── index.md          # Chapter overview (required for mkdocs sections)
├── command-types.md
└── variables.md
```

Both formats support this structure.

## Dependencies

### For mdbook
```bash
# Install mdbook
cargo install mdbook
# or
brew install mdbook
```

### For MkDocs Material
```bash
# Install mkdocs-material
pip install mkdocs-material

# Or use requirements.txt
pip install -r requirements.txt
```

Create `requirements.txt`:
```
mkdocs-material>=9.0.0
```

## GitHub Actions Deployment

The workflow automatically:

1. ✅ Checks out code
2. ✅ Renames `intro.md` → `index.md` (if needed for mkdocs)
3. ✅ Builds chosen format (mdbook OR mkdocs)
4. ✅ Deploys to GitHub Pages

**To switch formats**, just change `DEPLOY_FORMAT` in `.github/workflows/deploy-docs.yml`:

```yaml
env:
  DEPLOY_FORMAT: mkdocs  # Change to 'mdbook' to switch back
```

## Editing Workflow

1. **Edit files in `book/src/`** (single source)
2. **Update `book/src/SUMMARY.md`** (mdbook nav)
3. **Run sync script** (optional):
   ```bash
   python scripts/sync-mkdocs-nav.py
   ```
   Or manually update `mkdocs.yml` nav section
4. **Test both builds**:
   ```bash
   mdbook build book
   mkdocs build
   ```
5. **Commit changes**

## Advantages of This Approach

✅ **No Duplication**: Single source, no sync needed
✅ **No Symlinks**: Works on all platforms (Windows included)
✅ **Easy Switching**: One env var to toggle formats
✅ **Same Content**: Consistency guaranteed
✅ **Independent Navigation**: Each format can organize differently
✅ **Showcases Prodigy**: Both use the same workflow automation

## Troubleshooting

### "mkdocs serve" can't find files
**Solution**: Run from project root (where `mkdocs.yml` is)

### mdbook navigation broken
**Solution**: Check `book/src/SUMMARY.md` syntax

### Paths don't work in one format
**Solution**: Use relative paths from `book/src/` for all links

### Want to use Material-specific features
**Options**:
1. Accept they won't work in mdbook
2. Use HTML comments to hide format-specific content
3. Stick to standard markdown

## Migration Checklist

If you're converting from mdbook-only:

- [x] Create `mkdocs.yml` pointing to `book/src/` (✓ Done)
- [ ] Rename `intro.md` to `index.md` (or let CI do it)
- [ ] Update `SUMMARY.md` references to `index.md`
- [ ] Generate initial mkdocs nav (run sync script or manual)
- [ ] Test mkdocs build: `mkdocs build`
- [ ] Update GitHub Actions deploy workflow (✓ Done)
- [ ] Choose default format in workflow env var
- [ ] Deploy and verify

## Next Steps

1. **Test MkDocs build locally**:
   ```bash
   pip install mkdocs-material
   mkdocs serve
   ```

2. **Compare outputs**: View both `http://localhost:3000` (mdbook) and `http://localhost:8000` (mkdocs)

3. **Choose preferred format** or keep both available

4. **Update deploy workflow** to use your preferred default

5. **Document choice** in main README.md

## Resources

- [mdbook Documentation](https://rust-lang.github.io/mdBook/)
- [MkDocs Documentation](https://www.mkdocs.org/)
- [Material for MkDocs](https://squidfunk.github.io/mkdocs-material/)
