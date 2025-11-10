---
number: 157
title: Multi-Pass Codebase Analysis for Large Codebases
category: optimization
priority: medium
status: draft
dependencies: []
created: 2025-01-11
---

# Specification 157: Multi-Pass Codebase Analysis for Large Codebases

**Category**: optimization
**Priority**: medium
**Status**: draft
**Dependencies**: None

## Context

The current book documentation workflow (`book-docs-drift.yml`) uses a single Claude command (`/prodigy-analyze-features-for-book`) to analyze the entire codebase and generate a feature inventory. This works well for small to medium codebases (< 10K lines), but hits token limits for larger codebases.

**Current Limitations**:
- Claude Sonnet has a 200K token context window
- Realistic budget for file reading: ~100K-150K tokens
- Rule of thumb: ~1 token ≈ 4 characters, so ~5,000-10,000 lines of code
- Single-pass analysis fails silently or produces incomplete results for larger codebases

**Why This Matters**:
- Prodigy itself is ~30K lines and growing
- External projects using Prodigy may have 50K-200K+ lines
- Silent truncation leads to missing features in documentation
- No clear error message when codebase is too large

**Problem Example**:
```json
{
  "analysis_targets": [
    {
      "area": "workflow_basics",
      "source_files": ["src/workflow/config.rs", "src/workflow/mod.rs"]  // 2K lines
    },
    {
      "area": "mapreduce",
      "source_files": ["src/mapreduce/mod.rs", "src/mapreduce/orchestrator.rs"]  // 3K lines
    },
    // ... 20 more analysis targets with 50+ files totaling 100K lines
  ]
}
```

Attempting to read all 50+ files in one command exceeds context limits.

## Objective

Implement a multi-pass codebase analysis system that:
- Automatically chunks large codebases into token-safe pieces
- Processes chunks in parallel using MapReduce pattern
- Merges partial results into complete feature inventory
- Works transparently for small codebases (no extra overhead)
- Scales to unlimited codebase size
- Provides clear errors and guidance when issues occur

## Requirements

### Functional Requirements

- **Automatic Chunking**: Detect codebase size and create chunks if needed
- **Token Estimation**: Estimate token usage before reading files
- **Chunk Generation**: Group analysis targets into ~80K token chunks
- **Parallel Processing**: Use MapReduce to process chunks in parallel
- **Result Merging**: Combine partial feature inventories into complete result
- **Transparent Mode**: Small codebases (<80K tokens) use single-pass (current behavior)
- **Feature Deduplication**: Handle features appearing in multiple chunks
- **Error Handling**: Clear errors when chunks fail
- **Resume Support**: Leverage existing checkpoint/resume for failed chunks

### Non-Functional Requirements

- **Performance**: Parallel processing faster than sequential for large codebases
- **Memory Efficiency**: Each chunk processed independently
- **Fault Tolerance**: Failed chunks go to DLQ, can be retried
- **Debuggability**: Each chunk is isolated, easy to trace failures
- **Backward Compatibility**: Existing workflows continue working unchanged
- **Scalability**: Support codebases up to 500K+ lines

## Acceptance Criteria

- [ ] Workflow automatically detects if codebase requires chunking
- [ ] Chunking creates groups ≤ 80K tokens each
- [ ] Small codebases (< 80K tokens) use single-pass (no MapReduce overhead)
- [ ] Large codebases (≥ 80K tokens) use multi-pass MapReduce
- [ ] Chunks processed in parallel respecting max_parallel setting
- [ ] Partial features.json files merged correctly
- [ ] Feature deduplication works (same feature in multiple chunks)
- [ ] Meta-content aggregated from all chunks
- [ ] Failed chunks appear in DLQ with retry support
- [ ] Chunking preserves logical module boundaries
- [ ] Clear error messages for configuration issues
- [ ] Documentation includes token limit guidance
- [ ] End-to-end test with 100K line codebase succeeds

## Technical Details

### Architecture: Two-Workflow Design

**Workflow 1: Feature Analysis** (`book-features-analysis.yml`)
```yaml
name: book-features-analysis
mode: mapreduce

setup:
  - claude: "/prodigy-chunk-codebase --config $PROJECT_CONFIG --output chunks.json"

map:
  input: "chunks.json"
  json_path: "$[*]"

  agent_template:
    - claude: "/prodigy-analyze-features-chunk --chunk '${item}'"
      commit_required: true

reduce:
  - claude: "/prodigy-merge-feature-analysis --input-dir $ANALYSIS_DIR --output features.json"
    commit_required: true
```

