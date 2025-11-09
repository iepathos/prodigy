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

## Prerequisites

1. **Install Prodigy**:
   ```bash
   cargo install prodigy
   ```

2. **Install mdBook**:
   ```bash
   cargo install mdbook
   ```

3. **Claude Code CLI** with valid API credentials

4. **Git** - Version control system (git 2.25+ recommended) and an initialized git repository for your project

   ```bash
   # Verify git is installed
   git --version

   # Initialize a repository if needed
   git init
   ```


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
