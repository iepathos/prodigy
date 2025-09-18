//! Real-time streaming infrastructure for subprocess output
//!
//! This module provides line-by-line output capture, real-time event processing,
//! and multiplexed stream handling for subprocess execution. It enables visibility
//! into long-running processes while maintaining full backward compatibility.

pub mod backpressure;
pub mod claude_processor;
pub mod processor;
pub mod runner;
pub mod types;

#[cfg(test)]
mod tests;

pub use backpressure::{BufferedStreamProcessor, OverflowStrategy};
pub use claude_processor::{ClaudeJsonProcessor, ClaudeStreamHandler, LoggingClaudeHandler};
pub use processor::{JsonLineProcessor, LoggingProcessor, PatternMatchProcessor, StreamProcessor};
pub use runner::StreamingCommandRunner;
pub use types::{
    BufferConfig, OutputFormat, ProcessorConfig, StreamSource, StreamingConfig, StreamingMode,
    StreamingOutput,
};
