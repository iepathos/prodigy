## Troubleshooting

This guide helps you diagnose and fix issues when running automated documentation workflows. The workflow executes in three phases (setup, map, reduce), and each phase has specific failure modes and debugging techniques.

**Source**: Based on `workflows/book-docs-drift.yml` MapReduce workflow implementation and command definitions in `.claude/commands/prodigy-*-book*.md`

## Common Issues by Phase

### Setup Phase Issues

The setup phase analyzes your codebase and detects documentation gaps. Common failures:

#### Issue: "features.json not generated"

**Symptoms**: Setup completes but `.prodigy/book-analysis/features.json` doesn't exist

**Causes**:
- Invalid `book-config.json` configuration
- Missing or incorrect `analysis_targets` paths
- Source files specified in config don't exist

**Solution**:
```bash
# 1. Verify book-config.json is valid JSON
cat .prodigy/book-config.json | jq .

# 2. Check that source files exist
cat .prodigy/book-config.json | jq -r '.analysis_targets[].source_files[]' | while read f; do
  [ -f "$f" ] || echo "Missing: $f"
done

# 3. Re-run feature analysis manually
prodigy run workflows/book-docs-drift.yml --stop-after setup
```

**Source**: Configuration structure from `.prodigy/book-config.json:7-213`

#### Issue: "No gaps detected when gaps should exist"

**Symptoms**: `/prodigy-detect-documentation-gaps` reports no gaps but documentation is clearly incomplete

**Causes**:
- `chapters_file` path is incorrect in workflow
- Chapter definitions JSON is malformed
- Features not properly mapped to chapters

**Solution**:
```bash
# 1. Verify chapters file exists and is valid
cat workflows/data/prodigy-chapters.json | jq .

# 2. Check gap detection output
cat .prodigy/book-analysis/gaps.json | jq .

# 3. Validate flattened items were generated
cat .prodigy/book-analysis/flattened-items.json | jq 'length'
```

**Source**: Gap detection logic from `.claude/commands/prodigy-detect-documentation-gaps.md`

#### Issue: "Setup phase commits nothing"

**Symptoms**: Setup completes successfully but no commit is created

**Causes**: This is often **not an error** - setup commands use `commit_required: false` by default and only commit when:
- Features have changed since last analysis
- New documentation gaps are detected
- New stub files are created

**No action needed** if this occurs - the workflow will continue to map phase with existing analysis files.

**Source**: Setup phase design from `workflows/book-docs-drift.yml:24-34`

### Map Phase Issues

The map phase processes each chapter/subsection in parallel. Each runs in its own agent worktree.

#### Issue: "Agent failed with validation error"

**Symptoms**: Agent completes but fails validation threshold (100% required)

**Causes**:
- Documentation fix was incomplete
- Examples don't match codebase implementation
- Required sections are missing

**Solution**:
```bash
# 1. Check which agent failed
prodigy dlq show <job_id>

# 2. Find the drift report for the failed item
ls -la .prodigy/book-analysis/drift-*.json

# 3. Review what issues were identified
cat .prodigy/book-analysis/drift-<chapter-id>-<subsection-id>.json | jq '.issues'

# 4. Retry with DLQ
prodigy dlq retry <job_id>
```

**Source**: Validation configuration from `workflows/book-docs-drift.yml:49-57`

#### Issue: "Drift analysis creates empty report"

**Symptoms**: `/prodigy-analyze-subsection-drift` commits but drift report shows no issues when drift clearly exists

**Causes**:
- Feature mappings are incorrect for the subsection
- Source files referenced in feature analysis are empty
- Chapter metadata doesn't match actual file

**Solution**:
```bash
# 1. Inspect the drift report
cat .prodigy/book-analysis/drift-<chapter-id>-<subsection-id>.json | jq .

# 2. Verify feature mappings for this item
cat .prodigy/book-analysis/flattened-items.json | jq '.[] | select(.id=="<subsection-id>")'

# 3. Check that features.json has content for this area
cat .prodigy/book-analysis/features.json | jq 'keys'
```

**Source**: Drift analysis command from `.claude/commands/prodigy-analyze-subsection-drift.md`

#### Issue: "Multiple agents fail with same error"

**Symptoms**: Several parallel agents all fail with identical error messages

**Causes**:
- Shared resource conflict (rare with worktree isolation)
- Configuration error affecting all agents
- System resource exhaustion (disk space, memory)

