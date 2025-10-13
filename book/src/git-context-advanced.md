# Advanced Git Context

Advanced git context features enable powerful filtering and formatting of git information in your workflows. This chapter covers pattern filtering, format modifiers, and advanced file pattern matching.

## Overview

Prodigy provides sophisticated git context capabilities that go beyond basic variables. You can filter commits by file patterns, format output with custom modifiers, and combine multiple filters for precise control over what git information flows into your workflows.

## Pattern Filtering

Filter git context to include or exclude specific file patterns:

### Include Patterns

```yaml
# Only include changes to Rust files
- claude: "/analyze-changes"
  git_context:
    include:
      - "**/*.rs"
      - "Cargo.toml"
```

### Exclude Patterns

```yaml
# Exclude test files and generated code
- claude: "/review-code"
  git_context:
    exclude:
      - "**/tests/**"
      - "**/*_test.rs"
      - "**/generated/**"
```

### Combined Filters

```yaml
# Include Rust files but exclude tests
- claude: "/lint-code"
  git_context:
    include:
      - "src/**/*.rs"
    exclude:
      - "**/tests/**"
```

## Format Modifiers

Customize how git context is formatted in your workflows:

```yaml
# Example format modifiers
- shell: "echo ${git.diff:unified}"
- shell: "echo ${git.log:oneline}"
- shell: "echo ${git.status:short}"
```

**Available Modifiers:**
- `:unified` - Standard unified diff format
- `:oneline` - Single-line format for commit messages
- `:short` - Short format for status output
- `:full` - Full detailed format

## File Patterns

Use glob patterns to match files precisely:

```yaml
# Match specific file types
git_context:
  include:
    - "**/*.{rs,toml}"     # Rust and TOML files
    - "src/**/mod.rs"      # Module files
    - "tests/integration/**"  # Integration tests
```

**Pattern Syntax:**
- `*` - Match any characters except `/`
- `**` - Match any characters including `/`
- `?` - Match single character
- `{a,b}` - Match either `a` or `b`
- `[abc]` - Match character class

## Use Cases

### Code Review Workflows

```yaml
- claude: "/review-changes"
  git_context:
    include:
      - "src/**/*.rs"
    exclude:
      - "**/tests/**"
      - "**/*.md"
```

### Documentation Updates

```yaml
- claude: "/update-docs"
  git_context:
    include:
      - "**/*.md"
      - "docs/**"
```

### Test-Only Changes

```yaml
- claude: "/verify-tests"
  git_context:
    include:
      - "**/tests/**"
      - "**/*_test.rs"
```

## Best Practices

- **Be Specific**: Use precise patterns to avoid including irrelevant files
- **Exclude Noise**: Filter out generated files, dependencies, and build artifacts
- **Test Patterns**: Verify your patterns match the intended files
- **Document Intent**: Add comments explaining why certain patterns are included or excluded

## Troubleshooting

### Pattern Not Matching Files

**Issue**: Your include pattern doesn't match any files
**Solution**: Use `git ls-files` to verify file paths match your pattern

### Too Many Files Included

**Issue**: Git context includes unwanted files
**Solution**: Add exclude patterns to filter out noise

### Format Modifier Not Working

**Issue**: Format modifier syntax errors
**Solution**: Check modifier name and ensure proper syntax with colon separator

## See Also

- [Variables and Interpolation](variables.md) - Basic variable usage
- [Workflow Basics](workflow-basics.md) - Git integration fundamentals
- [MapReduce Workflows](mapreduce.md) - Using git context in parallel jobs
