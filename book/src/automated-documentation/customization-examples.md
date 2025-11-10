## Customization Examples

This section demonstrates how to adapt the automated documentation workflow for different project types, languages, and requirements. All examples are based on real configurations from the Prodigy project.

### Language-Specific Adaptations

The automated documentation workflow can be customized for different programming languages by adjusting the `analysis_targets` in your `book-config.json`.

#### Rust Project (Default Configuration)

**Source**: `.prodigy/book-config.json:7-31`

```json
{
  "project_name": "Prodigy",
  "project_type": "cli_tool",
  "book_dir": "book",
  "analysis_targets": [
    {
      "area": "workflow_basics",
      "source_files": [
        "src/config/workflow.rs",
        "src/cook/workflow/executor.rs"
      ],
      "feature_categories": [
        "structure",
        "execution_model",
        "commit_tracking"
      ]
    },
    {
      "area": "mapreduce",
      "source_files": [
        "src/config/mapreduce.rs",
        "src/cook/execution/mapreduce/"
      ],
      "feature_categories": [
        "phases",
        "capabilities",
        "configuration"
      ]
    }
  ]
}
```

**Key customization points**:
- `source_files`: Paths to Rust source files (`.rs` extension)
- Directory patterns: `src/cook/execution/mapreduce/` analyzes all files in that directory
- `feature_categories`: Organize by Rust-specific concepts (structs, enums, traits)

#### Python Project Adaptation

```json
{
  "project_name": "MyPythonProject",
  "project_type": "library",
  "book_dir": "docs",
  "analysis_targets": [
    {
      "area": "core_api",
      "source_files": [
        "src/myproject/api.py",
        "src/myproject/client.py"
      ],
      "feature_categories": [
        "classes",
        "functions",
        "decorators"
      ]
    },
    {
      "area": "data_models",
      "source_files": [
        "src/myproject/models/"
      ],
      "feature_categories": [
        "dataclasses",
        "validation",
        "serialization"
      ]
    }
  ]
}
```

**Differences for Python**:
- File extension: `.py` instead of `.rs`
- Feature categories: `classes`, `functions`, `decorators` (Python-specific)
- Common patterns: Separate `models/` directory, `api.py` modules

#### JavaScript/TypeScript Project

```json
{
  "project_name": "MyJSProject",
  "project_type": "web_framework",
  "book_dir": "book",
  "analysis_targets": [
    {
      "area": "components",
      "source_files": [
        "src/components/**/*.tsx",
        "src/components/**/*.ts"
      ],
      "feature_categories": [
        "react_components",
        "hooks",
        "context"
      ]
    },
    {
      "area": "api",
      "source_files": [
        "src/api/",
        "src/types/"
      ],
      "feature_categories": [
        "endpoints",
        "types",
        "interfaces"
      ]
    }
  ]
}
```

**JavaScript/TypeScript specifics**:
- Glob patterns: `**/*.tsx`, `**/*.ts` for nested directories
- Feature categories: `react_components`, `hooks` for React projects
- Type definitions: Separate `types/` directory common in TypeScript

### Workflow Customization Patterns

The workflow YAML can be customized for different use cases by adjusting parallelism, timeouts, and error handling.

#### High-Parallelism Workflow (Large Codebases)

**Source**: `workflows/book-docs-drift.yml:9-21`

```yaml
name: fast-documentation-update
mode: mapreduce

env:
  PROJECT_NAME: "LargeProject"
  PROJECT_CONFIG: ".prodigy/book-config.json"
  FEATURES_PATH: ".prodigy/book-analysis/features.json"
  BOOK_DIR: "book"
  ANALYSIS_DIR: ".prodigy/book-analysis"
  CHAPTERS_FILE: "workflows/data/chapters.json"
  MAX_PARALLEL: "10"  # High parallelism for large projects

setup:
  - shell: "mkdir -p $ANALYSIS_DIR"
  - claude: "/analyze-features --project $PROJECT_NAME --config $PROJECT_CONFIG"

map:
  input: "${ANALYSIS_DIR}/flattened-items.json"
  json_path: "$[*]"
  agent_template:
    - claude: "/analyze-drift --project $PROJECT_NAME --json '${item}' --features $FEATURES_PATH"
      commit_required: true
    - claude: "/fix-drift --project $PROJECT_NAME --json '${item}'"
      commit_required: true
  max_parallel: ${MAX_PARALLEL}  # Use environment variable
```

**Customization highlights**:
- `MAX_PARALLEL: "10"`: Process 10 chapters simultaneously (vs default 3)
- Use for projects with 50+ documentation chapters
- Requires adequate system resources (CPU, memory)

**Source reference**: Default parallelism setting from `workflows/book-docs-drift.yml:21`

#### Conservative Workflow (Strict Validation)

