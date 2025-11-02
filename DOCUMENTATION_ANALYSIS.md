# Prodigy Book Documentation Analysis

## Overview

Complete analysis of all 16 chapters in the Prodigy book documentation (`book/src/`). This document maps the documented topics, sections, code examples, and features.

## Documentation Files

All chapter analysis results are stored in:
- **JSON Map**: `workflows/data/documentation-map.json` - Structured map of all chapters and sections
- **This Report**: `DOCUMENTATION_ANALYSIS.md` - Human-readable summary

## Chapter Breakdown

### 1. Workflow Basics (`book/src/workflow-basics.md`)
**Focus**: Foundation for creating Prodigy workflows

**Key Sections**:
- Overview: Workflow types (Standard vs MapReduce)
- Simple Workflows: Array format, sequential execution
- Full Workflow Structure: All configuration fields (env, secrets, profiles, merge)
- Command Types: Overview of 7 command types
- Command-Level Options: 11 configuration fields per command
- Environment Configuration: Layered configuration approach
- Merge Workflows: Custom merge with variables

**Features Documented**:
- Workflow structure and formats
- Simple vs full configuration options
- Basic command types overview
- Environment variable configuration
- Git worktree merge workflows

---

### 2. MapReduce Workflows (`book/src/mapreduce.md`)
**Focus**: Parallel processing with setup, map, and reduce phases

**Key Sections**:
- Quick Start: Minimal working example
- Complete Structure: All MapReduce fields and phases
- Map Phase: Input, JSONPath, templates, filtering, sorting
- Backoff Strategies: 4 strategies with duration examples
- Error Collection: 3 strategies (aggregate, immediate, batched)
- Setup Phase: Capture outputs with detailed configuration
- Global Storage: ~/.prodigy/ directory structure
- Event Tracking: JSONL format, lifecycle events
- Checkpoint/Resume: Job recovery with state preservation
- Dead Letter Queue: Failed item management and retry
- Common Pitfalls: 6 documented mistakes with solutions
- Performance Tuning: max_parallel, timeouts, circuit breaker
- Real-World Use Cases: 4 examples with complete configs

**Features Documented**:
- Complete MapReduce syntax
- 4 backoff strategies with examples
- Capture outputs configuration
- DLQ retry mechanism
- Resume from checkpoints
- Circuit breaker patterns
- Performance tuning guidelines

---

### 3. Command Types (`book/src/commands.md`)
**Focus**: Detailed documentation of all 6 command types

**Key Sections**:
- Shell Commands: Execution, capture, failure handling, timeout
- Claude Commands: Simple, with args, commit tracking
- Goal-Seeking: Iterative refinement with validation
- Foreach: Iteration with optional parallelism
- Write File: Text/JSON/YAML with format validation
- Validation: Schema validation with incomplete handling
- Command Reference: Common fields for all types
- Capture Formats: 5 formats (string, number, json, lines, boolean)
- Deprecated Fields: Legacy syntax and migrations

**Features Documented**:
- 6 command types with full syntax
- Capture format options
- Goal-seeking workflow pattern
- Foreach with parallel execution
- File writing with format validation
- Validation with gap-filling
- On-failure and on-success handlers

---

### 4. Variable Interpolation (`book/src/variables.md`)
**Focus**: Variable types, scoping, and interpolation

**Key Sections**:
- Overview: Built-in vs custom variables
- Availability by Phase: 5-row matrix showing phase availability
- Standard Variables: workflow.*, step.*, system variables
- Output Variables: shell.output, claude.output, last.*
- MapReduce Variables: item.*, map.*, worker.id
- Map Results Handling: Special treatment for large ${map.results}
- Merge Variables: merge.*
- Validation Variables: validation.*
- Git Context Variables: step.files_*, step.commits, etc.
- Custom Capture: capture field with formats
- Nested JSON Access: Dot notation for JSON fields
- Scope and Precedence: Variable shadowing rules

**Features Documented**:
- 8+ variable categories
- Phase-specific availability
- Custom capture with 5 formats
- Git context tracking
- Variable scope inheritance
- Nested JSON field access

---

### 5. Environment Configuration (`book/src/environment.md`)
**Focus**: Environment variables, secrets, profiles, and precedence

**Key Sections**:
- Architecture Overview: WorkflowConfig vs EnvironmentConfig
- Global Environment: Static variables, inheritance
- MapReduce Env Variables: Usage in all 4 phases
- Environment Files: .env format, loading order, precedence
- Secrets Management: 5 providers (aws, vault, env, file, custom)
- Environment Profiles: Named configurations with descriptions
- Per-Command Overrides: Shell-level syntax
- Environment Precedence: 4-level hierarchy
- Best Practices: 5 guidelines
- Common Patterns: 4 examples

