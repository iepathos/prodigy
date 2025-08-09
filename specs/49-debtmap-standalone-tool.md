---
number: 49
title: Debtmap - Standalone Code Complexity and Technical Debt Analyzer
category: parallel
priority: high
status: draft
dependencies: []
created: 2025-08-09
---

# Specification 49: Debtmap - Standalone Code Complexity and Technical Debt Analyzer

**Category**: parallel
**Priority**: high
**Status**: draft
**Dependencies**: None

## Context

MMM currently includes comprehensive code analysis capabilities including complexity analysis, technical debt detection, and dependency mapping. However, these features are tightly coupled with MMM's coverage analysis which relies on cargo-tarpaulin, making them less portable across different languages and project types.

There is a need for a standalone tool that can perform complexity analysis and technical debt mapping independently, supporting multiple programming languages (initially Rust and Python) without requiring language-specific testing tools. This tool would extract the core analysis logic from MMM into a reusable, language-agnostic command-line utility.

The existing MMM analysis provides valuable metrics like:
- Cyclomatic and cognitive complexity measurements
- Complexity hotspot identification 
- Technical debt item detection and prioritization
- Code duplication analysis
- Dependency graph generation
- Architecture pattern detection

## Objective

Create a standalone Rust-based command-line tool called `debtmap` that provides language-agnostic code complexity analysis and technical debt mapping for Rust and Python projects, extracting and enhancing the analysis capabilities currently embedded in MMM while removing dependencies on coverage tools.

## Requirements

### Functional Requirements
- Support complexity analysis for Rust and Python source files
- Calculate cyclomatic and cognitive complexity metrics per function/method
- Identify complexity hotspots and rank them by severity
- Detect technical debt indicators (TODOs, FIXMEs, code smells)
- Find and map code duplication across the codebase
- Generate dependency graphs showing module relationships
- Detect circular dependencies and architectural violations
- Output results in multiple formats (JSON, YAML, Markdown, Terminal)
- Support incremental analysis for large codebases
- Provide configurable thresholds for complexity and debt scoring

### Non-Functional Requirements
- Cross-platform compatibility (Linux, macOS, Windows)
- Fast analysis performance (< 1 second per 1000 LOC)
- Low memory footprint for large codebases
- Single binary distribution without external dependencies
- Extensible architecture for adding new languages
- Integration-friendly output formats for CI/CD pipelines

## Acceptance Criteria

- [ ] Standalone binary can be installed and run without MMM
- [ ] Analyzes Rust projects using syn parser for AST analysis
- [ ] Analyzes Python projects using appropriate Python AST parser
- [ ] Generates complexity metrics matching MMM's current output format
- [ ] Identifies at least 5 types of technical debt (TODOs, complexity, duplication, etc.)
- [ ] Produces JSON output compatible with MMM's analysis format
- [ ] Supports filtering and threshold configuration via CLI flags
- [ ] Includes comprehensive --help documentation
- [ ] Binary size under 10MB for release builds
- [ ] Processes the MMM codebase (50k+ lines) in under 5 seconds

## Technical Details

### Implementation Approach

1. **Core Architecture**
   ```rust
   // Main modules structure
   src/
   ├── main.rs                 // CLI entry point
   ├── cli.rs                  // Command-line argument parsing
   ├── analyzer/
   │   ├── mod.rs             // Analyzer trait and common logic
   │   ├── rust.rs            // Rust-specific analysis using syn
   │   └── python.rs          // Python analysis implementation
   ├── complexity/
   │   ├── mod.rs             // Complexity calculation traits
   │   ├── cyclomatic.rs      // Cyclomatic complexity
   │   └── cognitive.rs       // Cognitive complexity
   ├── debt/
   │   ├── mod.rs             // Debt detection framework
   │   ├── patterns.rs        // Pattern-based debt detection
   │   └── duplication.rs     // Code duplication finder
   ├── output/
   │   ├── mod.rs             // Output formatting
   │   ├── json.rs            // JSON serialization
   │   ├── markdown.rs        // Markdown reports
   │   └── terminal.rs        // Terminal display
   └── utils/
       ├── cache.rs           // Analysis caching
       └── walker.rs          // File system traversal
   ```

2. **Language Support Architecture**
   - Abstract `Analyzer` trait for language implementations
   - Language detection based on file extensions
   - Pluggable parser backends (syn for Rust, python-parser for Python)
   - Common metrics calculation across languages

