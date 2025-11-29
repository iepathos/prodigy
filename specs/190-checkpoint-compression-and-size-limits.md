---
number: 190
title: Checkpoint Compression and Size Limits
category: optimization
priority: medium
status: draft
dependencies: [184]
created: 2025-11-29
---

# Specification 190: Checkpoint Compression and Size Limits

**Category**: optimization
**Priority**: medium
**Status**: draft
**Dependencies**: Spec 184 (Unified Checkpoint System)

## Context

Large workflow checkpoints can consume significant disk space and slow down save/load operations. Checkpoints containing:
- Large variable values (e.g., multi-MB JSON responses)
- Extensive MapReduce result sets (thousands of work items)
- Deep execution histories with retry states
- Captured output from long-running commands

These can grow to 10MB+ uncompressed, causing:
- Slow checkpoint writes (>500ms)
- Excessive disk usage (especially with checkpoint history)
- Network filesystem performance issues
- Resume delays due to large file reads

Additionally, unbounded checkpoint growth can lead to:
- Disk space exhaustion
- OOM errors when loading checkpoints
- Performance degradation over workflow execution

## Objective

Implement intelligent checkpoint compression and size management that:
1. Automatically compresses large checkpoints (>1MB)
2. Enforces reasonable size limits to prevent resource exhaustion
3. Optimizes compression for checkpoint data patterns
4. Maintains fast save/load times even with compression
5. Provides clear warnings and errors for oversized checkpoints
6. Supports configurable compression settings

## Requirements

### Functional Requirements

#### FR1: Automatic Compression
- **MUST** detect checkpoint size before write
- **MUST** apply compression if uncompressed size >1MB (configurable)
- **MUST** use zstd compression for fast compression/decompression
- **MUST** store compression metadata in checkpoint header
- **MUST** decompress automatically on load
- **MUST** preserve exact checkpoint data (lossless compression)

#### FR2: Size Limits and Warnings
- **MUST** warn if compressed checkpoint >10MB
- **MUST** error if compressed checkpoint >100MB (configurable)
- **MUST** log compression ratio and final size
- **MUST** suggest variable cleanup for oversized checkpoints
- **MUST** track checkpoint size metrics over time

#### FR3: Compression Configuration
- **MUST** support compression level configuration (1-22 for zstd)
- **MUST** default to level 3 (balanced speed/ratio)
- **MUST** allow disabling compression entirely
- **MUST** support per-workflow compression settings
- **MUST** expose compression stats in checkpoint metadata

#### FR4: Efficient Compression
- **MUST** compress in streaming mode for large checkpoints
- **MUST** use dictionary compression for repeated patterns
- **MUST** complete compression in <100ms for typical checkpoints (P95)
- **MUST** not block step execution during compression
- **SHOULD** compress asynchronously when possible

### Non-Functional Requirements

#### NFR1: Performance
- Compression overhead MUST be <50ms for 1MB checkpoint (P95)
- Decompression MUST be <30ms for 1MB checkpoint (P95)
- Compression MUST NOT increase checkpoint write latency >20%
- Memory usage during compression MUST be <2x checkpoint size

#### NFR2: Reliability
- Compression failures MUST fallback to uncompressed write
- Corrupted compressed data MUST be detected
- Decompression errors MUST provide clear error messages
- Checkpoint integrity hash MUST work with compressed data

#### NFR3: Observability
- Compression ratio MUST be logged
- Compression duration MUST be tracked in metrics
- Size reduction MUST be visible in checkpoint metadata
- Warnings MUST be emitted for poor compression (<10% reduction)

## Acceptance Criteria

### Automatic Compression

- [ ] **AC1**: Compression triggered for large checkpoints
  - Checkpoint size is 2.5MB uncompressed
  - Compression threshold is 1MB (default)
  - Checkpoint compressed with zstd level 3
  - Compressed size ~500KB (typical JSON compression ratio)
  - Compression completes in <50ms
  - Metadata indicates compression: `{ compressed: true, original_size: 2621440, compressed_size: 524288, compression_ratio: 0.20 }`

