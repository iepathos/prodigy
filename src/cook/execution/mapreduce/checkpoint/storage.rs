//! Checkpoint storage implementations and compression
//!
//! This module provides the storage layer for checkpoint persistence, including:
//! - Storage trait defining the interface for checkpoint storage backends
//! - Compression algorithms for checkpoint data
//! - File-based storage implementation

use super::types::*;
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs;

/// Trait for checkpoint storage implementations
#[async_trait::async_trait]
pub trait CheckpointStorage: Send + Sync {
    async fn save_checkpoint(&self, checkpoint: &MapReduceCheckpoint) -> Result<()>;
    async fn load_checkpoint(&self, checkpoint_id: &CheckpointId) -> Result<MapReduceCheckpoint>;
    async fn list_checkpoints(&self, job_id: &str) -> Result<Vec<CheckpointInfo>>;
    async fn delete_checkpoint(&self, checkpoint_id: &CheckpointId) -> Result<()>;
    async fn checkpoint_exists(&self, checkpoint_id: &CheckpointId) -> Result<bool>;
}

/// Supported compression algorithms
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompressionAlgorithm {
    None,
    Gzip,
    Zstd,
    Lz4,
}

impl CompressionAlgorithm {
    /// Get the file extension for this compression type
    pub fn extension(&self) -> &str {
        match self {
            Self::None => "json",
            Self::Gzip => "gz",
            Self::Zstd => "zst",
            Self::Lz4 => "lz4",
        }
    }

    /// Compress data using the selected algorithm
    pub async fn compress(&self, data: &[u8]) -> Result<Vec<u8>> {
        match self {
            Self::None => Ok(data.to_vec()),
            Self::Gzip => {
                use flate2::write::GzEncoder;
                use flate2::Compression;
                use std::io::Write;

                let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
                encoder.write_all(data)?;
                Ok(encoder.finish()?)
            }
            Self::Zstd => {
                let level = 3; // Default compression level
                zstd::encode_all(data, level).context("Failed to compress with zstd")
            }
            Self::Lz4 => {
                lz4::block::compress(data, None, true).context("Failed to compress with lz4")
            }
        }
    }

    /// Decompress data using the selected algorithm
    pub async fn decompress(&self, data: &[u8]) -> Result<Vec<u8>> {
        match self {
            Self::None => Ok(data.to_vec()),
            Self::Gzip => {
                use flate2::read::GzDecoder;
                use std::io::Read;

                let mut decoder = GzDecoder::new(data);
                let mut result = Vec::new();
                decoder.read_to_end(&mut result)?;
                Ok(result)
            }
            Self::Zstd => zstd::decode_all(data).context("Failed to decompress with zstd"),
            Self::Lz4 => {
                lz4::block::decompress(data, None).context("Failed to decompress with lz4")
            }
        }
    }
}

impl Default for CompressionAlgorithm {
    fn default() -> Self {
        Self::Gzip
    }
}

/// File-based checkpoint storage implementation
pub struct FileCheckpointStorage {
    base_path: PathBuf,
    compression_algorithm: CompressionAlgorithm,
}

impl FileCheckpointStorage {
    /// Create new file-based storage with optional compression
    pub fn new(base_path: PathBuf, compression_enabled: bool) -> Self {
        Self {
            base_path,
            compression_algorithm: if compression_enabled {
                CompressionAlgorithm::default()
            } else {
                CompressionAlgorithm::None
            },
        }
    }

    /// Create with specific compression algorithm
    pub fn with_compression(base_path: PathBuf, algorithm: CompressionAlgorithm) -> Self {
        Self {
            base_path,
            compression_algorithm: algorithm,
        }
    }

    fn checkpoint_path(&self, checkpoint_id: &CheckpointId) -> PathBuf {
        let extension = format!("checkpoint.{}", self.compression_algorithm.extension());
        self.base_path
            .join(format!("{}.{}", checkpoint_id, extension))
    }

    async fn compress_data(&self, data: &[u8]) -> Result<Vec<u8>> {
        self.compression_algorithm.compress(data).await
    }

    async fn decompress_data(&self, data: &[u8]) -> Result<Vec<u8>> {
        self.compression_algorithm.decompress(data).await
    }
}

#[async_trait::async_trait]
impl CheckpointStorage for FileCheckpointStorage {
    async fn save_checkpoint(&self, checkpoint: &MapReduceCheckpoint) -> Result<()> {
        let checkpoint_id = CheckpointId::from_string(checkpoint.metadata.checkpoint_id.clone());
        let path = self.checkpoint_path(&checkpoint_id);

        // Ensure directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }

        // Serialize checkpoint
        let json = serde_json::to_vec_pretty(checkpoint)?;

        // Compress using selected algorithm
        let data = self.compress_data(&json).await?;

        // Write atomically using temp file
        let temp_path = path.with_extension("tmp");
        fs::write(&temp_path, &data).await?;
        fs::rename(&temp_path, &path).await?;

        Ok(())
    }

    async fn load_checkpoint(&self, checkpoint_id: &CheckpointId) -> Result<MapReduceCheckpoint> {
        let path = self.checkpoint_path(checkpoint_id);

        if !path.exists() {
            return Err(anyhow!("Checkpoint {} not found", checkpoint_id));
        }

        // Read data
        let data = fs::read(&path).await?;

        // Decompress using selected algorithm
        let json = self.decompress_data(&data).await?;

        // Deserialize
        let checkpoint: MapReduceCheckpoint = serde_json::from_slice(&json)?;

        Ok(checkpoint)
    }

    async fn list_checkpoints(&self, job_id: &str) -> Result<Vec<CheckpointInfo>> {
        let mut checkpoints = Vec::new();

        if !self.base_path.exists() {
            return Ok(checkpoints);
        }

        let mut entries = fs::read_dir(&self.base_path).await?;

        while let Some(entry) = entries.next_entry().await? {
            if let Some(name) = entry.file_name().to_str() {
                if name.contains(".checkpoint") {
                    // Try to load checkpoint metadata
                    if let Some(checkpoint_id_str) = name.split('.').next() {
                        let checkpoint_id =
                            CheckpointId::from_string(checkpoint_id_str.to_string());
                        if let Ok(checkpoint) = self.load_checkpoint(&checkpoint_id).await {
                            if checkpoint.metadata.job_id == job_id {
                                checkpoints.push(CheckpointInfo {
                                    id: checkpoint.metadata.checkpoint_id,
                                    job_id: checkpoint.metadata.job_id,
                                    created_at: checkpoint.metadata.created_at,
                                    phase: checkpoint.metadata.phase,
                                    completed_items: checkpoint.metadata.completed_items,
                                    total_items: checkpoint.metadata.total_work_items,
                                    is_final: checkpoint.metadata.phase == PhaseType::Complete,
                                });
                            }
                        }
                    }
                }
            }
        }

        Ok(checkpoints)
    }

    async fn delete_checkpoint(&self, checkpoint_id: &CheckpointId) -> Result<()> {
        let path = self.checkpoint_path(checkpoint_id);
        if path.exists() {
            fs::remove_file(path).await?;
        }
        Ok(())
    }

    async fn checkpoint_exists(&self, checkpoint_id: &CheckpointId) -> Result<bool> {
        Ok(self.checkpoint_path(checkpoint_id).exists())
    }
}