**Workflow 2: Drift Detection** (`book-docs-drift.yml`)
```yaml
name: book-docs-drift
mode: mapreduce

setup:
  # Assumes features.json already exists from workflow 1
  - claude: "/prodigy-detect-documentation-gaps --features features.json"

map:
  input: "flattened-items.json"
  json_path: "$[*]"

  agent_template:
    - claude: "/prodigy-analyze-subsection-drift"
    - claude: "/prodigy-fix-subsection-drift"

reduce:
  - shell: "cd book && mdbook build"
  - claude: "/prodigy-validate-book-holistically"
```

### Chunking Algorithm

**Phase 1: Size Estimation**

```markdown
1. Read book-config.json to get analysis_targets
2. For each analysis_target:
   - Use Glob to list source_files
   - Get line count for each file (wc -l or Read tool)
   - Estimate tokens: lines * 5 tokens/line
3. Sum total estimated tokens across all files
```

**Phase 2: Chunking Decision**

```markdown
If total_tokens < 80,000:
  - Create single chunk with all analysis_targets
  - Output: [{ "chunk_id": "all", "analysis_targets": [...] }]
  - This triggers single-agent map phase (no parallelization needed)

If total_tokens >= 80,000:
  - Group analysis_targets into chunks
  - Each chunk ≤ 80K tokens
  - Preserve module boundaries when possible
  - Output: [
      { "chunk_id": "chunk-1", "analysis_targets": [...], "estimated_tokens": 75000 },
      { "chunk_id": "chunk-2", "analysis_targets": [...], "estimated_tokens": 65000 },
      ...
    ]
```

**Phase 3: Chunk Generation**

```json
[
  {
    "chunk_id": "chunk-1",
    "description": "Core workflow and command types",
    "analysis_targets": [
      {
        "area": "workflow_basics",
        "source_files": ["src/workflow/config.rs", "src/workflow/mod.rs"],
        "feature_categories": ["workflow_basics"]
      },
      {
        "area": "command_types",
        "source_files": ["src/commands/mod.rs"],
        "feature_categories": ["command_types"]
      }
    ],
    "output_file": ".prodigy/book-analysis/features-chunk-1.json",
    "estimated_tokens": 45000
  },
  {
    "chunk_id": "chunk-2",
    "description": "MapReduce and error handling",
    "analysis_targets": [
      {
        "area": "mapreduce",
        "source_files": ["src/mapreduce/config.rs", "src/mapreduce/orchestrator.rs"],
        "feature_categories": ["mapreduce"]
      },
      {
        "area": "error_handling",
        "source_files": ["src/error/mod.rs"],
        "feature_categories": ["error_handling"]
      }
    ],
    "output_file": ".prodigy/book-analysis/features-chunk-2.json",
    "estimated_tokens": 35000
  }
]
```

### Feature Merging Algorithm

**Merge Strategy**:

```markdown
1. Load all features-chunk-*.json files from analysis directory
2. For each feature in each chunk:
   - If feature doesn't exist in merged output → Add it
   - If feature exists in merged output:
     - Merge capabilities (union of all capabilities)
     - Keep most detailed description
     - Combine nested structures
     - Flag duplicates in log for review

3. Special handling for meta_content:
   - Combine best_practices from all chunks
   - Combine troubleshooting items
   - Deduplicate common_patterns by name

4. Add merge metadata:
   {
     "version_info": {
       "analyzed_version": "0.2.0+",
       "analysis_date": "2025-01-11",
       "merge_info": {
         "chunks_processed": 3,
         "chunk_ids": ["chunk-1", "chunk-2", "chunk-3"]
       }
     }
   }
```

**Deduplication Logic**:

```markdown
If same feature appears in multiple chunks:

1. Compare feature structures:
   - If identical → Keep one
   - If different → Merge capabilities

2. Priority for conflicts:
   - Prefer more detailed descriptions
   - Union of capabilities
   - Keep all nested structures
   - Log warning about merge

3. Example merge:
   Chunk 1: { "mapreduce": { "type": "major_feature", "phases": {...} } }
   Chunk 2: { "mapreduce": { "type": "major_feature", "core_capabilities": {...} } }

   Merged: { "mapreduce": { "type": "major_feature", "phases": {...}, "core_capabilities": {...} } }
```

