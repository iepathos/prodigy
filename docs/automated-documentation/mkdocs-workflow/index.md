# MkDocs Documentation Workflow

The `mkdocs-drift.yml` workflow automatically generates and maintains MkDocs Material documentation by detecting gaps, analyzing drift, and fixing documentation to stay synchronized with your codebase.

This workflow is designed for projects using **MkDocs Material** as their documentation system. It provides the same capabilities as the mdbook workflow but targets MkDocs-specific features and structure.

**Key Features:**
- Automatic gap detection for undocumented features
- Drift analysis comparing docs against source code
- Intelligent fixes with source attribution
- MkDocs build validation with `--strict` mode
- Navigation completeness checking
- Broken link detection

## Documentation Sections

### [Getting Started](getting-started.md)

Learn the basics of running the MkDocs workflow:

- **Overview** - Understanding the workflow purpose and capabilities
- **Quick Start** - Run your first documentation workflow
- **Configuration Options** - Environment variables, directories, and parallelism settings

### [Workflow Phases](workflow-phases.md)

Deep dive into each phase of the workflow:

- **Setup Phase** - Feature analysis and gap detection
- **Map Phase** - Parallel drift analysis and fixing
- **Reduce Phase** - Build validation and holistic checks
- **Workflow Commands Reference** - All available commands with parameters

### [Advanced Usage](advanced-usage.md)

Advanced configuration and best practices:

- **Advanced Configuration** - Custom project config, validation thresholds, error handling
- **Using with Existing MkDocs Projects** - Migration and CI/CD integration
- **Troubleshooting** - Common issues and solutions
- **Best Practices** - Recommendations for effective usage
- **Examples** - Complete workflow configurations for different scenarios