**Features Documented**:
- Global environment variables
- Environment files with precedence
- Secret providers (aws, vault, env, file)
- Profile support
- MapReduce environment variables
- Secrets masking
- Per-command overrides

---

### 6. Advanced Features (`book/src/advanced.md`)
**Focus**: Conditional execution and nested control flow

**Key Sections**:
- Conditional Execution: when clauses with expression syntax
- Expression Syntax: Operators, type coercion, complex expressions
- On Success Handlers: Follow-up commands on success
- On Failure Handlers: Recovery with retry options
- Nested Conditionals: Chained conditional execution

**Features Documented**:
- Conditional execution with when clauses
- Expression operators (==, !=, &&, ||)
- Type coercion rules
- On success/failure handlers
- Nested conditional execution

---

### 7. Error Handling (`book/src/error-handling.md`)
**Focus**: Error handling at command and workflow levels

**Key Sections**:
- Command-Level Error Handling: on_failure configuration
- Simple Forms: Boolean, string, array of commands
- Advanced Configuration: Full handler syntax
- Handler Strategies: recovery, fallback, cleanup, custom
- Handler Configuration: strategy, timeout, capture, commands

**Features Documented**:
- on_failure handler with 3 forms
- Handler strategies (4 types)
- Retry with max_attempts
- Handler timeouts
- Handler output capture
- Fail workflow options

---

### 8. Examples (`book/src/examples.md`)
**Focus**: Real-world workflow examples

**Examples Included**:
1. Simple Build and Test
2. Coverage Improvement with Goal Seeking
3. Foreach Iteration (sequential + parallel)
4. Parallel Code Review (MapReduce)
5. Conditional Deployment

**Features Demonstrated**:
- Basic workflows
- Goal-seeking patterns
- Foreach loops
- MapReduce jobs
- Conditional execution

---

### 9. Retry Configuration (`book/src/retry-configuration.md`)
**Focus**: Retry mechanisms and backoff strategies

**Key Sections**:
- RetryConfig Structure: 9 configuration fields
- Backoff Strategies: 4 strategies with examples
- Fixed Backoff: Constant delays
- Linear Backoff: Incrementally increasing delays
- Exponential Backoff: Doubling delays
- Fibonacci Backoff: Sequence-based delays

**Features Documented**:
- Retry configuration with 9 options
- 4 backoff strategy types
- Duration format (humantime)
- Jitter configuration
- Retry budget
- Conditional retry

---

### 10. Workflow Composition (`book/src/composition.md`)
**Focus**: Reusable workflow patterns through imports and inheritance

**Key Sections**:
- Overview: Composition features
- Workflow Imports: Basic, with alias, selective
- Workflow Extension: Base resolution paths
- Parameter Definition: Workflow parameters

**Features Documented**:
- Workflow imports
- Selective imports with aliases
- Workflow extension/inheritance
- Base resolution search paths
- Parameter-driven workflows

---

### 11. Configuration (`book/src/configuration.md`)
**Focus**: Project and global configuration management

**Key Sections**:
- Configuration File Locations: 3 locations with search order
- Project Configuration: .prodigy/config.yml
- Global Configuration: ~/.prodigy/config.yml
- Format Support: YAML only (not TOML/JSON)
- Configuration Precedence: 4-level hierarchy

**Features Documented**:
- Configuration file locations
- Project vs global configuration
- Precedence rules
- YAML format requirement
- Environment variable overrides

---

### 12. Advanced Git Context (`book/src/git-context-advanced.md`)
**Focus**: Automatic git tracking and context variables

**Key Sections**:
- Automatic Tracking: GitChangeTracker initialization
- Step-Level Variables: Files changed, commits, statistics
- Workflow-Level Variables: Cumulative changes across steps
- Git Variables: 9 step-level + 9 workflow-level variables

**Features Documented**:
- Automatic git tracking
- File change tracking (added, modified, deleted)
- Commit tracking
- Modification statistics (insertions/deletions)
- Step and workflow-level aggregation

---

### 13. MapReduce Worktree Architecture (`book/src/mapreduce-worktree-architecture.md`)
**Focus**: Git worktree hierarchy and merge flows

**Key Sections**:
- Worktree Hierarchy: Parent and child worktrees
- Branch Naming: Conventions for parent and agent branches
- Merge Flow: Agent merge, parent merge
- Isolation Guarantees: Safety properties

**Features Documented**:
- Worktree hierarchy (parent + child)
- Branch naming conventions
- Merge flow processes
- Isolation guarantees
- Debugging strategies

---

### 14. Troubleshooting (`book/src/troubleshooting.md`)
**Focus**: Common issues and solutions

**Issues Covered**:
1. Variables not interpolating
2. Capture not working
3. Validation failing

**Features Documented**:
- Variable syntax issues
- Capture format problems
- Validation configuration
- Debug techniques

---