```yaml
name: strict-documentation-workflow
mode: mapreduce

env:
  MAX_PARALLEL: "1"  # Sequential processing for careful review

map:
  input: "${ANALYSIS_DIR}/items.json"
  json_path: "$[*]"
  agent_template:
    - claude: "/analyze-drift --project $PROJECT_NAME --json '${item}'"
      commit_required: true
    - claude: "/fix-drift --project $PROJECT_NAME --json '${item}'"
      commit_required: true
      validate:
        claude: "/validate-fix --project $PROJECT_NAME --json '${item}' --output .prodigy/validation.json"
        result_file: ".prodigy/validation.json"
        threshold: 100  # 100% quality required
        on_incomplete:
          claude: "/complete-fix --project $PROJECT_NAME --json '${item}' --gaps ${validation.gaps}"
          max_attempts: 5  # More attempts for quality
          fail_workflow: true  # Fail if quality not met
  max_parallel: ${MAX_PARALLEL}

error_policy:
  on_item_failure: fail_immediately  # Stop on first error
  continue_on_failure: false
  max_failures: 0
```

**Use cases**:
- Critical documentation (public APIs, compliance docs)
- When every chapter must meet quality standards
- Pre-release documentation review

**Source reference**: Validation configuration from `workflows/book-docs-drift.yml:49-56`

#### CI/CD-Optimized Workflow

```yaml
name: ci-documentation-check
mode: mapreduce

env:
  MAX_PARALLEL: "5"
  CI_MODE: "true"

setup:
  - shell: "mkdir -p $ANALYSIS_DIR"
  - claude: "/analyze-features --project $PROJECT_NAME --config $PROJECT_CONFIG"
    timeout: 300  # 5-minute timeout for CI

map:
  input: "${ANALYSIS_DIR}/items.json"
  json_path: "$[*]"
  agent_template:
    - claude: "/analyze-drift --project $PROJECT_NAME --json '${item}'"
      timeout: 180  # 3-minute timeout per chapter
  max_parallel: ${MAX_PARALLEL}

reduce:
  - shell: "cd book && mdbook build"
  - shell: "test -d book/book || exit 1"  # Verify build output

error_policy:
  on_item_failure: dlq
  continue_on_failure: true
  max_failures: 5  # Allow some failures in CI
  error_collection: aggregate
```

**CI/CD features**:
- Timeouts on all commands to prevent CI hangs
- Error aggregation instead of fail-fast
- Verification steps in reduce phase
- DLQ for failed items (can retry later)

**Source reference**: Error policy from `workflows/book-docs-drift.yml:86-90`

#### Development Workflow (Verbose Output)

Set environment variables for detailed logging:

```bash
# Enable verbose Claude output
export PRODIGY_CLAUDE_CONSOLE_OUTPUT=true

# Run workflow with verbose flag
prodigy run workflows/book-docs-drift.yml -v
```

**Source reference**: Verbosity control documented in `workflows/book-docs-drift.yml` comments and CLAUDE.md

### Chapter Structure Customization

The `chapters.json` file defines your documentation structure. You can choose between flat and hierarchical organization.

#### Simple Flat Structure (Small Projects)

**Source**: `workflows/data/prodigy-chapters.json:245-258`

```json
{
  "chapters": [
    {
      "id": "commands",
      "title": "Command Types",
      "type": "single-file",
      "file": "book/src/commands.md",
      "topics": [
        "Shell commands",
        "Claude commands",
        "Goal-seeking"
      ],
      "validation": "Check all command types documented"
    }
  ]
}
```

**When to use**:
- Projects with < 10 documentation chapters
- Simple, linear documentation flow
- Quick setup and maintenance

#### Multi-Level Hierarchical Structure (Complex Projects)

**Source**: `workflows/data/prodigy-chapters.json:89-234`

```json
{
  "chapters": [
    {
      "id": "mapreduce",
      "title": "MapReduce Workflows",
      "type": "multi-subsection",
      "index_file": "book/src/mapreduce/index.md",
      "topics": [
        "MapReduce mode",
        "Setup phase",
        "Map phase",
        "Reduce phase"
      ],
      "subsections": [
        {
          "id": "checkpoint-and-resume",
          "title": "Checkpoint and Resume",
          "file": "book/src/mapreduce/checkpoint-and-resume.md",
          "topics": ["checkpoints", "resume", "recovery"],
          "feature_mapping": [
            "mapreduce.checkpoint",
            "mapreduce.resume"
          ]
        },
        {
          "id": "dead-letter-queue-dlq",
          "title": "Dead Letter Queue (DLQ)",
          "file": "book/src/mapreduce/dead-letter-queue-dlq.md",
          "topics": ["failed items", "retry", "DLQ"],
          "feature_mapping": [
            "mapreduce.dlq",
            "error_handling.dlq"
          ]
        }
      ]
    }
  ]
}
```

**Benefits**:
- Scoped feature analysis per subsection
- Parallel processing of subsections
- Clear topic boundaries
- Better organization for large chapters

**Migration path**: See [Advanced Configuration](advanced-configuration.md) for converting single-file chapters to multi-subsection format.

### Project Type Examples

Different project types require different documentation focus areas.

#### CLI Tool Configuration

**Source**: `.prodigy/book-config.json:2-3`