### Token Budget Management

**Per-Chunk Budget**:
- Target: 80,000 tokens
- Safety margin: Leave 20K for prompt + conversation
- File reading budget: 60,000 tokens (~12K lines)

**Estimation Formula**:
```
tokens = (lines * 5) + (files * 100)
# 5 tokens/line on average
# 100 tokens overhead per file for formatting
```

**Validation**:
```markdown
After creating chunks, validate:
- Each chunk ≤ 80K tokens
- No empty chunks
- All analysis_targets assigned to exactly one chunk
- Logical groupings preserved (related modules together)
```

### Error Handling

**Chunking Failures**:
- Invalid book-config.json → Clear error message
- Cannot estimate file sizes → Fall back to single-pass, warn user
- Chunks exceed token limit → Error with guidance to reduce source_files

**Analysis Failures**:
- Chunk analysis fails → Goes to DLQ
- Can retry individual chunks without re-running entire workflow
- Failed chunks don't block other chunks

**Merge Failures**:
- Missing chunk output → Error listing which chunks failed
- Malformed chunk JSON → Error with chunk_id for debugging
- Feature conflicts → Log warnings but complete merge

### Performance Characteristics

**Small Codebase (< 10K lines)**:
- Single chunk created
- 1 agent processes it
- Trivial merge
- Duration: ~5 minutes (same as before)

**Medium Codebase (10K-50K lines)**:
- 2-3 chunks created
- 2-3 agents process in parallel
- Simple merge
- Duration: ~8 minutes (slightly longer due to overhead)

**Large Codebase (100K lines)**:
- 8-10 chunks created
- 8-10 agents process in parallel (respecting max_parallel)
- Complex merge
- Duration: ~15 minutes (much faster than sequential would be)

**Extra Large Codebase (200K+ lines)**:
- 15-20 chunks created
- Processed in batches (max_parallel limit)
- Complex merge with deduplication
- Duration: ~30-40 minutes (scales linearly)

## Dependencies

**Prerequisites**: None (standalone feature)

**Affected Components**:
- `workflows/book-features-analysis.yml` - NEW workflow file
- `workflows/book-docs-drift.yml` - MODIFIED (remove feature analysis from setup)
- `.claude/commands/prodigy-chunk-codebase.md` - NEW command
- `.claude/commands/prodigy-analyze-features-chunk.md` - NEW command
- `.claude/commands/prodigy-merge-feature-analysis.md` - NEW command
- `.prodigy/book-config.json` - No changes (uses existing structure)

**External Dependencies**: None (uses existing Prodigy MapReduce infrastructure)

## Testing Strategy

### Unit Tests

**Chunking Algorithm Tests**:
- Test single-pass decision (< 80K tokens)
- Test multi-pass chunking (≥ 80K tokens)
- Test chunk boundary calculation
- Test token estimation accuracy
- Test empty analysis_targets handling

**Merge Algorithm Tests**:
- Test simple merge (no conflicts)
- Test feature deduplication
- Test meta_content aggregation
- Test conflict resolution
- Test partial chunk failure handling

### Integration Tests

**Small Codebase Test (5K lines)**:
- Run book-features-analysis.yml
- Verify single chunk created
- Verify features.json complete
- Verify no unnecessary MapReduce overhead
- Duration: < 10 minutes

**Large Codebase Test (100K lines)**:
- Run book-features-analysis.yml
- Verify multiple chunks created
- Verify parallel processing occurred
- Verify features.json complete and merged
- Verify no feature loss
- Duration: < 30 minutes

**Failure Recovery Test**:
- Simulate chunk failure
- Verify failed chunk in DLQ
- Retry failed chunk
- Verify successful merge after retry

### End-to-End Tests

**Full Workflow Test**:
```bash
# Step 1: Run feature analysis
prodigy run workflows/book-features-analysis.yml
# → Creates features.json

# Step 2: Run drift detection
prodigy run workflows/book-docs-drift.yml
# → Uses features.json to update docs

# Verify:
# - All major features documented
# - No features missed
# - Book builds successfully
```

