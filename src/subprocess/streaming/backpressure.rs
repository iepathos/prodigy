//! Backpressure management for stream processing

use super::processor::StreamProcessor;
use super::types::StreamSource;
use anyhow::Result;
use async_trait::async_trait;
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time;

/// Strategy for handling buffer overflow
#[derive(Debug, Clone)]
pub enum OverflowStrategy {
    /// Drop oldest lines when buffer is full
    DropOldest,
    /// Drop newest lines when buffer is full
    DropNewest,
    /// Block until space is available
    Block,
    /// Fail with an error
    Fail,
}

/// Buffered stream processor with backpressure management
pub struct BufferedStreamProcessor {
    inner: Box<dyn StreamProcessor>,
    buffer: Arc<Mutex<VecDeque<BufferedLine>>>,
    max_buffer_size: usize,
    overflow_strategy: OverflowStrategy,
    block_timeout: Duration,
}

/// Buffered line with metadata
#[derive(Clone)]
struct BufferedLine {
    line: String,
    source: StreamSource,
}

impl BufferedStreamProcessor {
    /// Create a new buffered stream processor
    pub fn new(
        inner: Box<dyn StreamProcessor>,
        max_buffer_size: usize,
        overflow_strategy: OverflowStrategy,
        block_timeout: Duration,
    ) -> Self {
        Self {
            inner,
            buffer: Arc::new(Mutex::new(VecDeque::new())),
            max_buffer_size,
            overflow_strategy,
            block_timeout,
        }
    }

    /// Process buffered lines with backpressure handling
    pub async fn process_with_backpressure(
        &self,
        line: String,
        source: StreamSource,
    ) -> Result<()> {
        let mut buffer = self.buffer.lock().await;

        if buffer.len() >= self.max_buffer_size {
            match self.overflow_strategy {
                OverflowStrategy::DropOldest => {
                    // Remove the oldest line
                    buffer.pop_front();
                    buffer.push_back(BufferedLine { line, source });
                    tracing::warn!("Buffer overflow: dropped oldest line");
                }
                OverflowStrategy::DropNewest => {
                    // Simply don't add the new line
                    tracing::warn!("Buffer overflow: dropped newest line");
                }
                OverflowStrategy::Block => {
                    // Release lock and wait for space
                    drop(buffer);

                    let start = std::time::Instant::now();
                    while start.elapsed() < self.block_timeout {
                        time::sleep(Duration::from_millis(10)).await;

                        let mut buffer = self.buffer.lock().await;
                        if buffer.len() < self.max_buffer_size {
                            buffer.push_back(BufferedLine { line, source });
                            return Ok(());
                        }
                    }

                    return Err(anyhow::anyhow!(
                        "Buffer overflow: timeout waiting for space"
                    ));
                }
                OverflowStrategy::Fail => {
                    return Err(anyhow::anyhow!(
                        "Buffer overflow: max size {} reached",
                        self.max_buffer_size
                    ));
                }
            }
        } else {
            buffer.push_back(BufferedLine { line, source });
        }

        Ok(())
    }

    /// Process all buffered lines
    pub async fn flush(&self) -> Result<()> {
        let mut buffer = self.buffer.lock().await;
        let lines: Vec<BufferedLine> = buffer.drain(..).collect();
        drop(buffer);

        for buffered in lines {
            self.inner
                .process_line(&buffered.line, buffered.source)
                .await?;
        }

        Ok(())
    }

    /// Get current buffer size
    pub async fn buffer_size(&self) -> usize {
        self.buffer.lock().await.len()
    }
}

#[async_trait]
impl StreamProcessor for BufferedStreamProcessor {
    async fn process_line(&self, line: &str, source: StreamSource) -> Result<()> {
        // Add to buffer
        self.process_with_backpressure(line.to_string(), source)
            .await?;

        // Try to process immediately if possible
        let mut buffer = self.buffer.lock().await;
        if let Some(buffered) = buffer.pop_front() {
            drop(buffer); // Release lock before processing
            self.inner
                .process_line(&buffered.line, buffered.source)
                .await?;
        }

        Ok(())
    }

    async fn on_complete(&self, exit_code: Option<i32>) -> Result<()> {
        // Flush remaining buffer
        self.flush().await?;

        // Forward to inner processor
        self.inner.on_complete(exit_code).await
    }

    async fn on_error(&self, error: &anyhow::Error) -> Result<()> {
        // Try to flush buffer even on error
        let _ = self.flush().await;

        // Forward to inner processor
        self.inner.on_error(error).await
    }
}

/// Rate-limited processor to prevent overwhelming downstream systems
pub struct RateLimitedProcessor {
    inner: Box<dyn StreamProcessor>,
    max_lines_per_second: usize,
    last_process_time: Arc<Mutex<std::time::Instant>>,
    lines_processed: Arc<Mutex<usize>>,
}

impl RateLimitedProcessor {
    /// Create a new rate-limited processor
    pub fn new(inner: Box<dyn StreamProcessor>, max_lines_per_second: usize) -> Self {
        Self {
            inner,
            max_lines_per_second,
            last_process_time: Arc::new(Mutex::new(std::time::Instant::now())),
            lines_processed: Arc::new(Mutex::new(0)),
        }
    }
}

#[async_trait]
impl StreamProcessor for RateLimitedProcessor {
    async fn process_line(&self, line: &str, source: StreamSource) -> Result<()> {
        {
            let mut last_time = self.last_process_time.lock().await;
            let mut count = self.lines_processed.lock().await;

            let now = std::time::Instant::now();
            let elapsed = now.duration_since(*last_time);

            // Reset counter every second
            if elapsed >= Duration::from_secs(1) {
                *last_time = now;
                *count = 0;
            }

            // Check rate limit
            if *count >= self.max_lines_per_second {
                // Calculate how long to wait
                let wait_time = Duration::from_secs(1) - elapsed;
                if wait_time > Duration::ZERO {
                    drop(last_time);
                    drop(count);
                    time::sleep(wait_time).await;

                    // Reset after waiting
                    let mut last_time = self.last_process_time.lock().await;
                    let mut count = self.lines_processed.lock().await;
                    *last_time = std::time::Instant::now();
                    *count = 0;
                } else {
                    *count += 1;
                }
            } else {
                *count += 1;
            }
        }

        // Forward to inner processor
        self.inner.process_line(line, source).await
    }

    async fn on_complete(&self, exit_code: Option<i32>) -> Result<()> {
        self.inner.on_complete(exit_code).await
    }

    async fn on_error(&self, error: &anyhow::Error) -> Result<()> {
        self.inner.on_error(error).await
    }
}
