# /prodigy-fix-workflow-syntax-drift

Update `docs/workflow-syntax.md` to fix all detected drift issues and ensure documentation matches the current codebase implementation.

## Variables

Uses `${drift_summary}` variable containing aggregated drift findings.

## Execute

### Context

You have completed drift analysis across multiple documentation sections. Now you need to update the documentation to fix all identified issues while maintaining quality and clarity.

## Input Variables

- `${drift_summary}` - JSON summary of all drift findings

## Available Data Files

1. `.prodigy/syntax-analysis/drift-summary.json` - Aggregated drift report
2. `.prodigy/syntax-analysis/drift-{section_id}.json` - Individual section reports
3. `.prodigy/syntax-analysis/features.json` - Ground truth feature analysis

## Update Process

### 1. Review Drift Summary

Read `.prodigy/syntax-analysis/drift-summary.json`:
```json
{
  "total_sections": 6,
  "sections_with_drift": 3,
  "total_issues": 12,
  "severity_breakdown": {"high": 4, "medium": 6, "low": 2},
  "all_reports": [...]
}
```

Prioritize by severity:
1. High/Critical severity issues first
2. Medium severity issues next
3. Low severity issues last

### 2. Process Each Drifted Section

For each section with drift:

#### a. Load Section Details
- Read drift report: `.prodigy/syntax-analysis/drift-{section_id}.json`
- Read current documentation: `docs/workflow-syntax.md` (extract section)
- Read source code: Use `source_reference` from drift issues

#### b. Analyze Issues
- Review all issues for the section
- Group related issues (e.g., all missing fields for same command type)
- Identify fix strategy (add, update, remove, reorganize)

#### c. Apply Fixes

**For Missing Features:**
- Add new subsection or expand existing
- Document all fields from struct definition
- Include practical YAML example
- Add description of when/why to use

**For Outdated Syntax:**
- Replace deprecated syntax with current
- Add deprecation notice if feature still works
- Show migration path (old → new)
- Update all examples using old syntax

**For Incorrect Examples:**
- Fix YAML syntax errors
- Add missing required fields
- Correct field types
- Ensure example works with current code

**For Missing Fields:**
- Add field to documentation
- Include type, required/optional status
- Add default value if applicable
- Provide example usage

**For Deprecated Features:**
- Add clear deprecation notice
- Show replacement syntax
- Indicate version deprecated/removed
- Keep brief to discourage use

### 3. Maintain Documentation Quality

While fixing drift:

#### Preserve Good Content
- Keep clear, working examples
- Maintain well-written descriptions
- Preserve helpful tips and best practices
- Keep table of contents and structure

#### Improve Clarity
- Use consistent terminology
- Add comments to complex examples
- Group related fields logically
- Use tables for field references

#### Update Examples
- Ensure all examples parse correctly
- Use realistic use cases
- Show both simple and advanced usage
- Include output/result examples where helpful

#### Formatting Consistency
- Use consistent YAML indentation (2 spaces)
- Use consistent heading levels
- Use consistent code block language tags
- Maintain existing style conventions

### 4. Verify Changes

After updating each section:

#### Check Completeness
- All fields from struct are documented
- All command types are covered
- All variable types are listed
- All error handling options shown

#### Check Accuracy
- Examples match current struct definitions
- Field types are correct (string, number, boolean, object, array)
- Required vs optional is accurate
- Default values are correct

#### Check Clarity
- Technical accuracy doesn't sacrifice readability
- Examples are practical and understandable
- Complex features have adequate explanation
- Beginners can follow along

### 5. Create Update Summary

Write summary to `.prodigy/syntax-analysis/updates-applied.md`:

