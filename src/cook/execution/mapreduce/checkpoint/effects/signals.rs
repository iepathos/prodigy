//! Signal handling for checkpoint-on-shutdown
//!
//! This module provides signal handlers that trigger checkpoint creation
//! when SIGINT (Ctrl+C) or SIGTERM is received.

use crate::cook::execution::mapreduce::checkpoint::{
    CheckpointReason, CheckpointStorage, MapReduceCheckpoint,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

/// Shutdown signal receiver
#[derive(Debug)]
pub struct ShutdownSignal {
    /// Flag indicating shutdown was requested
    shutdown_flag: Arc<AtomicBool>,
    /// Flag indicating graceful shutdown is in progress
    shutting_down: Arc<AtomicBool>,
}

impl Default for ShutdownSignal {
    fn default() -> Self {
        Self::new()
    }
}

impl ShutdownSignal {
    /// Create a new shutdown signal receiver
    pub fn new() -> Self {
        Self {
            shutdown_flag: Arc::new(AtomicBool::new(false)),
            shutting_down: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Check if shutdown was requested
    pub fn is_shutdown_requested(&self) -> bool {
        self.shutdown_flag.load(Ordering::Acquire)
    }

    /// Check if graceful shutdown is in progress
    pub fn is_shutting_down(&self) -> bool {
        self.shutting_down.load(Ordering::Acquire)
    }

    /// Request shutdown
    pub fn request_shutdown(&self) {
        self.shutdown_flag.store(true, Ordering::Release);
    }

    /// Mark that graceful shutdown has started
    pub fn start_shutdown(&self) {
        self.shutting_down.store(true, Ordering::Release);
    }

    /// Clone the shutdown flag for sharing
    pub fn clone_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.shutdown_flag)
    }
}

impl Clone for ShutdownSignal {
    fn clone(&self) -> Self {
        Self {
            shutdown_flag: Arc::clone(&self.shutdown_flag),
            shutting_down: Arc::clone(&self.shutting_down),
        }
    }
}

/// Configuration for checkpoint-on-shutdown behavior
#[derive(Debug, Clone)]
pub struct CheckpointOnShutdown {
    /// Whether to save checkpoint on shutdown
    pub enabled: bool,
    /// Maximum time to wait for checkpoint save (seconds)
    pub timeout_secs: u64,
    /// Whether to force shutdown if checkpoint save fails
    pub force_on_failure: bool,
}

impl Default for CheckpointOnShutdown {
    fn default() -> Self {
        Self {
            enabled: true,
            timeout_secs: 10,
            force_on_failure: true,
        }
    }
}

/// Create a shutdown signal that listens for SIGINT/SIGTERM
#[cfg(unix)]
pub fn shutdown_signal() -> ShutdownSignal {
    let signal = ShutdownSignal::new();
    let flag = signal.clone_flag();

    // Spawn signal handler task
    tokio::spawn(async move {
        use tokio::signal::unix::{signal, SignalKind};

        let mut sigint = signal(SignalKind::interrupt()).expect("Failed to install SIGINT handler");
        let mut sigterm =
            signal(SignalKind::terminate()).expect("Failed to install SIGTERM handler");

        tokio::select! {
            _ = sigint.recv() => {
                info!("Received SIGINT, initiating graceful shutdown");
            }
            _ = sigterm.recv() => {
                info!("Received SIGTERM, initiating graceful shutdown");
            }
        }

        flag.store(true, Ordering::Release);
    });

    signal
}

#[cfg(not(unix))]
pub fn shutdown_signal() -> ShutdownSignal {
    let signal = ShutdownSignal::new();
    let flag = signal.clone_flag();

    // Spawn signal handler task for Ctrl+C on Windows
    tokio::spawn(async move {
        match tokio::signal::ctrl_c().await {
            Ok(()) => {
                info!("Received Ctrl+C, initiating graceful shutdown");
            }
            Err(e) => {
                warn!("Failed to listen for Ctrl+C signal: {}", e);
            }
        }

        flag.store(true, Ordering::Release);
    });

    signal
}

/// Save checkpoint before shutdown
///
/// This is called when a shutdown signal is received. It will:
/// 1. Reset all in-progress items to pending
/// 2. Save the checkpoint with BeforeShutdown reason
/// 3. Log the checkpoint location
pub async fn save_checkpoint_on_shutdown(
    current_checkpoint: Arc<RwLock<Option<MapReduceCheckpoint>>>,
    storage: Arc<dyn CheckpointStorage>,
    config: &CheckpointOnShutdown,
) -> Result<String, String> {
    if !config.enabled {
        return Ok("Checkpoint on shutdown disabled".to_string());
    }

    info!("Saving checkpoint before shutdown...");

    // Read and prepare checkpoint
    let checkpoint = {
        let guard = current_checkpoint.read().await;
        match guard.as_ref() {
            Some(cp) => {
                use crate::cook::execution::mapreduce::checkpoint::pure::preparation::prepare_checkpoint;
                prepare_checkpoint(cp, CheckpointReason::BeforeShutdown)
            }
            None => {
                return Err("No checkpoint to save".to_string());
            }
        }
    };

    let checkpoint_id = checkpoint.metadata.checkpoint_id.clone();

    // Save with timeout
    let save_future = storage.save_checkpoint(&checkpoint);
    let timeout = std::time::Duration::from_secs(config.timeout_secs);

    match tokio::time::timeout(timeout, save_future).await {
        Ok(Ok(())) => {
            info!("Saved shutdown checkpoint: {}", checkpoint_id);
            Ok(checkpoint_id)
        }
        Ok(Err(e)) => {
            let msg = format!("Failed to save shutdown checkpoint: {}", e);
            warn!("{}", msg);
            if config.force_on_failure {
                Err(msg)
            } else {
                Ok(format!("Checkpoint save failed but continuing: {}", e))
            }
        }
        Err(_) => {
            let msg = format!("Checkpoint save timed out after {}s", config.timeout_secs);
            warn!("{}", msg);
            if config.force_on_failure {
                Err(msg)
            } else {
                Ok(format!("Checkpoint save timed out, continuing shutdown"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shutdown_signal_new() {
        let signal = ShutdownSignal::new();
        assert!(!signal.is_shutdown_requested());
        assert!(!signal.is_shutting_down());
    }

    #[test]
    fn test_shutdown_signal_request() {
        let signal = ShutdownSignal::new();
        signal.request_shutdown();
        assert!(signal.is_shutdown_requested());
    }

    #[test]
    fn test_shutdown_signal_start() {
        let signal = ShutdownSignal::new();
        signal.start_shutdown();
        assert!(signal.is_shutting_down());
    }

    #[test]
    fn test_shutdown_signal_clone() {
        let signal = ShutdownSignal::new();
        let cloned = signal.clone();

        signal.request_shutdown();
        assert!(cloned.is_shutdown_requested());
    }

    #[test]
    fn test_checkpoint_on_shutdown_default() {
        let config = CheckpointOnShutdown::default();
        assert!(config.enabled);
        assert_eq!(config.timeout_secs, 10);
        assert!(config.force_on_failure);
    }
}