**Stress Test (200K line codebase)**:
- Test with synthetic large codebase
- Verify chunking algorithm scales
- Verify merge handles 20+ chunks
- Verify memory usage stays reasonable
- Verify total duration < 1 hour

### Performance Benchmarks

| Codebase Size | Chunks | Agents | Duration | Tokens Used |
|---------------|--------|--------|----------|-------------|
| 5K lines | 1 | 1 | 5 min | ~25K |
| 20K lines | 2 | 2 | 8 min | ~100K |
| 50K lines | 5 | 5 | 15 min | ~250K |
| 100K lines | 10 | 8* | 25 min | ~500K |
| 200K lines | 20 | 8* | 45 min | ~1M |

*Limited by max_parallel setting

## Documentation Requirements

### Code Documentation

**Chunking Command**:
```markdown
# /prodigy-chunk-codebase

Analyze codebase structure and create chunks for parallel feature analysis.

## Token Budget

- Target per chunk: 80,000 tokens
- Safety margin: 20,000 tokens
- File reading budget: 60,000 tokens (~12K lines)

## Algorithm

1. Estimate total tokens from all source files
2. If < 80K: Create single chunk
3. If ≥ 80K: Group analysis_targets into chunks
4. Preserve module boundaries when possible
5. Output chunks.json for map phase

## Example Output

[See Technical Details section above]
```

**Merge Command**:
```markdown
# /prodigy-merge-feature-analysis

Merge multiple features-chunk-*.json files into complete features.json.

## Merge Strategy

1. Load all chunk files
2. Combine features (union)
3. Deduplicate conflicts
4. Aggregate meta_content
5. Add merge metadata

## Deduplication

- Same feature → Keep one
- Similar features → Merge capabilities
- Conflicts → Prefer more detailed, log warning
```

### User Documentation

**New Chapter: Large Codebase Support**

```markdown
# Large Codebase Support

## When to Use Multi-Pass Analysis

If your codebase is > 10K lines, use the two-workflow approach:

### Workflow 1: Feature Analysis
\`\`\`bash
prodigy run workflows/book-features-analysis.yml
\`\`\`

Runs once when:
- Major code changes
- New features added
- Weekly/monthly refresh

### Workflow 2: Drift Detection
\`\`\`bash
prodigy run workflows/book-docs-drift.yml
\`\`\`

Runs frequently:
- After documentation updates
- Daily/weekly checks
- After minor changes

## Configuration Guidelines

**Small Projects (< 10K lines)**:
- Use 5-8 analysis_targets
- 2-3 files per target
- Single-pass mode (automatic)

**Medium Projects (10K-50K lines)**:
- Use 10-15 analysis_targets
- 2-4 files per target
- Multi-pass with 2-5 chunks

**Large Projects (50K-200K lines)**:
- Use 20-30 analysis_targets
- 3-5 files per target
- Multi-pass with 8-20 chunks

## Troubleshooting

**"Context too large" errors**:
- Reduce files in source_files per target
- Split large files into multiple targets
- Focus on config files only

**Missing features**:
- Check DLQ for failed chunks
- Retry with: `prodigy dlq retry <job_id>`
- Verify all source files listed in book-config.json

**Slow performance**:
- Increase max_parallel in workflow
- Reduce files per chunk
- Use faster hardware
```

### Architecture Documentation

Update `ARCHITECTURE.md`:

```markdown
## Multi-Pass Codebase Analysis

For large codebases (> 80K tokens), feature analysis uses MapReduce:

1. **Chunking**: Automatically split analysis_targets into ~80K token chunks
2. **Parallel Processing**: Each chunk analyzed by separate agent
3. **Merging**: Results combined into complete feature inventory

Benefits:
- Scales to unlimited codebase size
- Parallel processing faster than sequential
- Fault-tolerant (failed chunks go to DLQ)
- Leverages existing MapReduce infrastructure
```

## Implementation Notes

### Chunking Heuristics

**Preserve Module Boundaries**:
```
Good chunking:
  Chunk 1: [workflow_basics, command_types]
  Chunk 2: [mapreduce, parallel_execution]
  Chunk 3: [storage, persistence]

Bad chunking:
  Chunk 1: [workflow_basics, mapreduce.setup]
  Chunk 2: [mapreduce.map, command_types]
  Chunk 3: [mapreduce.reduce, storage]
  # Splits mapreduce across chunks unnecessarily
```

