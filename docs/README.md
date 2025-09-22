# Prodigy Documentation

## Architecture & Design

- [**ARCHITECTURE.md**](../ARCHITECTURE.md) - System architecture overview including functional core/imperative shell pattern
- [**Functional Core Pattern**](functional-core-pattern.md) - In-depth guide to the functional core, imperative shell architecture
- [**Refactoring Guide**](refactoring-guide.md) - Practical guide for separating I/O from business logic

## Development Guides

- [**Testing Improvements**](testing-improvements.md) - Testing strategies and improvements
- [**Input Abstraction**](input_abstraction.md) - Input handling and abstraction patterns
- [**Goal Seeking**](goal-seeking.md) - Goal-seeking workflow implementation

## Features & Capabilities

- [**MapReduce Resume Guide**](mapreduce-resume-guide.md) - Guide to resuming MapReduce jobs
- [**Resume Workflows**](resume-workflows.md) - Workflow resumption capabilities
- [**Streaming**](streaming.md) - Streaming output and real-time monitoring

## Project Documentation

- [**Prodigy Whitepaper**](PRODIGY_WHITEPAPER.md) - Original design and vision document
- [**MVP Launch**](MVP_LAUNCH.md) - MVP launch planning and requirements
- [**Perfect Loop**](perfect_loop.md) - The perfect development loop concept

## Examples & Usage

- [**Debtmap Example Usage**](debtmap_example_usage.md) - Example usage patterns
- [**Improvements for MMM**](improvements_for_mmm.md) - Proposed improvements

## Quick Links

### For New Contributors

1. Start with [ARCHITECTURE.md](../ARCHITECTURE.md) to understand the system
2. Read [Functional Core Pattern](functional-core-pattern.md) to understand our architecture principles
3. Use [Refactoring Guide](refactoring-guide.md) when modifying existing code

### For Developers

1. [Testing Improvements](testing-improvements.md) - Write better tests
2. [Resume Workflows](resume-workflows.md) - Implement resumable workflows
3. [MapReduce Resume Guide](mapreduce-resume-guide.md) - Handle distributed execution

### Architecture Principles

The project follows the **Functional Core, Imperative Shell** pattern:

- **Pure business logic** in `src/core/` - No I/O, no side effects
- **I/O operations** in outer modules - Thin wrappers around core logic
- **Testability first** - Pure functions are easy to test
- **Clear separation** - Business logic separate from infrastructure