---
number: 126
title: GitHub Workflow Template System
category: foundation
priority: high
status: draft
dependencies: []
created: 2025-10-11
---

# Specification 126: GitHub Workflow Template System

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: None

## Context

When implementing automated documentation deployment across multiple repositories in the prodigy ecosystem, inconsistencies emerged in GitHub Actions workflow files. Different repositories used varying naming conventions, deployment methods, and job structures, leading to maintenance challenges and wasted CI resources.

The root cause analysis identified that while documentation exists for the recommended approach, there's no enforced standardization mechanism. Developers must manually ensure their workflows match the standard, which is error-prone and time-consuming.

**Example issues encountered**:
- Workflow files named `docs.yml` instead of standard `deploy-docs.yml`
- Using GitHub Pages Actions instead of the standardized gh-pages branch method
- Missing path filters causing workflows to run on every commit
- Overcomplicated job structures with unnecessary artifact passing

## Objective

Create a reusable GitHub Actions workflow template system that ensures consistency across all prodigy ecosystem repositories while remaining flexible enough to accommodate project-specific needs. This system will provide both reusable workflows that can be called from other repositories and copy-paste templates for common CI/CD patterns.

## Requirements

### Functional Requirements

- **Reusable Workflows**: Create centralized, reusable GitHub Actions workflows in the prodigy repository that other repos can invoke
- **Template Library**: Provide ready-to-copy workflow templates for common tasks (documentation deployment, testing, releases)
- **Parameterization**: Support customization through workflow inputs while maintaining standard structure
- **Version Control**: Allow repositories to pin to specific versions of reusable workflows
- **Documentation Integration**: Templates must include inline comments linking to detailed documentation

### Non-Functional Requirements

- **Discoverability**: Templates must be easily findable in the prodigy repository
- **Maintainability**: Updates to templates should propagate to repositories using reusable workflows
- **Backwards Compatibility**: Template changes should not break existing workflow invocations
- **Performance**: Reusable workflows should add minimal overhead compared to inline workflows

## Acceptance Criteria

- [ ] Reusable workflow created at `.github/workflows/deploy-docs-reusable.yml` in prodigy repository
- [ ] Template directory created at `.github/workflow-templates/` containing copy-paste examples
- [ ] Documentation deployment template uses gh-pages branch method (peaceiris/actions-gh-pages@v4)
- [ ] Templates include proper path filters, PR validation, and both main/master branch support
- [ ] Reusable workflows accept inputs for customization (branch names, build directories, etc.)
- [ ] Example usage documentation provided for each template
- [ ] At least one external repository successfully migrated to use reusable workflow
- [ ] Templates include inline comments explaining each section
- [ ] Version pinning mechanism documented (e.g., `@v1`, `@main`)

## Technical Details

### Implementation Approach

**Phase 1: Create Reusable Workflow**
1. Extract standardized workflow from `prodigy/.github/workflows/deploy-docs.yml`
2. Add workflow_call trigger with appropriate inputs
3. Parameterize customizable aspects (branch names, paths, build commands)
4. Document all inputs with descriptions and defaults

**Phase 2: Build Template Library**
1. Create `.github/workflow-templates/` directory in prodigy repo
2. Add copy-paste template for documentation deployment
3. Add templates for other common patterns (testing, releases)
4. Include comprehensive inline documentation in each template

**Phase 3: Documentation and Migration**
1. Update prodigy documentation to reference templates
2. Create migration guide for existing repositories
3. Migrate at least one project (e.g., debtmap) as validation
4. Document best practices for choosing reusable vs copy-paste approach

### Reusable Workflow Structure

```yaml
# .github/workflows/deploy-docs-reusable.yml
name: Reusable Documentation Deployment

on:
  workflow_call:
    inputs:
      book_directory:
        description: 'Path to mdBook directory'
        required: false
        default: 'book'
        type: string
      publish_directory:
        description: 'Directory to publish (relative to book_directory)'
        required: false
        default: 'book'
        type: string
      branches:
        description: 'Branches to deploy from (JSON array)'
        required: false
        default: '["main", "master"]'
        type: string
    secrets:
      GITHUB_TOKEN:
        required: true

permissions:
  contents: write

jobs:
  build-deploy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Setup mdBook
        uses: peaceiris/actions-mdbook@v2
        with:
          mdbook-version: 'latest'

      - name: Build book
        run: mdbook build ${{ inputs.book_directory }}

      - name: Deploy to GitHub Pages
        uses: peaceiris/actions-gh-pages@v4
        with:
          github_token: ${{ secrets.GITHUB_TOKEN }}
          publish_dir: ./${{ inputs.book_directory }}/${{ inputs.publish_directory }}
```

### Template Directory Structure

```
.github/
├── workflows/
│   └── deploy-docs-reusable.yml          # Reusable workflow
└── workflow-templates/
    ├── deploy-docs-template.yml          # Copy-paste template
    ├── deploy-docs-template.properties.json  # Template metadata
    ├── rust-ci-template.yml              # Testing template
    └── README.md                         # Template usage guide
```

### Template Metadata Format

```json
{
  "name": "Deploy Documentation",
  "description": "Standard mdBook documentation deployment to GitHub Pages",
  "iconName": "book",
  "categories": ["Deployment", "Documentation"],
  "filePatterns": ["deploy-docs.yml"]
}
```

