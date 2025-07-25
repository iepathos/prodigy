# Parallel Implement Specs Command

Implements multiple Git Good specifications concurrently using git worktrees for isolation, with each spec handled by a separate agent instance.

## Usage

```
/parallel-implement-specs <spec-numbers>
```

Examples:
- `/parallel-implement-specs 01,02,03` - Implement specs 01, 02, and 03 concurrently
- `/parallel-implement-specs 01-05` - Implement specs 01 through 05 concurrently
- `/parallel-implement-specs 01,03,05-07` - Mixed individual and range syntax

## What This Command Does

1. **Analyzes Specification Dependencies**
   - Reads all specified spec files
   - Identifies inter-spec dependencies
   - Creates dependency graph for proper ordering
   - Groups independent specs for parallel execution

2. **Creates Isolated Worktrees**
   - Creates separate git worktrees for each spec group
   - Names worktrees as `spec-{numbers}` (e.g., `spec-01-02-03`)
   - Ensures clean isolation between concurrent implementations

3. **Spawns Parallel Agents**
   - Launches separate Task agents for each worktree
   - Each agent implements one or more non-conflicting specs
   - Agents work independently with full tool access

4. **Coordinates Implementation**
   - Manages dependency ordering across worktrees
   - Handles cross-spec communication when needed
   - Monitors progress and collects results

5. **Merges Results**
   - Validates all implementations
   - Merges completed worktrees back to main branch
   - Updates .eidolon context files with combined results
   - Creates unified commit with all changes

## Worktree Strategy

### Naming Convention
- Single spec: `spec-{number}` (e.g., `spec-01`)
- Multiple specs: `spec-{start}-{end}` (e.g., `spec-01-03`)
- Complex groups: `spec-group-{id}` (e.g., `spec-group-1`)

### Directory Structure
```
gitgood/
├── .git/
├── spec-01/          # Worktree for spec 01
├── spec-02-03/       # Worktree for specs 02 and 03
├── spec-group-1/     # Worktree for dependency group
└── main/             # Original working directory
```

## Agent Coordination

### Independent Groups
Specs with no dependencies run in parallel:
```
Agent 1: spec-01 (project structure)
Agent 2: spec-02 (CLI framework) 
Agent 3: spec-03 (error handling)
```

### Dependent Groups
Specs with dependencies run in sequence:
```
Phase 1: Agent 1 implements spec-01
Phase 2: Agent 2 implements spec-02 (depends on 01)
Phase 3: Agent 3 implements spec-03 (depends on 02)
```

### Mixed Dependencies
Complex dependency graphs use multiple phases:
```
Phase 1: Agents 1,2 implement specs 01,04 (independent)
Phase 2: Agent 3 implements spec 02 (depends on 01)
Phase 3: Agent 4 implements spec 03 (depends on 02,04)
```

## Implementation Process

### Step 1: Dependency Analysis
```bash
# Analyze all spec files for dependencies
# Create dependency graph
# Group specs into execution phases
```

### Step 2: Worktree Creation
```bash
# For each group in parallel:
git worktree add spec-{group} HEAD
cd spec-{group}
git checkout -b implement-spec-{group}
```

### Step 3: Parallel Execution
For each worktree group, spawn a Task agent with prompt:
```
You are implementing Git Good specifications {spec-list} in isolation.

Your worktree is at: {worktree-path}
Specs to implement: {spec-numbers}
Dependencies completed: {completed-deps}

Execute the following for each spec:
1. Read spec file from specs/ directory
2. Implement according to specification
3. Update .eidolon context files
4. Run validation tests
5. Create commit for this spec

When all specs in your group are complete, report back with:
- Implementation status
- Files created/modified  
- Test results
- Any issues encountered
```

### Step 4: Progress Monitoring
- Monitor each agent's progress
- Handle failures gracefully
- Coordinate dependency handoffs
- Collect completion reports

