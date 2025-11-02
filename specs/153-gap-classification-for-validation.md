---
number: 153
title: Gap Classification System for Validation
category: testing
priority: high
status: draft
dependencies: []
created: 2025-10-29
---

# Specification 153: Gap Classification System for Validation

**Category**: testing
**Priority**: high
**Status**: draft
**Dependencies**: None

## Context

Prodigy workflows currently fail when validation is incomplete (<100%), even when the missing items are deferred/optional requirements rather than critical functionality. This creates a rigid system that doesn't distinguish between:

1. **Critical gaps**: Core functionality missing
2. **Important gaps**: Tests or documentation missing
3. **Optional gaps**: Deferred validation tests, nice-to-have features

**Real-world failure case (spec 137)**:
- Validation returned 91.7% complete
- Gap: "Integration test for ripgrep standard.rs" (explicitly deferred in spec)
- Recovery command (`/prodigy-complete-spec`) correctly determined no fixes needed
- Workflow failed with: "commit_required=true but no commits were created"
- **Root cause**: Workflow assumed incomplete validation always requires code changes

**Problems with current system**:
1. No way to mark gaps as "required" vs "optional/deferred"
2. Workflow always fails if validation <100%
3. `commit_required: true` assumes all gaps need code changes
4. No distinction between functional completeness and perfection
5. Forces implementation of deferred items or manual workflow overrides

## Objective

Implement a gap classification system that distinguishes between required, important, and optional gaps, allowing workflows to proceed when core functionality is complete while flagging legitimate implementation issues.

## Requirements

### Functional Requirements

1. **Gap Classification Schema**
   - Add `requirement_type` field to gap data: "required" | "important" | "optional"
   - Allow validation to specify which gaps block workflow completion
   - Enable workflows to filter gaps by requirement type
   - Support backward compatibility with existing gap structures

2. **Enhanced Validation Output**
   - Include `required_gaps` array with only blocking gaps
   - Separate `optional_gaps` for informational purposes
   - Add `functional_completion_percentage` (ignoring optional gaps)
   - Maintain existing `completion_percentage` for full scoring

3. **Workflow Threshold Options**
   - Support `functional_threshold` (defaults to 100% of required items)
   - Support `total_threshold` (defaults to 100% of all items)
   - Allow workflows to choose which threshold to enforce
   - Enable hybrid approach: require 100% functional, allow 80% total

4. **Smart Commit Detection**
   - Workflow only requires commit if `required_gaps` exist
   - Allow no-commit completion if only optional gaps remain
   - Provide clear messaging about what gaps remain
   - Support manual override for edge cases

5. **Gap Classification in Commands**
   - Update `/prodigy-validate-spec` to classify gaps by requirement type
   - Update `/prodigy-complete-spec` to prioritize required gaps
   - Add `--require-all` flag to treat optional gaps as required
   - Add `--functional-only` flag to ignore optional gaps

### Non-Functional Requirements

1. **Backward Compatibility**: Existing workflows continue to work without modification
2. **Clear Semantics**: Gap classification is intuitive and well-documented
3. **Flexibility**: Support different rigor levels for different spec types
4. **Transparency**: Users understand why workflows pass or fail
5. **Maintainability**: Gap classification rules are easy to update

## Acceptance Criteria

- [ ] Gap data structure includes `requirement_type` field
- [ ] Validation output includes both `functional_completion_percentage` and `completion_percentage`
- [ ] Validation separates `required_gaps` from `optional_gaps`
- [ ] Workflows support `functional_threshold` configuration option
- [ ] `/prodigy-validate-spec` classifies gaps based on spec requirements
- [ ] `/prodigy-complete-spec` prioritizes required gaps over optional ones
- [ ] Workflow `commit_required` is conditional on `required_gaps` being non-empty
- [ ] Spec 137 scenario passes validation with 100% functional completion
- [ ] Workflows can enforce different thresholds for different spec categories
- [ ] Documentation explains gap classification system
- [ ] Backward compatibility maintained for existing workflows
- [ ] Integration tests verify gap classification behavior

## Technical Details

### Implementation Architecture

**System Overview**: Gap classification extends the existing Prodigy workflow validation system without modifying the workflow engine itself. The implementation adds:

1. **Rust code** in debtmap binary for gap classification logic and data structures
2. **JSON-based communication** between slash commands and Rust validation code
3. **Enhanced slash commands** that orchestrate classification and threshold evaluation

**Component Boundary**:
- **Rust Implementation** (`debtmap` binary): Core gap classification algorithm, data structures, threshold calculations
- **Slash Commands** (`.claude/commands/*.md`): Workflow orchestration, JSON parsing, decision logic
- **Workflow Configuration** (`workflows/implement.yml`): Threshold settings, validation triggers

