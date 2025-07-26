# Spec 10: Smart Project Analyzer

## Objective

Create an intelligent project analyzer that automatically detects language, framework, structure, and health indicators to enable zero-configuration code improvement. This is the foundation for the "it just works" experience.

## Core Capabilities

### Language Detection

```rust
pub enum Language {
    Rust,
    Python,
    JavaScript,
    TypeScript,
    Go,
    Java,
    CSharp,
    Ruby,
    Swift,
    Kotlin,
    Other(String),
}

impl LanguageDetector {
    pub fn detect(path: &Path) -> Result<Language> {
        // Priority-based detection
        // 1. Build files (Cargo.toml, package.json, etc.)
        // 2. File extensions frequency
        // 3. Shebang lines
        // 4. Content patterns
    }
}
```

### Framework Detection

```rust
pub enum Framework {
    // Rust
    Actix, Axum, Rocket, Tauri, Yew,
    
    // JavaScript/TypeScript  
    React, Vue, Angular, Next, Svelte, Express, Nest,
    
    // Python
    Django, Flask, FastAPI, Pytest,
    
    // And more...
}
```

### Project Structure Analysis

```rust
pub struct ProjectStructure {
    pub root: PathBuf,
    pub src_dirs: Vec<PathBuf>,
    pub test_dirs: Vec<PathBuf>,
    pub config_files: Vec<ConfigFile>,
    pub entry_points: Vec<PathBuf>,
    pub important_files: Vec<PathBuf>,
    pub ignored_patterns: Vec<String>,
}

impl StructureAnalyzer {
    pub fn analyze(path: &Path) -> Result<ProjectStructure> {
        // Intelligent detection of:
        // - Source directories (src/, lib/, app/, etc.)
        // - Test directories (tests/, test/, spec/, etc.)
        // - Configuration files
        // - Entry points (main.rs, index.js, app.py, etc.)
        // - Important files (README, LICENSE, etc.)
        // - .gitignore patterns
    }
}
```

### Health Indicators

```rust
pub struct HealthIndicators {
    pub has_tests: bool,
    pub test_coverage: Option<f32>,
    pub has_ci: bool,
    pub has_linting: bool,
    pub has_formatting: bool,
    pub dependencies_updated: bool,
    pub documentation_level: DocLevel,
    pub code_complexity: ComplexityLevel,
    pub last_commit: Option<DateTime<Utc>>,
    pub open_todos: Vec<TodoItem>,
}

pub enum DocLevel {
    None,
    Minimal,
    Good,
    Comprehensive,
}

pub enum ComplexityLevel {
    Simple,
    Moderate,
    Complex,
    VeryComplex,
}
```

## Smart Analysis Features

### 1. Build Tool Detection

```rust
pub struct BuildInfo {
    pub tool: BuildTool,
    pub scripts: HashMap<String, String>,
    pub dependencies: Vec<Dependency>,
    pub dev_dependencies: Vec<Dependency>,
}

pub enum BuildTool {
    Cargo,      // Rust
    Npm,        // JavaScript
    Yarn,       // JavaScript
    Pnpm,       // JavaScript  
    Poetry,     // Python
    Pip,        // Python
    Maven,      // Java
    Gradle,     // Java/Kotlin
    Dotnet,     // C#
    Go,         // Go
}

impl BuildAnalyzer {
    pub fn analyze(path: &Path) -> Result<BuildInfo> {
        match detect_build_tool(path)? {
            BuildTool::Cargo => analyze_cargo_toml(path),
            BuildTool::Npm => analyze_package_json(path),
            // ... etc
        }
    }
}
```

### 2. Code Quality Signals

```rust
pub struct QualitySignals {
    pub avg_function_length: f32,
    pub max_function_length: usize,
    pub avg_file_length: f32,
    pub max_file_length: usize,
    pub duplicate_code_ratio: f32,
    pub comment_ratio: f32,
    pub test_ratio: f32,
    pub error_handling_score: f32,
}

impl QualityAnalyzer {
    pub async fn analyze(structure: &ProjectStructure) -> Result<QualitySignals> {
        // Quick static analysis for signals
        // Not deep analysis - just indicators
    }
}
```

### 3. Focus Area Detection

