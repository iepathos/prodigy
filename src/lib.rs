//! # Memento Mori (mmm)
//!
//! A dead simple Rust CLI tool that makes your code better through Claude CLI integration.
//!
//! ## Usage
//!
//! ```bash
//! mmm improve [--target 8.0] [--verbose] [--focus "area"]
//! ```
//!
//! ## Modules
//!
//! - `analyzer` - Project analysis functionality for language and framework detection
//! - `config` - Configuration management for the tool
//! - `improve` - Core improvement command implementation
//! - `project` - Project management utilities
//! - `simple_state` - Minimal state management with JSON persistence

pub mod analyzer;
pub mod config;
pub mod improve;
pub mod project;
pub mod simple_state;
