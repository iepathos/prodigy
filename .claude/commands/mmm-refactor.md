# Refactor Command

Refactors code to improve structure, readability, and maintainability without changing functionality.

## Usage

```
/mmm-refactor [focus-area]
```

Examples:
- `/mmm-refactor` - General refactoring based on code smells
- `/mmm-refactor modules` - Focus on module organization
- `/mmm-refactor error-handling` - Improve error handling patterns
- `/mmm-refactor naming` - Improve variable and function names

## What This Command Does

1. **Analyzes Code Structure**
   - Identifies code smells and anti-patterns
   - Detects duplicated code
   - Finds overly complex functions
   - Identifies poor naming conventions

2. **Creates Refactoring Spec**
   - Generates a temporary spec in specs/temp/
   - Lists specific refactoring tasks
   - Preserves all functionality
   - Focuses on code quality improvements

3. **Commits the Spec**
   - Creates a git commit with the refactoring plan
   - Includes spec ID in commit message
   - Ready for /mmm-implement-spec

## Focus Areas

- **modules**: Reorganize module structure for better separation of concerns
- **error-handling**: Standardize error handling patterns
- **naming**: Improve variable, function, and type names
- **duplication**: Remove duplicate code through abstraction
- **complexity**: Break down complex functions
- **types**: Improve type safety and clarity
- **tests**: Refactor test organization and helpers

## Output Format

The command generates a spec file and commits it:

```
review: generate refactoring spec for {focus-area} refactor-{timestamp}
```

## Automation Mode

When MMM_AUTOMATION=true:
- Automatically commits the spec
- No interactive confirmation
- Returns spec ID for pipeline integration