- [ ] **AC2**: Small checkpoints not compressed
  - Checkpoint size is 800KB uncompressed
  - Below 1MB threshold
  - Saved without compression
  - Load time identical to compressed checkpoints
  - No compression overhead

- [ ] **AC3**: Compression automatically decompressed on load
  - Compressed checkpoint loaded
  - Decompression transparent to caller
  - Original checkpoint data restored exactly
  - Decompression completes in <30ms
  - No user intervention required

### Size Management

- [ ] **AC4**: Warning for large compressed checkpoints
  - Compressed checkpoint is 12MB
  - Warning threshold is 10MB
  - Warning logged: "Large checkpoint detected: 12.0MB (original: 50.0MB, ratio: 0.24)"
  - Suggestion provided: "Consider removing large variables: analysis_result, raw_output"
  - Checkpoint still saved successfully

- [ ] **AC5**: Error for oversized checkpoints
  - Compressed checkpoint would be 120MB
  - Hard limit is 100MB
  - Error returned: "Checkpoint size exceeds limit: 120.0MB > 100.0MB"
  - Workflow execution fails with clear error
  - Checkpoint NOT saved
  - Previous checkpoint remains intact

- [ ] **AC6**: Size limit suggestions
  - Checkpoint size error occurs
  - Error message includes variable sizes
  - Top 5 largest variables listed: "results_array (45MB), raw_logs (30MB), ..."
  - Suggestion: "Remove or truncate large variables before checkpoint"
  - Reference to documentation on variable management

### Configuration

- [ ] **AC7**: Compression level configurable
  - Workflow config: `checkpoint: { compression_level: 9 }`
  - Higher compression ratio achieved (slower)
  - Level 9 takes ~3x longer than level 3
  - Better compression for archival checkpoints
  - Trade-off documented in configuration

- [ ] **AC8**: Compression disabled
  - Workflow config: `checkpoint: { compression: false }`
  - All checkpoints saved uncompressed
  - No compression overhead
  - Useful for debugging or fast local storage
  - Size limits still enforced

- [ ] **AC9**: Per-workflow compression settings
  - Workflow A: compression enabled, level 3
  - Workflow B: compression disabled
  - MapReduce job: compression enabled, level 6
  - Settings respected per workflow type
  - Global defaults applied when not specified

### Performance

- [ ] **AC10**: Fast compression for typical checkpoints
  - 500KB checkpoint compressed
  - Compression completes in <25ms (P50)
  - Compression completes in <40ms (P95)
  - No noticeable workflow delay
  - Metrics tracked and reported

- [ ] **AC11**: Streaming compression for large checkpoints
  - 50MB checkpoint to compress
  - Streamed in 1MB chunks
  - Memory usage stays <10MB during compression
  - Compression completes in <200ms
  - No memory exhaustion

## Technical Details

### Implementation Approach

#### 1. Compression Detection and Application

