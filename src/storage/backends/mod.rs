//! Storage backend implementations

pub mod file;
pub mod memory;

#[cfg(feature = "postgres")]
pub mod postgres;
#[cfg(feature = "redis")]
pub mod redis;
#[cfg(feature = "s3")]
pub mod s3;

pub use file::FileBackend;
pub use memory::MemoryBackend;

#[cfg(feature = "postgres")]
pub use postgres::PostgresBackend;
#[cfg(feature = "redis")]
pub use redis::RedisBackend;
#[cfg(feature = "s3")]
pub use s3::S3Backend;