**Solution**:
```bash
# 1. Check system resources
df -h  # Disk space
free -h  # Memory (Linux) or vm_stat (macOS)

# 2. Review error collection
prodigy events <job_id> | jq 'select(.event_type=="AgentFailed")'

# 3. Reduce parallelism and retry
# Edit workflow: max_parallel: 1
prodigy run workflows/book-docs-drift.yml
```

**Source**: Error handling policy from `workflows/book-docs-drift.yml:86-91`

### Reduce Phase Issues

The reduce phase rebuilds the book and cleans up analysis files.

#### Issue: "mdbook build fails with broken links"

**Symptoms**: `mdbook build` exits with error listing broken internal links

**Causes**:
- Cross-references to non-existent chapters
- Relative paths calculated incorrectly
- SUMMARY.md out of sync with actual files

**Solution**:
The workflow automatically handles this:
```yaml
- shell: "cd book && mdbook build"
  on_failure:
    claude: "/prodigy-fix-book-build-errors --project $PROJECT_NAME"
```

If manual fix needed:
```bash
# 1. See the exact broken links
cd book && mdbook build 2>&1 | grep "Broken link"

# 2. Fix broken links manually
# Edit the markdown files to use correct relative paths

# 3. Verify SUMMARY.md includes all files
cat book/src/SUMMARY.md
```

**Source**: Reduce phase from `workflows/book-docs-drift.yml:62-68`

#### Issue: "Analysis files not cleaned up"

**Symptoms**: `.prodigy/book-analysis/` directory still exists after workflow completion

**Causes**:
- Reduce phase didn't complete
- Cleanup command failed silently (uses `|| true`)

**Solution**:
```bash
# Manual cleanup is safe
rm -rf .prodigy/book-analysis

# Check if this was part of incomplete workflow
prodigy sessions list
```

This is cosmetic - analysis files are regenerated on each run.

**Source**: Cleanup step from `workflows/book-docs-drift.yml:81-82`

## Debugging Techniques

### Inspecting Analysis Artifacts

All intermediate analysis files are stored in `.prodigy/book-analysis/`:

```bash
# Feature inventory from codebase analysis
cat .prodigy/book-analysis/features.json | jq .

# Documentation gaps detected
cat .prodigy/book-analysis/gaps.json | jq .

# Flattened items for map phase (chapters + subsections)
cat .prodigy/book-analysis/flattened-items.json | jq .

# Drift reports (one per chapter/subsection)
ls -la .prodigy/book-analysis/drift-*.json
cat .prodigy/book-analysis/drift-<chapter-id>-<subsection-id>.json | jq .
```

**Source**: File locations from `workflows/book-docs-drift.yml:9-18` and setup phase commands

### Reviewing Event Logs

MapReduce workflows generate detailed event logs:

```bash
# List all events for a job
prodigy events <job_id>

# Filter to agent failures
prodigy events <job_id> | jq 'select(.event_type=="AgentFailed")'

# See what items completed successfully
prodigy events <job_id> | jq 'select(.event_type=="AgentCompleted") | .agent_id'

# Find Claude JSON log locations for failed agents
prodigy events <job_id> | jq 'select(.event_type=="AgentCompleted") | .json_log_location'
```

**Source**: Event tracking implementation from `src/cook/execution/events/event_types.rs` and CLI handler `src/cli/commands/events.rs`

### Checking Dead Letter Queue (DLQ)

Failed work items are sent to the DLQ for review and retry:

```bash
# Show all failed items for a job
prodigy dlq show <job_id>

# See failure details with JSON log locations
prodigy dlq show <job_id> | jq '.items[].failure_history'

# Get Claude log path for debugging
prodigy dlq show <job_id> | jq '.items[].failure_history[].json_log_location'

# Retry all failed items
prodigy dlq retry <job_id>

# Retry with custom parallelism
prodigy dlq retry <job_id> --max-parallel 10

# See DLQ statistics
prodigy dlq stats <job_id>
```

**Source**: DLQ implementation from `src/cook/execution/dlq.rs` and CLI handler `src/cli/commands/dlq.rs`

### Examining Claude Command Logs

Each Claude command execution creates a JSON log file with complete conversation history:

```bash
# Find the log location from workflow output (with -v flag)
prodigy run workflows/book-docs-drift.yml -v

# Or from DLQ item failure details
LOG_PATH=$(prodigy dlq show <job_id> | jq -r '.items[0].failure_history[0].json_log_location')

# View the full conversation
cat "$LOG_PATH" | jq .

# Extract tool invocations
cat "$LOG_PATH" | jq '.messages[] | select(.role=="assistant") | .content[] | select(.type=="tool_use")'

# Check for errors
cat "$LOG_PATH" | jq '.messages[] | select(.type=="error")'
```

