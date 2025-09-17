---
number: 75
title: Interactive Examples Directory
category: foundation
priority: high
status: draft
dependencies: []
created: 2025-01-17
---

# Specification 75: Interactive Examples Directory

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: None

## Context

Users learn best through examples they can run and modify. Prodigy lacks a structured examples directory with ready-to-run workflows demonstrating various features, patterns, and integrations. Interactive examples accelerate user onboarding and showcase best practices.

## Objective

Create a comprehensive examples directory with categorized, well-documented, runnable examples that demonstrate Prodigy's capabilities, from basic workflows to advanced MapReduce patterns and integrations.

## Requirements

### Functional Requirements
- Create hierarchical examples directory structure by category
- Provide 20+ examples covering all major features
- Include README.md in each category explaining examples
- Make examples self-contained and runnable without setup
- Provide expected output and success criteria
- Include progressive difficulty levels (basic, intermediate, advanced)
- Create example test harness to validate all examples
- Support parameterized examples with environment variables
- Include performance benchmarks for parallel examples
- Provide troubleshooting guide for each example

### Non-Functional Requirements
- Examples must run successfully on fresh installation
- Each example completes within 5 minutes
- Examples use minimal external dependencies
- Documentation follows consistent format
- Examples demonstrate best practices
- Code quality matches production standards

## Acceptance Criteria

- [ ] Examples directory contains 20+ working examples
- [ ] Each example has README with purpose, usage, and expected output
- [ ] All examples pass automated testing
- [ ] Examples cover: basic, parallel, goal-seek, testing, CI/CD workflows
- [ ] Progressive learning path from simple to complex
- [ ] Performance examples show measurable improvements
- [ ] Integration examples work with common tools (GitHub, Docker, etc.)
- [ ] Example validator runs in CI/CD pipeline
- [ ] Users can run any example with single command
- [ ] Examples are referenced from main README

## Technical Details

### Implementation Approach
1. Design category hierarchy for examples
2. Create example templates and standards
3. Implement example validation framework
4. Build progressive learning path
5. Add automated testing for all examples

### Directory Structure
```
examples/
├── README.md                    # Overview and learning path
├── 01-basic/                   # Basic workflows
│   ├── README.md
│   ├── hello-world.yaml
│   ├── simple-automation.yaml
│   └── multi-step.yaml
├── 02-parallel/                # MapReduce and parallel execution
│   ├── README.md
│   ├── parallel-tests.yaml
│   ├── code-analysis.yaml
│   └── batch-processing.yaml
├── 03-goal-seeking/            # Goal-seeking workflows
│   ├── README.md
│   ├── test-fixing.yaml
│   ├── performance-tuning.yaml
│   └── iterative-refinement.yaml
├── 04-testing/                 # Testing automation
│   ├── README.md
│   ├── unit-test-runner.yaml
│   ├── integration-tests.yaml
│   └── coverage-improvement.yaml
├── 05-cicd/                   # CI/CD integration
│   ├── README.md
│   ├── github-actions.yaml
│   ├── gitlab-ci.yaml
│   └── jenkins.yaml
├── 06-advanced/               # Advanced patterns
│   ├── README.md
│   ├── nested-mapreduce.yaml
│   ├── conditional-workflows.yaml
│   └── dynamic-generation.yaml
├── 07-integrations/           # Tool integrations
│   ├── README.md
│   ├── docker-workflows.yaml
│   ├── kubernetes.yaml
│   └── terraform.yaml
└── validate.sh               # Example validation script
```

### Example Template
```yaml
# examples/category/example-name.yaml
name: example-name
description: Brief description of what this example demonstrates
difficulty: basic|intermediate|advanced
estimated_time: "2 minutes"
requires: []  # External requirements

# Example-specific documentation
# Expected output: ...
# Success criteria: ...

commands:
  - shell: "echo 'Starting example...'"
  - claude: "/command-name"
  # ... example implementation
```

### APIs and Interfaces
- `prodigy run examples/01-basic/hello-world.yaml`
- `prodigy examples list` (future enhancement)
- `prodigy examples validate`
- `prodigy examples run <category>/<name>`

## Dependencies

- **Prerequisites**: None
- **Affected Components**: Documentation, CI/CD pipeline
- **External Dependencies**: None (examples self-contained)

## Testing Strategy

- **Validation Tests**: Ensure all examples are syntactically valid
- **Execution Tests**: Run all examples in CI/CD
- **Output Tests**: Verify expected outputs match
- **Performance Tests**: Benchmark parallel examples
- **User Acceptance**: Test with new users

## Documentation Requirements

- **Code Documentation**: Document example structure and standards
- **User Documentation**: Link examples from README
- **Architecture Updates**: Reference examples in architecture docs

## Implementation Notes

- Keep examples focused on single concepts
- Use realistic but simple scenarios
- Include comments explaining each step
- Test examples on all supported platforms
- Version examples with Prodigy releases
- Consider interactive example browser in future

## Migration and Compatibility

- No breaking changes
- Examples work with current Prodigy version
- Maintain backward compatibility in examples
- Update examples when features change