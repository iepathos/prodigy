---
number: 128
title: GitHub Workflow Documentation Standards
category: foundation
priority: high
status: draft
dependencies: [126, 127]
created: 2025-10-11
---

# Specification 128: GitHub Workflow Documentation Standards

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: [126 - GitHub Workflow Template System, 127 - GitHub Workflow Validation Tool]

## Context

The lessons learned from WORKFLOW_SETUP_ISSUES.md revealed that while prodigy has good workflow standards documented at https://iepathos.github.io/prodigy/automated-documentation.html, the documentation had several gaps that led to inconsistent implementations:

1. **Discoverability**: Standards were not easily found during workflow setup
2. **Completeness**: Missing quick-start templates and checklists
3. **Common Mistakes**: No documentation of frequently encountered issues
4. **Comparison Guidance**: Multiple valid approaches without clear recommendations
5. **Cross-referencing**: Slash commands and tools didn't link to workflow standards

Even with templates (Spec 126) and validation (Spec 127), developers need comprehensive documentation to understand the rationale behind standards, troubleshoot issues, and make informed decisions when customization is needed.

## Objective

Enhance prodigy's workflow documentation to provide comprehensive, easily discoverable guidance for setting up and maintaining GitHub Actions workflows. The documentation should serve as the single source of truth for workflow standards, include practical examples, common pitfalls, and integrate seamlessly with the template system and validation tools.

## Requirements

### Functional Requirements

- **Enhanced Automated Documentation Guide**: Expand existing page with comprehensive workflow setup guidance
- **Quick Start Section**: Provide copy-paste setup instructions with minimal explanation
- **Deployment Methods Comparison**: Document and compare different GitHub Pages deployment approaches
- **Common Mistakes Reference**: Document frequently encountered issues with solutions
- **Validation Checklist**: Provide pre-deployment validation checklist
- **Troubleshooting Guide**: Include debugging procedures for common workflow failures
- **Integration Documentation**: Document how workflows integrate with slash commands and prodigy features
- **Migration Guides**: Provide step-by-step migration from non-standard workflows
- **Cross-references**: Link from CLI help, slash commands, and error messages to relevant docs

### Non-Functional Requirements

- **Accessibility**: Documentation discoverable from multiple entry points
- **Maintainability**: Documentation structure supports easy updates as standards evolve
- **Clarity**: Examples are clear, complete, and copy-paste ready
- **Searchability**: Key terms and error messages are searchable
- **Versioning**: Document which standards apply to which prodigy versions

## Acceptance Criteria

- [ ] Automated Documentation page expanded with all required sections
- [ ] Quick Start section allows setup in < 5 minutes without reading full docs
- [ ] Deployment methods comparison table shows pros/cons of each approach
- [ ] Common Mistakes section covers all issues from WORKFLOW_SETUP_ISSUES.md
- [ ] Validation checklist matches rules from Spec 127
- [ ] Troubleshooting guide includes at least 5 common failure scenarios
- [ ] All code examples are tested and verified working
- [ ] Cross-references added from CLI help text to documentation
- [ ] Slash commands that create workflows link to documentation
- [ ] Migration guide tested with at least one repository
- [ ] Documentation includes visual diagrams of workflow execution
- [ ] All workflow YAML snippets include explanatory comments

## Technical Details

### Implementation Approach

**Phase 1: Content Structure**
1. Audit existing automated-documentation.html page
2. Identify gaps based on WORKFLOW_SETUP_ISSUES.md analysis
3. Create outline for expanded documentation
4. Define standard terminology and conventions

**Phase 2: Content Creation**
1. Write Quick Start guide with copy-paste templates
2. Create deployment methods comparison
3. Document common mistakes with before/after examples
4. Write troubleshooting procedures
5. Create validation checklist aligned with Spec 127

**Phase 3: Integration**
1. Add cross-references from CLI help text
2. Update slash commands to reference documentation
3. Link validation error messages to relevant doc sections
4. Create in-repo reference file (.github/WORKFLOW_STANDARDS.md)

