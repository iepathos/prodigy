---
number: 110
title: Terminal UI Foundation - Core Library Integration
category: foundation
priority: high
status: draft
dependencies: []
created: 2025-10-01
---

# Specification 110: Terminal UI Foundation - Core Library Integration

**Category**: foundation
**Priority**: high
**Status**: draft
**Dependencies**: None

## Context

Prodigy currently uses basic `println!` and `eprintln!` for output, with minimal use of the `indicatif` crate for progress bars. The CLI lacks modern terminal UI features such as:
- Rich table formatting
- Interactive prompts and selections
- Color-coded output with proper terminal capability detection
- Consistent styling across commands

This foundation specification establishes the core terminal UI libraries and abstractions that other UI improvements will build upon.

## Objective

Integrate and configure foundational terminal UI libraries (`console`, `dialoguer`, `comfy-table`) to provide a consistent, modern CLI experience with proper terminal capability detection and cross-platform support.

## Requirements

### Functional Requirements

**FR1**: Integrate `console` crate for terminal abstractions
- Add `console = "0.15"` dependency
- Create terminal capability detection module
- Implement color/styling utilities
- Support both Unicode and ASCII fallback modes

**FR2**: Integrate `dialoguer` crate for interactive prompts
- Add `dialoguer = "0.11"` dependency
- Configure with ColorfulTheme by default
- Create prompt builder utilities
- Support non-interactive mode for CI/CD

**FR3**: Integrate `comfy-table` crate for rich tables
- Add `comfy-table = "7"` dependency
- Create table builder utilities with consistent styling
- Support both bordered and simple table modes
- Handle terminal width detection

**FR4**: Terminal capability detection
- Detect color support (truecolor, 256-color, 16-color, none)
- Detect Unicode support (UTF-8 vs ASCII)
- Detect interactive vs non-interactive terminals
- Support TERM environment variable inspection

**FR5**: Consistent styling system
- Define color palette (success=green, warning=yellow, error=red, info=cyan)
- Create styled output utilities (success, warning, error, info messages)
- Implement icon system with Unicode/ASCII fallbacks
- Support NO_COLOR and CLICOLOR environment variables

### Non-Functional Requirements

**NFR1**: Performance - Library initialization overhead < 10ms
**NFR2**: Compatibility - Support Windows, macOS, Linux terminals
**NFR3**: Testability - All terminal output abstracted for unit testing
**NFR4**: Maintainability - Clear separation between UI and business logic

## Acceptance Criteria

- [ ] `console`, `dialoguer`, and `comfy-table` crates added to Cargo.toml
- [ ] Terminal capability detection module created in `src/terminal/capabilities.rs`
- [ ] Styling utilities module created in `src/terminal/styles.rs`
- [ ] Table builder utilities created in `src/terminal/tables.rs`
- [ ] Prompt builder utilities created in `src/terminal/prompts.rs`
- [ ] All existing `println!` error/warning messages migrated to styled utilities
- [ ] Terminal output properly degrades on non-color/non-Unicode terminals
- [ ] Non-interactive mode works correctly (no prompts hang)
- [ ] Unit tests for all terminal utilities with mock terminal output
- [ ] Integration tests verify output on different terminal types
- [ ] Documentation added for using terminal utilities in code

## Technical Details

### Implementation Approach

**Phase 1: Library Integration**
1. Add dependencies to Cargo.toml
2. Create `src/terminal/` module with submodules
3. Implement terminal capability detection
4. Create styling utilities with fallback support

**Phase 2: Utility Functions**
1. Implement table builder utilities
2. Implement prompt builder utilities
3. Create icon system with Unicode/ASCII variants
4. Build color palette and styling presets

**Phase 3: Migration**
1. Migrate existing error/warning messages to styled utilities
2. Update progress display to use new styling
3. Ensure backward compatibility with existing output

### Module Structure

```rust
src/terminal/
├── mod.rs              // Public API
├── capabilities.rs     // Terminal capability detection
├── styles.rs          // Styling utilities and color palette
├── tables.rs          // Table builder utilities
├── prompts.rs         // Interactive prompt builders
└── icons.rs           // Icon system with fallbacks
```

### Key APIs

