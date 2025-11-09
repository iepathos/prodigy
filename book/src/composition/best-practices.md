## Best Practices

Guidelines for effective workflow composition, organization, and maintenance.

### When to Use Each Feature

**Imports** - Use when:
- Sharing common configurations across workflows
- Referencing reusable command sets
- Building modular workflows from components
- Need namespace isolation with aliases

**Extends** - Use when:
- Creating environment-specific variations (dev/staging/prod)
- Building layered configurations
- Want parent-child inheritance relationship
- Need to override specific parent values

**Templates** - Use when:
- Creating standardized workflows for teams
- Need parameterized, reusable patterns
- Want centralized workflow distribution
- Building CI/CD pipeline standards

**Sub-Workflows** - Use when:
- Breaking complex workflows into smaller pieces
- Running independent tasks in parallel
- Isolating context between workflow stages
- Reusing validation/test workflows

### Workflow Organization

**Directory Structure:**
```
workflows/
├── templates/           # Reusable templates
│   ├── ci-pipeline.yml
│   ├── deployment.yml
│   └── testing.yml
├── shared/              # Common imports
│   ├── setup.yml
│   ├── validation.yml
│   └── utilities.yml
├── environments/        # Environment-specific (extends pattern)
│   ├── base.yml
│   ├── dev.yml
│   ├── staging.yml
│   └── production.yml
└── services/            # Per-service workflows
    ├── api/
    │   ├── test.yml
    │   └── deploy.yml
    └── worker/
        ├── test.yml
        └── deploy.yml
```

### Parameter Naming Conventions

**Good names:**
```yaml
parameters:
  definitions:
    deployment_environment  # Clear, descriptive
    max_retry_count        # Explicit purpose
    enable_debug_mode      # Boolean intent clear
```

**Avoid:**
```yaml
parameters:
  definitions:
    env      # Too brief, ambiguous
    count    # Unclear what it counts
    flag     # Doesn't indicate purpose
```

### Template Registry Best Practices

**Versioning Strategy:**
```
~/.prodigy/templates/
├── ci-pipeline-v1.yml
├── ci-pipeline-v2.yml    # Major version changes
├── deployment-stable.yml
├── deployment-beta.yml   # Testing new features
└── deprecated/
    └── old-workflow.yml  # Keep for reference
```

**Template Naming:**
- Use descriptive names: `rust-test-suite.yml` not `test.yml`
- Include purpose: `docker-build-and-push.yml`
- Version when needed: `deploy-v2.yml`
- Use prefixes for categories: `ci-*`, `deploy-*`, `test-*`

### Avoiding Circular Dependencies

**Bad:**
```yaml
# workflow-a.yml
extends: "workflow-b.yml"

# workflow-b.yml
extends: "workflow-a.yml"  # Circular!
```

**Good:**
```yaml
# base.yml
name: base

# workflow-a.yml
extends: "base.yml"

# workflow-b.yml
extends: "base.yml"  # Both extend common base
```

### Performance Considerations

**Template Caching:**
- Templates are cached after first load
- File-based templates reload if file changes
- Registry templates cache until registry updates
- Use registry for frequently-used templates

**Composition Depth:**
- Limit inheritance depth to 3-4 levels
- Deep hierarchies slow composition and debugging
- Prefer flatter structures with explicit composition

**Parallel Sub-Workflows:**
```yaml
sub_workflows:
  # Good: Independent tests run in parallel
  - name: "unit-tests"
    parallel: true
  - name: "integration-tests"
    parallel: true

  # Bad: Sequential dependencies forced to parallel
  - name: "build"
    parallel: true
  - name: "deploy"   # Needs build output!
    parallel: true
```

### Testing Composed Workflows

**Validate Before Running:**
```bash
# Dry-run to check composition
prodigy run workflow.yml --dry-run

# Show composition metadata
prodigy run workflow.yml --dry-run --show-composition
```

**Test Parameter Variations:**
```bash
# Test with different parameters
prodigy run workflow.yml --param environment=dev
prodigy run workflow.yml --param environment=staging
prodigy run workflow.yml --param environment=prod
```

**Verify Inheritance:**
```yaml
# Add debug output in child workflows
commands:
  - shell: "echo Inherited timeout: ${timeout}"
  - shell: "echo Overridden log_level: ${log_level}"
```

### Debugging Composed Workflows

**Track Dependency Chain:**
```bash
# Composition metadata shows:
# - Which files were imported
# - What workflows were extended
# - Which templates were applied
prodigy run workflow.yml --show-composition
```

**Verbose Execution:**
```bash
# See each composition step
prodigy run workflow.yml -vv
```

**Isolate Issues:**
- Test base workflow alone first
- Add composition features incrementally
- Verify each layer works before adding next

### Security Best Practices

**Parameter Validation:**
```yaml
parameters:
  definitions:
    deployment_target:
      type: String
      validation: "matches('^(dev|staging|prod)$')"  # Restrict values

    replicas:
      type: Number
      validation: "value >= 1 && value <= 100"  # Prevent resource abuse
```

**Sensitive Data:**
```yaml
# DON'T: Store secrets in workflow files
defaults:
  api_key: "sk-abc123"  # Bad!

# DO: Pass secrets via CLI or environment
```

```bash
# Pass secrets at runtime
prodigy run workflow.yml --param api_key="${API_KEY}"
```

### Maintenance and Evolution

**Deprecation Strategy:**
1. Mark template as deprecated in comments
2. Create new version with fixes
3. Update dependent workflows gradually
4. Move old template to `deprecated/` folder
5. Remove after migration period

**Documentation:**
```yaml
# Document template purpose and usage
# Template: Standard CI Pipeline
# Purpose: Runs lint, test, build for Rust projects
# Parameters:
#   - project_name (required): Name of the Rust project
#   - test_coverage (optional): Minimum coverage % (default: 80)
# Example:
#   template:
#     source:
#       registry: "rust-ci-pipeline"
#     with:
#       project_name: "my-service"

name: rust-ci-pipeline-template
```

### Team Collaboration

**Shared Template Registry:**
```bash
# Team shares templates via git
git clone git@github.com:company/prodigy-templates.git
ln -s ~/prodigy-templates ~/.prodigy/templates
```

**Code Review:**
- Review template changes carefully (affects all users)
- Test template changes with dry-runs
- Version templates for breaking changes
- Document parameter changes in commits

**Onboarding:**
- Provide example workflows using templates
- Document common patterns in team wiki
- Create starter templates for new projects
- Share troubleshooting guides