**Source**: Claude log tracking from Spec 121 (JSON Log Location Tracking) and `src/cook/execution/mapreduce/agent_result.rs`

### Testing Individual Commands

You can run workflow commands individually for debugging:

```bash
# Run feature analysis only
claude /prodigy-analyze-features-for-book --project Prodigy --config .prodigy/book-config.json

# Run gap detection only (requires features.json first)
claude /prodigy-detect-documentation-gaps --project Prodigy --config .prodigy/book-config.json --features .prodigy/book-analysis/features.json --chapters workflows/data/prodigy-chapters.json --book-dir book

# Analyze specific chapter for drift (requires features.json)
claude /prodigy-analyze-subsection-drift --project Prodigy --json '{"type":"subsection","id":"troubleshooting","parent_chapter_id":"automated-documentation","file":"book/src/automated-documentation/troubleshooting.md"}' --features .prodigy/book-analysis/features.json

# Fix specific chapter drift (requires drift report)
claude /prodigy-fix-subsection-drift --project Prodigy --json '{"type":"subsection","id":"troubleshooting","parent_chapter_id":"automated-documentation","file":"book/src/automated-documentation/troubleshooting.md"}'
```

**Source**: Command definitions from `.claude/commands/prodigy-*-book*.md`

## Resume and Recovery

### Resuming Interrupted Workflows

MapReduce workflows support checkpoint-based resume. See the [Checkpoint and Resume](../mapreduce/checkpoint-and-resume.md) documentation for details.

```bash
# Resume using session ID
prodigy resume session-mapreduce-1234567890

# Resume using job ID
prodigy resume-job mapreduce-1234567890

# Unified resume (auto-detects ID type)
prodigy resume mapreduce-1234567890
```

**Source**: Resume functionality from Spec 134 (MapReduce Checkpoint and Resume)

### Retrying Failed Items from DLQ

After a workflow completes with failures:

```bash
# 1. Review what failed
prodigy dlq show <job_id>

# 2. Retry all failed items
prodigy dlq retry <job_id>

# 3. Monitor progress
prodigy events <job_id>
```

The DLQ retry creates a new execution context but preserves correlation IDs for tracking.

**Source**: DLQ retry implementation from `src/cook/execution/dlq_reprocessor.rs`

## File Locations Reference

Key files and directories for troubleshooting:

| Location | Description | Phase |
|----------|-------------|-------|
| `.prodigy/book-config.json` | Project configuration for documentation | Setup input |
| `workflows/data/prodigy-chapters.json` | Chapter structure definitions | Setup input |
| `.prodigy/book-analysis/features.json` | Extracted codebase features | Setup output |
| `.prodigy/book-analysis/gaps.json` | Detected documentation gaps | Setup output |
| `.prodigy/book-analysis/flattened-items.json` | Work items for map phase | Setup output |
| `.prodigy/book-analysis/drift-*.json` | Per-chapter drift reports | Map output |
| `~/.prodigy/events/<repo>/<job_id>/` | Event logs (global storage) | All phases |
| `~/.prodigy/dlq/<repo>/<job_id>/` | Dead letter queue items | Map phase |
| `~/.prodigy/state/<repo>/mapreduce/jobs/<job_id>/` | Checkpoint files | All phases |
| `~/.local/state/claude/logs/session-*.json` | Claude command logs | All phases |

**Source**: Storage locations from global storage architecture (Spec 127) and workflow environment variables in `workflows/book-docs-drift.yml:9-18`

## Performance Tips

### Adjusting Parallelism

The workflow uses `max_parallel: 3` by default. Adjust based on your system:

```yaml
env:
  MAX_PARALLEL: "5"  # Process 5 chapters concurrently

map:
  max_parallel: ${MAX_PARALLEL}
```

**Trade-offs**:
- Higher parallelism = faster completion, more system resources
- Lower parallelism = slower completion, fewer failures from resource contention

**Source**: Parallelism configuration from `workflows/book-docs-drift.yml:21,59`

### Processing Subset of Chapters

Use JSONPath filters to target specific documentation:

```yaml
map:
  input: "${ANALYSIS_DIR}/flattened-items.json"
  json_path: "$[*]"
  filter: "item.parent_chapter_id == 'mapreduce'"  # Only MapReduce subsections
```