### Implementation Locations

**New Rust Modules**:
- `src/validation/mod.rs` - Core validation types and result structures
- `src/validation/gap_classifier.rs` - Gap classification algorithm
- `src/validation/spec_parser.rs` - Parse spec frontmatter for requirement metadata
- `src/validation/threshold.rs` - Threshold evaluation logic

**Modified Slash Commands**:
- `.claude/commands/prodigy-validate-spec.md` - Invoke Rust validation, classify gaps, output enhanced JSON
- `.claude/commands/prodigy-complete-spec.md` - Filter gaps by priority, fix only required/important gaps

**Modified Workflow Configuration**:
- `workflows/implement.yml` - Add optional threshold configuration

**Note**: No changes to Prodigy workflow engine itself - all enhancements work through existing extension points.

### Implementation Approach

#### 1. Enhanced Gap Data Structure

```json
{
  "gap_id": {
    "description": "No integration test validating god object analysis on ripgrep standard.rs",
    "location": "tests/ directory",
    "severity": "medium",
    "requirement_type": "optional",
    "suggested_fix": "This test was deferred as it validates quality, not functionality",
    "rationale": "Spec explicitly noted this test was 'deferred (validation test)'"
  }
}
```

**Requirement types**:
- **`required`**: Core functionality, must be implemented for spec to be complete
- **`important`**: Tests, documentation, error handling - should be implemented but not blocking
- **`optional`**: Deferred items, nice-to-have features, quality validation tests

#### 2. Enhanced Validation Output Format

```json
{
  "completion_percentage": 91.7,
  "functional_completion_percentage": 100.0,
  "status": "functionally_complete",
  "implemented": [
    "Call pattern-based responsibility naming",
    "Interface size estimation",
    "Unit tests for both features"
  ],
  "required_gaps": [],
  "important_gaps": [],
  "optional_gaps": [
    "Integration test for ripgrep standard.rs"
  ],
  "gaps": {
    "ripgrep_validation_test": {
      "description": "No integration test validating god object analysis on ripgrep standard.rs",
      "location": "tests/ directory",
      "severity": "medium",
      "requirement_type": "optional",
      "suggested_fix": "The spec explicitly noted this test was 'deferred (validation test)'",
      "rationale": "This is a validation test to verify recommendation quality on real-world code, not a critical functional requirement"
    }
  },
  "summary": {
    "total_requirements": 12,
    "implemented_requirements": 11,
    "required_implemented": 10,
    "required_total": 10,
    "important_implemented": 1,
    "important_total": 1,
    "optional_implemented": 0,
    "optional_total": 1
  }
}
```

**Status values**:
- `complete`: 100% of all requirements (including optional)
- `functionally_complete`: 100% of required requirements
- `incomplete`: Missing required requirements

#### 3. Workflow Configuration Extensions

**Current workflow configuration** (workflows/implement.yml):
```yaml
commands:
  - claude: "/prodigy-implement-spec $ARG"
    commit_required: true
    validate:
      claude: "/prodigy-validate-spec $ARG --output .prodigy/validation-result.json"
      result_file: ".prodigy/validation-result.json"
      threshold: 100  # Current: single threshold for all gaps
      on_incomplete:
        claude: "/prodigy-complete-spec $ARG --gaps ${validation.gaps}"
        max_attempts: 5
        fail_workflow: false
        commit_required: true
```

**Enhanced workflow configuration** (proposed):
```yaml
commands:
  - claude: "/prodigy-implement-spec $ARG"
    commit_required: true
    validate:
      claude: "/prodigy-validate-spec $ARG --output .prodigy/validation-result.json"
      result_file: ".prodigy/validation-result.json"

      # NEW: Support multiple threshold types (optional)
      thresholds:
        functional: 100    # Required gaps must be 100% complete
        total: 90          # Total completion can be 90%

      # Alternative: single threshold mode (backward compatible, current behavior)
      # threshold: 100

      on_incomplete:
        # Passes ${validation.required_gaps} to prioritize required items
        claude: "/prodigy-complete-spec $ARG --gaps ${validation.gaps} --priority required"
        max_attempts: 5
        fail_workflow: false
        commit_required: true
```

**Implementation note**: The `thresholds` configuration and gap filtering logic will be implemented in the slash commands themselves (`.claude/commands/prodigy-validate-spec.md`), not in the Prodigy workflow engine. This allows enhancement without modifying Prodigy's core.

#### 4. Gap Classification Logic

**Slash Command Integration** (`.claude/commands/prodigy-validate-spec.md`):

