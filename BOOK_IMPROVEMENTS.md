# Book Documentation Improvements

Based on analysis of README.md, the following topics need to be added to the book to make the README more concise while maintaining comprehensive documentation.

## 1. Add Retry Configuration Section

**Location**: `book/src/error-handling.md` (expand existing chapter)

**Missing Content**:
- Workflow-level `retry_defaults` configuration
- Per-command retry with backoff strategies:
  - Exponential backoff with `initial_delay`, `max_delay`, `jitter`
  - Fibonacci backoff
  - Linear backoff
  - Fixed delay
- `retry_budget` for limiting total retry time
- `retry_on` filters for conditional retry (e.g., `[network, timeout]`)

**Source Files to Document**:
- `src/config/command.rs` - Retry configuration structures
- `src/cook/workflow/retry.rs` - Retry logic implementation

**Example from README to migrate**:
```yaml
retry_defaults:
  attempts: 3
  backoff: exponential
  initial_delay: 2s
  max_delay: 30s
  jitter: true

steps:
  - shell: "deploy.sh"
    retry:
      attempts: 5
      backoff:
        fibonacci:
          initial: 1s
      retry_on: [network, timeout]
      retry_budget: 5m
```

---

## 2. Add Workflow Composition Chapter

**Location**: `book/src/composition.md` (NEW CHAPTER)

**Missing Content**:
- Workflow imports with `path` and `alias`
- Template definitions with parameters
- Template usage with `use` and `with`
- Workflow extension with `extends`
- Parameter type validation

**Source Files to Document**:
- `src/config/workflow.rs` - Import and template structures
- Look for template parsing and composition logic

**Example from README to migrate**:
```yaml
imports:
  - path: ./common/base.yml
    alias: base

templates:
  test-suite:
    parameters:
      - name: language
        type: string
    steps:
      - shell: "${language} test"

workflows:
  main:
    extends: base.default
    steps:
      - use: test-suite
        with:
          language: cargo
```

**Update needed**:
- Add to `.prodigy/book-config.json` analysis_targets
- Add to `workflows/data/prodigy-chapters.json`
- Add to `book/src/SUMMARY.md`

---

## 3. Expand Git Context Variables in Variables Chapter

**Location**: `book/src/variables.md` (expand existing section)

**Missing Content**:
- Pattern filtering syntax: `${step.files_added:*.md}`
- Format modifiers:
  - `:json` - JSON array format
  - `:lines` - Newline-separated
  - `:csv` - Comma-separated
  - `:space` - Space-separated (default)
- Combined pattern + format: `${step.files_modified:*.rs:json}`
- Use cases and examples

**Source Files to Document**:
- `src/cook/workflow/variables.rs` - Variable interpolation with filters
- `src/cook/workflow/git_context.rs` - Git tracking implementation

**Examples from README to migrate**:
```yaml
# Get only markdown files added
- shell: "echo '${step.files_added:*.md}'"

# Get only Rust source files modified in JSON format
- claude: "/review ${step.files_modified:*.rs:json}"

# Get specific directory changes
- shell: "echo '${workflow.files_changed:src/*}'"
```

---

## 4. Add Configuration Chapter

**Location**: `book/src/configuration.md` (NEW CHAPTER)

**Missing Content**:
- Configuration file locations and precedence:
  1. `.prodigy/config.yml` - Project-specific
  2. `~/.config/prodigy/config.yml` - User configuration
  3. `/etc/prodigy/config.yml` - System-wide
- Configuration structure:
  - `claude` settings (model, max_tokens)
  - `worktree` settings (max_parallel, cleanup_policy)
  - `retry` defaults
  - `storage` paths (events_dir, state_dir)
- Example configurations for different use cases

**Source Files to Document**:
- `src/config/mod.rs` - Configuration loading
- Look for config precedence logic

**Example from README to migrate**:
```yaml
# .prodigy/config.yml
claude:
  model: claude-3-opus
  max_tokens: 4096

worktree:
  max_parallel: 20
  cleanup_policy:
    idle_timeout: 300
    max_age: 3600

retry:
  default_attempts: 3
  default_backoff: exponential

storage:
  events_dir: ~/.prodigy/events
  state_dir: ~/.prodigy/state
```

**Update needed**:
- Add to `.prodigy/book-config.json` analysis_targets
- Add to `workflows/data/prodigy-chapters.json`
- Add to `book/src/SUMMARY.md`