**Balancing**:
- Prefer fewer, larger chunks over many small chunks (less merge complexity)
- But respect 80K token limit strictly
- Balance chunk sizes when possible (avoid 10K + 75K split)

### Merge Complexity

**Simple Case** (no conflicts):
```json
Chunk 1: { "feature_a": {...}, "feature_b": {...} }
Chunk 2: { "feature_c": {...}, "feature_d": {...} }
Merged:  { "feature_a": {...}, "feature_b": {...}, "feature_c": {...}, "feature_d": {...} }
```

**Complex Case** (overlapping features):
```json
Chunk 1: { "mapreduce": { "phases": {...} } }
Chunk 2: { "mapreduce": { "core_capabilities": {...} } }
Merged:  { "mapreduce": { "phases": {...}, "core_capabilities": {...} } }
```

### Debugging Tips

**Chunk Analysis**:
```bash
# View chunks.json to see how codebase was split
cat .prodigy/book-analysis/chunks.json | jq

# Check individual chunk outputs
cat .prodigy/book-analysis/features-chunk-1.json | jq
cat .prodigy/book-analysis/features-chunk-2.json | jq

# Compare merged result
cat .prodigy/book-analysis/features.json | jq '.merge_info'
```

**Token Estimation Validation**:
```bash
# Actual vs estimated tokens
for chunk in .prodigy/book-analysis/features-chunk-*.json; do
  echo "Chunk: $chunk"
  jq '.version_info.merge_info.estimated_tokens' $chunk
  jq '.version_info.merge_info.actual_tokens' $chunk
done
```

## Migration and Compatibility

### Backward Compatibility

**Existing Single-Workflow Projects**:
- Continue using `book-docs-drift.yml` (includes feature analysis in setup)
- Works for codebases < 10K lines
- No changes required

**Migration Path to Two-Workflow**:
1. Create `book-features-analysis.yml` in workflows/
2. Update `book-docs-drift.yml` to assume features.json exists
3. Run feature analysis workflow once
4. Run drift detection workflow
5. Automate: Run feature analysis weekly, drift daily

### Conditional Automation

**GitHub Actions Example**:
```yaml
name: Update Book Docs

on:
  schedule:
    - cron: '0 0 * * 0'  # Weekly (feature analysis)
    - cron: '0 0 * * *'  # Daily (drift detection)

jobs:
  feature-analysis:
    if: github.event.schedule == '0 0 * * 0'  # Only on Sundays
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Run feature analysis
        run: prodigy run workflows/book-features-analysis.yml

  drift-detection:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Run drift detection
        run: prodigy run workflows/book-docs-drift.yml
```

### Gradual Rollout

**Phase 1**: Implement chunking and merge commands
**Phase 2**: Test with Prodigy codebase (30K lines)
**Phase 3**: Document and release
**Phase 4**: Encourage adoption for projects > 20K lines
**Phase 5**: Make two-workflow pattern the default in templates

## Success Metrics

- Successfully analyze Prodigy's 30K line codebase in < 15 minutes
- Successfully analyze synthetic 100K line codebase in < 30 minutes
- Successfully analyze synthetic 200K line codebase in < 60 minutes
- Zero feature loss compared to single-pass (for small codebases)
- < 5% overhead for small codebases that don't need chunking
- DLQ retry success rate > 95%
- User documentation clear enough for external adoption
- Zero breaking changes to existing workflows

## Future Enhancements

**Intelligent Chunking** (Phase 2):
- Use AST parsing to understand module dependencies
- Group related modules together even if in different directories
- Respect import/dependency boundaries

**Incremental Analysis** (Phase 3):
- Only re-analyze changed modules
- Cache analysis results per module
- Track git history to identify changed files

**Adaptive Token Budgets** (Phase 4):
- Detect when files are documentation-heavy (more tokens per line)
- Adjust chunk sizes dynamically based on actual token usage
- Learn from previous runs to improve estimation

**Cross-Project Learning** (Phase 5):
- Share chunking strategies across similar projects
- Optimize based on historical performance data
- Recommend ideal chunk size per project type