```rust
use zstd::stream::{encode_all, decode_all};
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointMetadata {
    pub version: u32,
    pub compressed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_size: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compressed_size: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compression_level: Option<i32>,
    pub integrity_hash: String,
}

pub struct CheckpointCompressor {
    config: CompressionConfig,
}

impl CheckpointCompressor {
    pub fn should_compress(&self, size: usize) -> bool {
        self.config.enabled && size > self.config.threshold_bytes
    }

    pub fn compress(&self, data: &[u8]) -> Result<CompressedCheckpoint> {
        let original_size = data.len();

        // Check size before compression
        if original_size > self.config.max_uncompressed_bytes {
            return Err(CheckpointError::TooLarge {
                size: original_size,
                limit: self.config.max_uncompressed_bytes,
            });
        }

        let start = Instant::now();
        let compressed = encode_all(data, self.config.level)?;
        let compression_duration = start.elapsed();

        let compressed_size = compressed.len();
        let ratio = compressed_size as f64 / original_size as f64;

        // Check compressed size against limit
        if compressed_size > self.config.max_compressed_bytes {
            return Err(CheckpointError::CompressedTooLarge {
                compressed_size,
                limit: self.config.max_compressed_bytes,
            });
        }

        // Warn on poor compression
        if ratio > 0.9 {
            tracing::warn!(
                "Poor compression ratio: {:.2}% ({}B → {}B)",
                ratio * 100.0,
                original_size,
                compressed_size
            );
        }

        // Warn on large compressed size
        if compressed_size > self.config.warning_threshold_bytes {
            tracing::warn!(
                "Large checkpoint: {:.1}MB compressed (original: {:.1}MB, ratio: {:.2})",
                compressed_size as f64 / 1024.0 / 1024.0,
                original_size as f64 / 1024.0 / 1024.0,
                ratio
            );
        }

        // Track metrics
        self.record_compression_metrics(original_size, compressed_size, compression_duration);

        Ok(CompressedCheckpoint {
            data: compressed,
            metadata: CompressionMetadata {
                original_size,
                compressed_size,
                compression_level: self.config.level,
                compression_duration,
                ratio,
            },
        })
    }

    pub fn decompress(&self, data: &[u8]) -> Result<Vec<u8>> {
        let start = Instant::now();
        let decompressed = decode_all(data)?;
        let decompression_duration = start.elapsed();

        self.record_decompression_metrics(data.len(), decompressed.len(), decompression_duration);

        Ok(decompressed)
    }
}
```

#### 2. Checkpoint Save with Compression

```rust
pub async fn save_checkpoint(&self, checkpoint: &WorkflowCheckpoint) -> Result<()> {
    // Serialize to JSON
    let json_data = serde_json::to_vec_pretty(checkpoint)?;
    let uncompressed_size = json_data.len();

    tracing::debug!("Checkpoint uncompressed size: {}B", uncompressed_size);

    // Apply compression if needed
    let (final_data, metadata) = if self.compressor.should_compress(uncompressed_size) {
        tracing::debug!("Compressing checkpoint (threshold: {}B)", self.compressor.config.threshold_bytes);

        let compressed = self.compressor.compress(&json_data)?;

        tracing::info!(
            "Checkpoint compressed: {}B → {}B ({:.1}% reduction)",
            uncompressed_size,
            compressed.metadata.compressed_size,
            (1.0 - compressed.metadata.ratio) * 100.0
        );

        (compressed.data, Some(compressed.metadata))
    } else {
        tracing::debug!("Checkpoint below compression threshold, saving uncompressed");
        (json_data, None)
    };

    // Write with compression metadata
    let envelope = CheckpointEnvelope {
        version: CHECKPOINT_VERSION,
        compressed: metadata.is_some(),
        original_size: metadata.as_ref().map(|m| m.original_size),
        compressed_size: metadata.as_ref().map(|m| m.compressed_size),
        data: final_data,
        integrity_hash: compute_hash(&checkpoint)?,
    };

    // Atomic write
    self.storage.write_atomic(&envelope).await?;

    Ok(())
}
```

#### 3. Variable Size Analysis

```rust
pub fn analyze_large_variables(checkpoint: &WorkflowCheckpoint) -> Vec<VariableSize> {
    let mut sizes: Vec<_> = checkpoint
        .variables
        .iter()
        .map(|(name, value)| {
            let size = serde_json::to_string(value).map(|s| s.len()).unwrap_or(0);
            VariableSize {
                name: name.clone(),
                size,
                value_type: classify_value(value),
            }
        })
        .collect();

    sizes.sort_by(|a, b| b.size.cmp(&a.size));
    sizes
}

pub fn suggest_size_reduction(sizes: &[VariableSize]) -> Vec<String> {
    sizes
        .iter()
        .take(5)
        .filter(|v| v.size > 1024 * 1024) // >1MB
        .map(|v| {
            format!(
                "- Variable '{}' ({}) is {:.1}MB - consider removing or truncating",
                v.name,
                v.value_type,
                v.size as f64 / 1024.0 / 1024.0
            )
        })
        .collect()
}
```

