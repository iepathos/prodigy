# Spec 09: Dead Simple Improve Command

## Objective

Implement a single `mmm improve` command that automatically improves code quality with zero configuration required. This is the foundation for a radically simplified MMM that "just works" out of the box.

## User Experience

```bash
# The entire user flow
cd any-project
mmm improve

# Output
🚀 MMM starting code improvement...
📊 Analyzing project... 
  ✓ Detected: Rust project (12,847 lines)
  ✓ Found: 3 test files, 2 config files
  ✓ Focus areas: error handling, test coverage, documentation

🔍 Reviewing code quality...
  Current score: 6.8/10
  Issues found: 23 (4 high, 12 medium, 7 low)

🔧 Applying improvements...
  ✓ Fixed error handling in src/main.rs
  ✓ Added missing tests for auth module  
  ✓ Improved documentation for public APIs
  ✓ Refactored complex function in parser.rs

✅ Improvement complete!
  New score: 8.2/10 (+1.4)
  Files changed: 8
  Tests added: 12
  Coverage: 67% → 78%

💡 Run 'mmm improve' again for further improvements
```

## Core Principles

1. **Zero Configuration**: Works immediately after installation
2. **Smart Defaults**: Makes intelligent decisions without asking
3. **Clear Progress**: User always knows what's happening
4. **Safe Operation**: Never breaks working code
5. **Incremental**: Each run makes code better

## Technical Design

### Command Structure

```rust
// Single command, optional flags
mmm improve [OPTIONS]

Options:
  --focus <AREA>     Focus on specific area (e.g., tests, errors, perf)
  --target <SCORE>   Target quality score (default: 8.0)
  --auto-commit      Automatically commit improvements
  --dry-run          Show what would be improved without changes
  --verbose          Show detailed progress
```

### Core Flow

```rust
pub async fn improve(opts: ImproveOptions) -> Result<()> {
    // 1. Analyze project
    let project = ProjectAnalyzer::analyze(".")?;
    show_progress("Analyzing project", &project.summary());
    
    // 2. Build smart context
    let context = ContextBuilder::build(&project)?;
    
    // 3. Run improvement loop
    let mut session = ImproveSession::new(project, context);
    
    while !session.is_good_enough() {
        // Review current state
        let review = claude_review(&session).await?;
        show_progress("Reviewing code", &review.summary());
        
        // Apply improvements
        let changes = claude_improve(&session, &review).await?;
        show_progress("Applying improvements", &changes.summary());
        
        // Validate changes
        let valid = validate_changes(&changes)?;
        if !valid {
            rollback_changes(&changes)?;
            break;
        }
        
        session.update(changes);
    }
    
    // 4. Show results
    show_results(&session);
    Ok(())
}
```

### Project Analysis

```rust
struct ProjectInfo {
    language: Language,
    framework: Option<Framework>,
    size: ProjectSize,
    test_coverage: Option<f32>,
    structure: ProjectStructure,
    health_indicators: HealthIndicators,
}

impl ProjectAnalyzer {
    fn analyze(path: &str) -> Result<ProjectInfo> {
        // Auto-detect everything
        let language = detect_language(path)?;
        let framework = detect_framework(path, language)?;
        let size = calculate_size(path)?;
        let structure = analyze_structure(path)?;
        let health = analyze_health(path)?;
        
        Ok(ProjectInfo { ... })
    }
}
```

### Smart Context Building

```rust
impl ContextBuilder {
    fn build(project: &ProjectInfo) -> Result<Context> {
        // Automatically generate context based on project
        let mut context = Context::new();
        
        // Add project summary
        context.add_section("project", generate_project_summary(project));
        
        // Add relevant files (smart selection)
        let key_files = select_key_files(project);
        for file in key_files {
            context.add_file(file);
        }
        
        // Add improvement focus
        let focus_areas = determine_focus_areas(project);
        context.add_section("focus", focus_areas);
        
        Ok(context)
    }
}
```

### Simple State Management

```rust
// Just use JSON in .mmm/state.json
#[derive(Serialize, Deserialize)]
struct ImproveState {
    last_run: DateTime<Utc>,
    current_score: f32,
    improvement_history: Vec<ImprovementRun>,
    learned_patterns: Vec<Pattern>,
}

impl ImproveSession {
    fn save_state(&self) -> Result<()> {
        let state_file = ".mmm/state.json";
        fs::write(state_file, serde_json::to_string_pretty(&self.state)?)?;
        Ok(())
    }
}
```

## Implementation Priority

### Phase 1: Core Command (Week 1)
1. Implement `mmm improve` command structure
2. Basic project analyzer (language detection)
3. Simple Claude integration (review + improve)
4. Basic progress output

### Phase 2: Smart Features (Week 2)
1. Enhanced project analysis
2. Intelligent context building
3. Learning from patterns
4. Better error handling

### Phase 3: Polish (Week 3)
1. Beautiful progress UI
2. Detailed results summary
3. Git integration
4. Performance optimization

## Success Criteria

1. **Zero Config**: Works on any project without setup
2. **Fast Start**: Begins improving within 10 seconds
3. **Clear Progress**: User never wonders what's happening
4. **Safe**: Never breaks working code
5. **Effective**: Measurable improvement each run

## File Structure

```
src/
├── main.rs              # Simplified CLI entry
├── improve/
│   ├── mod.rs          # Public API
│   ├── command.rs      # CLI command handler
│   ├── analyzer.rs     # Project analysis
│   ├── context.rs      # Context building
│   ├── session.rs      # Session management
│   └── display.rs      # Progress display
├── claude/
│   ├── mod.rs          # Claude integration
│   └── prompts.rs      # Built-in prompts
└── utils/
    ├── git.rs          # Git operations
    └── validation.rs   # Change validation
```

## Migration Path

1. Keep existing code but mark as "advanced mode"
2. New `mmm improve` bypasses all existing complexity
3. Gradually migrate useful features to new architecture
4. Eventually deprecate old approach

## Example Implementation

```rust
// src/improve/command.rs
pub async fn run(opts: ImproveOptions) -> Result<()> {
    let spinner = ProgressSpinner::new("Analyzing project...");
    
    // Auto-detect everything
    let project = ProjectAnalyzer::analyze(".").await?;
    spinner.success(&format!("Detected {} project", project.language));
    
    // Run improvement
    let session = ImproveSession::start(project, opts).await?;
    let result = session.run().await?;
    
    // Show results
    println!("\n✅ Improvement complete!");
    println!("  Score: {} → {} (+{})", 
        result.initial_score, 
        result.final_score,
        result.improvement
    );
    
    Ok(())
}
```