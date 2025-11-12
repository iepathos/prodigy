## GitHub Actions Integration

Automate your mdBook documentation deployment to GitHub Pages using a standardized workflow that validates documentation on pull requests and deploys on merges to your main branch.

**Source**: This guide is based on Prodigy's production workflow (`.github/workflows/deploy-docs.yml`) and Spec 128 GitHub Workflow Documentation Standards.

### Quick Start

Deploy mdBook documentation to GitHub Pages in under 5 minutes:

**Step 1: Copy the workflow file**

```bash
curl -o .github/workflows/deploy-docs.yml \
  https://raw.githubusercontent.com/iepathos/prodigy/main/.github/workflows/deploy-docs.yml
```

Or create `.github/workflows/deploy-docs.yml` manually:

```yaml
name: Deploy Documentation

on:
  push:
    branches: [main, master]
    paths:
      - 'book/**'
      - '.github/workflows/deploy-docs.yml'
  pull_request:
    branches: [main, master]
    paths:
      - 'book/**'
      - '.github/workflows/deploy-docs.yml'

jobs:
  build-deploy:
    runs-on: ubuntu-latest
    permissions:
      contents: write
    steps:
      - name: Checkout repository
        uses: actions/checkout@v5

      - name: Setup mdBook
        uses: peaceiris/actions-mdbook@v2
        with:
          mdbook-version: 'latest'

      - name: Build book
        run: mdbook build book

      - name: Deploy to GitHub Pages
        if: github.event_name == 'push' && (github.ref == 'refs/heads/main' || github.ref == 'refs/heads/master')
        uses: peaceiris/actions-gh-pages@v4
        with:
          github_token: ${{ secrets.GITHUB_TOKEN }}
          publish_dir: ./book/book
```

**Source**: `.github/workflows/deploy-docs.yml:1-38`

**Step 2: Commit and push**

```bash
git add .github/workflows/deploy-docs.yml
git commit -m "Add documentation deployment workflow"
git push
```

**Step 3: Enable GitHub Pages**

1. Go to your repository's **Settings → Pages**
2. Under "Source", select **Deploy from a branch**
3. Choose branch: **gh-pages** and directory: **/ (root)**
4. Click **Save**

**Done!** Your documentation will deploy automatically on the next push to main/master.

**Source**: Spec 128:136-142

### How It Works

The workflow executes in two contexts:

1. **Pull Requests**: Validates that documentation builds successfully (prevents merging broken docs)
2. **Push to main/master**: Builds and deploys to GitHub Pages (publishes the documentation)

**Key Components**:

- **Triggers**: Runs only when documentation files change (`book/**`) or the workflow itself is modified
- **Path Filters**: Prevents unnecessary workflow runs, saving CI/CD minutes
- **Permissions**: `contents: write` allows pushing to the `gh-pages` branch
- **Conditional Deployment**: The `if: github.event_name == 'push'` condition ensures PRs only validate, never deploy

**Source**: `.github/workflows/deploy-docs.yml:3-13`, `.github/workflows/deploy-docs.yml:18-19`, `.github/workflows/deploy-docs.yml:33`

### Workflow File Structure Explained

```yaml
# Triggers: When to run this workflow
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
```

**Why path filters?**
- Prevents workflow from running on code changes unrelated to documentation
- Saves CI/CD minutes (workflow only runs when docs actually change)
- Faster feedback for non-documentation PRs

**Source**: `.github/workflows/deploy-docs.yml:6-13`, Spec 128:235-255

```yaml
# Permissions needed for deployment
permissions:
  contents: write  # Required to push to gh-pages branch
```

**Why `contents: write`?**

The `peaceiris/actions-gh-pages` action deploys by pushing to the `gh-pages` branch. This requires write access to repository contents.

**Note**: This differs from the newer `actions/deploy-pages` approach which uses `pages: write` and `id-token: write`. Prodigy standardizes on the `gh-pages` branch method for consistency and broader compatibility.

**Source**: `.github/workflows/deploy-docs.yml:18-19`, Spec 128:150-157

```yaml
steps:
  # Step 1: Get repository code
  - uses: actions/checkout@v5

  # Step 2: Install mdBook
  - name: Setup mdBook
    uses: peaceiris/actions-mdbook@v2
    with:
      mdbook-version: 'latest'

  # Step 3: Build documentation
  - name: Build book
    run: mdbook build book

  # Step 4: Deploy to GitHub Pages (only on push to main/master, not PRs)
  - name: Deploy to GitHub Pages
    if: github.event_name == 'push'
    uses: peaceiris/actions-gh-pages@v4
    with:
      github_token: ${{ secrets.GITHUB_TOKEN }}
      publish_dir: ./book/book
```

**Deployment Condition**: `if: github.event_name == 'push'`

This critical condition ensures:
- **Pull Requests**: Build and validate documentation (catches errors before merge)
- **Push to main/master**: Build, validate, AND deploy to GitHub Pages

Without this condition, every PR would attempt to deploy, which is unnecessary and can cause permission issues.

**Source**: `.github/workflows/deploy-docs.yml:22-37`, Spec 128:225-233

### Recommended Action Versions

Use these specific action versions for stability and security:

- **`actions/checkout@v5`** - Fetches repository code
- **`peaceiris/actions-mdbook@v2`** - Installs mdBook
- **`peaceiris/actions-gh-pages@v4`** - Deploys to gh-pages branch

**Source**: `.github/workflows/deploy-docs.yml:22,24,34`, Spec 128:669-674

### Repository Settings

After adding the workflow file, configure GitHub Pages in your repository settings:

1. Navigate to **Settings → Pages**
2. Under **Source**, select **Deploy from a branch**
3. Choose **Branch: gh-pages** and **Directory: / (root)**
4. Click **Save**