---

## 5. Expand Examples Chapter

**Location**: `book/src/examples.md` (expand existing)

**Missing Examples from README**:

### Example: Automated Testing Pipeline
- Multi-stage testing with intelligent retry
- Format checking with auto-fix
- Linting with Claude-powered fixes

### Example: Parallel Code Analysis
- Finding complex files
- Concurrent analysis with max_parallel
- Result aggregation
- Statistical reporting

### Example: Performance Optimization
- Goal-seeking for performance targets
- Benchmark validation with threshold
- Iterative optimization
- Documentation of improvements

**Source**: README.md lines 399-471

---

## 6. Update book-config.json

Add these new analysis targets:

```json
{
  "area": "retry_configuration",
  "source_files": [
    "src/config/command.rs",
    "src/cook/workflow/retry.rs"
  ],
  "feature_categories": [
    "retry_defaults",
    "backoff_strategies",
    "retry_budget",
    "conditional_retry"
  ]
},
{
  "area": "workflow_composition",
  "source_files": [
    "src/config/workflow.rs"
  ],
  "feature_categories": [
    "imports",
    "templates",
    "extends",
    "parameters"
  ]
},
{
  "area": "configuration",
  "source_files": [
    "src/config/mod.rs"
  ],
  "feature_categories": [
    "file_locations",
    "precedence",
    "claude_settings",
    "worktree_settings",
    "storage_settings"
  ]
}
```

---

## 7. Update prodigy-chapters.json

Add new chapters:

```json
{
  "id": "retry-configuration",
  "title": "Retry Configuration",
  "file": "book/src/retry-configuration.md",
  "topics": ["Retry defaults", "Backoff strategies", "Retry budget", "Conditional retry"],
  "validation": "Check retry configuration options match implementation"
},
{
  "id": "composition",
  "title": "Workflow Composition",
  "file": "book/src/composition.md",
  "topics": ["Imports", "Templates", "Extension", "Parameters"],
  "validation": "Check composition features and syntax"
},
{
  "id": "configuration",
  "title": "Configuration",
  "file": "book/src/configuration.md",
  "topics": ["Config files", "Precedence", "Settings"],
  "validation": "Check configuration structure and options"
}
```

---

## 8. Update SUMMARY.md

Add to the User Guide section:

```markdown
# User Guide

- [Workflow Basics](workflow-basics.md)
- [MapReduce Workflows](mapreduce.md)
- [Command Types](commands.md)
- [Variables and Interpolation](variables.md)
- [Environment Configuration](environment.md)
- [Configuration](configuration.md)  <!-- NEW -->

# Advanced Topics

- [Advanced Features](advanced.md)
- [Workflow Composition](composition.md)  <!-- NEW -->
- [Retry Configuration](retry-configuration.md)  <!-- NEW -->
- [Error Handling](error-handling.md)
- [Automated Documentation](automated-documentation.md)
```

---

## Implementation Plan

1. ✅ Create this analysis document
2. ⏳ Update `.prodigy/book-config.json` with new analysis targets
3. ⏳ Update `workflows/data/prodigy-chapters.json` with new chapters
4. ⏳ Update `book/src/SUMMARY.md` with new chapter links
5. ⏳ Run book-docs-drift workflow to auto-generate initial content
6. ⏳ Review and refine generated content
7. ⏳ Simplify README.md by removing duplicated content and pointing to book

---

## After Book is Complete

Once all content is migrated to the book, the README can be simplified to:

**Keep in README**:
- Tagline and badges
- Features (high-level bullets)
- Installation instructions
- ONE simple Quick Start example
- Basic Commands reference
- Link to full documentation (prominently placed)
- Troubleshooting (condensed common issues)
- Contributing section
- License

**Remove from README** (now in book):
- Advanced Workflows section (→ book)
- Git Context Variables detailed reference (→ book/variables.md)
- Workflow Syntax validation examples (→ book/advanced.md)
- Configuration structure (→ book/configuration.md)
- Examples 2 and 3 (→ book/examples.md, keep one simple example)
- Additional Resources section (redundant with Documentation)
- Quick Reference table (duplicates Basic Commands)

**Result**: README goes from ~700 lines to ~250 lines, staying focused on getting users started quickly while the book provides comprehensive reference.
