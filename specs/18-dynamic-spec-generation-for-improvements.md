# Spec 18: Dynamic Spec Generation for Code Improvements

## Overview
Bridge the gap between `/mmm-code-review` and `/mmm-implement-spec` by having the review command generate temporary specification files that the implement command can consume.

## Current Problem
The improvement loop has a mismatch:
- `/mmm-code-review` generates dynamic runtime issues  
- `/mmm-implement-spec` expects static spec numbers
- No clean way to pass review results to implementation

## Solution
Modify `/mmm-code-review` to generate temporary specification files for the issues it finds, then `/mmm-implement-spec` can consume these specs normally.

## Implementation Flow

### 1. Enhanced /mmm-code-review
The review command will:
1. Perform normal code analysis
2. **Generate a temporary spec file** for issues found
3. **Return the spec identifier** in JSON output

#### Temporary Spec Generation
```
specs/temp/iteration-{timestamp}-improvements.md
```

Example generated spec:
```markdown
# Iteration 1: Code Quality Improvements

## Overview
Temporary specification for code improvements identified in automated review.

## Issues to Address

### 1. Fix Error Handling in src/main.rs:42
**Severity**: High
**Category**: Error Handling
**Description**: Replace `.unwrap()` with proper error handling
**File**: src/main.rs
**Line**: 42

#### Current Code:
```rust
let config = load_config().unwrap();
```

#### Required Change:
```rust
let config = load_config().context("Failed to load configuration")?;
```

### 2. Add Unit Tests for Database Module  
**Severity**: Medium
**Category**: Testing
**Description**: Database functions lack test coverage
**File**: src/database.rs

#### Implementation:
Create `tests/database_tests.rs` with:
- Connection handling tests
- Error condition tests  
- Data validation tests

### 3. Optimize Loop in Parser
**Severity**: Low
**Category**: Performance
**File**: src/parser.rs
**Line Range**: 23-35

#### Current Code:
```rust
let mut results = Vec::new();
for item in items {
    if item.is_valid() {
        results.push(item.process());
    }
}
```

#### Required Change:
```rust
let results: Vec<_> = items
    .into_iter()
    .filter(|item| item.is_valid())
    .map(|item| item.process())
    .collect();
```

## Success Criteria
- [ ] Error handling fixed in src/main.rs
- [ ] Unit tests added for database module
- [ ] Parser loop optimized with iterators
- [ ] All files compile without warnings
- [ ] Tests pass
```

#### Enhanced JSON Output
```json
{
  "mmm_structured_output": {
    "review_id": "review-2025-01-26-uuid",
    "overall_score": 7.2,
    "generated_spec": "iteration-1234567890-improvements",
    "temp_spec_path": "specs/temp/iteration-1234567890-improvements.md",
    "actions": [
      {
        "id": "action_001",
        "type": "fix_error",
        "severity": "high",
        "file": "src/main.rs",
        "line": 42,
        "description": "Replace unwrap with proper error handling"
      }
    ],
    "summary": {
      "total_issues": 3,
      "critical": 0,
      "high": 1,
      "medium": 1,
      "low": 1
    }
  }
}
```

### 2. Updated Improvement Loop Logic

```rust
async fn run_improvement_iteration(&mut self) -> Result<bool> {
    // 1. Call Claude CLI for review
    let review = self.call_claude_review().await?;
    
    if review.issues.is_empty() {
        return Ok(false); // No more improvements needed
    }
    
    // 2. Extract generated spec identifier from review JSON
    let spec_id = review.generated_spec.ok_or_else(|| 
        anyhow::anyhow!("Review did not generate improvement spec"))?;
    
    // 3. Call implement-spec with the generated spec
    let implementation = self.call_claude_implement_spec(&spec_id).await?;
    
    // 4. Parse file changes from implementation output
    let changes = self.parse_file_changes(&implementation)?;
    
    // 5. Re-analyze project
    let new_score = self.reanalyze_project().await?;
    
    Ok(!changes.is_empty())
}

async fn call_claude_implement_spec(&self, spec_id: &str) -> Result<String> {
    let cmd = Command::new("claude")
        .arg("--dangerously-skip-permissions") 
        .arg("/mmm-implement-spec")
        .arg(spec_id)  // Pass the generated spec ID
        .env("MMM_AUTOMATION", "true")
        .output()
        .await?;
        
    // ... handle output
}
```

### 3. No Changes to /mmm-implement-spec
The implement command continues to work exactly as before:
- Reads spec files from `specs/` directory
- Follows the same implementation pattern
- Updates context files normally
- The only difference is it may be reading a temporary spec

### 4. Temporary Spec Cleanup
After successful implementation:
- Keep temp specs for debugging/audit trail
- Optionally clean up specs older than N days
- Or move completed specs to `specs/completed/` directory

## Benefits

### 1. Clean Architecture
- Maintains separation between review and implementation
- Uses existing command structure
- No complex parameter passing

### 2. Debuggable
- Temporary specs are human-readable
- Can inspect what the review command generated
- Can manually run implement-spec for debugging

### 3. Flexible
- Review can generate complex, multi-step specs
- Can include context, code examples, success criteria
- Implement command gets full specification context

### 4. Auditable
- Complete paper trail of what was reviewed and implemented
- Specs can be version controlled
- Easy to understand what happened in each iteration

## Implementation Steps

### Phase 1: Modify /mmm-code-review
1. Add spec generation logic after analysis
2. Create `specs/temp/` directory structure
3. Generate markdown specs with specific fix instructions
4. Include spec ID in JSON output

### Phase 2: Update Improvement Loop
1. Parse generated spec ID from review JSON  
2. Call `/mmm-implement-spec` with spec ID
3. Handle case where no spec is generated (no issues)

### Phase 3: Cleanup and Optimization
1. Add temp spec cleanup logic
2. Improve spec generation quality
3. Add error handling for spec generation failures

## Success Criteria
- Review command generates actionable specs
- Implement command successfully processes generated specs  
- Improvement loop works end-to-end
- File changes are applied correctly
- Temporary specs are human-readable and useful

This approach leverages the existing command architecture while creating a clean information flow between review and implementation phases.