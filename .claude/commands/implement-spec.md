# Implement Spec Command

Implements a Git Good specification by reading the spec file, executing the implementation, and updating .mmm context files.  Read the files in .mmm to get general project context.

## Usage

```
/implement-spec <spec-number>
```

Example: `/implement-spec 01` to implement the project structure specification.

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
   - Locates the spec file based on the provided number.  Specs are located in the specs/ subdirectory with additional subdirectories organizing them.
   - Spec documents start with the given identifier like 01-some-spec.md or 08a-some-other-spec.md.
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
- Look up the spec number in SPEC_INDEX.md
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

Update .mmm files:
- **PROJECT.md**: Update "Current State" percentage and "What Exists"
- **ARCHITECTURE.md**: Add architectural details for new components
- **ROADMAP.md**: Mark spec as completed, update progress
- **DECISIONS.md**: Add ADRs for significant implementation choices
- **CONVENTIONS.md**: Document any new patterns discovered

### Step 5: Validation and Commit

Final steps:
- Run `cargo fmt` and `cargo clippy`
- Run `cargo test` if tests exist
- Create git commit with format: "feat: implement spec {number} - {title}"

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
/implement-spec 01
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
