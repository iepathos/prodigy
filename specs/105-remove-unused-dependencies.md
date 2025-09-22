---
number: 105
title: Remove Unused Dependencies
category: optimization
priority: medium
status: draft
dependencies: []
created: 2025-09-21
---

# Specification 105: Remove Unused Dependencies

## Context

The dependency audit revealed 7 unused dependencies in the project, contributing to increased binary size (19MB), longer build times (27 seconds), and unnecessary maintenance burden. These unused dependencies include testing utilities (`assert_cmd`, `insta`, `predicates`, `tokio-test`), build tools (`clap_mangen`), and runtime libraries (`humantime-serde`, `reqwest`). Additionally, there's duplication with both `dirs` and `directories` crates serving similar purposes.

## Objective

Remove all unused dependencies and consolidate duplicate functionality to reduce binary size, improve build times, and simplify dependency management.

## Requirements

### Functional Requirements

1. Remove all unused runtime dependencies
2. Remove unused dev-dependencies that aren't utilized in tests
3. Consolidate duplicate directory handling crates
4. Verify no functionality is broken after removal
5. Update any code that might be relying on transitive dependencies

### Non-Functional Requirements

- Reduce binary size by at least 10%
- Improve build times by at least 15%
- Maintain all existing functionality
- Ensure no transitive dependency issues
- Document any dependencies kept for future use

## Acceptance Criteria

- [ ] All identified unused dependencies removed from Cargo.toml
- [ ] Binary size reduced from 19MB to under 17MB
- [ ] Build time improved to under 23 seconds
- [ ] All tests pass after dependency removal
- [ ] Cargo.lock updated with reduced dependency tree
- [ ] Documentation updated to reflect changes

## Technical Details

### Dependencies to Remove

**Runtime Dependencies:**
- `humantime-serde` - Time parsing utility not used in code
- `reqwest` - HTTP client not used (remove after verification)

**Dev Dependencies:**
- `assert_cmd` - Command testing utility not currently used
- `insta` - Snapshot testing library not utilized
- `predicates` - Test assertion library not used
- `tokio-test` - Async test utilities not used

**Build Dependencies:**
- `clap_mangen` - Man page generation (verify if needed)

### Consolidation Required

**Directory Libraries:**
- Keep `directories` (version 6.0) - More comprehensive
- Remove `dirs` (version 6.0) - Redundant with directories
- Update any code using `dirs` to use `directories` instead

### Implementation Steps

1. **Verification Phase**
   ```bash
   # Install cargo-machete if not present
   cargo install cargo-machete

   # Run dependency analysis
   cargo machete

   # Check for transitive dependencies
   cargo tree -d
   ```

2. **Removal Process**
   ```toml
   # Before in Cargo.toml
   [dependencies]
   humantime-serde = "1.1"
   reqwest = { version = "0.12", ... }
   dirs = "6.0"

   [dev-dependencies]
   assert_cmd = "2.0"
   insta = { version = "1.39", ... }
   predicates = "3.1"
   tokio-test = "0.4"

   # After in Cargo.toml
   [dependencies]
   # humantime-serde removed
   # reqwest removed (if confirmed unused)
   # dirs removed (using directories instead)

   [dev-dependencies]
   # Unused test dependencies removed
   ```

3. **Code Updates**
   ```rust
   // Before: Using dirs crate
   use dirs::home_dir;
   let home = home_dir().unwrap();

   // After: Using directories crate
   use directories::BaseDirs;
   let base_dirs = BaseDirs::new().unwrap();
   let home = base_dirs.home_dir();
   ```

### Verification Commands

```bash
# Clean build to ensure no cached dependencies
cargo clean

# Build to verify compilation
cargo build --release

# Run all tests
cargo test --all

# Check binary size
ls -lh target/release/prodigy

# Measure build time
time cargo build --release

# Verify no unused dependencies remain
cargo machete

# Check dependency tree
cargo tree | wc -l  # Count total dependencies
```

## Dependencies

- No functional dependencies on other specs
- Should be done before major refactoring work
- May affect CI/CD configurations

## Testing Strategy

1. **Compilation Tests**
   - Clean build from scratch
   - Verify no compilation errors
   - Check all feature combinations

2. **Runtime Tests**
   - Run full test suite
   - Execute integration tests
   - Test all CLI commands

3. **Performance Validation**
   - Measure and document binary size reduction
   - Benchmark build time improvements
   - Profile runtime memory usage

## Documentation Requirements

- Update README with new build requirements
- Document any kept dependencies with justification
- Create dependency audit checklist for future
- Update development setup documentation