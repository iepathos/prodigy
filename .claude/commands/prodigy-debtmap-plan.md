---
name: debtmap-plan
description: Analyze tech debt and create a phased implementation plan
---

# Create Implementation Plan for Top Tech Debt Item

Analyze the debtmap output and create a detailed, phased implementation plan for fixing the highest priority technical debt item.

## Process

### Step 1: Load Debtmap Analysis

Read the debtmap analysis that the workflow has already generated:

```bash
cat .prodigy/debtmap-before.json
```

Extract the top priority item:

```bash
jq '.items[0] | {
  score: .unified_score.final_score,
  location: .location,
  debt_type: .debt_type,
  action: .recommendation.primary_action,
  rationale: .recommendation.rationale,
  implementation_steps: .recommendation.implementation_steps,
  expected_impact: .expected_impact,
  coverage_info: .transitive_coverage,
  complexity: .cyclomatic_complexity
}' .prodigy/debtmap-before.json
```

### Step 2: Analyze the Problem

Based on the debt type, understand what needs to be fixed:

**For GOD OBJECT / High Complexity:**
- Identify the distinct responsibilities in the code
- Look for patterns that can be extracted
- Consider functional programming refactoring approaches
- Don't just move code - reduce actual complexity

**For LOW COVERAGE:**
- Determine if it's orchestration/I/O code (should extract pure logic)
- Or business logic (needs direct testing)
- Identify edge cases and scenarios to test

**For CODE DUPLICATION:**
- Find all instances of the duplicated code
- Determine the best abstraction pattern
- Plan extraction to a shared module

### Step 3: Read the Target Code

Read the file(s) identified in the debtmap output to understand the current implementation:

```bash
# Read the target file
cat <file_path_from_debtmap>
```

Analyze:
- Current structure and responsibilities
- Dependencies and coupling
- Complexity sources
- Testing gaps

### Step 4: Create the Implementation Plan

Create a file at `.prodigy/IMPLEMENTATION_PLAN.md` with this structure:

```markdown
# Implementation Plan: <Brief Description>

## Problem Summary

**Location**: <file:line from debtmap>
**Priority Score**: <unified_score from debtmap>
**Debt Type**: <debt_type from debtmap>
**Current Metrics**:
- Lines of Code: <from debtmap>
- Functions: <from debtmap>
- Cyclomatic Complexity: <from debtmap>
- Coverage: <from debtmap>

**Issue**: <primary_action and rationale from debtmap>

## Target State

**Expected Impact** (from debtmap):
- Complexity Reduction: <expected_impact.complexity_reduction>
- Coverage Improvement: <expected_impact.coverage_improvement>
- Risk Reduction: <expected_impact.risk_reduction>

**Success Criteria**:
- [ ] <Specific, measurable criteria>
- [ ] All existing tests continue to pass
- [ ] No clippy warnings
- [ ] Proper formatting

## Implementation Phases

Break the work into 3-5 incremental phases. Each phase should:
- Be independently testable
- Result in working, committed code
- Build on the previous phase

### Phase 1: <Name>

**Goal**: <What this phase accomplishes>

**Changes**:
- <Specific change 1>
- <Specific change 2>

**Testing**:
- <How to verify this phase works>

**Success Criteria**:
- [ ] <Specific criteria for this phase>
- [ ] All tests pass
- [ ] Ready to commit

### Phase 2: <Name>

**Goal**: <What this phase accomplishes>

**Changes**:
- <Specific change 1>
- <Specific change 2>

**Testing**:
- <How to verify this phase works>

**Success Criteria**:
- [ ] <Specific criteria for this phase>
- [ ] All tests pass
- [ ] Ready to commit

### Phase 3: <Name>

[Continue for each phase...]

## Testing Strategy

**For each phase**:
1. Run `cargo test --lib` to verify existing tests pass
2. Run `cargo clippy` to check for warnings
3. [Any phase-specific testing]

**Final verification**:
1. `just ci` - Full CI checks
2. `cargo tarpaulin` - Regenerate coverage
3. `debtmap analyze` - Verify improvement

## Rollback Plan

If a phase fails:
1. Revert the phase with `git reset --hard HEAD~1`
2. Review the failure
3. Adjust the plan
4. Retry

## Notes

<Any additional context, gotchas, or considerations>
```

### Step 5: Validate the Plan

Before writing the plan, ensure:

1. **Each phase is independently valuable**
   - Can commit after each phase
   - Tests pass after each phase
   - Code works after each phase

2. **Phases are ordered correctly**
   - Earlier phases don't depend on later ones
   - Build complexity gradually
   - Test coverage increases progressively

3. **Plan is realistic**
   - Not trying to fix everything at once
   - Focused on the specific debt item
   - Achievable in 3-5 phases

4. **Success criteria are measurable**
   - Can objectively verify completion
   - Aligned with debtmap metrics
   - Include test coverage targets

### Step 6: Write the Plan

Write the complete implementation plan to `.prodigy/IMPLEMENTATION_PLAN.md`:

```bash
# Example of writing the plan
cat > .prodigy/IMPLEMENTATION_PLAN.md << 'EOF'
# Implementation Plan: Extract Pure Functions from WorkflowExecutor

## Problem Summary
[...]
EOF
```

### Step 7: Output Summary

After creating the plan, output a brief summary:

```
Created implementation plan at .prodigy/IMPLEMENTATION_PLAN.md

Target: <file:line>
Priority: <score>
Phases: <number>

Next step: Run `/prodigy-debtmap-implement` to execute the plan
```

## Important Guidelines

### For God Object Refactoring:

**DO:**
- Extract pure functions that can be unit tested
- Separate I/O from business logic
- Create focused modules with single responsibilities
- Use functional programming patterns
- Keep changes incremental (10-20 functions per phase)

**DON'T:**
- Try to refactor everything at once
- Create helper methods only used in tests
- Break up legitimate patterns (match/visitor)
- Add complexity to reduce complexity
- Skip testing between phases

### For Coverage Improvements:

**DO:**
- Extract pure logic from I/O code first
- Test the extracted pure functions
- Cover edge cases and error conditions
- Follow existing test patterns

**DON'T:**
- Force tests on orchestration code
- Test implementation details
- Add tests without understanding the code
- Ignore test failures

### Plan Quality Checklist:

- [ ] Problem clearly identified from debtmap output
- [ ] Target state is specific and measurable
- [ ] 3-5 phases, each independently valuable
- [ ] Each phase has clear success criteria
- [ ] Testing strategy defined
- [ ] Rollback plan included
- [ ] Plan is realistic and achievable

## Output Format

The plan MUST be written to `.prodigy/IMPLEMENTATION_PLAN.md` in the format specified above.

The workflow will pass this plan to the implementation command.
