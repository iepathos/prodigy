# Prodigy: A Claude Code Orchestrator for Complex AI Workflows

## Abstract

Claude Code and similar AI coding assistants excel at individual tasks but struggle with complex, multi-step operations that require retries, parallel execution, and workflow orchestration. Prodigy fills this gap by providing a MapReduce-based orchestration layer that enables developers to run AI-assisted workflows that are too complex for a single session.

At its core, Prodigy brings the MapReduce pattern to AI-powered code transformation—enabling you to process hundreds or thousands of files in parallel with isolated Claude agents. Each agent runs in its own git worktree, failed items go to a Dead Letter Queue for retry, and results are automatically aggregated. Combined with retry logic and workflow orchestration, it's the practical tool for when you need to transform entire codebases or chain complex multi-step processes.

---

## 1. Introduction

### 1.1 The Problem

When using Claude Code (or similar AI coding assistants), developers encounter limitations:

1. **No retry logic** - Operations fail and stop with no recovery
2. **No parallel execution** - Processing many files takes forever sequentially  
3. **No MapReduce pattern** - Can't distribute work across multiple agents
4. **No workflow orchestration** - Can't chain operations with conditional logic
5. **Session limitations** - Complex operations timeout or exceed context
6. **No progress tracking** - Long operations provide no visibility into status

These limitations become painful when attempting:
- Large-scale refactoring across hundreds of files
- Complex multi-step migrations requiring validation between steps
- Parallel experimentation with different approaches
- Operations that need isolation to prevent interference

### 1.2 The Solution

Prodigy is a Claude Code orchestrator that enables:

- **MapReduce pattern** for massive parallel processing
- **Parallel execution** in isolated git worktrees
- **Automatic retries** with configurable strategies
- **Dead Letter Queue** for failed item recovery
- **Workflow orchestration** with conditional logic
- **Progress tracking** and resumable workflows

### 1.3 What Prodigy Is Not

To be clear about scope:

- **Not an AI provider** - Uses Claude Code (or other CLI tools)
- **Not a framework** - Simple orchestration tool
- **Not a context manager** - Relies on Claude's context handling
- **Not a learning system** - Each run is independent
- **Not revolutionary** - Practical tool for a specific need

---

## 2. Core Concepts

### 2.1 MapReduce for AI Workflows

Prodigy's killer feature is MapReduce for AI operations. Process hundreds of files or complex JSON data in parallel:

```yaml
mode: mapreduce

# Optional setup phase
setup:
  - shell: "analyze-codebase --output work-items.json"

map:
  # Two input modes:
  # 1. Command output: input: "find . -name '*.py'"
  # 2. JSON file: 
  input: "work-items.json"
  json_path: "$.items[*]"  # Extract items from JSON
  
  agent_template:
    commands:
      - claude: "/process-item '${item}'"
      - validate: "test ${item.path}"
  
  max_parallel: 10  # Concurrent agents
  filter: "item.priority == 'high'"  # Optional filtering
  sort_by: "item.score DESC"  # Process order

reduce:
  commands:
    - claude: "/summarize ${map.results}"
```

This pattern enables:
- **Setup phase**: Prepare data before parallel processing
- **JSON input**: Process structured data from analysis tools
- **Massive parallelism**: Process entire codebases efficiently
- **Smart filtering**: Focus on high-priority items
- **Isolation**: Each item processed in its own git worktree
- **Fault tolerance**: Failed items go to Dead Letter Queue
- **Aggregation**: Reduce phase consolidates results

### 2.2 Tasks

A task is a unit of work that can be retried and validated:

```yaml
tasks:
  - name: "Modernize Python code"
    claude: "/modernize-python user.py"
    retry: 3
    validate: "python -m py_compile user.py"
```

### 2.3 Parallel Execution

Beyond MapReduce, support simple parallel operations:

```yaml
tasks:
  - name: "Update all modules"
    foreach: "find . -name '*.py'"
    parallel: 5  # 5 concurrent worktrees
    do:
      claude: "/add-type-hints ${item}"
```

### 2.4 Workflow Control

Simple conditional logic and flow control:

```yaml
tasks:
  - name: "Refactor"
    claude: "/refactor module.py"
    validate: "pytest tests/"
    
  - name: "Deploy if tests pass"
    when: "${tests.passed}"
    shell: "./deploy.sh"
```

### 2.5 Retry Strategies

Automatic retry with backoff:

```yaml
retry:
  attempts: 3
  backoff: exponential
  on_failure: continue  # or 'stop'
```

### 2.6 Dead Letter Queue (DLQ)

Failed MapReduce items are saved for analysis and retry:

