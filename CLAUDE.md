# MMM Context Documentation for Claude

This document explains how MMM (memento-mori-management) stores and provides context information to Claude during development iterations. Understanding this structure helps Claude locate and utilize relevant project information effectively.

## Important: Context Optimization (v0.1.0+)

As of version 0.1.0, MMM's context generation has been optimized to reduce file sizes by over 90%:
- Technical debt files reduced from 8.2MB to under 500KB
- Analysis files reduced from 8.9MB to under 500KB  
- Total context size kept under 1MB for typical projects
- Maximal duplicate detection replaces inefficient sliding windows
- Smart aggregation limits items per category while preserving high-impact issues
- Hybrid coverage tracking combines test coverage with quality metrics for better prioritization

## Overview

MMM maintains rich project context in the `.mmm/` directory, providing structured data about code quality, architecture, metrics, and development history. This context is automatically made available to Claude during `mmm cook` operations.

## Directory Structure

```
.mmm/
├── context/                    # Project analysis data
│   ├── analysis.json          # Complete analysis results
│   ├── dependency_graph.json  # Module dependencies & cycles
│   ├── architecture.json      # Architecture patterns & violations
│   ├── conventions.json       # Code conventions & naming patterns
│   ├── technical_debt.json    # Debt items & complexity hotspots (optimized)
│   ├── test_coverage.json     # Test coverage data
│   ├── hybrid_coverage.json   # Hybrid coverage with quality metrics
│   └── analysis_metadata.json # Analysis timestamps & stats
├── metrics/                    # Performance & quality metrics
│   ├── current.json           # Latest metrics snapshot
│   ├── history.json           # Historical metrics data
│   └── reports/               # Generated metric reports
│       └── report-{id}.txt    # Individual iteration reports
├── state.json                 # Current session state
├── history/                   # Session history
│   └── {timestamp}_{id}.json  # Individual session records
├── cache/                     # Cached computations
└── workflow.yml               # Custom workflow configuration (optional)
```

## Context Integration

### Environment Variables

When Claude commands are executed, MMM sets these environment variables:

- `MMM_CONTEXT_AVAILABLE="true"` - Indicates context data is ready
- `MMM_CONTEXT_DIR="/path/to/.mmm/context"` - Path to analysis data
- `MMM_FOCUS="performance"` - Current improvement focus (if set)
- `MMM_AUTOMATION="true"` - Signals automated execution mode

### Command Integration Points

Context is provided to Claude during these commands:
- `/mmm-code-review` - Uses full project analysis for issue identification
- `/mmm-implement-spec` - Uses architecture & conventions for implementation
- `/mmm-lint` - Uses conventions & quality metrics for cleanup

## File Formats & Contents

### 1. Context Analysis Files

#### `context/analysis.json`
Complete project analysis combining all components:
```json
{
  "dependency_graph": { /* Module relationships */ },
  "architecture": { /* Patterns & violations */ },
  "conventions": { /* Code standards */ },
  "technical_debt": { /* Debt items */ },
  "test_coverage": { /* Coverage data */ },
  "metadata": {
    "timestamp": "2024-01-01T12:00:00Z",
    "duration_ms": 1500,
    "files_analyzed": 127,
    "incremental": false,
    "version": "0.1.0"
  }
}
```

#### `context/dependency_graph.json`
Module dependency analysis:
```json
{
  "nodes": ["src/main.rs", "src/lib.rs", /* ... */],
  "edges": [
    {"from": "src/main.rs", "to": "src/lib.rs", "import_type": "use"}
  ],
  "cycles": [
    ["module_a", "module_b", "module_a"]  // Circular dependencies
  ],
  "coupling_scores": {
    "src/lib.rs": 8  // Number of dependencies
  }
}
```

#### `context/architecture.json`
Architectural patterns and violations:
```json
{
  "patterns": ["MVC", "Repository", "Builder"],
  "layers": [
    {
      "name": "presentation",
      "modules": ["src/ui/", "src/handlers/"],
      "dependencies": ["business"]
    }
  ],
  "components": {
    "UserService": {
      "responsibility": "User management operations",
      "interfaces": ["UserRepository", "AuthService"],
      "dependencies": ["Database", "Cache"]
    }
  },
  "violations": [
    {
      "rule": "Layered Architecture",
      "location": "src/ui/mod.rs:45",
      "severity": "High",
      "description": "UI layer directly accessing database"
    }
  ]
}
```