The slash command will:
1. Parse spec file to extract requirement metadata
2. Invoke `debtmap validate-spec` (new Rust subcommand)
3. Read JSON output with classified gaps
4. Write enhanced validation result to output file

**Rust Implementation** (`src/validation/gap_classifier.rs`):

```rust
// Core gap classification algorithm
fn classify_gap(gap: &ValidationGap, spec: &Specification) -> RequirementType {
    // Check if spec explicitly marks this as optional/deferred
    if spec.deferred_requirements.contains(&gap.requirement_id) {
        return RequirementType::Optional;
    }

    // Check spec metadata for requirement classification
    if let Some(requirement) = spec.find_requirement(&gap.requirement_id) {
        if requirement.tags.contains("deferred") ||
           requirement.tags.contains("optional") {
            return RequirementType::Optional;
        }

        if requirement.tags.contains("validation-test") ||
           requirement.tags.contains("quality-check") {
            // Validation tests are optional unless spec says otherwise
            return RequirementType::Optional;
        }
    }

    // Classify by gap type
    match gap.gap_type {
        GapType::MissingCoreFunction => RequirementType::Required,
        GapType::MissingIntegration => RequirementType::Required,
        GapType::MissingErrorHandling => RequirementType::Important,
        GapType::MissingTests => RequirementType::Important,
        GapType::MissingDocumentation => RequirementType::Important,
        GapType::ValidationTest => RequirementType::Optional,
        GapType::PerformanceOptimization => RequirementType::Optional,
        GapType::NiceToHave => RequirementType::Optional,
    }
}

fn calculate_functional_completion(validation: &ValidationResult) -> f64 {
    let required_gaps: Vec<_> = validation.gaps
        .iter()
        .filter(|g| g.requirement_type == RequirementType::Required)
        .collect();

    if validation.summary.required_total == 0 {
        return 100.0; // No required items = functionally complete
    }

    let implemented = validation.summary.required_implemented;
    let total = validation.summary.required_total;

    (implemented as f64 / total as f64) * 100.0
}
```

#### 5. Spec Metadata for Requirement Classification

Extend spec frontmatter to support requirement classification:

```markdown
---
number: 137
title: Call Pattern-Based Analysis and Interface Size Estimation
category: foundation
priority: high
status: draft
dependencies: [133]
created: 2025-10-27

# NEW: Requirement classification
requirements:
  required:
    - "Call pattern-based responsibility naming"
    - "Interface size estimation"
  important:
    - "Unit tests for call pattern detection"
    - "Unit tests for interface size estimation"
  optional:
    - "Integration test for ripgrep standard.rs"

deferred:
  - "Integration test for ripgrep standard.rs"
  - reason: "External dependency not in test fixtures, deferred to future validation"
---

## Acceptance Criteria

- [x] Intra-module call graph built (required)
- [x] Functions grouped into cohesive clusters (required)
...
- [ ] ripgrep standard.rs validation test (optional, deferred)
```

#### 6. Updated `/prodigy-complete-spec` Behavior

**Enhanced command signature**:
```bash
/prodigy-complete-spec <spec-id> [--gaps <gaps-json>] [--priority <required|important|all>]
```

**Examples**:
```bash
# Fix only required gaps (default)
/prodigy-complete-spec 137 --gaps ${validation.gaps} --priority required

# Fix required and important gaps
/prodigy-complete-spec 137 --gaps ${validation.gaps} --priority important

# Fix all gaps including optional
/prodigy-complete-spec 137 --gaps ${validation.gaps} --priority all
```

**Implementation in slash command** (`.claude/commands/prodigy-complete-spec.md`):

1. Parse `--priority` flag (default: "required")
2. Filter `gaps` JSON to only include gaps with matching `requirement_type`
3. If no gaps match priority filter:
   - Output: "No {priority} gaps to fix, implementation is functionally complete"
   - Create **NO commit** (critical for workflow)
   - Return JSON with `completion_percentage: 100.0`
4. Otherwise:
   - Fix filtered gaps
   - Create commit with message: `fix: complete spec {spec-id} {priority} gaps`
   - Return completion status

**Key difference from current behavior**: Current command always creates a commit. Enhanced version skips commit when no required gaps remain.

#### 7. Threshold Evaluation Logic

**Implementation location**: `.claude/commands/prodigy-validate-spec.md` or helper Rust code

**Threshold evaluation** (determines if recovery should run):

```rust
// Rust helper function (src/validation/threshold.rs)
fn should_trigger_recovery(
    validation: &ValidationResult,
    functional_threshold: f64,
    total_threshold: Option<f64>
) -> bool {
    // Check functional threshold (required gaps only)
    if validation.functional_completion_percentage < functional_threshold {
        return true;
    }

    // Check total threshold if specified (all gaps)
    if let Some(total_threshold) = total_threshold {
        if validation.completion_percentage < total_threshold {
            // Only trigger if there are important gaps
            // (optional gaps don't block by default)
            return !validation.important_gaps.is_empty();
        }
    }

    false
}
```

