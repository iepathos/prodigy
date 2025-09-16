# Resume Workflows Guide

## Overview

Prodigy's resume functionality allows you to continue interrupted workflows from where they left off. This is essential for long-running operations, handling failures gracefully, and maintaining progress across sessions.

## Key Features

- **Automatic Checkpoint Creation**: Every successful command execution creates a checkpoint
- **Variable Preservation**: All workflow variables are saved and restored
- **Retry State Tracking**: Maintains retry attempts across resumptions
- **Smart Recovery**: Skips already-completed steps when resuming

## Basic Usage

### Resume Last Interrupted Workflow

```bash
# Resume the most recently interrupted workflow
prodigy resume

# Resume a specific workflow by ID
prodigy resume workflow-12345
```

### Force Restart

```bash
# Start workflow from beginning, ignoring checkpoints
prodigy resume workflow-12345 --force
```

### Resume from Specific Checkpoint

```bash
# Resume from a specific checkpoint version
prodigy resume workflow-12345 --from-checkpoint v3
```

## Checkpoint Management

### List Available Checkpoints

```bash
# List all checkpoints
prodigy checkpoints list

# List checkpoints for specific workflow
prodigy checkpoints list --workflow-id workflow-12345

# Show detailed checkpoint information
prodigy checkpoints list --verbose
```

### Show Checkpoint Details

```bash
# Show detailed information for a specific checkpoint
prodigy checkpoints show workflow-12345

# Show specific version of checkpoint
prodigy checkpoints show workflow-12345 --version 2
```

### Clean Up Checkpoints

```bash
# Clean completed workflow checkpoints
prodigy checkpoints clean --all

# Clean specific workflow checkpoint
prodigy checkpoints clean --workflow-id workflow-12345

# Force cleanup without confirmation
prodigy checkpoints clean --all --force
```

## Example Workflows

### 1. Long-Running Code Refactoring

```yaml
name: refactor-codebase
description: Refactor entire codebase with checkpoints

commands:
  - shell: "find . -name '*.py' > files.txt"
    capture_output: file_list

  - claude: "/analyze-code ${file_list}"
    id: analysis
    checkpoint: true  # Explicit checkpoint after analysis

  - shell: "python scripts/prepare_refactor.py"

  - claude: "/refactor-module src/core"
    checkpoint: true
    retry: 3

  - claude: "/refactor-module src/utils"
    checkpoint: true
    retry: 3

  - shell: "python -m pytest"
    id: test_suite

  - claude: "/fix-test-failures"
    when: "${test_suite.exit_code} != 0"
```

Resume after interruption:
```bash
# Check status
prodigy checkpoints list --workflow-id refactor-codebase

# Resume from last checkpoint
prodigy resume refactor-codebase
```

### 2. Data Processing Pipeline

```yaml
name: data-pipeline
description: Process large dataset with resumable stages

variables:
  batch_size: 1000
  output_dir: "processed_data"

commands:
  - shell: "mkdir -p ${output_dir}"

  - shell: "python extract_data.py --batch-size ${batch_size}"
    id: extract
    timeout: 3600
    checkpoint: true

  - shell: "python transform_data.py --input raw_data --output ${output_dir}"
    id: transform
    checkpoint: true
    retry: 2

  - shell: "python validate_data.py ${output_dir}"
    id: validate

  - claude: "/generate-report ${output_dir}/validation.json"
    when: "${validate.success}"
```

Resume with monitoring:
```bash
# Start pipeline
prodigy cook data-pipeline.yaml

# If interrupted, check progress
prodigy checkpoints show data-pipeline --verbose

# Resume from interruption
prodigy resume data-pipeline

# Force restart if needed
prodigy resume data-pipeline --force
```

### 3. MapReduce with Checkpoint Recovery

```yaml
name: analyze-large-codebase
mode: mapreduce

setup:
  - shell: "find . -name '*.js' -o -name '*.ts' | jq -R . | jq -s . > files.json"

map:
  input: "files.json"
  agent_template:
    - claude: "/analyze-file ${item}"
    - shell: "eslint ${item} --fix"
  max_parallel: 10
  checkpoint_frequency: 50  # Checkpoint every 50 items

reduce:
  - claude: "/summarize-analysis ${map.results}"
  - shell: "python generate_report.py"
```

Resume MapReduce job:
```bash
# Start MapReduce job
prodigy cook analyze-large-codebase.yaml

# If interrupted, check job status
prodigy resume-job analyze-large-codebase

# Resume with additional workers
prodigy resume-job analyze-large-codebase --max-parallel 20

# Reprocess failed items from DLQ
prodigy dlq retry analyze-large-codebase
```

### 4. Multi-Stage Deployment

```yaml
name: staged-deployment
description: Deploy application with resumable stages

stages:
  - name: build
    commands:
      - shell: "npm run build"
      - shell: "npm run test"
    checkpoint: true

  - name: staging
    commands:
      - shell: "npm run deploy:staging"
      - claude: "/run-integration-tests staging"
    checkpoint: true
    retry: 2

  - name: production
    commands:
      - claude: "/verify-staging-metrics"
      - shell: "npm run deploy:production"
    checkpoint: true
    requires_confirmation: true
```