```yaml
mapreduce:
  on_item_failure: dlq  # Save to DLQ for later
  continue_on_failure: true  # Don't stop entire job
```

Later, reprocess failed items:
```bash
prodigy dlq retry workflow-id
```

---

## 3. Language Design

### 3.1 Minimal YAML Syntax

Prodigy uses simple YAML that maps directly to operations:

```yaml
name: workflow-name
parallel_worktrees: 5  # Max concurrent operations

tasks:
  - name: "Task name"
    claude: "/command"     # Claude Code command
    shell: "command"       # Shell command
    retry: 3              # Retry count
    validate: "command"   # Validation command
    when: "${condition}"  # Conditional execution
```

### 3.2 MapReduce Syntax

Prodigy's MapReduce pattern supports both command output and JSON input:

```yaml
mode: mapreduce

# Optional setup phase: Prepare data before map phase
setup:
  - shell: "generate-work-items.sh"
  - shell: "analyze-codebase --output items.json"

# Map phase: Process items in parallel
map:
  # Input can be command output or JSON file
  input: "items.json"  # JSON file with array of items
  json_path: "$.items[*]"  # JSONPath to extract items
  # OR: input: "find . -name '*.py'"  # Command that outputs list
  
  agent_template:
    commands:
      - claude: "/process '${item}'"
      - validate: "test ${item.path}"
  
  max_parallel: 10  # Number of concurrent agents
  filter: "item.score >= 5"  # Optional: filter items
  sort_by: "item.priority DESC"  # Optional: process order
  max_items: 100  # Optional: limit items per run

# Reduce phase: Aggregate results
reduce:
  commands:
    - claude: "/summarize ${map.results}"
    - shell: "echo 'Processed ${map.successful}/${map.total} items'"
```

### 3.3 Iteration Constructs

For simpler parallel processing:

```yaml
tasks:
  - foreach: "find . -name '*.js'"
    parallel: true
    do:
      claude: "/convert-to-typescript ${item}"
```

### 3.4 Variable Reference

Simple variable interpolation:

```yaml
tasks:
  - name: "Build"
    shell: "npm build"
    capture: build_output
    
  - name: "Deploy"  
    when: "${build_output.success}"
    shell: "npm deploy"
```

---

## 4. Execution Model

### 4.1 Worktree Isolation

Each parallel task runs in an isolated git worktree:

1. Create worktree for task
2. Execute operations
3. Merge changes back
4. Clean up worktree

This prevents conflicts and enables true parallelism.

### 4.2 Retry Logic

```
for attempt in 1..max_retries:
    result = execute_task()
    if result.success:
        return result
    if validate_command:
        if execute(validate_command).success:
            return success
    sleep(backoff_time)
return failure
```

### 4.3 Progress Tracking

Prodigy maintains state to enable:
- Resume from failure point
- Progress visualization
- Execution history
- Performance metrics

---

## 5. Real-World Use Cases

### 5.1 MapReduce: Technical Debt Elimination

```yaml
name: debtmap-parallel-elimination
mode: mapreduce

# Setup: Analyze codebase and identify debt
setup:
  - shell: "just coverage-lcov"
  - shell: "debtmap analyze src --output debtmap.json"

map:
  # Read debt items from JSON file
  input: debtmap.json
  json_path: "$.items[*]"
  
  agent_template:
    commands:
      # Each agent gets full JSON context
      - claude: "/fix-debt-item --json '${item}'"
        commit_required: true
      - shell: "just test"
  
  max_parallel: 5
  filter: "unified_score.final_score >= 5"
  sort_by: "unified_score.final_score DESC"
  max_items: 10

reduce:
  commands:
    - shell: "debtmap analyze src --output after.json"
    - claude: "/compare-debt-results before/after"
```

### 5.2 MapReduce: Large-Scale Refactoring

```yaml
name: modernize-python-codebase
mode: mapreduce

map:
  input: "find . -name '*.py' -type f"
  
  agent_template:
    commands:
      - claude: "/add-type-hints ${item}"
      - shell: "mypy --strict ${item}"
      - on_failure:
          claude: "/fix-type-errors ${item}"
  
  max_parallel: 20

reduce:
  commands:
    - shell: "mypy ."
    - claude: "/generate-migration-report ${map.results}"
```

### 5.3 Complex Migration

```yaml
name: migrate-to-async
tasks:
  - name: "Convert callbacks to promises"
    foreach: "grep -l 'callback' *.js"
    do:
      claude: "/convert-to-promises ${item}"
      validate: "npm test -- ${item}"
      
  - name: "Convert promises to async/await"
    foreach: "grep -l '\\.then' *.js"
    do:
      claude: "/convert-to-async ${item}"
      validate: "npm test -- ${item}"
      
  - name: "Update documentation"
    when: "${all_tests.pass}"
    claude: "/update-async-docs"
```