**Phase 4: Validation**
1. Test all examples in fresh repository
2. Verify Quick Start can be completed in < 5 minutes
3. Get feedback from developers unfamiliar with standards
4. Iterate based on feedback

### Documentation Structure

```markdown
# Automated Documentation with GitHub Actions

## Table of Contents
1. Quick Start (For the Impatient)
2. Overview
3. Deployment Methods Comparison
4. Standard Setup (Recommended)
5. Customization Guide
6. Validation Checklist
7. Common Mistakes and Solutions
8. Troubleshooting
9. Integration with Prodigy
10. Migration from Other Approaches
11. Advanced Topics
12. Reference

## 1. Quick Start (For the Impatient)

**Goal**: Deploy mdBook documentation to GitHub Pages in 5 minutes

**Step 1**: Copy workflow file
```bash
curl -o .github/workflows/deploy-docs.yml \
  https://raw.githubusercontent.com/iepathos/prodigy/main/.github/workflow-templates/deploy-docs-template.yml
```

**Step 2**: Commit and push
```bash
git add .github/workflows/deploy-docs.yml
git commit -m "Add documentation deployment workflow"
git push
```

**Step 3**: Enable GitHub Pages
- Go to repository Settings → Pages
- Source: Deploy from a branch
- Branch: gh-pages / (root)
- Save

**Done!** Documentation will deploy on next push to main/master.

## 2. Overview

[Purpose and benefits of automated documentation deployment]

## 3. Deployment Methods Comparison

| Feature | gh-pages Branch | GitHub Pages Actions |
|---------|----------------|---------------------|
| **Setup Complexity** | ✅ Simple | ⚠️ Moderate |
| **GitHub Settings** | ✅ No changes needed | ⚠️ Requires "Actions" source |
| **Permissions** | `contents: write` | `pages: write`, `id-token: write` |
| **Job Structure** | ✅ Single job | ⚠️ Separate build/deploy |
| **Compatibility** | ✅ Works with existing setups | ⚠️ May conflict |
| **Recommended For** | ✅ All prodigy projects | ⚠️ New projects only |

**Prodigy Standard**: Use gh-pages branch method for consistency.

### Why gh-pages Branch?

✅ **Advantages**:
- No repository settings changes required
- Compatible with existing gh-pages setups
- Simpler workflow structure (single job)
- Easier to understand and debug
- Works with GitHub Enterprise Server

⚠️ **Disadvantages**:
- Creates additional git branch (gh-pages)
- Slightly older approach

### When to Use GitHub Pages Actions?

Only consider this for brand new projects if:
- Starting fresh with no existing Pages setup
- Want native GitHub Pages integration
- Deploying to GitHub.com (not Enterprise)

**Note**: Mixing approaches in the ecosystem creates maintenance burden.

## 4. Standard Setup (Recommended)

[Detailed walkthrough of standard setup]

### Workflow File Structure

```yaml
name: Deploy Documentation

# Triggers: when to run this workflow
on:
  push:
    branches: [main, master]    # Deploy on push to default branch
    paths:                      # Only when these files change
      - 'book/**'              # Documentation content
      - '.github/workflows/deploy-docs.yml'  # This workflow file
  pull_request:                 # Validate documentation in PRs
    branches: [main, master]
    paths:
      - 'book/**'

# Permissions needed for deployment
permissions:
  contents: write  # Required to push to gh-pages branch

jobs:
  build-deploy:
    runs-on: ubuntu-latest
    steps:
      # Step 1: Get repository code
      - uses: actions/checkout@v4

      # Step 2: Install mdBook
      - name: Setup mdBook
        uses: peaceiris/actions-mdbook@v2
        with:
          mdbook-version: 'latest'

      # Step 3: Build documentation
      - name: Build book
        run: mdbook build book

      # Step 4: Deploy to GitHub Pages
      # Only on push to main/master (not on PRs)
      - name: Deploy to GitHub Pages
        if: github.event_name == 'push'
        uses: peaceiris/actions-gh-pages@v4
        with:
          github_token: ${{ secrets.GITHUB_TOKEN }}
          publish_dir: ./book/book