### Step 5: Integration and Cleanup

#### Phase-Based Merging
Merge worktrees in dependency order:

```bash
# Phase 1: Merge independent specs first
cd main
git merge spec-01 spec-04  # Independent specs merge cleanly

# Phase 2: Merge specs that depend on Phase 1  
git merge spec-02  # Depends on spec-01, now available

# Phase 3: Merge remaining dependent specs
git merge spec-03-05  # Depends on spec-02, now available
```

#### Conflict Resolution Strategy
When merge conflicts occur:

1. **Structural Conflicts** (different files):
   - Auto-merge since specs work on different components
   - Validate no duplicate functionality

2. **Context File Conflicts** (.eidolon files):
   - Combine PROJECT.md capabilities lists
   - Merge ARCHITECTURE.md component descriptions  
   - Consolidate ROADMAP.md progress updates
   - Append all DECISIONS.md entries

3. **Code Conflicts** (same files):
   - Detect early via pre-merge analysis
   - Fall back to sequential implementation
   - Manual resolution with user guidance

#### Context File Reconciliation
Special handling for .eidolon files:

```bash
# Merge PROJECT.md - combine capabilities
# Merge ARCHITECTURE.md - combine component docs
# Merge ROADMAP.md - consolidate progress  
# Merge DECISIONS.md - append all entries
# Merge CONVENTIONS.md - combine new patterns
```

#### Final Integration
```bash
# Run comprehensive validation
cargo build --workspace
cargo test --workspace  
cargo fmt && cargo clippy

# Create unified commit
git commit -m "feat: implement specs {all-numbers} in parallel

Implemented specifications:
- Spec 01: {title}
- Spec 02: {title} 
- Spec 03: {title}"

# Clean up worktrees
git worktree remove spec-01
git worktree remove spec-02-03
```

## Error Handling

### Agent Failures
- Retry failed specs in fresh worktrees
- Continue with independent specs
- Report failures clearly

### Dependency Conflicts
- Detect merge conflicts early
- Provide resolution guidance
- Fall back to sequential implementation

### Resource Limits
- Limit concurrent agents (default: 4)
- Queue excess specs for later phases
- Monitor system resources

## Validation

### Per-Worktree Validation
Each agent runs:
```bash
cargo fmt -- --check
cargo clippy -- -D warnings  
cargo test --workspace
```

### Integration Validation
After merging:
```bash
cargo build --workspace
cargo test --workspace --release
cargo bench --no-run
```

### Consistency Checks
- Verify .eidolon files are consistent
- Check for duplicate implementations
- Validate dependency satisfaction

## Example Workflow

```
/parallel-implement-specs 01,02,03,04,05
```

This would:
1. Analyze dependencies between specs 01-05
2. Create dependency groups (e.g., [01,04], [02], [03,05])  
3. Create worktrees: `spec-01-04`, `spec-02`, `spec-03-05`
4. Launch 3 agents in parallel
5. Agent 1 implements specs 01+04 independently
6. Agent 2 waits for spec 01, then implements spec 02
7. Agent 3 waits for spec 02, then implements specs 03+05
8. Merge all results and create unified commit

## Performance Benefits

- **Reduced Total Time**: Independent specs run concurrently
- **Clean Isolation**: No conflicts between implementations  
- **Parallel Testing**: Each worktree runs tests independently
- **Efficient Resource Use**: Optimal agent utilization

## Limitations

- Requires careful dependency analysis
- Higher memory usage (multiple worktrees)
- Complex coordination logic
- Not suitable for tightly coupled specs

## Configuration

Environment variables:
- `MAX_PARALLEL_AGENTS=4` - Maximum concurrent agents
- `WORKTREE_PREFIX=spec-` - Worktree naming prefix  
- `CLEANUP_ON_SUCCESS=true` - Remove worktrees after merge
- `VALIDATE_BEFORE_MERGE=true` - Run full validation before merge
