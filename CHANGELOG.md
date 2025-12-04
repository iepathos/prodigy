# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Comprehensive changelog management system

## [0.4.1] - 2024-12-04

### Added
- Configuration value tracing (spec 181)
- Validation accumulation patterns with Stillwater (spec 176)
- Stillwater migration testing refinement (spec 177)
- StorageConfig re-export for better API ergonomics

### Changed
- Migrated configuration system to premortem (spec 179)
- Replaced test env::set_var with MockEnv for isolation (spec 180)
- Use impl Effect return types and stillwater helpers
- Use FileSystem service for parallel-safe glob tests
- Clean up deprecated config APIs

### Fixed
- Ignored doctests now properly configured
- PRODIGY_ARG set as OS env var for $ARG variable substitution
- Allow resume from Failed workflow status
- Complete spec 182 implementation - remove remaining goal_seek references

### Removed
- Goal-seek feature (spec 182)
- Duplicate tests using deprecated config methods
- Unused deprecated TestEnv and IsolatedTestContext fixtures

## [0.4.0] - 2024-11-28

### Added
- Reader pattern environment access helpers (spec 175)
- Effect-based parallel execution foundation (spec 173)
- Stillwater foundation integration (spec 172)
- Pure execution planning module (spec 174a)
- Pure workflow transformations (spec 174b)
- Pure session updates (spec 174c)
- Effect modules for workflow and session (spec 174d)
- Comprehensive testing for effect-based execution (spec 174g)

### Changed
- Refactored orchestrator core to under 500 LOC (spec 174e)
- Refactored workflow executor with pure/effects separation (spec 174f)
- Updated zerocopy dependencies to 0.8.30

### Fixed
- Use approximate equality for Average in property tests
- Complete spec 173 implementation gaps
- Complete spec 174f integration of pure/ and effects/ modules

### Removed
- Unused analytics infrastructure
- Unused scoring and health metrics infrastructure

## [0.3.1] - 2024-11-15

### Added
- Semigroup-based variable aggregation with validation (spec 171)
- Reader pattern for environment access in MapReduce (spec 175)
- Comprehensive testing for aggregation functions

### Changed
- Improved error messages for variable aggregation type mismatches
- Enhanced documentation for Stillwater patterns

### Fixed
- Edge cases in aggregation validation
- Type safety in parallel execution

## [0.3.0] - 2024-11-08

### Added
- MapReduce workflow support with parallel execution
- Checkpoint and resume functionality (spec 134)
- Dead Letter Queue (DLQ) for failed work items
- Concurrent resume protection with RAII locking (spec 140)
- Worktree isolation for parallel agent execution (spec 127)
- Commit validation for MapReduce agents (spec 163)
- Orphaned worktree cleanup handling (spec 136)

### Changed
- Unified session storage model
- Storage architecture with improved isolation

### Fixed
- Memory safety in parallel worktree operations
- Race conditions in checkpoint writes

## [0.2.9] - 2024-10-25

### Added
- Claude command retry with Stillwater effects (spec 185)
- Non-MapReduce workflow resume support (spec 186)
- UnifiedSession checkpoint storage (spec 184)
- Checkpoint-on-signal for graceful interruption

### Fixed
- Workflow hash validation on resume
- Test failures with functional patterns
- Checkpoint write error handling

## [0.2.8] - 2024-10-18

### Added
- Effect-based workflow execution (spec 183)
- MapReduce incremental checkpoint system (spec 162)

### Changed
- Increased functional programming adoption (spec 108)

### Fixed
- Complete spec 183 implementation gaps

## [0.2.0] - 2024-09-15

### Added
- Basic MapReduce workflow mode
- Setup and reduce phases
- Work item processing with templates
- Parallel execution with configurable limits

### Changed
- Enhanced CLI with workflow mode support
- Improved error handling and reporting

## [0.1.0] - 2024-08-01

### Added
- Initial release
- Basic workflow execution engine
- CLI interface
- Git worktree integration
- Session management
- Command execution (Claude and shell)
- Variable interpolation

[Unreleased]: https://github.com/anthropics/prodigy/compare/0.4.1...HEAD
[0.4.1]: https://github.com/anthropics/prodigy/compare/0.4.0...0.4.1
[0.4.0]: https://github.com/anthropics/prodigy/compare/0.3.1...0.4.0
[0.3.1]: https://github.com/anthropics/prodigy/compare/0.3.0...0.3.1
[0.3.0]: https://github.com/anthropics/prodigy/compare/0.2.9...0.3.0
[0.2.9]: https://github.com/anthropics/prodigy/compare/0.2.8...0.2.9
[0.2.8]: https://github.com/anthropics/prodigy/compare/0.2.0...0.2.8
[0.2.0]: https://github.com/anthropics/prodigy/compare/0.1.0...0.2.0
[0.1.0]: https://github.com/anthropics/prodigy/releases/tag/0.1.0
