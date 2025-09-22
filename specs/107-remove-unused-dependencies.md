---
number: 107
title: Remove Unused Dependencies and Optimize Binary Size
category: optimization
priority: medium
status: draft
dependencies: []
created: 2025-09-22
---

# Specification 107: Remove Unused Dependencies and Optimize Binary Size

## Context

Analysis revealed unused dependencies that contribute to build time and binary size without providing value. Specifically, the `log` crate is included but never used (0 files), while `tracing` is used extensively (58 files). There's also potential redundancy between `futures` and `futures-util` dependencies.

Current issues:
- `log = "0.4"` dependency with zero usage
- Potential redundancy in futures crate dependencies
- Opportunity to reduce compile time and binary size
- Need for regular dependency auditing process

## Objective

Remove unused dependencies, eliminate redundancy, and establish a process for ongoing dependency hygiene to maintain optimal build performance and binary size.

## Requirements

### Functional Requirements
- Remove completely unused dependencies
- Eliminate redundant dependency combinations
- Maintain all current functionality
- Establish dependency auditing process
- Document dependency decisions

### Non-Functional Requirements
- Reduce build time through fewer dependencies
- Maintain binary size well under 20MB target
- No impact on application performance
- Simplified dependency tree
- Clear rationale for all remaining dependencies

## Acceptance Criteria

- [ ] `log` dependency removed from Cargo.toml
- [ ] Redundant futures dependencies consolidated
- [ ] All code compiles and tests pass after removal
- [ ] Binary size reduction measured and documented
- [ ] Build time improvement measured
- [ ] Dependency audit process documented
- [ ] Remaining dependencies have clear justification

## Technical Details

### Dependencies to Remove

1. **log crate**
   - Current usage: 0 files
   - Status: Completely unused
   - Action: Remove from Cargo.toml

2. **futures-util redundancy**
   - `futures` crate re-exports `futures-util`
   - Current usage: `futures` (15 files), `futures-util` (1 file)
   - Action: Remove explicit `futures-util`, use re-export

### Implementation Approach

1. **Phase 1: Safe Removal**
   - Remove `log` dependency
   - Verify no transitive dependencies require it
   - Run full test suite to confirm no breakage

2. **Phase 2: Consolidation**
   - Analyze futures usage patterns
   - Replace explicit `futures-util` with `futures` re-export
   - Verify async functionality unaffected

3. **Phase 3: Process Establishment**
   - Document dependency decision rationale
   - Create regular audit checklist
   - Add CI check for unused dependencies

### Dependency Audit Process

```toml
# Add to CI workflow
[dev-dependencies]
cargo-udeps = "0.1"  # For detecting unused dependencies

# Regular audit commands
cargo +nightly udeps
cargo tree --duplicates
cargo audit
```

### Before/After Analysis

```toml
# Before
[dependencies]
log = "0.4"                    # UNUSED
futures = "0.3"                # Used in 15 files
futures-util = "0.3"          # Used in 1 file (redundant)

# After
[dependencies]
futures = "0.3"                # Consolidated usage
# log removed
# futures-util removed (use futures re-export)
```

## Dependencies

None - this is foundational optimization work.

## Testing Strategy

- Full test suite execution after each dependency removal
- Build time measurement before and after changes
- Binary size comparison
- Functionality verification for async operations
- CI pipeline validation

## Documentation Requirements

- Document dependency removal rationale
- Create dependency decision matrix
- Add dependency audit process to development guide
- Update build documentation with new timings
- Document futures usage patterns for developers