**Slash command integration** (`.claude/commands/prodigy-validate-spec.md`):

The slash command reads workflow config (if available) and:
1. Defaults to `functional_threshold: 100.0` if not specified
2. Checks `validation.functional_completion_percentage >= functional_threshold`
3. Checks `validation.completion_percentage >= total_threshold` (if configured)
4. Returns appropriate status

**Note**: For initial implementation, threshold logic can be simple boolean checks in the slash command. Rust helper functions are optional enhancements for code reuse.

### Architecture Changes

**Modified components**:

1. **New Rust modules** (`src/validation/`):
   - `mod.rs` - Core validation types (`ValidationResult`, `ValidationGap`, `RequirementType`)
   - `gap_classifier.rs` - Gap classification algorithm
   - `spec_parser.rs` - Parse spec frontmatter for requirement metadata
   - `threshold.rs` - Threshold evaluation logic (optional, can be in slash commands)

2. **Modified slash commands**:
   - `.claude/commands/prodigy-validate-spec.md`:
     - Invoke Rust validation code
     - Classify gaps based on spec metadata
     - Output enhanced JSON format with `functional_completion_percentage` and categorized gaps
   - `.claude/commands/prodigy-complete-spec.md`:
     - Parse `--priority` flag
     - Filter gaps by `requirement_type`
     - Skip commit when no required gaps remain (key behavioral change)

3. **Optional workflow config enhancement** (`workflows/implement.yml`):
   - Add `thresholds` configuration (backward compatible)
   - Document gap filtering in `on_incomplete` section

4. **Spec template updates**:
   - Add optional `requirements` section to frontmatter
   - Add optional `deferred` section with rationale
   - Document how to classify requirements

**No changes required**: Prodigy workflow engine itself (works through existing extension points)

