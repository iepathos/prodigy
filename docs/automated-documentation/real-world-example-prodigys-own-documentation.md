## Real-World Example: Prodigy's Own Documentation

This documentation you're reading is maintained by the same workflow described in this chapter. This is a complete, production-ready workflow that demonstrates:

- **MapReduce parallelism** for processing multiple chapters/subsections concurrently
- **Validation with thresholds** to ensure documentation meets quality standards
- **Automatic gap-filling** to complete incomplete documentation
- **Multi-subsection chapter support** for organizing complex topics
- **Subsection-aware commands** that handle both single-file chapters and individual subsections
- **Error handling with DLQ** for robust failure recovery

You can examine the actual configuration files used to maintain this documentation:

### Book Configuration

**File**: `.prodigy/book-config.json`

```json
{
  "project_name": "Prodigy",
  "project_type": "cli_tool",
  "book_dir": "book",
  "book_src": "book/src",
  "book_build_dir": "book/book",
  "analysis_targets": [
    {
      "area": "workflow_basics",
      "source_files": ["src/config/workflow.rs", "src/cook/workflow/executor.rs"],
      "feature_categories": ["structure", "execution_model", "commit_tracking"]
    },
    {
      "area": "mapreduce",
      "source_files": ["src/config/mapreduce.rs", "src/cook/execution/mapreduce/"],
      "feature_categories": ["phases", "capabilities", "configuration"]
    },
    {
      "area": "command_types",
      "source_files": ["src/config/command.rs"],
      "feature_categories": ["shell", "claude", "foreach", "validation"]
    }
  ]
}
```

**Source**: `.prodigy/book-config.json:1-46`

### Chapter Structure

**File**: `workflows/data/prodigy-chapters.json`

This file defines both **single-file chapters** (one markdown file per chapter) and **multi-subsection chapters** (chapters split across multiple files):

```json
{
  "chapters": [
    {
      "id": "workflow-basics",
      "title": "Workflow Basics",
      "type": "multi-subsection",
      "topics": ["Standard workflows", "Basic structure", "Command execution"],
      "validation": "Check basic workflow syntax and structure documentation",
      "index_file": "book/src/workflow-basics/index.md",
      "subsections": [
        {
          "id": "command-types",
          "title": "Command Types",
          "file": "book/src/workflow-basics/command-types.md",
          "topics": ["Command Types"],
          "validation": "Check command types documentation matches implementation"
        },
        {
          "id": "environment-configuration",
          "title": "Environment Configuration",
          "file": "book/src/workflow-basics/environment-configuration.md",
          "topics": ["Environment Configuration"],
          "validation": "Check environment configuration documentation matches implementation"
        }
      ]
    }
  ]
}
```

**Source**: `workflows/data/prodigy-chapters.json:1-80`

The setup phase command `/prodigy-detect-documentation-gaps` creates a `flattened-items.json` file containing both single-file chapters and individual subsections with parent metadata. This enables the map phase to process each subsection independently with full awareness of its parent chapter context.

### Workflow Configuration

**File**: `workflows/book-docs-drift.yml`

This MapReduce workflow orchestrates the entire documentation maintenance process:

**Source**: `workflows/book-docs-drift.yml:1-101`

**Key Features Demonstrated:**

**1. Setup Phase** (lines 24-34):
- Analyzes codebase for feature coverage
- Detects documentation gaps and creates missing chapters/subsections
- Generates `flattened-items.json` for subsection-aware processing

**2. Map Phase** (lines 36-58):
- Processes each chapter/subsection in parallel using subsection-aware commands:
  - `/prodigy-analyze-subsection-drift` - Analyzes drift for single-file chapters or individual subsections
  - `/prodigy-fix-subsection-drift` - Fixes drift while preserving subsection scope and cross-references
  - `/prodigy-validate-doc-fix` - Validates documentation meets quality standards
  - `/prodigy-complete-doc-fix` - Fills gaps if validation score is below threshold

**3. Validation with Threshold** (lines 49-57):
```yaml
validate:
  claude: "/prodigy-validate-doc-fix --project $PROJECT_NAME --json '${item}' --output .prodigy/validation-result.json"
  result_file: ".prodigy/validation-result.json"
  threshold: 100  # Documentation must meet 100% quality standards
  on_incomplete:
    claude: "/prodigy-complete-doc-fix --project $PROJECT_NAME --json '${item}' --gaps ${validation.gaps}"
    max_attempts: 3
    fail_workflow: false  # Continue even if we can't reach 100%
    commit_required: true  # Require commit to verify improvements were made
```

The validation step ensures documentation quality by checking against a score threshold. If the score is below 100, the `on_incomplete` handler attempts to fill gaps with up to 3 attempts.

**4. Error Handling** (lines 86-90):
```yaml
error_policy:
  on_item_failure: dlq
  continue_on_failure: true
  max_failures: 2
  error_collection: aggregate
```

Failed items are sent to the Dead Letter Queue (DLQ) for later retry, allowing the workflow to continue processing other items.

**5. Custom Merge Workflow** (lines 93-101):
```yaml
merge:
  commands:
    - shell: "git fetch origin"
    - claude: "/prodigy-merge-master --project ${PROJECT_NAME}"
    - claude: "/prodigy-merge-worktree ${merge.source_branch} ${merge.target_branch}"
```

Custom merge commands handle integration with the main branch and final merge back to the original branch.

### Study These Files

To understand the complete implementation:
- **Configuration**: `.prodigy/book-config.json` - Book and analysis configuration
- **Chapter Structure**: `workflows/data/prodigy-chapters.json` - Chapter and subsection definitions
- **Workflow**: `workflows/book-docs-drift.yml` - Complete MapReduce workflow
- **Commands**: `.claude/commands/prodigy-analyze-subsection-drift.md` - Subsection-aware drift analysis
- **Commands**: `.claude/commands/prodigy-fix-subsection-drift.md` - Subsection-aware drift fixing