The workflow will create the `gh-pages` branch automatically on the first deployment. You don't need to create it manually.

**Source**: Spec 128:136-141

### Integration with Prodigy Workflows

This GitHub Actions workflow **deploys** the documentation that Prodigy's book workflow **generates and maintains**.

**How they work together**:

1. **Prodigy MapReduce Workflow** (`book-docs-drift.yml`):
   - Analyzes code for features and changes
   - Detects documentation drift
   - Updates markdown files in `book/src/`
   - Commits fixes to your repository

2. **GitHub Actions Workflow** (`deploy-docs.yml`):
   - Detects changes to `book/**` files
   - Builds the mdBook
   - Deploys to GitHub Pages

**In Practice**:
- Prodigy keeps your docs accurate and up-to-date
- GitHub Actions makes your docs publicly accessible
- Together, they create a fully automated documentation system

**Source**: `book/src/automated-documentation/index.md:82-135`, Spec 128:518-562

### Common Mistakes and Solutions

#### Mistake 1: Wrong Filename

❌ **Wrong**:
```
.github/workflows/docs.yml
.github/workflows/documentation.yml
.github/workflows/mdbook.yml
```

✅ **Correct**:
```
.github/workflows/deploy-docs.yml
```

**Why it matters**: Consistent naming across projects aids discovery and maintenance.

**Source**: Spec 128:296-312

#### Mistake 2: Wrong Deployment Action

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

**Why it matters**: Different actions require different permissions and repository settings. Using `actions/deploy-pages` requires changing your GitHub Pages source to "GitHub Actions" and using different permissions (`pages: write`, `id-token: write`).

**Source**: Spec 128:314-330

#### Mistake 3: Missing Path Filters

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

**Impact**: Workflow runs on every commit (even code-only changes), wasting CI resources and creating unnecessary deployments.

**Source**: Spec 128:332-351

#### Mistake 4: Wrong Permissions

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

**Why it matters**: The `gh-pages` deployment method needs `contents: write` to push to the gh-pages branch. The permissions shown in "Wrong" are for the `actions/deploy-pages` approach.

**Source**: Spec 128:353-368

#### Mistake 5: Missing PR Validation

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

**Why it matters**: Without PR validation, documentation build errors aren't caught until after merge, potentially breaking your deployed documentation.

**Source**: Spec 128:370-390

#### Mistake 6: Deploying on PR

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

**Why it matters**: PRs should validate documentation builds but not deploy them. Deploying on PR can cause conflicts and unnecessary deployments.

**Source**: Spec 128:392-410

### Troubleshooting

#### Issue: Workflow Runs on Every Commit

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

**Source**: Spec 128:415-428

#### Issue: Permission Denied When Deploying

**Symptom**: Error like "failed to push some refs" or "permission denied"

**Cause**: Missing `contents: write` permission

**Solution**:
```yaml
permissions:
  contents: write
```

Also verify repository settings:
- Go to **Settings → Actions → General**
- Under **Workflow permissions**, select **Read and write permissions**
- Click **Save**

**Source**: Spec 128:430-445

#### Issue: Documentation Not Updating

**Symptom**: Workflow succeeds but GitHub Pages shows old content

**Causes and Solutions**:

1. **Verify gh-pages branch updated**:
   ```bash
   git fetch origin gh-pages
   git log origin/gh-pages
   ```
   Check if the latest commit matches your expectations.

2. **Check GitHub Pages settings**:
   - Go to **Settings → Pages**
   - Verify **Source: Deploy from branch**
   - Verify **Branch: gh-pages** and **Directory: / (root)**

3. **Force clear browser cache**:
   - Add query parameter to URL: `https://username.github.io/repo?v=2`
   - Hard refresh: Ctrl+Shift+R (Windows/Linux) or Cmd+Shift+R (Mac)

4. **Check publish_dir path**:
   ```yaml
   publish_dir: ./book/book  # Correct - points to mdBook output
   # NOT ./book              # Wrong - points to source directory
   ```

**Source**: Spec 128:447-471

#### Issue: 404 Error on GitHub Pages

**Symptom**: Page shows "404 There isn't a GitHub Pages site here"

**Causes and Solutions**:

1. **GitHub Pages not enabled**:
   - Go to **Settings → Pages**
   - Enable Pages if it shows as disabled

2. **Wrong source branch**:
   - Change source to **gh-pages** branch

3. **Wrong root directory**:
   - Ensure source is **/ (root)** not **/docs**

4. **Private repository without GitHub Pro**:
   - GitHub Pages on private repositories requires GitHub Pro, Team, or Enterprise
   - Make repository public or upgrade your GitHub plan

**Source**: Spec 128:473-489

#### Issue: Workflow Syntax Error

**Symptom**: Workflow doesn't appear in Actions tab

**Cause**: Invalid YAML syntax

**Solution**:
```bash
# Validate YAML locally
yamllint .github/workflows/deploy-docs.yml

# Or use GitHub's workflow validator
# (GitHub shows syntax errors when you navigate to the workflow file)
```

**Source**: Spec 128:491-503

#### Issue: Deployment Job Skipped on Push

**Symptom**: Build job runs but deploy step is skipped on push to main

**Cause**: Missing or incorrect condition

**Solution**:
```yaml
- name: Deploy to GitHub Pages
  if: github.event_name == 'push'  # This line is critical
  uses: peaceiris/actions-gh-pages@v4
```

Verify the condition matches exactly. Common mistakes:
- Typo in `github.event_name`
- Using single `=` instead of `==`
- Wrong event name (e.g., `if: github.event == 'push'`)

**Source**: Spec 128:505-516
