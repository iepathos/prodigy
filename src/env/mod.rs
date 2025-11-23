//! Environment abstraction layer for dependency injection and testing
//!
//! This module provides trait-based abstractions for all I/O operations, enabling:
//! - Separation of pure business logic from side effects
//! - Easy mocking and testing without actual I/O
//! - Explicit dependency tracking through environment types
//! - Composable effects using the stillwater library
//!
//! # Architecture
//!
//! The environment system follows these principles:
//! - **Traits**: Define capabilities (FileEnv, ProcessEnv, GitEnv, DbEnv)
//! - **Real Implementations**: Actual I/O operations for production use
//! - **Mock Implementations**: In-memory operations for testing
//! - **Combined Environment**: AppEnv bundles all capabilities together
//!
//! # Usage
//!
//! ## Production Code
//!
//! ```no_run
//! use prodigy::env::AppEnv;
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let env = AppEnv::real();
//!     let content = env.fs.read_to_string(std::path::Path::new("config.yml"))?;
//!     Ok(())
//! }
//! ```
//!
//! ## Testing
//!
//! ```
//! use prodigy::env::{AppEnv, MockFileEnv};
//! use std::sync::Arc;
//! use std::path::Path;
//!
//! let env = AppEnv::mock();
//!
//! // Setup mock data
//! let mock_fs = MockFileEnv::new();
//! mock_fs.add_file("config.yml", "test data");
//!
//! // Create custom env with our mock
//! let test_env = AppEnv::custom(
//!     Arc::new(mock_fs),
//!     env.process.clone(),
//!     env.git.clone(),
//!     env.db.clone(),
//! );
//!
//! // Test without actual file I/O
//! let content = test_env.fs.read_to_string(Path::new("config.yml")).unwrap();
//! assert_eq!(content, "test data");
//! ```

mod app;
mod mock;
mod real;
mod traits;

pub use app::AppEnv;
pub use mock::{MockDbEnv, MockFileEnv, MockGitEnv, MockProcessEnv};
pub use real::{RealDbEnv, RealFileEnv, RealGitEnv, RealProcessEnv};
pub use traits::{DbEnv, FileEnv, GitEnv, ProcessEnv};