```rust
// Terminal capabilities
pub struct TerminalCapabilities {
    pub colors_supported: bool,
    pub unicode_supported: bool,
    pub is_interactive: bool,
    pub width: Option<usize>,
}

// Styling utilities
pub fn success(msg: &str) -> String;
pub fn warning(msg: &str) -> String;
pub fn error(msg: &str) -> String;
pub fn info(msg: &str) -> String;

// Table builder
pub struct TableBuilder {
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
    style: TableStyle,
}

// Prompt builder
pub struct PromptBuilder {
    message: String,
    default: Option<String>,
    validator: Option<Box<dyn Fn(&String) -> Result<(), String>>>,
}
```

### Environment Variables

Support standard terminal environment variables:
- `NO_COLOR` - Disable all color output
- `CLICOLOR` - Enable/disable color (0/1)
- `CLICOLOR_FORCE` - Force color even in non-TTY
- `TERM` - Terminal type detection
- `LANG`/`LC_ALL` - Unicode support detection

## Dependencies

- **Prerequisites**: None
- **Affected Components**:
  - All CLI commands that produce output
  - Progress display system
  - Error handling and display
- **External Dependencies**:
  - `console = "0.15"`
  - `dialoguer = "0.11"`
  - `comfy-table = "7"`

## Testing Strategy

### Unit Tests

```rust
#[test]
fn test_terminal_capability_detection() {
    // Test with various TERM values
}

#[test]
fn test_color_fallback_no_color_env() {
    // Verify NO_COLOR is respected
}

#[test]
fn test_unicode_fallback() {
    // Test ASCII fallback when Unicode unsupported
}

#[test]
fn test_table_builder() {
    // Verify table formatting
}

#[test]
fn test_prompt_non_interactive_mode() {
    // Ensure prompts don't hang in non-interactive mode
}
```

### Integration Tests

- Test output on color vs no-color terminals
- Test Unicode vs ASCII fallback
- Test interactive vs non-interactive modes
- Test Windows Terminal, iTerm2, standard Linux terminal

### Manual Testing

- Verify output in: iTerm2 (macOS), Terminal.app, Windows Terminal, Linux GNOME Terminal
- Test with `NO_COLOR=1` environment variable
- Test in CI/CD environment (GitHub Actions)

## Documentation Requirements

### Code Documentation

- Document all public APIs in `src/terminal/` module
- Add usage examples in doc comments
- Document environment variable behavior

### User Documentation

- Update CLAUDE.md with terminal capability information
- Document supported terminal types and features
- Add troubleshooting section for rendering issues

### Developer Documentation

- Create `docs/terminal-ui.md` with design patterns
- Document how to use terminal utilities in new commands
- Add migration guide for existing println! calls

## Implementation Notes

### Terminal Capability Detection

Use layered detection approach:
1. Check environment variables (NO_COLOR, CLICOLOR, etc.)
2. Check if stdout is TTY
3. Parse TERM environment variable
4. Detect Unicode from LANG/LC_ALL

### Color Palette

Standardize on semantic colors:
- Success: Green (`console::Color::Green`)
- Warning: Yellow (`console::Color::Yellow`)
- Error: Red (`console::Color::Red`)
- Info: Cyan (`console::Color::Cyan`)
- Dim: Gray (`console::Color::Black` with bright)

### Icon System

Provide Unicode/ASCII variants:
- Success: ✓ / [OK]
- Error: ✗ / [ERR]
- Warning: ⚠ / [WARN]
- Info: ℹ / [INFO]
- Progress: ⏳ / ...

### Non-Interactive Mode

Detect non-interactive mode:
- `!stdout().is_terminal()`
- CI environment variables (CI=true, GITHUB_ACTIONS, etc.)
- Explicit `--non-interactive` flag

In non-interactive mode:
- All prompts use default values
- No spinners or progress bars (use simple text)
- Log all assumed defaults

## Migration and Compatibility

### Breaking Changes

None - This is additive functionality. Existing output will be enhanced but remain functional.

### Migration Path

1. Phase 1: Add new terminal utilities alongside existing output
2. Phase 2: Gradually migrate commands to use new utilities
3. Phase 3: Deprecate direct println! usage in favor of terminal utilities

### Backward Compatibility

- All terminal utilities gracefully degrade on unsupported terminals
- ASCII fallbacks ensure functionality on basic terminals
- Non-interactive mode ensures CI/CD compatibility
- Existing output behavior preserved as baseline

## Success Metrics

- All new terminal libraries integrated without breaking existing functionality
- Terminal capability detection correctly identifies 95%+ of terminal types
- Output properly degrades on non-color/non-Unicode terminals
- No prompt hangs in non-interactive mode (CI/CD tests pass)
- Code using terminal utilities is more readable than raw println! calls
