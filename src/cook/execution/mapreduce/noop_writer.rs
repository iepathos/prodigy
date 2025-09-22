//! No-op event writer for fallback scenarios

use crate::cook::execution::events::{EventWriter, EventRecord};
use anyhow::Result;
use async_trait::async_trait;

/// A no-op event writer that silently discards all events
/// Used as a last resort when no other event writer can be created
#[derive(Clone)]
pub struct NoOpEventWriter;

impl NoOpEventWriter {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl EventWriter for NoOpEventWriter {
    async fn write(&self, _events: &[EventRecord]) -> Result<()> {
        // Silently discard the events
        Ok(())
    }

    async fn flush(&self) -> Result<()> {
        // Nothing to flush
        Ok(())
    }

    fn clone(&self) -> Box<dyn EventWriter> {
        Box::new(Self)
    }
}