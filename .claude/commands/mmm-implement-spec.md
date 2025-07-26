# Implement Spec Command

Implements a Git Good specification by reading the spec file, executing the implementation, and updating .mmm context files.  Read the files in .mmm to get general project context.

## Usage

```
/mmm-implement-spec <spec-identifier>
```

Examples: 
- `/mmm-implement-spec 01` to implement the project structure specification
- `/mmm-implement-spec iteration-1234567890-improvements` to implement a temporary improvement spec

## What This Command Does

1. **Reads the Project Context**
   - Read the .mmm context files to get general understanding of the project.
   - Files are read in this order to build context:
     - PROJECT.md (current state and capabilities)
     - ARCHITECTURE.md (system design)
     - CONVENTIONS.md (coding standards)
     - ROADMAP.md (progress tracking)
     - DECISIONS.md (technical decisions)
2. **Reads the Specification**
   - Locates the spec file based on the provided identifier
   - **Permanent specs**: Located in specs/ subdirectory (e.g., 01-some-spec.md)
   - **Temporary specs**: Located in specs/temp/ (e.g., iteration-1234567890-improvements.md)
   - Parses the specification content and requirements
   - Identifies implementation tasks and success criteria

3. **Implements the Specification**
   - Creates necessary files and directories
   - Writes implementation code according to the spec
   - Follows conventions defined in CONVENTIONS.md
   - Ensures all success criteria are met

4. **Updates Context Files**
   - Updates PROJECT.md with new capabilities and current state
   - Updates ARCHITECTURE.md with implementation details
   - Updates ROADMAP.md to mark spec as completed
   - Adds new decisions to DECISIONS.md if needed
   - Documents any new conventions in CONVENTIONS.md

5. **Validates Implementation**
   - Runs tests if applicable
   - Runs lint checks
   - Verifies success criteria from the spec

6. **Commits Changes**
   - Creates a git commit with appropriate message
   - Follows commit message format from CONVENTIONS.md

## Execution Process

### Step 1: Read Context Files and Locate Specification

The command will:
- First read all .mmm context files in order (PROJECT.md, ARCHITECTURE.md, CONVENTIONS.md, ROADMAP.md, DECISIONS.md)
- Build comprehensive understanding of project state and conventions
- Locate specification file:
  - **Numeric IDs** (e.g., "01", "08a"): Look up in SPEC_INDEX.md and find in specs/ subdirectory
  - **Iteration IDs** (e.g., "iteration-1234567890-improvements"): Find directly in specs/temp/
- Read the corresponding spec file
- Extract implementation requirements and success criteria

### Step 2: Analyze Current State

Before implementing:
- Review current codebase structure
- Check for existing related code
- Identify dependencies and prerequisites

### Step 3: Implementation

Based on the spec type:
- **Foundation specs**: Create core structures and modules
- **Parallel specs**: Implement concurrent processing features
- **Storage specs**: Add storage optimization features
- **Compatibility specs**: Ensure Git compatibility
- **Testing specs**: Create test suites and benchmarks
- **Optimization specs**: Improve performance

### Step 4: Context Updates

Update .mmm files (skip for temporary iteration specs):
- **Permanent specs only**:
  - **PROJECT.md**: Update "Current State" percentage and "What Exists"
  - **ARCHITECTURE.md**: Add architectural details for new components
  - **ROADMAP.md**: Mark spec as completed, update progress
  - **DECISIONS.md**: Add ADRs for significant implementation choices
  - **CONVENTIONS.md**: Document any new patterns discovered
- **Temporary specs**: Skip context updates, focus on implementing fixes

### Step 5: Validation and Commit

Final steps:
- Run `cargo fmt` and `cargo clippy`
- Run `cargo test` if tests exist
- **Report modified files** (for automation tracking):
  - List all files that were created, modified, or deleted
  - Include brief description of changes made
  - Format: "Modified: src/main.rs", "Created: tests/new_test.rs"
- Create git commit:
  - **Permanent specs**: "feat: implement spec {number} - {title}"
  - **Temporary specs**: "fix: apply automated improvements from iteration {timestamp}"

## Implementation Guidelines

1. **Follow Existing Patterns**
   - Use the module organization from ARCHITECTURE.md
   - Follow naming conventions from CONVENTIONS.md
   - Maintain consistency with existing code

2. **Incremental Progress**
   - Implement specs in order when possible
   - Ensure each spec builds on previous work
   - Don't skip prerequisites

3. **Documentation**
   - Add inline documentation for new code
   - Update module-level documentation
   - Keep .mmm files current

4. **Testing**
   - Add unit tests for new functionality
   - Create integration tests where applicable
   - Ensure existing tests still pass

## Error Handling

The command will:
- Fail gracefully if spec doesn't exist
- Report validation failures clearly
- Rollback changes if tests fail
- Provide helpful error messages

## Example Workflow

```
/mmm-implement-spec 01
```

This would:
1. Read `specs/foundation/01-project-structure.md`
2. Create the Rust workspace structure
3. Set up Cargo.toml files
4. Create directory layout
5. Update PROJECT.md to show project structure is complete
6. Update ROADMAP.md to mark spec 01 as done
7. Run cargo fmt and clippy
8. Commit: "feat: implement spec 01 - project structure and build system"