```markdown
# Workflow Syntax Documentation Updates

## Summary
- Analyzed: 6 sections
- Found drift: 3 sections
- Total issues fixed: 12
- Severity: 4 high, 6 medium, 2 low

## Sections Updated

### Command Types (High severity - 5 issues fixed)
- ✓ Added goal_seek command type documentation
- ✓ Added capture_streams field to shell commands
- ✓ Removed deprecated test: command syntax
- ✓ Added timeout field to all command types
- ✓ Updated foreach command with parallel configuration

### Variable Interpolation (Medium severity - 4 issues fixed)
- ✓ Added capture_streams variables (${output.exit_code}, etc.)
- ✓ Added validation.* variables
- ✓ Updated git context variable examples
- ✓ Added merge.* variables for custom merge workflows

### MapReduce Workflows (Medium severity - 3 issues fixed)
- ✓ Documented array format for agent_template
- ✓ Added merge workflow configuration
- ✓ Updated error_policy fields

## Examples Updated
- 8 YAML examples corrected
- 3 new examples added
- 2 deprecated examples removed

## Deprecation Notices Added
- test: command (use shell: with on_failure:)
- command: in validation (use shell:)
- capture_output: boolean (use capture: variable_name)

## Source Files Referenced
- src/config/command.rs
- src/cook/workflow/executor.rs
- src/config/mapreduce.rs
- src/cook/workflow/validation.rs
- src/cook/goal_seek/mod.rs
```

### 6. Create Git Commit

Create a clear commit with:

**Commit Message Format:**
```
docs: fix workflow syntax drift - update {N} sections

Fixed drift in workflow syntax documentation:
- Command Types: added goal_seek, capture_streams, removed deprecated test:
- Variable Interpolation: added capture_streams vars, validation vars
- MapReduce: documented array formats, merge workflow config

Issues resolved:
- 4 high severity (missing features, outdated syntax)
- 6 medium severity (missing fields, incorrect examples)
- 2 low severity (deprecation notices)

🤖 Generated with [Claude Code](https://claude.com/claude-code)

Co-Authored-By: Claude <noreply@anthropic.com>
```

**Commit Contents:**
- Updated `docs/workflow-syntax.md`
- Updated summary in `.prodigy/syntax-analysis/updates-applied.md` (will be cleaned up)

## Specific Update Guidelines

### Adding New Command Types

```yaml
### N. {Command Type Name}

{Brief description of purpose}

```yaml
- {command_type}:
    {field1}: {value}
    {field2}: {value}
```

**Fields:**
- `{field1}`: {description}
- `{field2}`: {description}

**Example:**
{Practical example with explanation}
```

### Updating Field Lists

When adding fields to existing command types:
- Add to existing field list in alphabetical order
- Mark as (optional) or (required)
- Include type in description
- Show example if non-obvious

### Deprecation Notices

```markdown
**Note:** The `old_field` syntax is deprecated. Use `new_field` instead:

```yaml
# ❌ Deprecated (still works but not recommended)
old_field: value

# ✅ Current syntax
new_field: value
```

*The old syntax will be removed in version X.Y*
```

### Complex Examples

For complex features:
1. Show simple example first
2. Show advanced example second
3. Explain each part
4. Link related features

## Important Guidelines

### Don't Over-Document
- Focus on user-facing features
- Skip internal implementation details
- Avoid redundant explanations
- Keep examples concise but complete

### Don't Break Existing
- Don't remove working examples unless deprecated
- Don't change section structure unnecessarily
- Don't alter working links or references
- Don't change table of contents unless needed

### Do Add Value
- Explain "why" not just "what"
- Show common use cases
- Highlight gotchas or limitations
- Link related concepts

### Do Maintain Consistency
- Follow existing example patterns
- Use same terminology throughout
- Match existing formatting style
- Keep technical level consistent

## Success Criteria

The updated documentation must:
1. ✓ Fix ALL issues identified in drift reports
2. ✓ Include ALL fields from struct definitions
3. ✓ Have working YAML examples (valid syntax)
4. ✓ Mark deprecated features clearly
5. ✓ Be accurate to current codebase
6. ✓ Remain clear and readable
7. ✓ Follow existing style conventions
8. ✓ Include version compatibility notes

## Edge Cases

### Multiple Formats Supported
When code supports multiple formats (untagged enum):
```yaml
# Format 1: Simple array
reduce:
  - shell: "command"

# Format 2: Full config
reduce:
  commands:
    - shell: "command"
```

### Optional with Defaults
When field has serde default:
```yaml
threshold: 100  # Optional, defaults to 100
```

### Complex Types
When field is HashMap or nested struct:
```yaml
capture_streams:
  stdout: true
  stderr: true
  exit_code: true
```

Show structure clearly with proper indentation.