```json
{
  "project_type": "cli_tool",
  "analysis_targets": [
    {
      "area": "commands",
      "source_files": ["src/cli/", "src/commands/"],
      "feature_categories": [
        "subcommands",
        "arguments",
        "flags",
        "output_formatting"
      ]
    },
    {
      "area": "configuration",
      "source_files": ["src/config/"],
      "feature_categories": [
        "config_files",
        "precedence",
        "validation"
      ]
    }
  ]
}
```

**CLI-specific focus**:
- Command-line interface documentation
- Configuration file formats
- Usage examples and flags
- Exit codes and error messages

#### Library Configuration

```json
{
  "project_type": "library",
  "analysis_targets": [
    {
      "area": "public_api",
      "source_files": ["src/lib.rs", "src/api/"],
      "feature_categories": [
        "public_functions",
        "types",
        "traits",
        "error_types"
      ]
    },
    {
      "area": "examples",
      "source_files": ["examples/"],
      "feature_categories": [
        "usage_patterns",
        "integration_examples"
      ]
    }
  ]
}
```

**Library-specific focus**:
- Public API documentation
- Usage examples
- Integration patterns
- Error handling for library users

### Environment Variable Customization

Customize workflow behavior through environment variables without modifying YAML.

**Source**: `workflows/book-docs-drift.yml:9-21`

#### Development Environment

```bash
# Development: Verbose output, sequential processing
export PROJECT_NAME="MyProject"
export MAX_PARALLEL="1"
export PRODIGY_CLAUDE_CONSOLE_OUTPUT="true"

prodigy run workflows/book-docs-drift.yml -v
```

#### CI/CD Environment

```bash
# CI/CD: Moderate parallelism, no console output
export PROJECT_NAME="MyProject"
export MAX_PARALLEL="5"
export PRODIGY_CLAUDE_STREAMING="false"  # Disable for CI logs

prodigy run workflows/book-docs-drift.yml
```

#### Production Environment

```bash
# Production: High parallelism, optimized for speed
export PROJECT_NAME="MyProject"
export MAX_PARALLEL="10"
export ANALYSIS_DIR=".prodigy/book-analysis"

prodigy run workflows/book-docs-drift.yml
```

### Custom Claude Commands

You can create project-specific Claude commands for specialized analysis or formatting.

**Example location**: `.claude/commands/` directory

#### Custom Analysis Command

Create `.claude/commands/my-custom-analysis.md`:

```markdown
# /my-custom-analysis

Analyze code for project-specific patterns.

## Variables
- `--file <path>` - File to analyze
- `--pattern <name>` - Pattern to check

## Execute
1. Read the file at ${file}
2. Check for pattern: ${pattern}
3. Generate report
4. Commit findings
```

**Usage in workflow**:
```yaml
map:
  agent_template:
    - claude: "/my-custom-analysis --file '${item.file}' --pattern 'async-await'"
```

**Source reference**: Custom commands structure from `.prodigy/book-config.json:671`

### Quick Customization Checklist

**For a new project, customize**:

1. **book-config.json**:
   - [ ] Set `project_name` and `project_type`
   - [ ] Define `analysis_targets` for your language
   - [ ] Set `book_dir` path
   - [ ] Configure `feature_categories`

2. **chapters.json**:
   - [ ] Define chapter structure (flat or hierarchical)
   - [ ] Set validation rules per chapter
   - [ ] Add feature mappings for subsections

3. **workflow YAML**:
   - [ ] Set `MAX_PARALLEL` for your system
   - [ ] Configure timeouts for your project size
   - [ ] Choose error handling strategy
   - [ ] Add project-specific validation

4. **Environment variables**:
   - [ ] Set `PROJECT_NAME`
   - [ ] Configure `ANALYSIS_DIR`
   - [ ] Set parallelism level

### See Also

- [Advanced Configuration](advanced-configuration.md) - Detailed configuration options
- [Understanding the Workflow](understanding-the-workflow.md) - How the workflow executes
- [Quick Start (30 Minutes)](quick-start-30-minutes.md) - Get started quickly
- [GitHub Actions Integration](github-actions-integration.md) - Automate with CI/CD
- [Best Practices](best-practices.md) - Recommended patterns

### Troubleshooting Customizations

**Issue: Analysis not finding features**
- Verify `source_files` patterns match your project structure
- Check file extensions are correct for your language
- Use glob patterns (`**/*.ext`) for nested directories

**Issue: Workflow too slow**
- Increase `MAX_PARALLEL` gradually (3 → 5 → 10)
- Check system resources (CPU, memory)
- Consider splitting large chapters into subsections

**Issue: Quality validation failing**
- Lower `threshold` from 100 to 80 for initial setup
- Increase `max_attempts` in `on_incomplete`
- Review validation error messages in `.prodigy/validation-result.json`

**Issue: Wrong language patterns detected**
- Ensure `feature_categories` match your language paradigm
- Customize analysis prompts in custom Claude commands
- Use language-specific terminology in topic definitions