#### `context/conventions.json`
Code conventions and style patterns:
```json
{
  "naming_patterns": {
    "file_naming": "snake_case",
    "function_naming": "snake_case",
    "type_naming": "PascalCase",
    "constant_naming": "SCREAMING_SNAKE_CASE"
  },
  "code_patterns": [
    "Result<T, Error> for error handling",
    "Builder pattern for complex constructors",
    "newtype pattern for type safety"
  ],
  "project_idioms": [
    "Use ? operator for error propagation",
    "Prefer &str over String in function parameters",
    "Use #[derive] for common traits"
  ],
  "violations": {
    "src/models.rs": [
      "Function 'getData' should be 'get_data'",
      "Type 'userInfo' should be 'UserInfo'"
    ]
  }
}
```

#### `context/technical_debt.json`
Technical debt analysis:
```json
{
  "debt_items": [
    {
      "title": "High cyclomatic complexity in main.rs",
      "description": "parse_args function has complexity of 15, should be < 10",
      "debt_type": "Complexity",
      "location": "src/main.rs",
      "impact": 7,
      "effort": 4,
      "tags": ["complexity", "refactoring"]
    }
  ],
  "hotspots": [
    {
      "file": "src/parser.rs",
      "complexity_score": 23,
      "change_frequency": 12,
      "risk_level": "High"
    }
  ],
  "duplication_map": {
    "hash_1234abcd": [
      {
        "file": "src/utils.rs", 
        "start_line": 45,
        "end_line": 60,
        "content_hash": "hash_1234abcd"
      },
      {
        "file": "src/helpers.rs",
        "start_line": 23, 
        "end_line": 38,
        "content_hash": "hash_1234abcd"
      }
    ]
  }
}
```

#### `context/test_coverage.json`
Test coverage information:
```json
{
  "overall_coverage": 0.73,
  "file_coverage": {
    "src/lib.rs": {
      "coverage_percentage": 0.85,
      "lines_covered": 120,
      "lines_total": 141
    }
  },
  "untested_functions": [
    "src/utils.rs:handle_error",
    "src/db.rs:migrate_schema"
  ],
  "critical_gaps": [
    {
      "file": "src/auth.rs",
      "functions": ["validate_token", "refresh_session"],
      "risk": "High"
    }
  ]
}
```

#### `context/hybrid_coverage.json`
Hybrid coverage information combining test coverage with quality metrics:
```json
{
  "coverage_map": { /* Standard test coverage data */ },
  "priority_gaps": [
    {
      "gap": {
        "file": "src/critical.rs",
        "functions": ["process_payment"],
        "coverage_percentage": 20.0,
        "risk": "High"
      },
      "quality_metrics": {
        "file": "src/critical.rs",
        "complexity_trend": "Degrading",
        "lint_warnings_trend": "Stable",
        "duplication_trend": "Improving",
        "recent_changes": 15,
        "bug_frequency": 0.8
      },
      "priority_score": 25.0,
      "priority_reason": "Low coverage with increasing complexity, frequent changes"
    }
  ],
  "quality_correlation": {
    "positive_correlations": [],
    "negative_correlations": [],
    "correlation_coefficient": 0.65
  },
  "critical_files": [
    {
      "file": "src/payment.rs",
      "coverage_percentage": 15.0,
      "complexity": 20,
      "lint_warnings": 8,
      "recent_bugs": 5,
      "risk_score": 8.5
    }
  ],
  "hybrid_score": 65.0
}
```

### 2. Metrics Files

#### `metrics/current.json`
Latest performance and quality metrics:
```json
{
  "test_coverage": 73.5,
  "type_coverage": 89.2,
  "lint_warnings": 12,
  "code_duplication": 8.3,
  "doc_coverage": 67.1,
  "benchmark_results": {
    "parse_file": "15.2ms",
    "process_data": "43.1ms"
  },
  "compile_time": "12.5s",
  "binary_size": 4194304,
  "cyclomatic_complexity": {
    "main": 8,
    "parser": 15,
    "utils": 6
  },
  "max_nesting_depth": 4,
  "total_lines": 2847,
  "tech_debt_score": 7.2,
  "improvement_velocity": 1.3,
  "timestamp": "2024-01-01T12:00:00Z",
  "iteration_id": "iteration-1704110400"
}
```

#### `metrics/history.json`
Historical metrics for trend analysis:
```json
{
  "snapshots": [
    {
      "metrics": { /* metrics object */ },
      "iteration": 1,
      "commit_sha": "abc123",
      "timestamp": "2024-01-01T12:00:00Z"
    }
  ],
  "trends": {
    "coverage_trend": "Improving(5.2)",
    "complexity_trend": "Stable", 
    "performance_trend": "Degrading(2.1)",
    "quality_trend": "Improving(8.7)"
  }
}
```

### 3. State Management Files

