---
number: 105
title: Remove Unused Dependencies
category: optimization
priority: high
status: draft
dependencies: [102]
created: 2025-01-21
---

# Specification 105: Remove Unused Dependencies

**Category**: optimization
**Priority**: high
**Status**: draft
**Dependencies**: [102 - Consolidate Storage Systems]

## Context

The Prodigy codebase includes many heavy dependencies that were added for future features but are not actively used. These dependencies increase build times, binary size, security surface area, and complexity. The evaluation identified that 30-40% of dependencies could be removed without affecting current functionality.

Major unused dependencies include:
- AWS SDK (S3) - for future containerization
- Redis client - for distributed state
- PostgreSQL/sea-orm - for persistent storage
- Axum web framework - minimal usage
- Various other libraries added speculatively

## Objective

Remove all unused and underutilized dependencies to reduce build times, binary size, and maintenance burden while maintaining all current functionality.

## Requirements

### Functional Requirements
- Identify all unused dependencies through code analysis
- Remove dependencies not required for current features
- Replace over-engineered dependencies with simpler alternatives where appropriate
- Ensure all functionality remains intact after removal
- Update documentation to reflect removed dependencies

### Non-Functional Requirements
- Reduce build time by at least 30%
- Reduce binary size by at least 25%
- Reduce total dependency count by 30-40%
- Improve security by reducing attack surface
- Simplify deployment with fewer requirements

## Acceptance Criteria

- [ ] All unused dependencies removed from Cargo.toml
- [ ] Build time reduced by at least 30%
- [ ] Binary size reduced by at least 25%
- [ ] All tests pass after dependency removal
- [ ] No functionality regression
- [ ] Cargo.lock updated and committed
- [ ] Documentation updated to remove references to unused features

## Technical Details

### Implementation Approach

1. **Phase 1: Dependency Audit**
   ```bash
   # Use cargo-udeps to find unused dependencies
   cargo install cargo-udeps
   cargo +nightly udeps

   # Use cargo-bloat to analyze binary size
   cargo install cargo-bloat
   cargo bloat --release

   # Analyze dependency tree
   cargo tree --duplicates
   cargo tree --depth 1
   ```

2. **Phase 2: Remove Cloud Dependencies**
   Remove from Cargo.toml:
   ```toml
   # Remove these cloud/distributed dependencies
   aws-config = "1.8.6"
   aws-sdk-s3 = "1.106.0"
   deadpool-redis = "0.18.0"
   redis = "0.27.5"
   sea-orm = "1.2.0"
   sqlx = "0.8.2"
   ```

3. **Phase 3: Remove/Replace Web Dependencies**
   ```toml
   # Remove if not actively used
   axum = "0.8.4"
   tower = "0.5.2"
   tower-http = "0.6.2"
   hyper = "1.5.2"

   # Keep only if needed for HTTP client functionality
   reqwest = { version = "0.12", features = ["json"] }
   ```

4. **Phase 4: Consolidate Utility Dependencies**
   ```toml
   # Remove duplicate functionality
   # Keep either once_cell OR lazy_static, not both
   # Keep either regex OR fancy-regex, not both
   # Consolidate serialization (json/yaml/toml)
   ```

5. **Phase 5: Optimize Feature Flags**
   ```toml
   # Reduce features to minimum required
   tokio = { version = "1", features = ["rt-multi-thread", "fs", "process", "time"] }
   serde = { version = "1", features = ["derive"] }
   # Remove unused features from other dependencies
   ```

### Dependencies to Remove

**Definitely Remove:**
- aws-config, aws-sdk-s3 (unused S3 storage)
- deadpool-redis, redis (unused Redis backend)
- sea-orm, sqlx (unused database backends)
- axum, tower, tower-http (unless actively used)
- mime_guess (if not needed)
- flate2 (unless compression actively used)

**Evaluate for Removal:**
- notify (file watching - check if used)
- lru (caching - check usage)
- hostname (check if necessary)
- gray_matter (markdown frontmatter - check usage)

**Optimize:**
- Reduce tokio features to minimum
- Reduce serde features
- Consolidate duplicate functionality

### Expected Improvements

| Metric | Current | Target | Improvement |
|--------|---------|--------|-------------|
| Dependencies | 150+ | 90-100 | 40% reduction |
| Build Time | ~3 min | ~2 min | 33% faster |
| Binary Size | ~25 MB | ~18 MB | 28% smaller |
| Cargo.lock lines | 5000+ | 3000- | 40% smaller |

## Dependencies

- **Prerequisites**:
  - Spec 102 (Remove container storage abstractions first)
- **Affected Components**:
  - Build configuration
  - CI/CD pipelines
  - Docker images
  - Installation documentation
- **External Dependencies**: None added

## Testing Strategy

- **Build Tests**: Verify clean build after removal
- **Feature Tests**: Test all features still work
- **Performance Tests**: Measure build time improvements
- **Size Tests**: Verify binary size reduction
- **Deployment Tests**: Test installation process

## Documentation Requirements

- **Dependency Documentation**: Update list of required dependencies
- **Build Documentation**: Update build instructions
- **Feature Documentation**: Remove references to unused features
- **Installation Guide**: Simplify installation requirements

## Implementation Notes

- Use cargo-udeps to find unused dependencies
- Remove dependencies incrementally, testing after each
- Check transitive dependencies for duplicates
- Consider creating a "minimal" feature flag for core functionality
- Document why each remaining dependency is needed

## Migration and Compatibility

For library consumers:
```rust
// If they were using removed features
#[cfg(feature = "s3-storage")]
compile_error!("S3 storage has been removed. Please use file storage instead.");

#[cfg(feature = "redis-backend")]
compile_error!("Redis backend has been removed. Please use file storage instead.");
```

Build time improvements:
```
Before dependency removal:
  Compiling 287 crates
  Finished release [optimized] in 3m 12s
  Binary size: 25.3 MB

After dependency removal:
  Compiling 178 crates
  Finished release [optimized] in 2m 05s
  Binary size: 18.1 MB
```