Or manually edit `flattened-items.json` to include only desired items.

**Source**: Filter syntax from MapReduce workflow specification

### Skipping Validation for Drafts

For faster iteration during development, reduce validation threshold:

```yaml
map:
  agent_template:
    - claude: "/prodigy-fix-subsection-drift --project $PROJECT_NAME --json '${item}'"
      commit_required: true
      validate:
        threshold: 70  # Accept 70% quality instead of 100%
```

**Warning**: This may result in lower quality documentation.

**Source**: Validation configuration from `workflows/book-docs-drift.yml:49-57`

## Configuration Issues

### Invalid book-config.json

**Symptoms**: Setup phase fails immediately or generates no features

**Solution**:
```bash
# Validate JSON syntax
cat .prodigy/book-config.json | jq empty

# Check required fields exist
cat .prodigy/book-config.json | jq '{project_name, analysis_targets, chapter_file}'
```

**Required fields**:
- `project_name` - Project display name
- `analysis_targets` - Array of areas with source files
- `chapter_file` - Path to chapter definitions

**Source**: Configuration structure from `.prodigy/book-config.json:1-220`

### Missing Source Files in analysis_targets

**Symptoms**: Features not extracted for certain areas

**Solution**:
```bash
# Check all referenced source files exist
cat .prodigy/book-config.json | jq -r '.analysis_targets[].source_files[]' | while read file; do
  if [ ! -e "$file" ]; then
    echo "Missing: $file"
  fi
done
```

Update paths in `book-config.json` to match actual source file locations.

**Source**: Analysis targets from `.prodigy/book-config.json:7-213`

### Incorrect Chapter Definitions

**Symptoms**: Gaps detected for chapters that already exist, or no gaps when chapters are missing

**Solution**:
```bash
# Verify chapter definitions match actual book structure
diff <(cat workflows/data/prodigy-chapters.json | jq -r '.chapters[].id' | sort) \
     <(find book/src -name "index.md" -o -name "[!index]*.md" | sed 's|book/src/||; s|/index.md||; s|\.md||' | sort)
```

Update `workflows/data/prodigy-chapters.json` to match your book structure.

**Source**: Chapter definitions referenced in `workflows/book-docs-drift.yml:18`

## FAQ

**Q: Why does setup phase show "No changes" even though I modified source code?**

A: Feature analysis only commits when features **change**. Code changes don't always mean feature changes (e.g., bug fixes, refactoring). This is expected behavior.

---

**Q: Can I run the workflow on a subset of chapters?**

A: Yes. Either:
1. Use `filter` in map phase to select specific items
2. Manually edit `.prodigy/book-analysis/flattened-items.json` after setup
3. Modify chapter definitions to exclude certain chapters

---

**Q: What happens if I interrupt the workflow?**

A: Use `prodigy resume <job_id>` to continue from the last checkpoint. See [Checkpoint and Resume](../mapreduce/checkpoint-and-resume.md) for details.

---

**Q: How do I debug why a specific chapter failed validation?**

A:
```bash
# 1. Find the validation result
cat .prodigy/validation-result.json | jq .

# 2. Check the drift report for this chapter
cat .prodigy/book-analysis/drift-<chapter-id>-<subsection-id>.json | jq .

# 3. Review Claude's attempt to fix it
prodigy dlq show <job_id> | jq '.items[] | select(.id=="<subsection-id>")'
```

---

**Q: Can I customize what gets analyzed?**

A: Yes. Edit `.prodigy/book-config.json` to:
- Add/remove `analysis_targets` areas
- Change which source files are analyzed per area
- Adjust `feature_categories` to extract different information
- Enable/disable examples, best practices, troubleshooting in `custom_analysis`

---

**Q: The workflow is too slow. How can I speed it up?**

A:
1. Increase `max_parallel` (default: 3)
2. Process fewer chapters using filters
3. Use `--stop-after setup` to only regenerate analysis files
4. Reduce validation threshold for draft iterations

## See Also

- [Understanding the Workflow](understanding-the-workflow.md) - Workflow phase details
- [Checkpoint and Resume](../mapreduce/checkpoint-and-resume.md) - Resume interrupted workflows
- [Dead Letter Queue](../mapreduce/dead-letter-queue-dlq.md) - Handling persistent failures
- [Event Tracking](../mapreduce/event-tracking.md) - Monitoring workflow execution
- [Advanced Configuration](advanced-configuration.md) - Customizing the workflow