3. **Analysis Pipeline**
   ```rust
   pub trait Analyzer {
       fn analyze_file(&self, path: &Path) -> Result<FileMetrics>;
       fn detect_language(path: &Path) -> Option<Language>;
   }
   
   pub struct FileMetrics {
       pub complexity: ComplexityMetrics,
       pub debt_items: Vec<DebtItem>,
       pub dependencies: Vec<Dependency>,
       pub duplications: Vec<DuplicationBlock>,
   }
   ```

### Data Structures

```rust
// Core data structures matching MMM's format
pub struct AnalysisResults {
    pub project_path: PathBuf,
    pub timestamp: DateTime<Utc>,
    pub complexity: ComplexityReport,
    pub technical_debt: TechnicalDebtReport,
    pub dependencies: DependencyReport,
    pub summary: AnalysisSummary,
}

pub struct ComplexityReport {
    pub total_lines: usize,
    pub max_nesting_depth: u32,
    pub average_cyclomatic: f32,
    pub average_cognitive: f32,
    pub hotspots: Vec<ComplexityHotspot>,
}

pub struct ComplexityHotspot {
    pub file: String,
    pub function: String,
    pub cyclomatic: u32,
    pub cognitive: u32,
    pub line_number: u32,
}

pub struct TechnicalDebtReport {
    pub total_items: usize,
    pub items_by_type: HashMap<DebtType, Vec<DebtItem>>,
    pub complexity_hotspots: usize,
    pub duplication_areas: usize,
    pub priority_items: Vec<DebtItem>,
}
```

### APIs and Interfaces

**CLI Interface:**
```bash
# Basic usage
debtmap analyze <path>

# With options
debtmap analyze <path> \
  --format json \
  --output report.json \
  --threshold-complexity 10 \
  --threshold-duplication 50 \
  --languages rust,python

# Specific analysis types
debtmap complexity <path>    # Only complexity analysis
debtmap debt <path>          # Only debt detection
debtmap deps <path>          # Only dependency analysis

# Configuration
debtmap init                 # Create .debtmap.toml config
debtmap validate             # Validate against thresholds
```

**Library Interface:**
```rust
// Can be used as a library
use debtmap::{analyze, Config, Language};

let config = Config::builder()
    .languages(vec![Language::Rust, Language::Python])
    .complexity_threshold(10)
    .build();

let results = analyze("./src", config)?;
```

## Dependencies

- **Prerequisites**: None (standalone tool)
- **Build Dependencies**:
  - syn - Rust AST parsing
  - rustpython-parser - Python AST parsing  
  - clap - CLI argument parsing
  - serde/serde_json - Serialization
  - walkdir - Directory traversal
  - rayon - Parallel processing
  - sha2 - Content hashing for duplication
- **Runtime Dependencies**: None (static binary)

## Testing Strategy

- **Unit Tests**: 
  - Test complexity calculations with known examples
  - Verify debt detection patterns
  - Test duplication detection algorithm
  - Validate output formatting
- **Integration Tests**:
  - Test against real Rust projects (including MMM itself)
  - Test against sample Python projects
  - Verify incremental analysis correctness
  - Test large codebase performance
- **Comparison Tests**:
  - Compare output with MMM's current analysis
  - Validate metric calculations match expected values
- **Performance Tests**:
  - Benchmark analysis speed on various codebase sizes
  - Memory usage profiling for large projects

## Documentation Requirements

- **Code Documentation**:
  - Comprehensive rustdoc for all public APIs
  - Examples in documentation
  - Architecture decision records
- **User Documentation**:
  - README with installation instructions
  - User guide with examples
  - Configuration file documentation
  - Integration guide for CI/CD
- **Migration Guide**:
  - How to migrate from MMM analysis to debtmap
  - Output format compatibility notes

## Implementation Notes

- Start by extracting relevant code from MMM's analysis modules
- Focus on Rust support first, then add Python
- Use parallel processing with rayon for performance
- Implement incremental analysis using file modification times
- Consider using tree-sitter for more language support in the future
- Cache analysis results to speed up repeated runs
- Use content hashing for reliable duplication detection
- Ensure output format compatibility with MMM for easy migration

## Migration and Compatibility

- Output format should be compatible with MMM's existing analysis.json
- Provide a migration path for MMM to use debtmap as an external tool
- Support MMM's context directory structure for seamless integration
- Consider making debtmap a drop-in replacement for MMM's analysis module
- Maintain backward compatibility with existing MMM workflows

## Future Enhancements

- Support for additional languages (JavaScript, TypeScript, Go)
- IDE integrations and plugins
- Web-based visualization dashboard
- Historical tracking and trend analysis
- Integration with issue tracking systems
- Custom rule definitions for debt detection
- Machine learning-based code smell detection