```rust
pub struct FocusAreas {
    pub primary: Vec<ImprovementArea>,
    pub secondary: Vec<ImprovementArea>,
    pub ignore: Vec<String>,
}

pub enum ImprovementArea {
    ErrorHandling,
    TestCoverage,
    Documentation,
    Performance,
    Security,
    Accessibility,
    CodeOrganization,
    TypeSafety,
    Dependencies,
    Configuration,
}

impl FocusDetector {
    pub fn detect(
        project: &ProjectInfo,
        health: &HealthIndicators,
        quality: &QualitySignals,
    ) -> FocusAreas {
        // Smart detection based on:
        // - Missing tests → TestCoverage
        // - Many unwraps → ErrorHandling  
        // - Low comment ratio → Documentation
        // - Old dependencies → Dependencies
        // - Complex functions → CodeOrganization
    }
}
```

### 4. Context File Generation

```rust
impl ContextGenerator {
    pub fn generate(analyzer_result: &AnalyzerResult) -> String {
        format!(r#"
# Project Analysis

## Overview
- Language: {}
- Framework: {}
- Size: {} files, {} lines
- Health: {}

## Structure
- Source: {}
- Tests: {}
- Entry: {}

## Quality Indicators
- Test Coverage: {}
- Documentation: {}
- Complexity: {}

## Suggested Improvements
{}

## Key Files
{}
"#, /* ... values ... */)
    }
}
```

## Usage Examples

### Basic Analysis

```rust
let analyzer = ProjectAnalyzer::new();
let result = analyzer.analyze(".").await?;

println!("Detected: {} project", result.language);
println!("Framework: {:?}", result.framework);
println!("Health score: {}/10", result.health_score);
```

### Smart Improvement Targeting

```rust
let focus = FocusDetector::detect(&result);

match focus.primary.first() {
    Some(ImprovementArea::TestCoverage) => {
        println!("🎯 Focus: Improving test coverage");
        // Target test generation
    },
    Some(ImprovementArea::ErrorHandling) => {
        println!("🎯 Focus: Better error handling");
        // Target error handling patterns
    },
    _ => {
        println!("🎯 Focus: General improvements");
    }
}
```

## Implementation Strategy

### Phase 1: Core Detection
1. Language detection via file extensions and build files
2. Basic structure mapping (src, tests, configs)
3. Simple health indicators (has tests, has CI)

### Phase 2: Smart Analysis
1. Framework detection
2. Code quality signals
3. Dependency analysis
4. TODO/FIXME extraction

### Phase 3: Intelligence
1. Pattern learning from successful improvements
2. Project-type specific heuristics
3. Team coding style detection

## File Detection Patterns

```rust
const PATTERNS: &[(&str, DetectionRule)] = &[
    // Build files
    ("Cargo.toml", DetectionRule::Language(Language::Rust)),
    ("package.json", DetectionRule::Language(Language::JavaScript)),
    ("requirements.txt", DetectionRule::Language(Language::Python)),
    ("go.mod", DetectionRule::Language(Language::Go)),
    ("pom.xml", DetectionRule::Language(Language::Java)),
    
    // Framework indicators
    ("next.config.js", DetectionRule::Framework(Framework::Next)),
    ("vue.config.js", DetectionRule::Framework(Framework::Vue)),
    (".angular.json", DetectionRule::Framework(Framework::Angular)),
    
    // Test directories
    ("tests/", DetectionRule::TestDir),
    ("test/", DetectionRule::TestDir),
    ("spec/", DetectionRule::TestDir),
    ("__tests__/", DetectionRule::TestDir),
    
    // CI/CD
    (".github/workflows/", DetectionRule::HasCI),
    (".gitlab-ci.yml", DetectionRule::HasCI),
    ("Jenkinsfile", DetectionRule::HasCI),
];
```

## Success Metrics

1. **Accuracy**: 95%+ correct language/framework detection
2. **Speed**: Full analysis < 2 seconds for average project
3. **Coverage**: Supports top 10 languages/frameworks
4. **Intelligence**: Correctly identifies improvement areas 80%+ of time

## Example Output

```json
{
  "language": "rust",
  "framework": "axum",
  "size": {
    "files": 127,
    "lines": 12847,
    "test_files": 23,
    "test_lines": 3421
  },
  "health": {
    "score": 7.2,
    "has_tests": true,
    "test_coverage": 68.5,
    "has_ci": true,
    "has_linting": true,
    "documentation_level": "minimal"
  },
  "focus_areas": [
    "error_handling",
    "documentation",
    "test_coverage"
  ],
  "key_files": [
    "src/main.rs",
    "src/lib.rs",
    "src/api/mod.rs",
    "Cargo.toml",
    "README.md"
  ]
}
```