Resume deployment:
```bash
# Start deployment
prodigy cook staged-deployment.yaml

# If interrupted at staging
prodigy checkpoints list --workflow-id staged-deployment
# Output: Stage 2/3 - staging (in progress)

# Resume from staging
prodigy resume staged-deployment

# Skip to production (if staging verified manually)
prodigy resume staged-deployment --from-checkpoint production
```

### 5. Error Recovery with Checkpoints

```yaml
name: resilient-workflow
description: Workflow with error handling and recovery

commands:
  - shell: "python initialize.py"
    id: init

  - shell: "python risky_operation.py"
    id: risky_op
    retry: 3
    on_failure:
      - shell: "python cleanup.py"
      - claude: "/diagnose-failure ${risky_op.error}"
    checkpoint: true

  - shell: "python continue_processing.py"
    when: "${risky_op.success}"

  - claude: "/handle-partial-success"
    when: "${risky_op.partial_success}"
```

Resume with error recovery:
```bash
# If workflow fails
prodigy cook resilient-workflow.yaml

# Check failure state
prodigy checkpoints show resilient-workflow --verbose

# Fix issue and resume
# ... fix the issue ...
prodigy resume resilient-workflow

# Resume with modified retry count
prodigy resume resilient-workflow --max-retries 5
```

## Best Practices

### 1. Strategic Checkpoint Placement

Place checkpoints after:
- Time-consuming operations
- Operations that modify state
- Network or API calls
- Before risky operations

```yaml
commands:
  - claude: "/large-refactor"
    checkpoint: true  # Save progress after expensive operation

  - shell: "deploy-to-production.sh"
    checkpoint: true  # Save before risky operation
```

### 2. Variable Preservation

Ensure critical variables are captured for resume:

```yaml
commands:
  - shell: "generate-config.sh"
    capture_output: config
    checkpoint: true  # Config preserved for resume

  - claude: "/process --config ${config}"
```

### 3. Idempotent Commands

Design commands to be safely re-run:

```yaml
commands:
  # Good: Idempotent
  - shell: "mkdir -p output"
  - shell: "cp -f source.txt dest.txt"

  # Bad: Not idempotent
  - shell: "echo 'data' >> file.txt"  # Appends each time
```

### 4. Checkpoint Cleanup

Regular maintenance of checkpoints:

```bash
# Clean old completed checkpoints
prodigy checkpoints clean --all

# Set up automated cleanup
0 0 * * * prodigy checkpoints clean --all --force
```

### 5. Testing Resume Behavior

Test workflow resumption during development:

```bash
# Simulate interruption
prodigy cook workflow.yaml &
PID=$!
sleep 10
kill -INT $PID

# Test resume
prodigy resume
```

## Checkpoint Storage

Checkpoints are stored in:
- Local: `.prodigy/checkpoints/{workflow-id}.json`
- Global: `~/.prodigy/state/{repo}/checkpoints/{workflow-id}.json`

### Checkpoint Format

```json
{
  "workflow_id": "workflow-12345",
  "workflow_path": "workflow.yaml",
  "timestamp": "2024-01-20T10:30:00Z",
  "execution_state": {
    "status": "interrupted",
    "current_step_index": 3,
    "total_steps": 10
  },
  "variables": {
    "custom_var": "value",
    "shell": {
      "output": "last command output"
    }
  },
  "completed_steps": [
    {
      "step_index": 0,
      "command": "shell: initialize.sh",
      "success": true,
      "timestamp": "2024-01-20T10:25:00Z"
    }
  ],
  "retry_state": {
    "current_attempt": 2,
    "max_attempts": 3,
    "last_error": "Connection timeout"
  }
}
```

## Troubleshooting

### Checkpoint Not Found

```bash
# Verify checkpoint exists
ls .prodigy/checkpoints/

# Check global storage
ls ~/.prodigy/state/$(basename $(pwd))/checkpoints/

# List all available checkpoints
prodigy checkpoints list
```

### Resume Fails

```bash
# Check checkpoint validity
prodigy checkpoints show workflow-id --verbose

# Force restart if checkpoint corrupted
prodigy resume workflow-id --force

# Clean corrupted checkpoint
rm .prodigy/checkpoints/workflow-id.json
```

### Variable Loss

Ensure variables are properly captured:

```yaml
# Correct: Variable captured
- shell: "echo 'value'"
  capture_output: my_var

# Use variable in resume
- claude: "/process ${my_var}"
```

## Performance Considerations

- Checkpoint operations add <5% overhead
- Checkpoints are saved asynchronously
- Large variable states may impact save time
- Use `checkpoint_frequency` for MapReduce jobs

## Migration from Older Versions

For workflows created before checkpoint support:

1. Add explicit checkpoints to critical steps
2. Ensure variable capture for state preservation
3. Test resume behavior in development
4. Update error handling for checkpoint-aware recovery

## Related Commands

- `prodigy cook --resume <session-id>`: Resume cooking session
- `prodigy resume-job <job-id>`: Resume MapReduce job
- `prodigy dlq retry <workflow-id>`: Reprocess failed items
- `prodigy events ls --job-id <id>`: View workflow events