### 5.3 Parallel Experimentation

```yaml
name: find-best-approach
parallel_worktrees: 3

tasks:
  - name: "Approach 1: Caching"
    worktree: "approach-caching"
    do:
      - claude: "/optimize-with-caching"
      - shell: "npm run benchmark > caching.txt"
      
  - name: "Approach 2: Indexing"
    worktree: "approach-indexing"
    do:
      - claude: "/optimize-with-indexing"
      - shell: "npm run benchmark > indexing.txt"
      
  - name: "Compare results"
    shell: "python compare_benchmarks.py"
```

---

## 6. Integration

### 6.1 Claude Code Integration

Prodigy wraps Claude Code CLI commands:

```yaml
claude: "/refactor --style functional"
# Executes: claude-code "/refactor --style functional"
```

### 6.2 Shell Commands

Direct shell execution for validation and setup:

```yaml
shell: "pytest tests/ && mypy src/"
```

### 6.3 Git Integration

Automatic worktree management:
- Creates isolated branches
- Merges successful changes
- Cleans up on completion

---

## 7. Why Use Prodigy?

### 7.1 The MapReduce Advantage

**Process entire codebases in parallel:**
- Traditional: Hours to process 500 files sequentially
- Prodigy MapReduce: Minutes with 20 parallel agents
- Each agent isolated in its own git worktree
- Failed items saved to DLQ for retry
- Automatic result aggregation

### 7.2 When Prodigy Makes Sense

✅ **Perfect for:**
- **Large-scale refactoring** (100-1000s of files)
- **Codebase-wide transformations** via MapReduce
- **Parallel testing and validation**
- **Complex multi-step workflows**
- **Operations needing retries**
- **Parallel experimentation**

❌ **Use Claude Code directly for:**
- Single file operations
- Simple one-off tasks
- Interactive development
- Exploratory coding

### 7.3 Value Proposition

**"MapReduce for AI-powered code transformation"**

Prodigy brings the MapReduce pattern to AI development—process entire codebases in parallel with isolated Claude agents.

**The MapReduce difference:**
- **Without Prodigy**: Process 500 files sequentially, taking hours
- **With Prodigy MapReduce**: Process 500 files with 20 parallel agents in minutes
- **Fault tolerance**: Failed files go to DLQ, don't stop the workflow
- **Isolation**: Each agent works in its own git worktree, no conflicts

---

## 8. Implementation

### 8.1 Architecture

```
prodigy/
├── orchestrator/     # Workflow execution engine
├── worktree/        # Git worktree management
├── retry/           # Retry strategies
├── parallel/        # Parallel execution
└── claude/          # Claude Code wrapper
```

### 8.2 Core Components

1. **Workflow Engine**: Parses YAML, executes tasks
2. **Worktree Manager**: Creates/manages git worktrees
3. **Task Executor**: Runs Claude/shell commands with retries
4. **Progress Tracker**: Maintains state and enables resume

### 8.3 CLI Interface

```bash
# Run workflow
prodigy run workflow.yml

# Resume from failure
prodigy resume workflow.yml

# Run single command with retries
prodigy exec "/refactor user.py" --retry 3

# Parallel batch operation
prodigy batch "*.py" --command "/add-types" --parallel 5
```

---

## 9. Limitations

### 9.1 Current Limitations

- **Local execution only** - No distributed processing
- **Git repositories only** - Requires git for worktrees
- **Claude Code dependency** - Requires Claude Code CLI
- **No learning** - Each run independent
- **Simple workflows** - No complex DAGs or conditions

### 9.2 Future Possibilities

- **Task Decomposition Tool**: A separate AI tool to intelligently break complex tasks into MapReduce-compatible work items
- Support for other AI tools beyond Claude Code
- Distributed execution across machines
- More sophisticated workflow patterns
- Integration with CI/CD systems

### 9.3 Task Decomposition Tool (Planned)

A companion tool to automatically generate MapReduce workflows:

```bash
# Future: Automatically decompose complex task
prodigy decompose "Add error handling to all API endpoints" \
  --analyze-codebase \
  --output workflow.yml
```

Would generate:
```yaml
tasks:
  - name: "Add error handling to API endpoints"
    mapreduce:
      map:
        input: "find . -path '*/api/*.js' -type f"
        parallel: 15
        operation:
          claude: "/add-error-handling ${item}"
          validate: "npm test ${item}"
```