## Dependencies

### Prerequisites
- None - this is a foundational specification

### Affected Components
- Prodigy repository `.github/` directory
- Documentation website (add new section on workflow templates)
- Any repository in the prodigy ecosystem that deploys documentation

### External Dependencies
- GitHub Actions workflow_call feature
- peaceiris/actions-gh-pages action
- peaceiris/actions-mdbook action

## Testing Strategy

### Unit Tests
- Not applicable for YAML workflows
- Validate YAML syntax using `yamllint`
- Use `actionlint` to validate GitHub Actions syntax

### Integration Tests
1. Create test repository that consumes reusable workflow
2. Verify successful deployment with default parameters
3. Verify customization through workflow inputs
4. Test with both `main` and `master` branches
5. Verify path filters prevent unnecessary workflow runs

### Performance Tests
- Measure workflow execution time with reusable workflow vs inline
- Ensure overhead is < 5 seconds
- Verify path filters prevent runs on non-documentation changes

### User Acceptance
1. Migrate debtmap repository to use reusable workflow
2. Verify documentation deploys correctly
3. Confirm maintainer satisfaction with ease of use
4. Validate that updates to reusable workflow propagate correctly

## Documentation Requirements

### Code Documentation
- Comprehensive inline comments in reusable workflow explaining each step
- Input parameter descriptions in workflow_call definition
- Comments linking to full documentation

### User Documentation
Add new section to prodigy documentation:

**"GitHub Workflow Templates"**
- Overview of reusable workflows vs copy-paste templates
- When to use each approach
- Step-by-step setup guide for each template
- Migration guide for existing repositories
- Troubleshooting common issues

### Architecture Updates
Update `ARCHITECTURE.md` to document:
- Location of workflow templates
- Template versioning strategy
- Process for updating templates
- Guidelines for creating new templates

## Implementation Notes

### Reusable vs Copy-Paste Templates

**Use Reusable Workflows When**:
- Workflow logic is standardized across all repos
- Centralized updates are desired
- Minimal customization needed
- Repository count > 3

**Use Copy-Paste Templates When**:
- Significant customization expected
- Repository needs to deviate from standard
- Learning/educational purposes
- One-off workflows

### Versioning Strategy

Reusable workflows should use semantic versioning tags:
```yaml
uses: iepathos/prodigy/.github/workflows/deploy-docs-reusable.yml@v1
```

Create git tags for workflow versions:
- `v1.0.0` - Initial release
- `v1.1.0` - Backwards-compatible additions
- `v2.0.0` - Breaking changes

Maintain a `main` reference for latest:
```yaml
uses: iepathos/prodigy/.github/workflows/deploy-docs-reusable.yml@main
```

### Security Considerations

- Reusable workflows should not access secrets unless explicitly passed
- Use `secrets: inherit` sparingly
- Document required secrets in workflow comments
- Validate all inputs to prevent injection attacks

### Common Pitfalls

1. **Forgetting to pass secrets**: Reusable workflows don't automatically inherit secrets
2. **Path differences**: Ensure publish_dir paths are correct in calling repository
3. **Branch protection**: Ensure gh-pages branch allows force pushes
4. **Permissions**: Verify `contents: write` permission is granted

## Migration and Compatibility

### Breaking Changes
None - this is a new feature

### Migration Path for Existing Repositories

**Step 1**: Identify current workflow file
```bash
ls .github/workflows/docs*.yml
```

**Step 2**: Choose migration approach

**Option A: Migrate to Reusable Workflow**
```yaml
# .github/workflows/deploy-docs.yml
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

jobs:
  deploy:
    uses: iepathos/prodigy/.github/workflows/deploy-docs-reusable.yml@v1
    with:
      book_directory: 'book'
    secrets:
      GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

**Option B: Update to Match Template**
- Copy latest template from `.github/workflow-templates/deploy-docs-template.yml`
- Customize as needed for repository

**Step 3**: Verify deployment
- Commit changes to feature branch
- Verify PR build runs successfully
- Merge to main/master
- Verify documentation deploys to GitHub Pages

### Compatibility Requirements
- Works with existing gh-pages deployments
- Compatible with GitHub Enterprise Server 3.4+
- Supports both `main` and `master` default branches
- Works with both public and private repositories

## Rollout Plan

### Phase 1: Template Creation (Week 1)
- Create reusable workflow in prodigy
- Create template directory with examples
- Add documentation

### Phase 2: Validation (Week 2)
- Migrate debtmap to use reusable workflow
- Test all customization inputs
- Gather feedback

### Phase 3: Ecosystem Rollout (Week 3-4)
- Document migration process
- Announce to prodigy users
- Migrate additional repositories
- Address issues and refine

## Success Metrics

- **Adoption**: At least 3 repositories using reusable workflow within 1 month
- **Consistency**: 100% of prodigy ecosystem repos using standard workflow structure
- **Maintenance**: Updates to reusable workflow propagate without manual intervention
- **Resource Efficiency**: Reduction in unnecessary workflow runs due to proper path filters
- **Developer Satisfaction**: Positive feedback on ease of setup and maintenance

## Future Enhancements

- Additional templates for Rust CI, releases, security scanning
- Automated migration tool to convert existing workflows
- Workflow linter that validates against standards
- Template gallery in prodigy documentation
- Support for composite actions for reusable steps
