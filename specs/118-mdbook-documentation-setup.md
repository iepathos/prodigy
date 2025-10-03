---
number: 118
title: mdBook Documentation Setup
category: foundation
priority: medium
status: draft
dependencies: []
created: 2025-10-02
---

# Specification 118: mdBook Documentation Setup

**Category**: foundation
**Priority**: medium
**Status**: draft
**Dependencies**: None

## Context

Prodigy currently has comprehensive workflow syntax documentation in `docs/workflow-syntax.md` that serves as the primary reference for users creating YAML workflow files. This documentation is well-structured and complete, but it exists as a single markdown file within the repository.

The Rust ecosystem has standardized on [mdBook](https://rust-lang.github.io/mdBook/) for project documentation, following the pattern established by official Rust projects like The Rust Book, The Cargo Book, and many other prominent Rust tools. mdBook provides:

- **Static site generation** from markdown files
- **Built-in search** across all documentation
- **Sidebar navigation** for easy browsing
- **Syntax highlighting** for code examples
- **Mobile-friendly** responsive design
- **GitHub Pages integration** for hosting

This specification covers the initial setup of mdBook infrastructure within the Prodigy repository, organizing existing documentation into a book structure, and establishing a CI/CD pipeline for automatic deployment to GitHub Pages.

## Objective

Set up mdBook documentation infrastructure for Prodigy, migrating the existing workflow syntax documentation into a proper book format and establishing automated deployment to GitHub Pages.

## Requirements

### Functional Requirements

1. **mdBook Infrastructure**
   - Initialize mdBook structure in `book/` directory
   - Create `book.toml` configuration file
   - Establish proper directory structure following mdBook conventions
   - Configure book metadata (title, authors, description)

2. **Content Organization**
   - Migrate `docs/workflow-syntax.md` into logical book chapters
   - Create introduction/welcome page
   - Establish navigation structure via `SUMMARY.md`
   - Break down large sections into separate chapters for better navigation

3. **Book Structure**
   - Introduction and Getting Started
   - Workflow syntax reference (from existing docs)
   - Examples and use cases
   - API reference (placeholder for future)
   - Troubleshooting guide

4. **GitHub Actions CI/CD**
   - Create workflow to build mdBook on commits to main
   - Automatic deployment to GitHub Pages
   - Build verification on pull requests (no deployment)
   - Cache mdBook installation for faster builds

5. **Development Workflow**
   - Document how to build book locally (`mdbook serve`)
   - Update contributing guidelines with documentation workflow
   - Establish conventions for documentation updates

### Non-Functional Requirements

1. **Maintainability**
   - Keep book source files in sync with code changes
   - Clear separation between code and documentation
   - Easy to add new chapters and sections
   - Follow mdBook best practices

2. **User Experience**
   - Fast page loads and search
   - Clear navigation hierarchy
   - Mobile-responsive design
   - Accessible to screen readers

3. **Performance**
   - Fast CI/CD builds (< 2 minutes)
   - Efficient caching of dependencies
   - Quick local preview builds

## Acceptance Criteria

- [ ] `book/` directory created with proper mdBook structure
- [ ] `book/book.toml` configuration file with project metadata
- [ ] `book/src/SUMMARY.md` with complete navigation structure
- [ ] Existing `docs/workflow-syntax.md` migrated into logical book chapters
- [ ] Introduction page created explaining Prodigy and documentation purpose
- [ ] GitHub Actions workflow created for building and deploying book
- [ ] GitHub Actions workflow configured to deploy to GitHub Pages on main branch commits (actual deployment verification not required until merged)
- [ ] PR builds verify book compiles without deployment
- [ ] Local development documented in README or CONTRIBUTING.md
- [ ] All code examples in book use proper syntax highlighting
- [ ] Book builds without errors or warnings
- [ ] Search functionality works for all content

## Technical Details

### Implementation Approach

**Phase 1: mdBook Installation and Configuration**

1. Install mdBook locally for development:
   ```bash
   cargo install mdbook
   ```

2. Initialize book structure:
   ```bash
   mdbook init book --title "Prodigy Documentation"
   ```

3. Configure `book/book.toml`:
   ```toml
   [book]
   title = "Prodigy Documentation"
   authors = ["Prodigy Contributors"]
   description = "Complete guide to Prodigy workflow orchestration"
   src = "src"
   language = "en"

   [build]
   build-dir = "book"

   [output.html]
   default-theme = "rust"
   git-repository-url = "https://github.com/yourusername/prodigy"
   edit-url-template = "https://github.com/yourusername/prodigy/edit/main/book/{path}"

   [output.html.search]
   enable = true
   ```

**Phase 2: Content Migration Strategy**

The existing `docs/workflow-syntax.md` is ~1075 lines and should be broken into logical chapters:

1. **Introduction** (`intro.md`) - New content
   - What is Prodigy?
   - Why use Prodigy?
   - Quick start example
   - Link to installation

2. **Workflow Basics** (`workflow-basics.md`)
   - Workflow Types section
   - Standard Workflows section
   - Basic concepts

3. **MapReduce Workflows** (`mapreduce.md`)
   - Complete MapReduce structure
   - Setup phase
   - Map phase
   - Reduce phase
   - MapReduce-specific features

4. **Command Types** (`commands.md`)
   - Shell commands
   - Claude commands
   - Goal-seeking commands
   - Foreach commands
   - Validation commands

5. **Variables and Interpolation** (`variables.md`)
   - Variable interpolation section
   - All variable types
   - Custom capture
   - Variable scoping

6. **Environment Configuration** (`environment.md`)
   - Global environment
   - Secrets management
   - Environment profiles
   - Step-level configuration

7. **Advanced Features** (`advanced.md`)
   - Conditional execution
   - Output capture formats
   - Nested conditionals
   - Timeout configuration

8. **Error Handling** (`error-handling.md`)
   - Workflow-level policies
   - Command-level handling
   - Dead Letter Queue
   - Retry strategies

9. **Examples** (`examples.md`)
   - All examples from original doc
   - Real-world use cases
   - Best practices

10. **Troubleshooting** (`troubleshooting.md`)
    - Common issues
    - Debug tips
    - FAQ

**Phase 3: GitHub Actions Setup**

Create `.github/workflows/deploy-docs.yml`:

```yaml
name: Deploy Documentation

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  build-deploy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Setup mdBook
        uses: peaceiris/actions-mdbook@v1
        with:
          mdbook-version: 'latest'

      - name: Build book
        run: mdbook build book

      - name: Deploy to GitHub Pages
        if: github.event_name == 'push' && github.ref == 'refs/heads/main'
        uses: peaceiris/actions-gh-pages@v3
        with:
          github_token: ${{ secrets.GITHUB_TOKEN }}
          publish_dir: ./book/book
          cname: prodigy.yourdomain.com  # Optional custom domain
```

**Phase 4: Documentation Updates**

Update `README.md`:
```markdown
## Documentation

Comprehensive documentation is available at [https://yourusername.github.io/prodigy](https://yourusername.github.io/prodigy)

### Building Documentation Locally

```bash
# Install mdBook
cargo install mdbook

# Serve documentation with live reload
mdbook serve book --open

# Build static site
mdbook build book
```
```

### Architecture Changes

**New Directory Structure**:
```
prodigy/
├── book/
│   ├── book.toml              # mdBook configuration
│   └── src/
│       ├── SUMMARY.md         # Table of contents
│       ├── intro.md           # Introduction
│       ├── workflow-basics.md # Workflow fundamentals
│       ├── mapreduce.md       # MapReduce workflows
│       ├── commands.md        # Command types reference
│       ├── variables.md       # Variable interpolation
│       ├── environment.md     # Environment configuration
│       ├── advanced.md        # Advanced features
│       ├── error-handling.md  # Error handling guide
│       ├── examples.md        # Examples and use cases
│       └── troubleshooting.md # Troubleshooting guide
├── docs/
│   └── workflow-syntax.md     # Keep as single-file reference
├── .github/
│   └── workflows/
│       └── deploy-docs.yml    # CI/CD for documentation
└── README.md                  # Links to online docs
```

**Preservation Strategy**:
- Keep `docs/workflow-syntax.md` as comprehensive single-file reference
- Book chapters source from this file (consider symlinking or build script)
- OR: Make book the source of truth and generate single-file from book

### Data Structures

**book.toml Configuration**:
```toml
[book]
title = "Prodigy Documentation"
authors = ["Prodigy Contributors"]
description = "AI-powered workflow orchestration for development teams"
src = "src"
language = "en"

[build]
build-dir = "book"
create-missing = false  # Don't create missing chapters automatically

[preprocessor.links]
# Enable {{#include}} for shared content

[output.html]
default-theme = "rust"
preferred-dark-theme = "navy"
curly-quotes = true
mathjax-support = false
copy-fonts = true
no-section-label = false
git-repository-url = "https://github.com/yourusername/prodigy"
git-repository-icon = "fa-github"
edit-url-template = "https://github.com/yourusername/prodigy/edit/main/book/{path}"

[output.html.search]
enable = true
limit-results = 30
teaser-word-count = 30
use-boolean-and = true
boost-title = 2
boost-hierarchy = 1
boost-paragraph = 1
expand = true
heading-split-level = 3

[output.html.fold]
enable = true
level = 1

[output.html.playground]
editable = false
copyable = true
copy-js = true
line-numbers = true
```

**SUMMARY.md Structure**:
```markdown
# Summary

[Introduction](intro.md)

# User Guide

- [Workflow Basics](workflow-basics.md)
- [MapReduce Workflows](mapreduce.md)
- [Command Types](commands.md)
- [Variables and Interpolation](variables.md)
- [Environment Configuration](environment.md)

# Advanced Topics

- [Advanced Features](advanced.md)
- [Error Handling](error-handling.md)

# Reference

- [Examples](examples.md)
- [Troubleshooting](troubleshooting.md)

# Contributing

- [Development Guide](contributing.md)
```

### APIs and Interfaces

**No Code APIs Changed** - This is purely documentation infrastructure.

**Documentation Build Interface**:
```bash
# Local development
mdbook serve book          # Serve with live reload on http://localhost:3000
mdbook serve book --open   # Serve and open browser
mdbook build book          # Build static site to book/book/
mdbook test book           # Test code examples in book
mdbook clean book          # Clean build artifacts

# CI/CD usage
mdbook build book --dest-dir ./output
```

## Dependencies

**Prerequisites**: None (pure documentation infrastructure)

**Affected Components**:
- CI/CD pipeline (new GitHub Actions workflow)
- Repository structure (new `book/` directory)
- README.md (documentation links)
- CONTRIBUTING.md (documentation workflow)

**External Dependencies**:
- mdBook (Rust crate, installed via cargo or CI action)
- GitHub Pages (hosting platform)
- peaceiris/actions-mdbook@v1 (GitHub Action)
- peaceiris/actions-gh-pages@v3 (GitHub Action)

## Testing Strategy

### Build Validation

1. **Local Build Tests**
   - `mdbook build book` succeeds without errors
   - `mdbook build book` produces no warnings
   - All links resolve correctly (internal and external)
   - All code blocks have proper syntax highlighting
   - Search index generates successfully

2. **Content Validation**
   - All sections from original `workflow-syntax.md` present
   - Code examples are syntactically correct
   - YAML examples validate against schema
   - No broken links or references
   - Table of contents complete

3. **CI/CD Validation**
   - GitHub Actions workflow runs successfully
   - Book deploys to GitHub Pages
   - PR builds succeed without deploying
   - Build caching works correctly

### Integration Tests

1. **Navigation Testing**
   - All SUMMARY.md links work
   - Search finds expected content
   - Mobile navigation functional
   - Breadcrumbs show correct hierarchy

2. **Rendering Tests**
   - All pages render correctly
   - Code blocks have syntax highlighting
   - Tables display properly
   - Lists and formatting correct

### Performance Tests

- Book builds in < 30 seconds locally
- CI/CD completes in < 2 minutes
- Search responds in < 100ms
- Page loads in < 1 second

## Documentation Requirements

### Code Documentation

Not applicable - this is documentation infrastructure.

### User Documentation

**README.md Updates**:
```markdown
## 📚 Documentation

Full documentation is available at **[https://yourusername.github.io/prodigy](https://yourusername.github.io/prodigy)**

Quick links:
- [Getting Started](https://yourusername.github.io/prodigy/intro.html)
- [Workflow Syntax](https://yourusername.github.io/prodigy/workflow-basics.html)
- [MapReduce Guide](https://yourusername.github.io/prodigy/mapreduce.html)
- [Examples](https://yourusername.github.io/prodigy/examples.html)

### Building Documentation Locally

```bash
# Install mdBook
cargo install mdbook

# Serve with live reload
mdbook serve book --open
```
```

**CONTRIBUTING.md Updates**:
```markdown
## Documentation

Documentation is built with mdBook and lives in the `book/` directory.

### Working on Documentation

1. Install mdBook: `cargo install mdbook`
2. Start live server: `mdbook serve book --open`
3. Edit files in `book/src/`
4. Changes rebuild automatically

### Documentation Structure

- `book/src/SUMMARY.md` - Table of contents
- `book/src/*.md` - Individual chapters
- `book/book.toml` - mdBook configuration

### Style Guidelines

- Use present tense ("Prodigy executes" not "Prodigy will execute")
- Include code examples for all features
- Use proper YAML syntax highlighting with ```yaml
- Keep examples practical and runnable
- Test all code examples before committing
```

### Book Content

**Introduction Page** (`book/src/intro.md`):
```markdown
# Introduction

Prodigy is an AI-powered workflow orchestration tool that enables development teams to automate complex tasks using Claude AI through structured YAML workflows.

## What is Prodigy?

Prodigy combines the power of Claude AI with workflow orchestration to:

- **Automate repetitive development tasks** - Code reviews, refactoring, testing
- **Process work in parallel** - MapReduce-style parallel execution across git worktrees
- **Maintain quality** - Built-in validation, error handling, and retry mechanisms
- **Track changes** - Full git integration with automatic commits and merge workflows

## Quick Start

Create a simple workflow in `workflow.yml`:

```yaml
- shell: "cargo build"
- shell: "cargo test"
  on_failure:
    claude: "/fix-failing-tests"
- shell: "cargo clippy"
```

Run it:

```bash
prodigy run workflow.yml
```

## Key Concepts

- **Workflows**: YAML files defining sequences of commands
- **Commands**: Shell commands, Claude AI invocations, or control flow
- **Variables**: Dynamic values captured and interpolated across steps
- **MapReduce**: Parallel processing across multiple git worktrees
- **Validation**: Automatic testing and quality checks

## Next Steps

- [Workflow Basics](workflow-basics.md) - Learn workflow fundamentals
- [Command Types](commands.md) - Explore available command types
- [Examples](examples.md) - See real-world workflows
```

## Implementation Notes

### Content Migration Strategy

**Option 1: Manual Migration** (Recommended for initial setup)
- Copy sections from `docs/workflow-syntax.md` into separate chapter files
- Allows reorganization and improvement during migration
- Gives opportunity to update examples and add context
- One-time effort with full control

**Option 2: Automated Split with Script**
- Write Rust/Python script to split workflow-syntax.md by headers
- Faster initial migration
- Risk of awkward splits at header boundaries
- May need manual cleanup anyway

**Recommendation**: Start with Option 1 for better quality, migrate manually with improvements.

### Maintaining Two Versions

**Challenge**: Keep `docs/workflow-syntax.md` and `book/src/*.md` in sync

**Options**:

1. **Book as Source of Truth** (Recommended)
   - Book chapters are canonical
   - Generate single-file from book using mdbook preprocessor or script
   - Users who want single file can generate it
   - Book workflow is primary

2. **Both Maintained Separately**
   - Keep both updated manually
   - Risk of drift over time
   - More maintenance burden
   - Not recommended

3. **Single File as Source, Include in Book**
   - Use `{{#include}}` in book chapters to pull from workflow-syntax.md
   - Workflow-syntax.md remains source of truth
   - Book just organizes includes
   - Simpler to maintain
   - May have awkward chapter boundaries

**Recommendation**: Start with Option 1 (book as source), generate single-file if needed later.

### GitHub Pages Configuration

**Enable GitHub Pages**:
1. Repository Settings → Pages
2. Source: Deploy from a branch
3. Branch: `gh-pages` (created by action)
4. Directory: `/` (root)
5. Save

**Custom Domain** (Optional):
1. Add CNAME record: `docs.prodigy.dev` → `yourusername.github.io`
2. Add `cname: docs.prodigy.dev` to deploy action
3. Enable HTTPS in GitHub Pages settings

**URL Structure**:
- Default: `https://yourusername.github.io/prodigy/`
- Custom: `https://docs.prodigy.dev/`

### Search Configuration

mdBook search is client-side JavaScript:
- Builds search index during book compilation
- Index stored in `searchindex.json`
- No server-side processing needed
- Fast, works offline
- Configure in `book.toml` under `[output.html.search]`

### Code Example Testing

mdBook can test code examples:
```bash
mdbook test book
```

For YAML examples (not executable), use explicit language tags:
````markdown
```yaml,ignore
# This won't be tested
name: example
```
````

For executable Rust examples in future API docs:
````markdown
```rust
// This will be compiled and tested
fn example() {
    assert_eq!(2 + 2, 4);
}
```
````

## Migration and Compatibility

### Breaking Changes

**None** - This is purely additive infrastructure.

### Migration Path

**Phase 1: Initial Setup**
1. Initialize mdBook structure
2. Create minimal book with introduction
3. Set up CI/CD pipeline
4. Verify deployment to GitHub Pages

**Phase 2: Content Migration**
1. Migrate workflow-syntax.md chapter by chapter
2. Add introduction and getting started content
3. Improve examples and add context
4. Update cross-references

**Phase 3: Integration**
1. Update README.md with documentation links
2. Update CONTRIBUTING.md with doc workflow
3. Announce documentation site to users
4. Gather feedback and iterate

**Phase 4: Expansion**
1. Add API reference documentation
2. Add architecture documentation
3. Add contributor guides
4. Add tutorials and workshops

### Compatibility Guarantees

- `docs/workflow-syntax.md` remains available
- No code changes required
- Existing links continue to work
- Additive only, no removals

## Success Metrics

- [ ] mdBook builds without errors or warnings
- [ ] GitHub Actions workflow configured correctly for deployment (actual deployment not required)
- [ ] All content from workflow-syntax.md present in book
- [ ] Search functionality works for key terms
- [ ] Mobile navigation functional
- [ ] CI/CD completes in < 2 minutes
- [ ] Documentation site linked from README
- [ ] No broken links in documentation
- [ ] Code examples properly highlighted
- [ ] Table of contents complete and logical

## Future Enhancements

After initial setup, consider:

1. **API Documentation**
   - Generate from Rust doc comments
   - Integrate with mdBook using mdbook-api or similar

2. **Interactive Examples**
   - Add runnable examples using mdbook-cmdrun
   - Live YAML validation in browser

3. **Multi-Version Documentation**
   - Document multiple Prodigy versions
   - Version selector in documentation

4. **Localization**
   - Translate to other languages
   - mdBook supports multiple languages

5. **Enhanced Search**
   - Add algolia integration for better search
   - Add search analytics

6. **PDF/ePub Generation**
   - Enable `[output.pdf]` in book.toml
   - Provide downloadable documentation

7. **Video Tutorials**
   - Embed video walkthroughs
   - Link to YouTube playlist