This tool would:
- Analyze codebase structure
- Identify parallelizable work units
- Generate optimal MapReduce configurations
- Suggest validation strategies

---

## 10. Getting Started

### 10.1 Installation

```bash
# Install from cargo
cargo install prodigy

# Or download binary
curl -L https://github.com/user/prodigy/releases/latest/download/prodigy
```

### 10.2 Simple MapReduce Example

```yaml
# refactor.yml
name: simple-refactor
tasks:
  - name: "Improve code quality across codebase"
    mapreduce:
      map:
        input: "find src -name '*.py'"
        parallel: 10  # 10 concurrent Claude agents
        operation:
          claude: "/improve-code ${item}"
          validate: "pylint ${item}"
          retry: 2
      reduce:
        claude: "/generate-refactoring-summary ${map.results}"
```

```bash
prodigy run refactor.yml
```

### 10.3 Monitor Progress

```bash
# Watch progress
prodigy status

# View logs
prodigy logs workflow-id

# Resume failed workflow
prodigy resume workflow-id
```

---

## 11. Conclusion

Prodigy fills a specific gap in the AI-assisted development toolkit: orchestrating complex Claude Code operations that are too large or complex for a single session. The MapReduce pattern is its secret weapon—enabling developers to process entire codebases in parallel with isolated Claude agents.

It's not revolutionary—it's a practical tool born from the real need to run AI operations on hundreds of files with retries and parallelism. By bringing MapReduce to AI-powered code transformation, Prodigy makes previously impractical large-scale refactoring tasks feasible.

---

## Appendix A: Full MapReduce Example

```yaml
# Complete example: Modernize JavaScript codebase using MapReduce
name: modernize-js
description: "Convert old JS patterns to modern syntax across entire codebase"

settings:
  parallel_worktrees: 20  # Support up to 20 concurrent agents
  continue_on_error: true
  
tasks:
  # Phase 1: Syntax modernization with MapReduce
  - name: "Convert var to let/const across all files"
    mapreduce:
      map:
        input: "find . -name '*.js' -type f | grep -v node_modules"
        parallel: 20  # Process 20 files simultaneously
        operation:
          - shell: "cp ${item} ${item}.backup"
          - claude: "/modernize-syntax ${item}"
          - validate: "node --check ${item}"
          - on_failure:
              shell: "mv ${item}.backup ${item}"
      reduce:
        - shell: "echo 'Modernized ${map.success_count} files'"
        - claude: "/summarize-syntax-changes ${map.results}"
        
  # Phase 2: Convert to ES modules with MapReduce
  - name: "Update imports across codebase"
    mapreduce:
      map:
        input: "find . -name '*.js' -type f"
        parallel: 15
        operation:
          claude: "/convert-to-es-modules ${item}"
          validate: "npm test -- ${item}"
          retry: 3
      reduce:
        shell: "npm run build"  # Verify entire project still builds
      
  # Phase 3: Add TypeScript with MapReduce
  - name: "Convert to TypeScript"
    when: "${phase2.success}"
    mapreduce:
      map:
        input: "find . -name '*.js' -type f"
        parallel: 20
        operation:
          - claude: "/convert-to-typescript ${item}"
          - shell: "mv ${item} ${item%.js}.ts"
          - validate: "tsc ${item%.js}.ts --noEmit"
      reduce:
        - shell: "tsc --noEmit"  # Validate entire project
        - claude: "/generate-typescript-migration-guide ${map.results}"
      
  # Phase 4: Final validation and reporting
  - name: "Run full test suite"
    shell: "npm test"
    retry: 2
    
  - name: "Create comprehensive summary"
    claude: "/summarize-all-changes"
    output: "MODERNIZATION_SUMMARY.md"
    
  - name: "Process DLQ if needed"
    when: "${dlq.has_items}"
    shell: "prodigy dlq list modernize-js"
```

---

## Appendix B: Configuration Reference

```yaml
# ~/.prodigy/config.yml
defaults:
  parallel_worktrees: 5
  retry_attempts: 3
  retry_backoff: exponential
  timeout: 300s
  
claude:
  command: "claude-code"  # or "claude" or custom path
  model: "claude-3-sonnet"
  
git:
  worktree_prefix: "prodigy-"
  auto_cleanup: true
  merge_strategy: "merge"  # or "rebase"
  
logging:
  level: "info"
  file: "~/.prodigy/logs/prodigy.log"
```

---

**Project Status**: Alpha  
**License**: MIT  
**Repository**: github.com/iepathos/prodigy  
**Requirements**: Git, Claude Code CLI