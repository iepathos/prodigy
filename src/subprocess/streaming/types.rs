//! Core types for streaming infrastructure

use std::time::Duration;

/// Streaming mode configuration
#[derive(Debug, Clone)]
pub enum StreamingMode {
    /// Traditional batch capture (default)
    Batch,
    /// Line-by-line streaming
    Streaming,
    /// Streaming with structured parsing
    StructuredStreaming { format: OutputFormat },
}

impl Default for StreamingMode {
    fn default() -> Self {
        Self::Batch
    }
}

/// Output format for structured streaming
#[derive(Debug, Clone)]
pub enum OutputFormat {
    PlainText,
    JsonLines,
    YamlStream,
}

/// Stream source identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamSource {
    Stdout,
    Stderr,
}

/// Buffer configuration for streaming
#[derive(Debug, Clone)]
pub struct BufferConfig {
    /// Line buffer size in bytes
    pub line_buffer_size: usize,
    /// Maximum number of lines to keep in memory
    pub max_lines: Option<usize>,
    /// Buffer overflow strategy
    pub overflow_strategy: crate::subprocess::streaming::backpressure::OverflowStrategy,
    /// Maximum time to wait for buffer space (for Block strategy)
    pub block_timeout: Duration,
}

impl Default for BufferConfig {
    fn default() -> Self {
        Self {
            line_buffer_size: 8192,
            max_lines: Some(10000),
            overflow_strategy:
                crate::subprocess::streaming::backpressure::OverflowStrategy::DropOldest,
            block_timeout: Duration::from_secs(5),
        }
    }
}

/// Streaming configuration
#[derive(Debug, Clone)]
pub struct StreamingConfig {
    /// Enable streaming mode
    pub enabled: bool,
    /// Streaming mode
    pub mode: StreamingMode,
    /// Buffer configuration
    pub buffer_config: BufferConfig,
    /// Processor configurations
    pub processors: Vec<ProcessorConfig>,
}

impl Default for StreamingConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            mode: StreamingMode::Batch,
            buffer_config: BufferConfig::default(),
            processors: Vec::new(),
        }
    }
}

/// Processor configuration
#[derive(Debug, Clone)]
pub enum ProcessorConfig {
    /// Parse JSON lines and emit events
    JsonLines { emit_events: bool },
    /// Match patterns in output
    PatternMatcher { patterns: Vec<regex::Regex> },
    /// Emit events of a specific type
    EventEmitter { event_type: String },
    /// Custom processor (not cloneable, so we use a marker)
    Custom { id: String },
}

/// Output from streaming command execution
#[derive(Debug)]
pub struct StreamingOutput {
    /// Process exit status
    pub status: std::process::ExitStatus,
    /// Captured stdout lines
    pub stdout: Vec<String>,
    /// Captured stderr lines
    pub stderr: Vec<String>,
    /// Execution duration
    pub duration: Duration,
}
