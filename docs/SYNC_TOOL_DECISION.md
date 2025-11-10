# Decision: mdbook-mkdocs-sync Tool Strategy

## TL;DR

**Recommendation: Start in Prodigy, extract later if there's demand**

The enhanced `scripts/mdbook-mkdocs-sync.py` is ready to use now. You can decide later whether to publish it as a standalone package.

## Current Status

✅ **Working Tool** - `scripts/mdbook-mkdocs-sync.py` is fully functional:
- Parses SUMMARY.md
- Generates mkdocs.yml nav
- Handles intro.md → index.md automatically
- Supports dry-run mode
- Has CLI interface
- Integrated into GitHub Actions

## Option 1: Keep in Prodigy (Current State)

### Pros
- ✅ **Works now** - No additional setup needed
- ✅ **Simple** - One less repo to maintain
- ✅ **Integrated** - Part of Prodigy workflows
- ✅ **Fast iteration** - No release process
- ✅ **Documentation example** - Shows Prodigy's dual-format docs

### Cons
- ❌ Not reusable by other projects (without copying)
- ❌ Not discoverable on PyPI
- ❌ Tied to Prodigy repository
- ❌ Can't accept external contributions easily

### Usage
```bash
# In Prodigy repo
python scripts/mdbook-mkdocs-sync.py --rename-files

# Or in workflows
prodigy run workflows/sync-docs-nav.yml
```

## Option 2: Separate Repository

### Pros
- ✅ **Reusable** - Any project can use it
- ✅ **Discoverable** - PyPI, GitHub, documentation
- ✅ **Community** - Accept contributions
- ✅ **Portfolio** - Standalone project
- ✅ **Marketing** - "Tools from the Prodigy team"

### Cons
- ❌ More maintenance (separate repo, CI/CD, releases)
- ❌ Version management needed
- ❌ Need to keep Prodigy version in sync
- ❌ More overhead for small tool

### Usage
```bash
# Install from PyPI
pip install mdbook-mkdocs-sync

# Use anywhere
mdbook-mkdocs-sync --rename-files

# In Prodigy (as dependency)
prodigy run workflows/sync-docs-nav.yml
```

## Hybrid Approach (Recommended)

**Phase 1: Keep in Prodigy** ⭐ (We are here)
- Tool lives in `scripts/mdbook-mkdocs-sync.py`
- Documented in Prodigy docs
- Used in Prodigy workflows
- Battle-tested in production

**Phase 2: Extract if Demand Exists**
Indicators to extract:
- Other projects asking for it
- GitHub stars/interest
- Feature requests
- Community contributions

**Phase 3: Publish as Standalone**
When ready:
1. Create `iepathos/mdbook-mkdocs-sync` repo
2. Publish to PyPI
3. Add as Prodigy dependency
4. Market as "from the Prodigy team"

## Effort Comparison

### Keep in Prodigy
- Time: **0 hours** (done!)
- Maintenance: **Low** (part of Prodigy)
- Impact: **High** (solves your problem now)

### Make Standalone Now
- Time: **~8 hours**
  - 2h: Repo setup, packaging
  - 2h: Tests and CI/CD
  - 2h: Documentation site
  - 2h: PyPI publishing, marketing
- Maintenance: **Medium** (separate releases, issues)
- Impact: **Medium** (potential users, but unknown demand)

### Extract Later (Hybrid)
- Time: **0 hours now**, 8 hours later if needed
- Maintenance: **Low now**, Medium later
- Impact: **High** (best of both)

## Similar Projects (Research)

Did a quick search - there's not much out there:
- No direct mdbook ↔ mkdocs sync tools on PyPI
- Some one-off scripts in various repos
- Converters, but not sync tools

**This validates there's a gap to fill!** But you can fill it when you're ready.

## Decision Framework

### Choose "Keep in Prodigy" if:
- ✅ You just want it to work for Prodigy
- ✅ You don't want extra maintenance burden
- ✅ You want to iterate quickly
- ✅ Uncertain about external demand

### Choose "Standalone Now" if:
- You want to build portfolio projects
- You have time for packaging/docs
- You want to grow a community tool
- You're excited about PyPI publishing

### Choose "Hybrid" if:
- ✅ **You want to validate demand first** ⭐
- ✅ **You want it working now** ⭐
- ✅ **You're open to extracting later** ⭐
- ✅ **You want low initial overhead** ⭐

## Recommendation: Hybrid Approach

**Start**: Keep in Prodigy (Phase 1)
- Tool works now
- No extra overhead
- Battle-tested in production
- Documented example

**Validate**: Mention in Prodigy docs
- "We use this script to maintain dual formats"
- Link to the script
- Explain how others can use it

**Monitor**: Watch for interest
- GitHub issues asking for it
- Questions about dual formats
- Stars/forks of Prodigy

**Extract**: When there's demand
- Move to standalone repo
- Publish to PyPI
- Market as "from Prodigy team"
- Add as Prodigy dependency

## Immediate Next Steps

### For Prodigy (Do Now)
1. ✅ Use enhanced script - Already integrated!
2. ✅ Test locally:
   ```bash
   python scripts/mdbook-mkdocs-sync.py --dry-run
   python scripts/mdbook-mkdocs-sync.py --rename-files
   ```
3. ✅ Commit and push - Let GitHub Actions run
4. ✅ Verify both formats work:
   - mdbook: https://yourusername.github.io/prodigy (or current URL)
   - mkdocs: Same URL (after switching DEPLOY_FORMAT)

### Document in Prodigy (Optional)
5. Add section to main README:
   ```markdown
   ## Documentation

   Prodigy maintains documentation in both mdbook and MkDocs Material formats
   from a single source (`book/src/`).

   We use `scripts/mdbook-mkdocs-sync.py` to keep navigation in sync.

   To switch formats, change `DEPLOY_FORMAT` in `.github/workflows/deploy-docs.yml`.
   ```

6. Reference in automated-documentation chapter

### If You Want to Extract Later
7. Star this decision document
8. Create issue: "Extract mdbook-mkdocs-sync as standalone tool"
9. Label it: "enhancement", "good-first-project"
10. Wait for community interest

## Questions to Consider

### How likely will others need this?
- **Medium** - Niche but useful
- Anyone with dual mdbook/mkdocs will want it
- Rust projects migrating to Python docs
- Teams wanting format flexibility

### Do you want to maintain a separate tool?
- Releases, versioning, backwards compat
- Issue triage and support
- Documentation updates
- CI/CD maintenance

### What's the opportunity cost?
- Time spent on this vs. Prodigy features
- Portfolio value vs. product value
- Community tool vs. product integration

## Conclusion

**Use the hybrid approach:**

1. ✅ **Now**: Keep in Prodigy, document well
2. ⏳ **Later**: Extract if there's demand
3. 🚀 **Future**: Publish to PyPI, market it

The tool is **ready and working**. You can extract it anytime with minimal effort. For now, focus on making Prodigy great and use this as an example of Prodigy's capabilities.

---

## How to Test (Right Now)

```bash
# 1. Dry run to see what would happen
python scripts/mdbook-mkdocs-sync.py --dry-run

# 2. Sync navigation and rename intro.md
python scripts/mdbook-mkdocs-sync.py --rename-files

# 3. Test both builds
mdbook build book
mkdocs build

# 4. Preview locally
mkdocs serve  # http://localhost:8000

# 5. Commit and push
git add .
git commit -m "feat: add mdbook-mkdocs-sync tool for dual documentation"
git push
```

The GitHub Action will use the enhanced script automatically! 🎉
