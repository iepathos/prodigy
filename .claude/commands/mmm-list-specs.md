# List Specs Command

Lists all unimplemented specifications by scanning the specs directory and parsing frontmatter metadata. This replaces the need for a manually maintained SPEC_INDEX.md file.

Arguments: $ARGUMENTS (optional filter like "testing", "foundation", etc.)

## Usage

```
/mmm-list-specs [filter]
```

Examples:
- `/mmm-list-specs` to list all unimplemented specs
- `/mmm-list-specs testing` to list only testing-related specs
- `/mmm-list-specs high` to list only high-priority specs

## What This Command Does

1. **Scans Specification Directory**
   - Scans the `specs/` directory for all `.md` files
   - Ignores `SPEC_INDEX.md` and any other non-spec files
   - Processes both permanent specs and temporary specs in `specs/temp/`

2. **Parses Frontmatter Metadata**
   - Extracts YAML frontmatter from each spec file
   - Collects: number, title, category, priority, status, dependencies, created date
   - Handles missing or malformed frontmatter gracefully

3. **Filters Results (if requested)**
   - Filters by category, priority, or status based on the provided argument
   - Case-insensitive matching for convenience
   - Partial matches supported (e.g., "test" matches "testing")

4. **Displays Organized List**
   - Shows specs sorted by number
   - Groups by category or priority if appropriate
   - Displays key information in a readable format

## Output Format

### Default Output (no filter)
```
Unimplemented Specifications:

Foundation:
  45 - Context Window Management (high priority)
      Dependencies: [44, 11]
      Created: 2024-01-15

Testing:  
  61 - Fix Ignored Tests with Proper Mocking (high priority)
      Dependencies: [57]
      Created: 2024-01-15

Optimization:
  65 - Cook Module Refactoring (high priority)
      Dependencies: none
      Created: 2024-01-15
      
  66 - Unified Scoring System (high priority)
      Dependencies: [46]
      Created: 2024-01-15

Parallel:
  67 - Worktree Cleanup After Merge (high priority)
      Dependencies: [24, 25, 26, 41]
      Created: 2024-01-15

Total: 5 specifications
```

### Filtered Output (e.g., /mmm-list-specs high)
```
High Priority Specifications:

45 - Context Window Management (foundation)
61 - Fix Ignored Tests with Proper Mocking (testing)
65 - Cook Module Refactoring (optimization)
66 - Unified Scoring System (optimization)
67 - Worktree Cleanup After Merge (parallel)

Total: 5 high priority specifications
```

## Implementation Details

### Frontmatter Format
```yaml
---
number: 68
title: Simplified Spec Management System
category: foundation
priority: high
status: draft
dependencies: []
created: 2024-01-15
---
```

### Parsing Logic
1. Read each .md file in specs/ directory
2. Extract content between `---` markers at file start
3. Parse YAML content into structured data
4. Skip files without valid frontmatter
5. Collect all valid specs into a list

### Sorting and Grouping
- Primary sort: by spec number (ascending)
- Secondary group: by category (when no filter)
- Alternative group: by priority (when filtering by priority)

### Error Handling
- Skip files that can't be read
- Skip files with invalid YAML
- Report parsing errors but continue processing
- Show warning if no specs found

## Benefits Over SPEC_INDEX.md

1. **Always Accurate**: Shows exactly what spec files exist
2. **Zero Maintenance**: No manual updates required
3. **Real-time**: Reflects current state of specs directory
4. **Simpler**: No index to get out of sync
5. **Flexible**: Easy to add new metadata fields

## Integration with Workflow

This command integrates with the simplified spec management system:
- Use `/mmm-list-specs` to see available specs
- Use `/mmm-implement-spec NUMBER` to implement a spec
- Spec file is automatically deleted after implementation
- No need to update any index files

## Future Enhancements

- Support for multiple filters (e.g., "high testing")
- Export to different formats (JSON, CSV)
- Show implementation history from git log
- Estimate implementation effort based on spec content