### Data Structures

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RequirementType {
    Required,
    Important,
    Optional,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationGap {
    pub description: String,
    pub location: String,
    pub severity: Severity,
    pub requirement_type: RequirementType,
    pub suggested_fix: String,
    pub rationale: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationSummary {
    pub total_requirements: usize,
    pub implemented_requirements: usize,
    pub required_total: usize,
    pub required_implemented: usize,
    pub important_total: usize,
    pub important_implemented: usize,
    pub optional_total: usize,
    pub optional_implemented: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub completion_percentage: f64,
    pub functional_completion_percentage: f64,
    pub status: ValidationStatus,
    pub implemented: Vec<String>,
    pub required_gaps: Vec<String>,
    pub important_gaps: Vec<String>,
    pub optional_gaps: Vec<String>,
    pub gaps: HashMap<String, ValidationGap>,
    pub summary: ValidationSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationStatus {
    Complete,
    FunctionallyComplete,
    Incomplete,
}
```

## Dependencies

- **Prerequisites**: None (foundational improvement)
- **Affected Components**:
  - Prodigy workflow engine
  - `.claude/commands/prodigy-validate-spec.md`
  - `.claude/commands/prodigy-complete-spec.md`
  - `workflows/implement.yml`
  - Spec template
- **External Dependencies**: None

## Testing Strategy

### Unit Tests

**Location**: `src/validation/gap_classifier.rs` and `src/validation/threshold.rs`

**Test gap classification logic**:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_deferred_validation_test() {
        let gap = ValidationGap {
            requirement_id: "ripgrep_validation",
            gap_type: GapType::ValidationTest,
            ...
        };

        let spec = Specification {
            deferred_requirements: vec!["ripgrep_validation".to_string()],
            ...
        };

        assert_eq!(classify_gap(&gap, &spec), RequirementType::Optional);
    }

    #[test]
    fn test_classify_missing_core_function() {
        let gap = ValidationGap {
            gap_type: GapType::MissingCoreFunction,
            ...
        };

        assert_eq!(classify_gap(&gap, &spec), RequirementType::Required);
    }

    #[test]
    fn test_functional_completion_calculation() {
        let validation = ValidationResult {
            summary: ValidationSummary {
                required_total: 10,
                required_implemented: 10,
                optional_total: 1,
                optional_implemented: 0,
                ...
            },
            ...
        };

        assert_eq!(validation.functional_completion_percentage, 100.0);
        assert_eq!(validation.completion_percentage, 90.9);
    }

    // Edge case: All requirements are optional
    #[test]
    fn test_all_requirements_optional() {
        let validation = ValidationResult {
            summary: ValidationSummary {
                required_total: 0,
                required_implemented: 0,
                optional_total: 5,
                optional_implemented: 2,
                ...
            },
            ...
        };

        // When all requirements are optional, functional_completion should be 100%
        assert_eq!(validation.functional_completion_percentage, 100.0);
        assert_eq!(validation.completion_percentage, 40.0);
    }

    // Edge case: Spec with no acceptance criteria
    #[test]
    fn test_spec_with_no_criteria() {
        let spec = Specification {
            requirements: vec![],
            deferred_requirements: vec![],
            ...
        };

        let validation = validate_spec(&spec, &implementation);

        // Empty spec should be 100% complete by default
        assert_eq!(validation.functional_completion_percentage, 100.0);
        assert_eq!(validation.completion_percentage, 100.0);
    }

    // Edge case: Malformed spec frontmatter
    #[test]
    fn test_malformed_spec_classification() {
        let spec_content = r#"
---
number: 999
requirements:
  invalid_structure: "not a list"
---
        "#;

        let spec = parse_spec(spec_content).unwrap();
        let gap = ValidationGap::missing_function("test");

        // Should fall back to safe default (Required)
        assert_eq!(classify_gap(&gap, &spec), RequirementType::Required);
    }

    // Edge case: Gap with no matching requirement in spec
    #[test]
    fn test_gap_with_no_spec_match() {
        let gap = ValidationGap {
            requirement_id: "unknown_requirement",
            ...
        };

        let spec = Specification {
            requirements: HashMap::new(),
            ...
        };

        // Unknown gaps default to Required (safe default)
        assert_eq!(classify_gap(&gap, &spec), RequirementType::Required);
    }

    // Threshold evaluation edge cases
    #[test]
    fn test_threshold_exactly_at_boundary() {
        let validation = ValidationResult {
            functional_completion_percentage: 100.0,
            completion_percentage: 90.0,
            ...
        };

        // Exactly at threshold should pass
        assert!(!should_trigger_recovery(&validation, 100.0, Some(90.0)));
    }

    #[test]
    fn test_threshold_just_below_boundary() {
        let validation = ValidationResult {
            functional_completion_percentage: 99.9,
            completion_percentage: 89.9,
            ...
        };

        // Just below threshold should trigger recovery
        assert!(should_trigger_recovery(&validation, 100.0, Some(90.0)));
    }
}
```

### Integration Tests

**Test spec 137 scenario**:
```bash
# Should pass with functional completion
/prodigy-validate-spec 137 --output test-result.json

# Expected output:
{
  "functional_completion_percentage": 100.0,
  "completion_percentage": 91.7,
  "status": "functionally_complete",
  "required_gaps": [],
  "optional_gaps": ["Integration test for ripgrep standard.rs"]
}

# Workflow should NOT trigger recovery for optional gaps
# If it does, recovery should return success without commit
```

**Test workflow threshold evaluation**:
```yaml
# Test Case 1: Functional threshold met, total threshold not met
validate:
  thresholds:
    functional: 100
    total: 95
# Should: Pass if functional=100%, total=91.7% (no recovery needed)

# Test Case 2: Functional threshold not met
validate:
  thresholds:
    functional: 100
# Should: Trigger recovery if functional<100%

# Test Case 3: Backward compatibility
validate:
  threshold: 100
# Should: Behave as before (both functional and total must be 100%)
```

### Manual Validation Tests

1. **Run spec 137 through workflow**:
   - Should pass validation with 100% functional completion
   - Should NOT fail on missing optional ripgrep test
   - Should create commit for implementation, but not for recovery

2. **Test with spec having missing required gaps**:
   - Should trigger recovery
   - Recovery should fix required gaps
   - Recovery should create commit
   - Should fail if required gaps can't be fixed

3. **Test with spec having only important gaps**:
   - Should trigger recovery (configurable)
   - Recovery should fix important gaps
   - Should pass at lower total threshold if configured

## Documentation Requirements

### Code Documentation

**Location**: Inline Rust doc comments in `src/validation/`

- [ ] `src/validation/mod.rs`: Module-level docs explaining gap classification system
- [ ] `src/validation/gap_classifier.rs`: Document classification algorithm with examples
- [ ] `RequirementType` enum: Explain when to use Required/Important/Optional
- [ ] `ValidationResult` struct: Document all fields, especially `functional_completion_percentage`
- [ ] Public functions: Include examples showing classification logic

**Example documentation**:
```rust
/// Classifies a validation gap based on spec metadata and gap type.
///
/// # Classification Rules
///
/// - **Required**: Core functionality, critical features, security
/// - **Important**: Tests, documentation, error handling
/// - **Optional**: Deferred items, validation tests, nice-to-haves
///
/// # Examples
///
/// ```rust
/// // Gap marked as deferred in spec → Optional
/// let gap = ValidationGap { requirement_id: "ripgrep_test", ... };
/// let spec = Specification { deferred: vec!["ripgrep_test"], ... };
/// assert_eq!(classify_gap(&gap, &spec), RequirementType::Optional);
/// ```
pub fn classify_gap(gap: &ValidationGap, spec: &Specification) -> RequirementType
```

### User Documentation

**1. Workflow Documentation** (`docs/workflows/validation.md` - new file)

Contents:
- [ ] Overview of gap classification system
- [ ] How to read validation output (functional vs total completion)
- [ ] How to configure thresholds in `workflows/implement.yml`
- [ ] When to use functional vs total thresholds (with decision tree)
- [ ] Examples of successful and failed validations
- [ ] Troubleshooting common validation issues

**2. Spec Writing Guide** (`docs/specs/requirement-classification.md` - new file)

Contents:
- [ ] How to classify requirements using frontmatter
- [ ] Best practices for marking deferred items
- [ ] Examples of required vs important vs optional classification
- [ ] When to use explicit classification vs relying on defaults
- [ ] Template for `requirements` and `deferred` sections

**Example**:
```markdown
## Classifying Requirements

### Default Classification

Most specs work fine without explicit classification:
- Core functionality → Required (default)
- Tests and docs → Important (default)
- Validation tests → Optional (default)

### Explicit Classification

Use frontmatter when defaults don't match your needs:

\`\`\`yaml
---
number: 137
requirements:
  required:
    - "Call pattern-based responsibility naming"
    - "Interface size estimation"
  important:
    - "Unit tests for both features"
  optional:
    - "Integration test for ripgrep standard.rs"

deferred:
  - "Integration test for ripgrep standard.rs"
    reason: "External dependency not in test fixtures"
---
\`\`\`
```

**3. Slash Command Documentation** (update existing `.claude/commands/*.md`)

- [ ] `.claude/commands/prodigy-validate-spec.md`: Document new JSON output format
- [ ] `.claude/commands/prodigy-complete-spec.md`: Document `--priority` flag options
- [ ] Add examples showing gap filtering behavior

**4. Architecture Documentation** (`ARCHITECTURE.md` - update)

Add new section:
- [ ] **Validation and Gap Classification** (lines 200-300):
  - System overview and component interaction
  - Data flow: spec → classification → validation → threshold evaluation
  - Design rationale for functional vs total completion
  - Extension points for future enhancements

**Example section**:
```markdown
## Validation and Gap Classification

### Overview

Prodigy validates spec implementations by comparing requirements against code changes.
Gap classification extends this with priority levels (required/important/optional).

### Component Architecture

1. **Spec Parser** (`src/validation/spec_parser.rs`)
   - Parses frontmatter to extract requirement metadata
   - Builds `Specification` struct with classified requirements

2. **Gap Classifier** (`src/validation/gap_classifier.rs`)
   - Analyzes validation gaps against spec metadata
   - Assigns `RequirementType` to each gap

3. **Validation Result** (`src/validation/mod.rs`)
   - Computes both `completion_percentage` (all gaps) and
     `functional_completion_percentage` (required gaps only)
   - Separates gaps by requirement type

4. **Slash Commands** (`.claude/commands/`)
   - Orchestrate validation workflow
   - Evaluate thresholds and trigger recovery
   - Filter gaps by priority for targeted fixes

### Design Rationale

**Why functional vs total completion?**
- Allows workflows to pass when core features are complete
- Defers optional validation tests without blocking progress
- Maintains high quality bar while enabling iteration
```

### Documentation Deliverables Summary

| Document | Type | Status | Priority |
|----------|------|--------|----------|
| `src/validation/*.rs` inline docs | Code | Required | High |
| `docs/workflows/validation.md` | User guide | Required | High |
| `docs/specs/requirement-classification.md` | Spec template guide | Required | High |
| `.claude/commands/*.md` updates | Slash command help | Required | Medium |
| `ARCHITECTURE.md` section | Architecture docs | Required | Medium |
| Migration guide (in spec) | Implementation guide | Included above | High |

## Implementation Notes

### Backward Compatibility

**Maintain existing behavior**:
- Single `threshold` parameter works as before (100% total)
- Existing validation output format still supported
- Gaps without `requirement_type` default to "required"
- Workflows without threshold config default to 100% total

**Migration path**:
1. Phase 1: Add new fields to validation output (additive change)
2. Phase 2: Update workflows to use new threshold options (opt-in)
3. Phase 3: Update spec template with requirement classification (opt-in)
4. Phase 4: Deprecate old format (long-term, optional)

### Requirement Type Guidelines

**Required**:
- Core functionality specified in spec objective
- Critical integrations
- Security features
- Data integrity features

**Important**:
- Unit tests for new functionality
- Error handling
- Documentation
- Integration with existing systems

**Optional**:
- Validation tests on external codebases
- Performance optimizations
- Nice-to-have features
- Deferred items with explicit rationale

### Default Classification Rules

When spec doesn't provide explicit classification:
1. Check acceptance criteria tags (required/important/optional)
2. Check if item is marked as deferred
3. Classify by gap type (core function = required, tests = important, etc.)
4. Default to "required" if uncertain (safe default)

## Migration and Compatibility

### Breaking Changes

None - fully backward compatible

### Backward Compatibility Strategy

1. **Validation output**: Include both old and new fields
2. **Workflow config**: Support both old `threshold` and new `thresholds`
3. **Gap structure**: Make `requirement_type` optional (default: "required")
4. **Commands**: Support both old and new argument formats

### Backward Compatibility Verification Checklist

**Before implementation** (baseline):
- [ ] Run `just test` and record all passing tests
- [ ] Run validation on 10 existing specs (e.g., 133, 137, 138b, 150, 153, 155, 156, 157)
- [ ] Document current workflow behavior for spec 137 scenario
- [ ] Record validation times for performance baseline

**During implementation**:
- [ ] All existing unit tests continue to pass
- [ ] No changes to existing JSON fields (only additions)
- [ ] Gaps without `requirement_type` default to "required" (safe default)
- [ ] Workflows without `thresholds` config use old behavior (100% total threshold)
- [ ] All existing specs validate without modification

**After implementation** (verification):
- [ ] Run `just test` - all tests that passed before still pass
- [ ] Validate same 10 specs - results identical or better (no regressions)
- [ ] Spec 137 now passes with functional completion (improvement)
- [ ] Performance regression <100ms (run benchmarks)
- [ ] Existing validation JSON files still parse correctly (test on old .prodigy/*.json files)

**Migration validation** (comprehensive):
- [ ] Test workflow with old-format validation JSON (no `functional_completion_percentage`)
- [ ] Test workflow with new-format validation JSON (with gap classification)
- [ ] Test slash command with no `--priority` flag (backward compatible default)
- [ ] Test slash command with new `--priority` flag (new functionality)
- [ ] Ensure no breaking changes to command signatures

**Rollback safety**:
- [ ] Can revert Rust code changes without breaking existing workflows
- [ ] Can revert slash command changes without breaking Prodigy
- [ ] No database or persistent state changes (all changes are code-only)

### Migration Guide

**For existing workflows**:
```yaml
# Old format (still works)
validate:
  threshold: 100

# New format (recommended)
validate:
  thresholds:
    functional: 100  # Must complete all required items
    total: 90        # Can leave optional items for later
```

**For existing specs**:
- No changes required immediately
- Can add requirement classification to frontmatter
- Can mark deferred items in acceptance criteria
- Default classification will be applied automatically

## Success Metrics

### Measurable Success Criteria

1. **Spec 137 Scenario** (Automated integration test)
   - [ ] Workflow completes successfully with 91.7% total, 100% functional completion
   - [ ] No workflow failure on optional gap (ripgrep validation test)
   - [ ] Recovery command runs but creates no commit (gaps are optional)
   - **Verification**: Create integration test that implements spec 137 and validates workflow behavior

2. **Backward Compatibility** (Migration validation)
   - [ ] All existing specs (n=150+) validate successfully with default classification
   - [ ] No workflow failures introduced by classification changes
   - [ ] Existing validation JSON format still accepted
   - **Verification**: Run `just test-all-specs` before and after implementation, compare results

3. **False Failure Elimination** (Integration test suite)
   - [ ] Zero workflow failures when only optional gaps remain
   - [ ] Test suite includes 5+ scenarios with optional/deferred requirements
   - **Verification**: Create test cases for each scenario in `tests/validation_classification_test.rs`

4. **Classification Accuracy** (Usage analytics baseline)
   - [ ] <5% of specs require explicit `requirements` frontmatter overrides
   - [ ] Default classification rules handle common patterns (tests, docs, deferred items)
   - **Verification**: Analyze 20 recent specs, measure how many need explicit classification

5. **Workflow Transparency** (User feedback)
   - [ ] Validation output clearly shows which gaps are required vs optional
   - [ ] Error messages explain why workflow passed or failed
   - [ ] Developers understand completion percentage vs functional completion
   - **Verification**: Review validation JSON output format with team, gather feedback

6. **Performance Regression** (Benchmark)
   - [ ] Gap classification adds <100ms to validation time
   - [ ] No memory regression for large codebases (1000+ files)
   - **Verification**: Benchmark validation before/after on large test project

### Success Baseline

**Before implementation**:
- Spec 137 workflow fails at 91.7% completion (false failure)
- Recovery command creates commit even when no required gaps remain
- No distinction between required and optional requirements

**After implementation**:
- Spec 137 workflow passes with 100% functional, 91.7% total completion
- Recovery command skips commit when only optional gaps remain
- Clear classification: required (100%), important (100%), optional (0%)

## Example Scenarios

### Scenario 1: Functional Complete, Optional Gap

**Input**: Spec 137 validation
```json
{
  "functional_completion_percentage": 100.0,
  "completion_percentage": 91.7,
  "required_gaps": [],
  "optional_gaps": ["ripgrep validation test"]
}
```

**Workflow behavior**:
- Threshold: `functional: 100, total: 90`
- Result: PASS (functional=100%, total=91.7% > 90%)
- Recovery: NOT triggered
- Commit: Initial implementation only

### Scenario 2: Missing Required Functionality

**Input**: Spec validation
```json
{
  "functional_completion_percentage": 80.0,
  "completion_percentage": 85.0,
  "required_gaps": ["Interface size estimation function"],
  "important_gaps": ["Unit tests for estimation"]
}
```

**Workflow behavior**:
- Threshold: `functional: 100`
- Result: FAIL (functional=80% < 100%)
- Recovery: TRIGGERED for required gaps
- Commit: Required after recovery

### Scenario 3: Missing Tests Only

**Input**: Spec validation
```json
{
  "functional_completion_percentage": 100.0,
  "completion_percentage": 85.0,
  "required_gaps": [],
  "important_gaps": ["Unit tests for feature X"]
}
```

**Workflow behavior**:
- Threshold: `functional: 100, total: 90`
- Result: PASS or FAIL depending on `total` threshold
- Recovery: Triggered only if `total < 90%`
- Commit: Required if recovery runs

---

## Summary of Improvements to Spec 153

This revision addresses all high-priority recommendations from the evaluation:

### Key Clarifications Added

1. **Rust vs Command Boundary** (lines 103-131)
   - Clear separation: Rust implements algorithms, slash commands orchestrate
   - Explicit module locations: `src/validation/` for Rust code
   - No changes to Prodigy workflow engine itself

2. **Implementation Locations** (lines 117-131)
   - New Rust modules specified: `gap_classifier.rs`, `spec_parser.rs`, `threshold.rs`
   - Modified slash commands identified with specific changes
   - Workflow configuration enhancements marked as optional

3. **Workflow Configuration** (lines 200-243)
   - Showed current YAML format alongside proposed enhancements
   - Explained that enhancements happen in slash commands, not workflow engine
   - Clarified backward compatibility approach

4. **Edge Case Tests** (lines 572-666)
   - Added 8 new test cases covering edge scenarios
   - Tests for malformed specs, empty requirements, boundary conditions
   - Performance and threshold evaluation edge cases

5. **Measurable Success Metrics** (lines 837-882)
   - Converted vague metrics to specific, testable criteria
   - Added verification methods for each metric
   - Established before/after baselines

6. **Backward Compatibility Verification** (lines 815-846)
   - Comprehensive checklist for before/during/after implementation
   - Migration validation steps
   - Rollback safety verification

7. **Documentation Deliverables** (lines 728-877)
   - Specific file locations for each doc (not just "add docs")
   - Content outlines for user guides
   - Example documentation snippets
   - Summary table of all deliverables

### What Was Fixed

**Problem**: Spec mixed Rust and YAML workflow concepts, unclear where code goes
**Solution**: Clear architecture section explaining Rust/command/workflow boundaries

**Problem**: No guidance on where to put new code in codebase
**Solution**: Explicit module locations in `src/validation/`

**Problem**: Success metrics were unmeasurable ("zero false failures")
**Solution**: Specific test cases, baselines, and verification methods

**Problem**: Missing edge case coverage
**Solution**: 8 new test cases for malformed specs, boundaries, edge conditions

**Problem**: Documentation requirements too vague
**Solution**: Specific files, content outlines, and examples

### Confidence Level

**Implementation Readiness**: 95% (up from 85%)

The spec now provides:
- Clear separation of concerns (Rust vs commands vs workflow)
- Specific code locations for all new components
- Comprehensive test coverage including edge cases
- Measurable success criteria with baselines
- Detailed documentation deliverables

**Ready for**: Creating `IMPLEMENTATION_PLAN.md` and beginning development
