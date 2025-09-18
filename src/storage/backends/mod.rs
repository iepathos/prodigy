//! Storage backend implementations

pub mod file;
pub mod memory;

pub use file::FileBackend;
pub use memory::MemoryBackend;