### 15. Automated Documentation (`book/src/automated-documentation.md`)
**Focus**: Automated book documentation system

**Key Sections**:
- Overview: Feature analysis, drift detection, updates
- Prerequisites: Tools and setup
- Quick Start: 3 steps (init, structure, config)

**Features Documented**:
- Automated documentation workflow
- Feature analysis
- Drift detection
- Documentation updates
- mdBook integration

---

### 16. Summary (`book/src/SUMMARY.md`)
**Note**: Metadata file for mdBook structure

---

## Documentation Coverage Summary

### Well-Documented Features (100% coverage)

1. **Workflow Structure**
   - Simple and full formats
   - All field types and options
   - Multiple examples

2. **MapReduce Workflows**
   - All 4 phases (setup, map, reduce, merge)
   - Error policies
   - Retry and backoff strategies
   - DLQ and resume
   - Performance tuning

3. **Command Types**
   - 6 command types fully documented
   - Capture formats and streams
   - Error handling per command
   - Timeout and conditional execution

4. **Variables**
   - 8+ variable categories
   - Phase availability matrix
   - Custom capture
   - Git context tracking
   - Scope and precedence

5. **Environment Configuration**
   - Global and per-command overrides
   - Secrets providers
   - Profile support
   - Precedence rules
   - MapReduce environment variables

6. **Error Handling**
   - on_failure handlers
   - Handler strategies
   - Retry configuration
   - Timeout handling

### Partially Documented Features

1. **Advanced Composition**
   - Selective imports documented
   - Parameter definition mentioned but limited examples
   - Template registry not fully documented

2. **Custom Handlers**
   - strategy field documented
   - Custom strategy examples limited

3. **Profile Activation**
   - Profiles defined in YAML
   - Runtime activation not documented (CLI flag missing)

4. **Per-Command Environment**
   - Modern syntax uses shell ENV=value style
   - No native per-command env field in WorkflowStepCommand

### Missing or Minimal Coverage

1. **Analysis Handlers**
   - `analyze:` command type mentioned but not detailed
   - Coverage metrics handling

2. **Complex Circuit Breaker Patterns**
   - Basic configuration documented
   - Advanced tuning patterns minimal

3. **Session Management**
   - UnifiedSession structure mentioned in CLAUDE.md
   - Book documentation focuses on operational usage

## Code Examples in Documentation

### Total Count: 150+ examples

**By Command Type**:
- Shell commands: 25+ examples
- Claude commands: 20+ examples
- MapReduce workflows: 15+ examples
- Goal-seeking: 5+ examples
- Foreach: 8+ examples
- Write file: 10+ examples
- Validation: 8+ examples

**By Feature**:
- Error handling: 20+ examples
- Variable interpolation: 15+ examples
- Environment configuration: 20+ examples
- Conditional execution: 12+ examples
- Capture formats: 10+ examples
- Backoff strategies: 8+ examples

## Documentation Statistics

- **Total Chapters**: 16
- **Total Sections**: 100+
- **Code Examples**: 150+
- **Configuration Options Documented**: 200+
- **Variables Documented**: 50+
- **Error Patterns Covered**: 20+
- **Best Practices**: 40+

## Key Documentation Strengths

1. **Comprehensive Examples**: Nearly every feature has 2-5 examples
2. **Practical Use Cases**: Real-world workflows for common patterns
3. **Troubleshooting**: Common errors with clear solutions
4. **Clear Progression**: Basic → Advanced → Real-world use cases
5. **Phase Availability Matrix**: Clear guidance on when variables are available
6. **Comparison Tables**: Easy comparison of options (simple vs full format)
7. **Cross-References**: Links between related chapters

## Recommendations for Content Gaps

1. **Expand Advanced Composition**
   - More template examples
   - Parameter definition patterns

2. **Add Circuit Breaker Patterns**
   - Advanced tuning guide
   - Production scenarios

3. **Enhance Analysis Handlers**
   - Complete documentation of `analyze:` command
   - Coverage metrics patterns

4. **Add Profile Activation Guide**
   - When profile support is available at runtime
   - CLI usage examples

5. **Document Session Management**
   - UnifiedSession usage
   - Resume with session IDs

## How to Use This Analysis

The `documentation-map.json` file provides a structured index of:
- Chapter IDs and paths
- Section headings with sub-topics
- Features documented in each chapter
- Configuration options covered

Use this to:
1. **Find Documentation**: Locate which chapter documents a feature
2. **Identify Gaps**: Spot missing or incomplete documentation
3. **Plan Updates**: Track which features need documentation
4. **Compare Features**: Map features to documented sections
5. **Generate Reports**: Build documentation coverage reports

## Files Generated

1. `/workflows/data/documentation-map.json` - Structured chapter map
2. `/DOCUMENTATION_ANALYSIS.md` - This report (you are here)