```

### Path Filters Explained

**Why use path filters?**
- Prevents workflow from running on unrelated changes
- Saves CI/CD minutes
- Faster feedback for non-documentation PRs

**What paths to include?**
- Documentation source: `book/**`
- Workflow file itself: `.github/workflows/deploy-docs.yml`
- Custom theme files: `book/theme/**` (if applicable)
- Book configuration: `book.toml` (if at root)

**Example with multiple paths**:
```yaml
paths:
  - 'book/**'
  - 'docs/**'
  - 'book.toml'
  - '.github/workflows/deploy-docs.yml'
```

## 5. Validation Checklist

Before deploying your workflow, verify:

**Workflow File**:
- [ ] File named `deploy-docs.yml` (not `docs.yml`)
- [ ] Located in `.github/workflows/` directory
- [ ] YAML syntax is valid (`yamllint .github/workflows/deploy-docs.yml`)

**Triggers**:
- [ ] Includes `push` trigger for main/master branches
- [ ] Includes `pull_request` trigger for PR validation
- [ ] Path filters include `book/**`
- [ ] Path filters include workflow file path

**Permissions**:
- [ ] `contents: write` permission set (not `pages: write`)
- [ ] No unnecessary permissions granted

**Deployment**:
- [ ] Uses `peaceiris/actions-gh-pages@v4` (not `actions/deploy-pages`)
- [ ] `publish_dir` points to correct build output
- [ ] Deployment only runs on push (not PRs): `if: github.event_name == 'push'`

**Repository Settings**:
- [ ] GitHub Pages enabled
- [ ] Pages source set to "gh-pages branch"
- [ ] Branch protection allows workflow to push to gh-pages

**Testing**:
- [ ] Workflow validates on PR
- [ ] Deployment succeeds on merge to main
- [ ] Documentation accessible at https://username.github.io/repo

Run automated validation:
```bash
prodigy validate-workflows
```

## 6. Common Mistakes and Solutions

### Mistake 1: Wrong Filename

❌ **Wrong**:
```bash
.github/workflows/docs.yml
.github/workflows/documentation.yml
.github/workflows/mdbook.yml
```

✅ **Correct**:
```bash
.github/workflows/deploy-docs.yml
```

**Why it matters**: Consistent naming across projects aids discovery and maintenance.

### Mistake 2: Wrong Deployment Action

❌ **Wrong**:
```yaml
- uses: actions/upload-pages-artifact@v3
- uses: actions/deploy-pages@v4
```

✅ **Correct**:
```yaml
- uses: peaceiris/actions-gh-pages@v4
  with:
    github_token: ${{ secrets.GITHUB_TOKEN }}
    publish_dir: ./book/book
```

**Why it matters**: Different actions require different permissions and repository settings.

### Mistake 3: Missing Path Filters

❌ **Wrong**:
```yaml
on:
  push:
    branches: [main]
```

✅ **Correct**:
```yaml
on:
  push:
    branches: [main, master]
    paths:
      - 'book/**'
      - '.github/workflows/deploy-docs.yml'
```

**Impact**: Workflow runs on every commit, wasting CI resources.

### Mistake 4: Wrong Permissions

❌ **Wrong**:
```yaml
permissions:
  pages: write
  id-token: write
```

✅ **Correct**:
```yaml
permissions:
  contents: write
```

**Why it matters**: gh-pages deployment needs `contents: write` to push to branch.

### Mistake 5: Missing PR Validation

❌ **Wrong**:
```yaml
on:
  push:
    branches: [main]
```

✅ **Correct**:
```yaml
on:
  push:
    branches: [main, master]
    paths: ['book/**']
  pull_request:
    branches: [main, master]
    paths: ['book/**']
```

**Why it matters**: Catches documentation build errors before merge.

### Mistake 6: Deploying on PR

❌ **Wrong**:
```yaml
- uses: peaceiris/actions-gh-pages@v4
  with:
    github_token: ${{ secrets.GITHUB_TOKEN }}
    publish_dir: ./book/book
```

✅ **Correct**:
```yaml
- uses: peaceiris/actions-gh-pages@v4
  if: github.event_name == 'push'
  with:
    github_token: ${{ secrets.GITHUB_TOKEN }}
    publish_dir: ./book/book
```

**Why it matters**: PRs should validate but not deploy.

## 7. Troubleshooting

### Issue: Workflow Runs on Every Commit

**Symptom**: Workflow executes even when documentation hasn't changed

**Cause**: Missing or incorrect path filters

**Solution**:
```yaml
on:
  push:
    paths:
      - 'book/**'
      - '.github/workflows/deploy-docs.yml'
```

### Issue: Permission Denied When Deploying

**Symptom**: Error like "failed to push some refs" or "permission denied"

**Cause**: Missing `contents: write` permission

**Solution**:
```yaml
permissions:
  contents: write
```

Also verify repository settings:
- Settings → Actions → General
- Workflow permissions: "Read and write permissions"

### Issue: Documentation Not Updating

**Symptom**: Workflow succeeds but GitHub Pages shows old content

**Cause**: Multiple possible causes

**Solutions**:
1. **Verify gh-pages branch updated**:
   ```bash
   git fetch origin gh-pages
   git log origin/gh-pages
   ```

2. **Check GitHub Pages settings**:
   - Settings → Pages
   - Source: "Deploy from branch"
   - Branch: "gh-pages" / "(root)"

3. **Force clear cache**:
   - Add query parameter: `https://user.github.io/repo?v=2`

4. **Check publish_dir path**:
   ```yaml
   publish_dir: ./book/book  # Not ./book
   ```

### Issue: 404 Error on GitHub Pages

**Symptom**: Page shows "404 There isn't a GitHub Pages site here"

**Causes and Solutions**:

1. **GitHub Pages not enabled**:
   - Settings → Pages → Enable Pages

2. **Wrong source branch**:
   - Change source to "gh-pages" branch

3. **Wrong root directory**:
   - Ensure source is "(root)" not "/docs"

4. **Private repository**:
   - GitHub Pages requires public repo (or GitHub Pro)

### Issue: Workflow Syntax Error

**Symptom**: Workflow doesn't appear in Actions tab

**Cause**: Invalid YAML syntax

**Solution**:
```bash
# Validate YAML locally
yamllint .github/workflows/deploy-docs.yml

# Or use GitHub's workflow validator
# (GitHub shows syntax errors when you navigate to the workflow file)
```

### Issue: Deployment Job Skipped on Push

**Symptom**: Build job runs but deploy step is skipped

**Cause**: Missing or incorrect condition

**Solution**:
```yaml
- name: Deploy to GitHub Pages
  if: github.event_name == 'push'  # This line is critical
  uses: peaceiris/actions-gh-pages@v4
```

## 8. Integration with Prodigy

### Slash Commands

When setting up workflows via slash commands, reference this documentation:

```markdown
When creating documentation workflows:
1. Follow https://iepathos.github.io/prodigy/automated-documentation.html
2. Use template from .github/workflow-templates/deploy-docs-template.yml
3. Validate with: prodigy validate-workflows
```

### Validation Tool Integration

Prodigy's workflow validator enforces these standards:

```bash
# Validate all workflows
prodigy validate-workflows

# See violations with line numbers
prodigy validate-workflows --verbose
```

Validation checks:
- ✅ Filename matches convention
- ✅ Uses correct GitHub Actions
- ✅ Includes path filters
- ✅ Has PR validation
- ✅ Correct permissions set

### Automated Fixes

Some violations can be auto-fixed:

```bash
prodigy validate-workflows --fix
```

Auto-fixable issues:
- Missing path filters
- Incorrect action versions
- Missing PR triggers

## 9. Migration from Other Approaches

### From GitHub Pages Actions

**Before** (actions/deploy-pages):
```yaml
permissions:
  contents: read
  pages: write
  id-token: write

jobs:
  build:
    steps:
      - uses: actions/upload-pages-artifact@v3
  deploy:
    needs: build
    steps:
      - uses: actions/deploy-pages@v4
```

**After** (gh-pages branch):
```yaml
permissions:
  contents: write

jobs:
  build-deploy:
    steps:
      - name: Deploy to GitHub Pages
        uses: peaceiris/actions-gh-pages@v4
        with:
          github_token: ${{ secrets.GITHUB_TOKEN }}
          publish_dir: ./book/book
```

**Migration steps**:
1. Replace entire workflow with standard template
2. Update repository settings: Pages source to "gh-pages branch"
3. Test with a PR before merging
4. Delete old artifacts (Pages Deployments in Actions tab)

### From Manual Deployment

**Before** (manual process):
```bash
mdbook build book
cd book/book
git init
git checkout -b gh-pages
git add .
git commit -m "Deploy docs"
git push -f origin gh-pages
```

**After** (automated):
1. Add workflow file: `.github/workflows/deploy-docs.yml`
2. Commit and push
3. Remove manual deployment scripts
4. Update documentation to reference GitHub Pages URL

## 10. Advanced Topics

### Custom Build Commands

For projects needing custom build steps:

```yaml
- name: Build book with preprocessors
  run: |
    mdbook build book
    ./scripts/post-process-docs.sh book/book
```

### Multi-Version Documentation

Deploy different versions to subdirectories:

```yaml
- uses: peaceiris/actions-gh-pages@v4
  with:
    github_token: ${{ secrets.GITHUB_TOKEN }}
    publish_dir: ./book/book
    destination_dir: v1.0
```

### Custom Domain

```yaml
- uses: peaceiris/actions-gh-pages@v4
  with:
    github_token: ${{ secrets.GITHUB_TOKEN }}
    publish_dir: ./book/book
    cname: docs.example.com
```

## 11. Reference

### Workflow Template

Full standard template:
```yaml
[Include complete working template]
```

### Action Versions

Recommended versions (as of 2025-10-11):
- `actions/checkout@v4`
- `peaceiris/actions-mdbook@v2`
- `peaceiris/actions-gh-pages@v4`

### External Resources

- [mdBook Documentation](https://rust-lang.github.io/mdBook/)
- [peaceiris/actions-gh-pages](https://github.com/peaceiris/actions-gh-pages)
- [GitHub Actions Documentation](https://docs.github.com/en/actions)
- [Prodigy Workflow Templates](https://github.com/iepathos/prodigy/tree/main/.github/workflow-templates)
```

### In-Repository Reference File

Create `.github/WORKFLOW_STANDARDS.md` in new repositories:

```markdown
# GitHub Workflow Standards

This repository follows prodigy's workflow standards.

## Documentation Deployment

**Workflow File**: `.github/workflows/deploy-docs.yml`
**Standard**: [Automated Documentation Guide](https://iepathos.github.io/prodigy/automated-documentation.html)

### Quick Reference

✅ **Naming**: Use `deploy-docs.yml`
✅ **Action**: Use `peaceiris/actions-gh-pages@v4`
✅ **Permissions**: `contents: write`
✅ **Filters**: Include `book/**` and workflow file
✅ **PR Validation**: Include pull_request trigger

### Validation

Run validation locally:
```bash
prodigy validate-workflows
```

### Template

See: https://github.com/iepathos/prodigy/blob/main/.github/workflow-templates/deploy-docs-template.yml
```

## Dependencies

### Prerequisites
- **Spec 126**: Templates must exist before documenting them
- **Spec 127**: Validation tool must exist to reference in documentation

### Affected Components
- Prodigy documentation website (book/)
- CLI help text
- Slash command prompts
- Validation tool error messages
- Repository templates

### External Dependencies
None - documentation updates only

## Testing Strategy

### Documentation Quality Tests

1. **Readability Test**:
   - Have developer unfamiliar with standards follow Quick Start
   - Measure time to successful deployment
   - Target: < 5 minutes

2. **Completeness Test**:
   - For each issue in WORKFLOW_SETUP_ISSUES.md, verify documentation addresses it
   - Check that all common mistakes section entries have solutions

3. **Example Verification**:
   - Create fresh repository
   - Copy each code example exactly as documented
   - Verify successful execution
   - All examples must work without modification

4. **Link Validation**:
   - Use link checker on documentation
   - Verify all cross-references resolve correctly
   - Check external links are valid

### User Acceptance

1. **Developer Feedback**:
   - Share with 3+ developers unfamiliar with standards
   - Collect feedback on clarity and usefulness
   - Iterate based on feedback

2. **Migration Test**:
   - Take existing repository with non-standard workflow
   - Follow migration guide
   - Verify successful migration without external help

## Documentation Requirements

### Code Documentation
- Inline comments in workflow templates explaining each section
- README.md in workflow-templates directory

### User Documentation

**Primary Documentation**:
- Enhanced automated-documentation.html page (as detailed in Technical Details)

**Supporting Documentation**:
- `.github/WORKFLOW_STANDARDS.md` template for new repositories
- CLI help text updates to reference documentation
- Slash command prompt updates with documentation links
- Validation error messages linking to relevant doc sections

### Architecture Updates
Update `book/src/SUMMARY.md`:
```markdown
- [Automated Documentation](./automated-documentation.md)
  - [Quick Start](./automated-documentation.md#quick-start)
  - [Common Mistakes](./automated-documentation.md#common-mistakes)
  - [Troubleshooting](./automated-documentation.md#troubleshooting)
```

## Implementation Notes

### Documentation Writing Best Practices

**Progressive Disclosure**:
1. Quick Start: Minimal explanation, maximum copy-paste
2. Overview: High-level concepts
3. Details: Deep dives for those who need them

**Code Examples**:
- Always include complete, runnable examples
- Add comments explaining non-obvious parts
- Show both "wrong" and "right" approaches for common mistakes
- Test every example before publishing

**Error Messages**:
- Include actual error text users might see
- Provide specific solutions, not general advice
- Link to documentation from validation tool errors

### Maintenance Strategy

**Regular Updates**:
- Review quarterly for accuracy
- Update action versions when new releases occur
- Add new common mistakes as they're discovered
- Incorporate feedback from validation tool usage

**Change Management**:
- Document what changed and why
- Provide migration path for breaking changes
- Archive old standards for reference

### Search Engine Optimization

Include key error messages in documentation:
- "failed to push some refs"
- "permission denied"
- "404 There isn't a GitHub Pages site here"
- "pages: write vs contents: write"

This helps developers find solutions via search engines.

## Migration and Compatibility

### Breaking Changes
None - documentation enhancements are additive

### Content Migration

Existing automated-documentation.html will be expanded, not replaced:
- Keep existing content that's still valid
- Reorganize into new structure
- Add missing sections
- Update outdated information

### Version Compatibility

Documentation should note which prodigy versions support which features:
```markdown
> **Note**: Workflow validation requires prodigy >= 0.13.0
> For earlier versions, manual validation is required.
```

## Success Metrics

- **Time to Deploy**: < 5 minutes using Quick Start
- **Search Rankings**: Top 3 results for "prodigy github actions workflow"
- **Support Reduction**: 80% reduction in workflow-related questions
- **Adoption**: All new prodigy ecosystem repositories use standard
- **Validation Pass Rate**: 95%+ of workflows pass validation on first try
- **Developer Satisfaction**: Positive feedback on documentation clarity

## Future Enhancements

- **Interactive Tutorial**: Step-by-step wizard for workflow setup
- **Video Walkthrough**: Screen recording of Quick Start process
- **Troubleshooting Decision Tree**: Interactive flowchart for debugging
- **Template Gallery**: Showcase of workflows from real repositories
- **Documentation Testing**: Automated tests that verify code examples work
- **Localization**: Translations for non-English speakers
- **IDE Integration**: Snippets and IntelliSense for workflow files
