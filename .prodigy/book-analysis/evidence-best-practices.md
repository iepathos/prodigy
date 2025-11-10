# Evidence for Best Practices

## Source Definitions Found
- Workflow structure: workflows/book-docs-drift.yml
- Environment variables: workflows/environment-example.yml:4-39
- MapReduce configuration: workflows/book-docs-drift.yml:36-90
- Goal-seeking patterns: workflows/goal-seeking-examples.yml:6-95
- Error handling: workflows/mapreduce-example.yml:18-20
- Testing workflows: workflows/tests/minimal-mapreduce.yml:1-50

## Workflow Examples Found
- workflows/book-docs-drift.yml - Complete automated documentation workflow
- workflows/environment-example.yml - Environment variable examples
- workflows/minimal-mapreduce.yml - Minimal test workflow
- workflows/goal-seeking-examples.yml - Goal-seeking patterns
- workflows/debtmap-reduce.yml - Filter/sort examples
- workflows/mapreduce-example.yml - Basic MapReduce structure

## Configuration Examples Found
- env block: workflows/book-docs-drift.yml:8-21
- profiles: workflows/environment-example.yml:30-39
- secrets: workflows/environment-example.yml:21-23
- error_policy: workflows/book-docs-drift.yml:86-90
- max_parallel settings: workflows/book-docs-drift.yml:59

## Documentation References
- book/src/configuration/environment-variables.md - Environment variable guide
- book/src/advanced/goal-seeking-operations.md - Goal-seeking details
- book/src/mapreduce/checkpoint-and-resume.md - Resume functionality

## Validation Results
✓ All config fields verified against workflow files
✓ All YAML syntax validated from actual examples
✓ All CLI commands verified (prodigy events, dlq show, dlq retry)
✓ Real-world examples extracted from workflows/

## Discovery Notes
- Test directories found: ./tests, ./workflows/tests
- Example directories found: ./workflows, ./examples
- Source directories searched: workflows/*.yml (15 workflow files examined)
- All best practices grounded in real workflow patterns from the codebase
