# MMM Merge Worktree Command

Intelligently merges MMM worktree branches with automatic conflict resolution.

## Variables

BRANCH_NAME: $ARGUMENTS (required - the worktree branch name to merge, optionally followed by --target <target-branch>)

## Usage

```
/mmm-merge-worktree <branch-name> [--target <target-branch>]
```

Examples:
- `/mmm-merge-worktree mmm-performance-1234567890` - Merge to current branch
- `/mmm-merge-worktree mmm-security-1234567891 --target main` - Merge to main branch

## What This Command Does

0. **Parse Arguments**
   - Extract branch name from BRANCH_NAME (first argument before any --flags)
   - Extract target branch if --target flag is present
   - If no branch name provided, list available worktrees and ask user to select

1. **Attempts Standard Merge**
   - Switches to target branch (or current branch if not specified)
   - Attempts `git merge --no-ff` to preserve commit history
   - If successful, creates merge commit and exits

2. **Handles Merge Conflicts**
   - Detects and analyzes all conflicted files
   - Understands the intent of changes from both branches
   - Resolves conflicts intelligently based on context
   - Preserves functionality from both branches where possible

3. **Applies Resolution**
   - Resolves conflict markers in all files
   - Stages resolved files
   - Creates detailed merge commit explaining resolutions

4. **Verifies Merge**
   - Runs basic validation (syntax checks, etc.)
   - Ensures no conflict markers remain
   - Commits the merge with comprehensive message

## Conflict Resolution Strategy

### Priority Order
1. **Functionality**: Ensure code remains functional
2. **Latest Intent**: Prefer changes that represent newest understanding
3. **Completeness**: Include additions from both branches
4. **Safety**: When uncertain, preserve both versions with clear separation

### Resolution Patterns

**Function/Method Conflicts**:
- If same function modified differently, analyze which version is more complete
- Merge beneficial changes from both when possible
- Preserve all test additions

**Import/Dependency Conflicts**:
- Combine imports from both branches
- Remove duplicates
- Maintain correct ordering

**Documentation Conflicts**:
- Merge documentation additions
- Prefer more comprehensive explanations
- Combine examples from both branches

**New File Conflicts**:
- If same filename but different content, rename one with branch suffix
- Alert in merge commit about the rename

**Deletion Conflicts**:
- If deleted in one branch but modified in another, prefer modification
- Document the decision in merge commit

## Merge Commit Format

```
Merge worktree '<branch-name>' into <target>

Successfully merged with <N> conflicts resolved:

Resolved Conflicts:
- path/to/file1.rs: Combined performance improvements with security fixes
- path/to/file2.py: Merged test additions from both branches
- path/to/file3.md: Combined documentation updates

Resolution Strategy:
<Brief explanation of how conflicts were resolved>

Original commits from worktree:
<List of commits being merged>
```

## Error Handling

**If arguments are invalid**:
1. Check if BRANCH_NAME is provided in $ARGUMENTS
2. If missing and MMM_AUTOMATION=true, fail with: "Error: Branch name required for automated merge"
3. If missing and interactive, list available worktrees for selection

**If merge cannot be completed**:
1. Abort the merge to maintain clean state
2. Provide clear error message with:
   - Which files have unresolvable conflicts
   - Why they couldn't be resolved automatically
   - Suggested manual steps

**Common unresolvable scenarios**:
- Binary file conflicts
- Fundamental architectural conflicts
- Mutually exclusive changes

## Best Practices

1. **Always verify** the target branch is correct before merging
2. **Run tests** after merge to ensure functionality
3. **Review** the merge commit to understand what was merged
4. **Clean up** the worktree after successful merge

## Example Workflow

```bash
# Check what needs merging
$ mmm worktree list
Active MMM worktrees:
  mmm-performance-1234567890 - /path/to/.mmm/worktrees/... (focus: performance)
  mmm-security-1234567891 - /path/to/.mmm/worktrees/... (focus: security)

# Merge first worktree
$ claude /mmm-merge-worktree mmm-performance-1234567890
Attempting merge...
Found 2 conflicts in:
  - src/main.rs
  - src/lib.rs
Resolving conflicts...
✓ src/main.rs: Combined performance optimization with existing structure
✓ src/lib.rs: Merged both import additions
Creating merge commit...
✓ Successfully merged 'mmm-performance-1234567890' into master

# Merge second worktree (may have conflicts with first merge)
$ claude /mmm-merge-worktree mmm-security-1234567891
Attempting merge...
Found 3 conflicts...
<resolution details>
✓ Successfully merged 'mmm-security-1234567891' into master
```

## Automation Support

When `MMM_AUTOMATION=true` is set:
- The command must parse BRANCH_NAME from $ARGUMENTS
- No interactive prompts should be shown
- If branch name is missing or invalid, fail with clear error message
- Example: `$ARGUMENTS = "mmm-test-coverage-123 --target main"` should extract:
  - Branch: mmm-test-coverage-123
  - Target: main

## Notes

- This command requires git 2.5+ for worktree support
- Always backs up current state before attempting merge
- Preserves full git history from worktree branches
- Can be run multiple times safely (idempotent)