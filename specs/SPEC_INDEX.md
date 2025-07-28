# Specification Index

This index provides a comprehensive overview of all specifications in the MMM project, organized by category and implementation status.

## Categories

### Foundation Specifications
Core architecture and essential system components that form the base of MMM.

- **Spec 01**: Core Architecture - Basic system structure and components
- **Spec 09**: Dead Simple Improve - Minimal viable improvement command
- **Spec 10**: Smart Project Analyzer - Language and framework detection
- **Spec 11**: Simple State Management - JSON-based state tracking
- **Spec 14**: Implement Real Claude Loop - Working Claude CLI integration ✅
- **Spec 19**: Git-Native Improvement Flow - Commit-based workflow ✅

### Parallel Specifications
Features enabling concurrent execution and parallel processing.

- **Spec 24**: Git Worktree Isolation - Parallel session support ✅
- **Spec 25**: Claude-Assisted Worktree Merge - Automatic conflict resolution ✅
- **Spec 26**: Worktree CLI Flag - User-friendly parallel execution ✅
- **Spec 29**: Centralized Worktree State - Enhanced state management ✅
- **Spec 30**: Interrupted Worktree Recovery - Resume interrupted sessions (draft)

### Storage Specifications
Data persistence, caching, and storage optimizations.

- **Spec 11**: Simple State Management - JSON state persistence ✅
- **Spec 16**: Simplify State Management - Streamlined state structures ✅
- **Spec 29**: Centralized Worktree State - Worktree metadata storage ✅

### Compatibility Specifications
Integration with external systems and cross-platform support.

- **Spec 03**: Claude Integration - Claude CLI subprocess management ✅
- **Spec 21**: Configurable Workflow - Custom workflow support ✅
- **Spec 23**: Command Line Config Option - Flexible configuration ✅

### Testing Specifications
Test infrastructure and quality assurance.

- **Spec 08**: Iterative Improvement Loop - Automated testing in loop ✅

### Optimization Specifications
Performance improvements and system efficiency.

- **Spec 15**: Remove Developer Experience Bloat - Code simplification ✅
- **Spec 16**: Simplify State Management - Reduced complexity ✅
- **Spec 17**: Consolidate Core Modules - Module consolidation ✅
- **Spec 22**: Configurable Iteration Limit - Performance control ✅
- **Spec 28**: Structured Command Objects - Type-safe commands ✅

## Implementation Status

### ✅ Completed (18 specs)
- Spec 09: Dead Simple Improve
- Spec 10: Smart Project Analyzer  
- Spec 11: Simple State Management
- Spec 14: Implement Real Claude Loop
- Spec 15: Remove Developer Experience Bloat
- Spec 16: Simplify State Management
- Spec 17: Consolidate Core Modules
- Spec 18: Dynamic Spec Generation
- Spec 19: Git-Native Improvement Flow
- Spec 20: Focus-Directed Improvements
- Spec 21: Configurable Workflow
- Spec 22: Configurable Iteration Limit
- Spec 23: Command Line Config Option
- Spec 24: Git Worktree Isolation
- Spec 25: Claude-Assisted Worktree Merge
- Spec 26: Worktree CLI Flag
- Spec 28: Structured Command Objects
- Spec 29: Centralized Worktree State

### 📝 Draft (1 spec)
- Spec 30: Interrupted Worktree Recovery

### 🚧 In Progress (0 specs)
None currently in progress.

### ❌ Deprecated (7 specs)
- Spec 02: Project Management (removed - over-engineering)
- Spec 04: Workflow Automation (superseded by Spec 21)
- Spec 05: Monitoring and Reporting (removed - unnecessary complexity)
- Spec 06: Plugin System (removed - against dead simple philosophy)
- Spec 07: Claude CLI UX (merged into core functionality)
- Spec 12: Developer Experience (removed by Spec 15)
- Spec 13: Progressive Enhancement (replaced by focused specs)

## Specification Numbering

Specifications are numbered sequentially (01, 02, 03...) in order of creation. The number is permanent and never reused, even if a specification is deprecated or removed.

## Adding New Specifications

When adding a new specification:
1. Use the next available number
2. Choose the appropriate category
3. Follow the standard specification template
4. Update this index with the new entry
5. Update ROADMAP.md if needed

## Specification Dependencies

### Dependency Graph
```
Foundation Layer:
├── 01: Core Architecture
├── 09: Dead Simple Improve
├── 10: Smart Project Analyzer
├── 11: Simple State Management
└── 14: Real Claude Loop
    ├── 19: Git-Native Flow
    │   ├── 20: Focus-Directed
    │   ├── 21: Configurable Workflow
    │   └── 24: Worktree Isolation
    │       ├── 25: Claude-Assisted Merge
    │       ├── 26: Worktree CLI Flag
    │       └── 29: Centralized State
    │           └── 30: Interrupted Recovery
    └── 22: Iteration Limit

Optimization Layer:
├── 15: Remove Bloat
├── 16: Simplify State
├── 17: Consolidate Modules
├── 23: Config Options
└── 28: Structured Commands
```

## Quick Reference

### Latest Specifications
- Spec 30: Interrupted Worktree Recovery (draft) - Recovery from interrupted sessions
- Spec 29: Centralized Worktree State ✅ - Enhanced worktree metadata
- Spec 28: Structured Command Objects ✅ - Type-safe command system

### High Priority Specifications
- Spec 30: Interrupted Worktree Recovery - Critical for robust parallel execution
- (All other high priority specs are completed)

### Most Impactful Completed Specs
1. Spec 19: Git-Native Improvement Flow - Revolutionized the audit trail
2. Spec 24: Git Worktree Isolation - Enabled true parallel execution
3. Spec 14: Real Claude Loop - Made the tool actually functional