#### `state.json`
Current project state:
```json
{
  "version": "1.0",
  "project_id": "mmm-abc123",
  "last_run": "2024-01-01T12:00:00Z",
  "total_runs": 15
}
```

#### `history/{timestamp}_{id}.json`
Individual session records:
```json
{
  "session_id": "session-abc123",
  "started_at": "2024-01-01T12:00:00Z",
  "completed_at": "2024-01-01T12:30:00Z",
  "iterations": 3,
  "files_changed": 7,
  "summary": "Fixed 5 clippy warnings, improved test coverage to 75%"
}
```

### 4. Worktree State (when using --worktree)

Worktree sessions maintain additional state for isolation:
```json
{
  "session_id": "wt-abc123",
  "worktree_name": "mmm-performance-1704110400",
  "branch": "mmm/performance-improvements",
  "created_at": "2024-01-01T12:00:00Z",
  "status": "in_progress",
  "focus": "performance",
  "iterations": {"completed": 2, "max": 5},
  "stats": {"files_changed": 4, "commits": 6},
  "last_checkpoint": {
    "iteration": 2,
    "last_command": "/mmm-implement-spec",
    "last_spec_id": "iteration-1704110400-improvements",
    "files_modified": ["src/main.rs", "src/utils.rs"]
  },
  "resumable": true
}
```

## Usage Patterns for Claude

### 1. Accessing Context Data

When executing MMM commands, Claude can read context via environment variables:

```bash
# Check if context is available
if [ "$MMM_CONTEXT_AVAILABLE" = "true" ]; then
  # Read analysis data
  CONTEXT_DIR="$MMM_CONTEXT_DIR"
  analysis=$(cat "$CONTEXT_DIR/analysis.json")
  
  # Parse specific components
  debt=$(cat "$CONTEXT_DIR/technical_debt.json")
  coverage=$(cat "$CONTEXT_DIR/test_coverage.json")
fi
```

### 2. Focus-Driven Analysis

Use the `MMM_FOCUS` environment variable to tailor analysis:

```python
focus = os.environ.get('MMM_FOCUS')
if focus == 'performance':
    # Prioritize performance-related debt items
    # Focus on benchmark results and compilation metrics
elif focus == 'security':
    # Look for security-related violations
    # Examine authentication and validation patterns
```

### 3. Incremental Context Understanding

Context files are updated after each analysis, so Claude can:

1. **Compare States**: Check `analysis_metadata.json` timestamp to determine if context is stale
2. **Track Progress**: Use metrics history to understand improvement trends  
3. **Resume Work**: Check for interrupted sessions via worktree state
4. **Avoid Rework**: Review recent changes in session history

### 4. Targeted Improvements

Use context to make informed decisions:

- **High-Impact Issues**: Focus on debt items with high impact scores
- **Coverage Gaps**: Prioritize untested critical functions
- **Architecture Violations**: Address high-severity architectural issues
- **Performance Regressions**: Track metrics trends to identify degradation

## Context Refresh

Context is automatically refreshed:
- Before each `mmm cook` session starts
- When files change (incremental updates)
- After each iteration completes (metrics only)

Context age can be checked via `analysis_metadata.json` timestamp. Context older than 1 hour is automatically regenerated.

## Configuration

### Custom Workflows

Place workflow configuration in `.mmm/workflow.yml`:
```yaml
name: "Custom Analysis Workflow"
steps:
  - name: "Security Review"
    command: "/security-analysis"
    focus: "security"
  - name: "Performance Check" 
    command: "/perf-analysis"
    focus: "performance"
```

### Cache Management

MMM uses `.mmm/cache/` for expensive computations:
- Dependency analysis results
- Complexity calculations  
- Coverage report parsing

Cache is invalidated when source files change.

## Best Practices

1. **Always Check Context**: Verify `MMM_CONTEXT_AVAILABLE` before assuming context exists
2. **Use Specific Files**: Read targeted context files rather than the complete analysis
3. **Respect Focus**: Honor the `MMM_FOCUS` environment variable when prioritizing issues
4. **Track Progress**: Use metrics history to validate improvements
5. **Handle Missing Data**: Gracefully handle missing or incomplete context files

## Troubleshooting

### Context Not Available
- Check if `.mmm/context/` directory exists
- Verify `analysis_metadata.json` has recent timestamp
- Run `mmm analyze context` to regenerate

### Stale Context  
- Context older than 1 hour is automatically refreshed
- Force refresh with `mmm analyze context --save`

### Missing Metrics
- Requires Rust project with Cargo.toml
- Some metrics need external tools (cargo-tarpaulin, cargo-bench)
- Enable metrics with `mmm cook --metrics`

This context system enables Claude to make informed, project-specific improvements rather than generic suggestions, leading to more effective code enhancement iterations.