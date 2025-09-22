---
number: 105
title: Extract CLI Logic from Main Module
category: foundation
priority: high
status: draft
dependencies: [101]
created: 2025-09-22
---

# Specification 105: Extract CLI Logic from Main Module

## Context

The `main.rs` file is 2,921 lines and mixes CLI argument parsing, business logic, error handling, and application initialization. This violates VISION.md principles of separation of concerns and functional programming. The file is difficult to test and maintain.

Current issues:
- CLI parsing mixed with business logic
- Application initialization scattered throughout
- Difficult to unit test business logic
- Poor separation of I/O from pure functions
- Multiple unwrap() calls in critical paths

## Objective

Extract CLI parsing and argument handling into dedicated modules, leaving main.rs as a thin entry point that composes the application from focused, testable components.

## Requirements

### Functional Requirements
- Extract CLI parsing into `cli/` module
- Extract application initialization into `app/` module
- Main.rs should be under 200 lines
- Pure business logic separated from I/O
- Comprehensive error handling for CLI inputs
- Maintain all current CLI functionality

### Non-Functional Requirements
- Improved testability of CLI logic
- Clear separation of concerns
- Consistent error handling patterns
- Better help text organization
- Faster build times from smaller main.rs

## Acceptance Criteria

- [ ] `main.rs` reduced to under 200 lines
- [ ] CLI parsing extracted to `cli/` module
- [ ] Application logic extracted to `app/` module
- [ ] All CLI functionality preserved
- [ ] Comprehensive unit tests for CLI parsing
- [ ] Integration tests for application flows
- [ ] Help text generation centralized and testable
- [ ] Error messages consistent across all commands

## Technical Details

### Proposed Module Structure

```
src/
├── main.rs                # Entry point only (<200 lines)
├── cli/
│   ├── mod.rs            # CLI command definitions
│   ├── args.rs           # Argument parsing
│   ├── commands.rs       # Command implementations
│   ├── help.rs           # Help text generation
│   └── validation.rs     # Input validation
├── app/
│   ├── mod.rs            # Application initialization
│   ├── config.rs         # Configuration loading
│   ├── logging.rs        # Logging setup
│   └── runtime.rs        # Runtime initialization
└── [existing modules]
```

### Implementation Approach

1. **Phase 1: Extract CLI Definitions**
   - Move clap command definitions to cli module
   - Extract argument parsing logic
   - Create testable command handlers

2. **Phase 2: Extract Application Logic**
   - Move initialization code to app module
   - Separate configuration loading
   - Extract runtime setup

3. **Phase 3: Refactor Main Entry Point**
   - Slim down main.rs to pure composition
   - Implement functional error handling
   - Use dependency injection patterns

### Functional Programming Patterns

```rust
// main.rs - Before (mixed concerns)
#[tokio::main]
async fn main() -> Result<()> {
    // 100+ lines of argument parsing
    // 200+ lines of initialization
    // 500+ lines of command logic
    // Error handling scattered throughout
}

// main.rs - After (pure composition)
#[tokio::main]
async fn main() -> Result<()> {
    let args = cli::parse_args()?;
    let app = app::initialize(&args).await?;
    cli::execute_command(args, app).await
}

// cli/commands.rs - Pure business logic
pub async fn execute_command(
    args: Args,
    app: App,
) -> Result<Output> {
    match args.command {
        Command::Run(opts) => run::execute(opts, &app).await,
        Command::Init(opts) => init::execute(opts, &app).await,
        // ... other commands
    }
}
```

### Error Handling Improvement

```rust
// Before: Unwraps in main.rs
let current_dir = std::env::current_dir().unwrap();

// After: Proper error handling in cli module
pub fn parse_args() -> Result<Args, CliError> {
    let current_dir = std::env::current_dir()
        .context("Failed to determine current working directory")?;
    // ... continue with error context
}
```

## Dependencies

- **Spec 101**: Requires proper error handling foundation

## Testing Strategy

- Unit tests for CLI argument parsing
- Integration tests for complete command flows
- Property-based tests for argument validation
- Error scenario testing for all CLI paths
- Help text generation tests

## Documentation Requirements

- Update CLI development guidelines
- Document command addition process
- Create testing guide for CLI components
- Update help text maintenance procedures
- Document error handling patterns for CLI