### Architecture Changes

**New modules:**
- `src/cook/workflow/checkpoint/compression.rs` - Compression logic
- `src/cook/workflow/checkpoint/size_limits.rs` - Size enforcement
- `src/cook/workflow/checkpoint/variable_analysis.rs` - Variable size analysis

**Modified components:**
- `CheckpointManager` - Integrate compression
- `WorkflowCheckpoint` - Add compression metadata
- Configuration system - Add compression settings

### Configuration Schema

```yaml
# Global checkpoint configuration
checkpoint:
  compression:
    enabled: true
    level: 3              # 1 (fast) to 22 (max)
    threshold_mb: 1       # Compress if >1MB
    warning_mb: 10        # Warn if compressed >10MB
    max_compressed_mb: 100  # Error if compressed >100MB
    max_uncompressed_mb: 500  # Error if uncompressed >500MB

  # Per-workflow override
  workflows:
    large-data-processing:
      compression:
        enabled: true
        level: 9  # Higher compression for large workflows
        threshold_mb: 5
```

## Dependencies

- **Prerequisites**: Spec 184 (Unified Checkpoint System)
- **External Dependencies**: zstd crate for compression
- **Affected Components**: CheckpointManager, storage layer, configuration system

## Testing Strategy

### Unit Tests
- Compression/decompression correctness
- Size limit enforcement
- Configuration parsing and application
- Variable size analysis accuracy
- Metadata preservation

### Integration Tests
- End-to-end checkpoint save/load with compression
- Large checkpoint handling
- Compression failure recovery
- Performance benchmarks (compression speed, ratio)

### Performance Tests
- Compression overhead measurement
- Memory usage profiling
- Compression ratio validation
- Decompression latency testing

## Documentation Requirements

### Code Documentation
- Document compression algorithm choice (zstd)
- Explain compression level trade-offs
- Document size limit rationale
- Add examples for configuration

### User Documentation
- Compression configuration guide
- Troubleshooting oversized checkpoints
- Performance tuning guidelines
- Variable management best practices

### Architecture Updates
- Document compression architecture
- Update checkpoint format specification
- Add compression metrics to observability guide

## Implementation Notes

### Compression Algorithm Choice

**zstd selected because:**
- Excellent compression ratio (~70-80% for JSON)
- Very fast decompression (<30ms for 1MB)
- Adjustable compression levels
- Streaming support for large data
- Industry standard (Facebook, Linux kernel)

**Alternatives considered:**
- gzip: Slower, worse ratio
- lz4: Faster but worse ratio
- brotli: Better ratio but much slower
- snappy: Worse ratio, no level tuning

### Size Limit Rationale

**Default limits:**
- Compression threshold: 1MB (balance overhead vs. savings)
- Warning threshold: 10MB (large but manageable)
- Hard limit: 100MB (prevent resource exhaustion)

**Rationale:**
- Most checkpoints <500KB
- 1MB threshold captures 10-20% of checkpoints
- 100MB limit prevents OOM, disk exhaustion
- Configurable for special cases

### Performance Considerations

- Compression should be <10% of step execution time
- Async compression for steps >100ms
- Dictionary training for repeated patterns
- Memory-efficient streaming for large checkpoints

## Migration and Compatibility

### Backward Compatibility

- Uncompressed checkpoints still supported
- Compression detection automatic on load
- Metadata version allows format evolution
- No breaking changes to checkpoint API

### Migration Path

1. Deploy with compression disabled (default: off)
2. Enable compression for new checkpoints
3. Monitor compression metrics
4. Tune compression levels per workflow
5. Eventually default to enabled

### Rollback Strategy

- Disable compression via configuration
- Existing compressed checkpoints still loadable
- Graceful degradation on decompression errors
- No data loss on rollback
