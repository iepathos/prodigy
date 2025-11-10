## Further Reading

This section provides additional resources for deepening your understanding of Prodigy's workflow composition system, including source code references, related documentation, and external resources.

### Source Code Implementation

For developers interested in understanding the implementation details, explore these core modules:

**Core Composition Engine** (`src/cook/workflow/composition/`)
- The main composition implementation directory containing the template registry, dependency resolution logic, and workflow composition engine
- Entry point: `mod.rs` defines the public API and core types (`ComposableWorkflow`, `CompositionMetadata`, `ParameterDefinitions`)

**Template Registry** (`src/cook/workflow/composition/registry.rs`)
- Implements template storage, retrieval, and management
- Provides the `TemplateRegistry` struct with methods for registering, loading, and validating workflow templates
- Includes file-based storage implementation with template versioning support
- Referenced in: [Template System](template-system.md)

**Dependency Resolution** (`src/cook/workflow/composition/composer.rs`)
- Implements the `WorkflowComposer` that handles workflow composition from multiple sources
- Resolves dependencies between workflows, templates, and imports
- Performs parameter interpolation and validation
- Handles circular dependency detection and resolution order
- Referenced in: [Workflow Extension & Inheritance](workflow-extension-inheritance.md)

**Sub-Workflow Execution** (`src/cook/workflow/composition/sub_workflow.rs`)
- Defines types and structures for sub-workflow execution (implementation in progress)
- Includes `SubWorkflow` definition with execution modes (parallel/sequential)
- Referenced in: [Sub-Workflows](sub-workflows.md)

**Integration Tests** (`tests/workflow_composition_test.rs`)
- Comprehensive test suite covering workflow composition, template registration, parameter validation, and dependency resolution
- Provides practical examples of API usage for:
  - Basic workflow composition
  - Parameter definitions with type checking
  - Template registry operations
  - Import and extension workflows
  - Circular dependency detection
- Excellent starting point for understanding how to use the composition API programmatically

### Related Subsections

To explore specific aspects of workflow composition in depth:

- **[Template System](template-system.md)** - Learn about creating and using reusable workflow templates
- **[Workflow Extension & Inheritance](workflow-extension-inheritance.md)** - Understand how to extend base workflows and inherit configurations
- **[Parameter Definitions](parameter-definitions.md)** - Master parameter type validation and default values
- **[Sub-Workflows](sub-workflows.md)** - Execute workflows within workflows for complex orchestration patterns
- **[Default Values](default-values.md)** - Configure sensible defaults for parameters and settings
- **[Composition Metadata](composition-metadata.md)** - Track composition sources and dependencies
- **[Best Practices](best-practices.md)** - Follow established patterns for maintainable workflow composition
- **[Troubleshooting](troubleshooting.md)** - Resolve common composition issues and errors
- **[Complete Examples](complete-examples.md)** - See real-world examples of workflow composition in action

### Related Chapters

Workflow composition integrates with other Prodigy features:

- **[Configuration](../configuration/index.md)** - Global and project-level configuration for composition behavior
- **[Environment Variables](../environment/index.md)** - Use environment variables in composed workflows
- **[Variables](../variables/index.md)** - Variable interpolation and capture in composed workflows
- **[Workflow Basics](../workflow-basics/index.md)** - Foundation for understanding workflow structure

### Implementation Specifications

The workflow composition system is implemented according to these specifications:

- **Spec 131**: Template Registry and Storage
- **Spec 132**: Workflow Composition Engine and Dependency Resolution
- **Spec 133**: Sub-Workflow Execution Framework

These specs are referenced in the main codebase and track the implementation progress of composition features. See the [Implementation Status](index.md#implementation-status) section for current progress.

### External Resources

For broader context on workflow composition and orchestration:

**YAML Best Practices**
- [YAML Specification](https://yaml.org/spec/) - Official YAML 1.2 specification
- [YAML Ain't Markup Language](https://yaml.org/) - YAML homepage with tutorials and examples
- [CloudBees YAML Best Practices](https://www.cloudbees.com/blog/yaml-tutorial-everything-you-need-get-started) - Practical tips for writing maintainable YAML

**Workflow Orchestration Patterns**
- [Workflow Patterns](http://www.workflowpatterns.com/) - Academic resource on workflow patterns and control flow
- [Airflow Concepts](https://airflow.apache.org/docs/apache-airflow/stable/concepts/index.html) - DAG composition and dependencies (similar concepts to Prodigy)
- [GitHub Actions Reusable Workflows](https://docs.github.com/en/actions/using-workflows/reusing-workflows) - Another approach to workflow composition

**Template Design Principles**
- [Jinja2 Template Designer Documentation](https://jinja.palletsprojects.com/en/stable/templates/) - Similar template concepts (parameter passing, inheritance)
- [Helm Templates Guide](https://helm.sh/docs/chart_template_guide/) - Template best practices from Kubernetes ecosystem
- [JSON Schema](https://json-schema.org/) - Parameter validation patterns (similar to Prodigy's parameter definitions)

**Configuration Management**
- [The Twelve-Factor App: Config](https://12factor.net/config) - Principles for configuration management
- [TOML vs YAML vs JSON](https://gohugohq.com/partials/yaml-vs-toml-vs-json/) - Comparison of configuration formats

### Learning Path

**For Beginners:**
1. Start with [Workflow Basics](../workflow-basics/index.md) to understand fundamental workflow structure
2. Read [Template System](template-system.md) to learn about creating reusable components
3. Explore [Complete Examples](complete-examples.md) to see composition in practice
4. Review the integration tests (`tests/workflow_composition_test.rs`) for API usage examples

**For Advanced Users:**
1. Study [Workflow Extension & Inheritance](workflow-extension-inheritance.md) for complex composition patterns
2. Master [Parameter Definitions](parameter-definitions.md) with advanced type validation
3. Explore the source code in `src/cook/workflow/composition/` to understand implementation details
4. Read [Best Practices](best-practices.md) for production-ready composition strategies
5. Contribute: Examine Spec 131-133 and help complete sub-workflow execution implementation

**For Contributors:**
1. Review the composition source code (`src/cook/workflow/composition/`)
2. Study existing tests in `tests/workflow_composition_test.rs`
3. Check the [Implementation Status](index.md#implementation-status) for areas needing work
4. Follow the patterns established in `composer.rs` and `registry.rs`
5. Add test coverage for any new composition features
