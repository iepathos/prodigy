# Automated Documentation with mdBook

This guide shows you how to set up automated, always-up-to-date documentation for any project using Prodigy's book workflow system. This same system maintains the documentation you're reading right now.

## Overview

The book workflow system:
- **Analyzes your codebase** to build a feature inventory
- **Detects documentation drift** by comparing docs to implementation
- **Updates documentation** automatically using Claude
- **Maintains consistency** across all chapters
- **Runs on any project** - just configure and go

The generalized commands work for any codebase: Rust, Python, JavaScript, etc.

## Quick Start

Ready to get started? Here's the fastest path:

1. **Install prerequisites** (see below)
2. **Initialize your book** structure:
   ```bash
   mdbook init book
   cd book
   # Edit book.toml and src/SUMMARY.md as needed
   ```
3. **Create a documentation workflow** (see [Quick Start (30 Minutes)](quick-start-30-minutes.md))
4. **Run the workflow**:
   ```bash
   prodigy run workflows/book-docs.yml
   ```
5. **Review and merge** the generated documentation

For a detailed walkthrough, see [Quick Start (30 Minutes)](quick-start-30-minutes.md) or [Quick Start](quick-start.md).

## Prerequisites

Ensure you have the following tools installed and configured:

1. **Prodigy** (latest version recommended):
   ```bash
   cargo install prodigy
   ```
   - Minimum: Any recent release
   - Tested with: Prodigy 1.0.0+
   - Source: [github.com/iepathos/prodigy](https://github.com/iepathos/prodigy)

2. **mdBook** (0.4.0 or later):
   ```bash
   cargo install mdbook
   ```
   - Minimum: 0.4.0
   - Tested with: Latest stable release
   - Note: Older versions may lack features used in book.toml
   - Verify installation: `mdbook --version`

3. **Claude Code CLI** with valid API credentials:
   - Requires active Anthropic API key
   - Set up authentication before running workflows
   - Used for automated documentation analysis and generation
   - See [Claude Code documentation](https://docs.anthropic.com/claude/docs)

4. **Git** (2.25 or later) with initialized repository:
   ```bash
   # Verify git is installed (minimum 2.25)
   git --version

   # Initialize a repository if needed
   git init
   ```
   - Minimum: Git 2.25+ (for worktree support)
   - Recommended: Git 2.30+ for improved worktree handling
   - Required: Repository must be initialized (`git init`)

5. **Rust toolchain** (for Cargo-based installation):
   - Edition 2021 or later
   - Required to build Prodigy and mdBook from source
   - Install via [rustup.rs](https://rustup.rs)


## How It Works

The documentation workflow uses a **MapReduce pattern** to process your codebase in parallel:

### Workflow Phases

1. **Setup Phase** (Feature Analysis):
   - Analyzes your codebase to build a complete feature inventory
   - Detects documentation gaps by comparing existing docs to implementation
   - Creates missing chapter/subsection files with placeholders
   - Generates work items for the map phase
   - Source: workflows/book-docs-drift.yml:24-34

2. **Map Phase** (Parallel Processing):
   - Processes each chapter/subsection in parallel using isolated git worktrees
   - For each documentation item:
     - Analyzes drift between documentation and implementation
     - Fixes identified issues with real code examples
     - Validates fixes meet quality standards
   - Runs up to 3 items concurrently (configurable via MAX_PARALLEL)
   - Failed items go to Dead Letter Queue (DLQ) for retry
   - Source: workflows/book-docs-drift.yml:37-59

3. **Reduce Phase** (Validation):
   - Rebuilds the entire book to ensure chapters compile together
   - Checks for broken links between chapters
   - Fixes any build errors discovered during compilation
   - Cleans up temporary analysis files
   - Source: workflows/book-docs-drift.yml:62-82

4. **Merge Phase** (Integration):
   - Merges updated documentation back to your original branch
   - Preserves your working tree state
   - Uses Claude to handle any merge conflicts
   - Source: workflows/book-docs-drift.yml:93-100

### Worktree Isolation

All phases execute in an isolated git worktree:
- Your main repository remains untouched during execution
- Each map agent runs in its own child worktree
- Changes merge back only after successful completion
- Failed workflows don't pollute your working directory
- Learn more: [Understanding the Workflow](understanding-the-workflow.md)

### Quality Guarantees

The workflow ensures documentation quality through:
- **Code-grounded examples**: All examples extracted from actual implementation
- **Validation checkpoints**: Each fix validated before proceeding
- **Build verification**: Full book rebuild ensures no broken references
- **Source attribution**: Examples include file paths and line numbers
- **Automatic retry**: Failed items can be retried via `prodigy dlq retry`

For detailed information about each phase, see the subsections below.

## Additional Topics

See also:
- [Quick Start (30 Minutes)](quick-start-30-minutes.md)
- [Installation](installation.md)
- [Quick Start](quick-start.md)
- [Understanding the Workflow](understanding-the-workflow.md)
- [Automatic Gap Detection](automatic-gap-detection.md)
- [GitHub Actions Integration](github-actions-integration.md)
- [Customization Examples](customization-examples.md)
- [Best Practices](best-practices.md)
- [Troubleshooting](troubleshooting.md)
- [Advanced Configuration](advanced-configuration.md)
- [Real-World Example: Prodigy's Own Documentation](real-world-example-prodigys-own-documentation.md)
- [Documentation Versioning](documentation-versioning.md)
- [Next Steps](next-steps.md)
- [Benefits](benefits.md)
