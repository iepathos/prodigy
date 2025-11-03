//! Worktree command implementation
//!
//! This module handles git worktree management for parallel sessions.
//!
//! # Architecture
//!
//! This module follows a functional programming architecture with clear separation of concerns:
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                      Public API (mod.rs)                    │
//! │  - run_worktree_command: Main entry point                  │
//! │  - parse_duration: Utility export                          │
//! └─────────────────────────────────────────────────────────────┘
//!                              │
//!                              ▼
//! ┌─────────────────────────────────────────────────────────────┐
//! │                     CLI Layer (cli.rs)                      │
//! │  - Command routing and argument handling                    │
//! │  - Thin orchestration (< 30 lines per function)            │
//! │  - Dependency initialization                                │
//! └─────────────────────────────────────────────────────────────┘
//!          │                    │                    │
//!          ▼                    ▼                    ▼
//! ┌──────────────┐   ┌──────────────────┐   ┌──────────────┐
//! │ Operations   │   │  Presentation    │   │   Utils      │
//! │ (operations) │   │  (presentation)  │   │   (utils)    │
//! └──────────────┘   └──────────────────┘   └──────────────┘
//!
//!   Business Logic      Output Formatting     Pure Functions
//!   - list_sessions     - format_table        - parse_duration
//!   - merge_session     - format_result
//!   - cleanup           - format_summary
//! ```
//!
//! ## Modules
//!
//! ### `cli`
//! Command handlers that orchestrate operations and presentation.
//! - Handles CLI argument parsing
//! - Initializes dependencies (WorktreeManager, etc.)
//! - Calls operation functions
//! - Calls presentation functions for output
//! - Keeps functions thin (< 30 lines each)
//!
//! ### `operations`
//! Pure business logic functions that return structured results.
//! - Takes dependencies as parameters (dependency injection)
//! - Returns Result types with structured data
//! - No direct I/O (printing, file operations)
//! - Fully testable without mocking I/O
//! - Examples: `list_sessions_operation`, `merge_session_operation`
//!
//! ### `presentation`
//! Pure functions that format data for display.
//! - Takes data structures and returns formatted strings
//! - No I/O operations (printing done by caller)
//! - Easy to test output formatting
//! - Examples: `format_sessions_table`, `format_merge_result`
//!
//! ### `utils`
//! Stateless utility functions.
//! - Pure functions with no side effects
//! - 100% test coverage
//! - Example: `parse_duration` (parses duration strings like "1h", "7d")
//!
//! ## Design Principles
//!
//! 1. **Functional Core, Imperative Shell**
//!    - Pure business logic in `operations` and `utils`
//!    - I/O at edges in `cli`
//!    - Clear separation makes code testable
//!
//! 2. **Dependency Injection**
//!    - Functions take dependencies as parameters
//!    - No global state or hidden dependencies
//!    - Easy to mock for testing
//!
//! 3. **Single Responsibility**
//!    - Each module has one clear purpose
//!    - Small, focused functions (< 20 lines preferred)
//!    - Easy to understand and maintain
//!
//! 4. **Structured Results**
//!    - Operations return typed result structs
//!    - Makes data flow explicit
//!    - Enables clean presentation layer
//!
//! ## Examples
//!
//! ### Listing Sessions
//! ```text
//! CLI: run_worktree_ls()
//!   │
//!   ├─> Operations: list_sessions_operation() → SessionListResult
//!   │
//!   └─> Presentation: format_sessions_table() → String
//!         └─> CLI: println!(formatted_output)
//! ```
//!
//! ### Merging Sessions
//! ```text
//! CLI: run_worktree_merge()
//!   │
//!   ├─> Operations: merge_session_operation() → MergeResult
//!   │
//!   └─> Presentation: format_merge_result() → String
//!         └─> CLI: println!(formatted_output)
//! ```
//!
//! ## Testing Strategy
//!
//! - **Utils**: Unit tests for all pure functions (100% coverage)
//! - **Operations**: Unit tests for data structures, integration tests for operations
//! - **Presentation**: Unit tests for formatting logic with various inputs
//! - **CLI**: Integration tests for end-to-end flows
//!
//! ## Adding New Commands
//!
//! To add a new worktree command:
//!
//! 1. Add operation function in `operations.rs`
//!    - Define result type if needed
//!    - Write pure business logic
//!    - Add unit tests
//!
//! 2. Add presentation function in `presentation.rs`
//!    - Write formatting logic
//!    - Add unit tests
//!
//! 3. Add CLI handler in `cli.rs`
//!    - Initialize dependencies
//!    - Call operation
//!    - Call presentation
//!    - Handle errors
//!
//! 4. Update `run_worktree_command` match statement
//!
//! 5. Add integration test

mod cli;
mod operations;
mod presentation;
mod utils;

pub use cli::run_worktree_command;
pub use utils::parse_duration;
