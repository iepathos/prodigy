# Spec 13: Progressive Enhancement

## Objective

Design MMM to start dead simple but reveal power features as users need them. Create a natural learning curve from beginner to power user without overwhelming new users.

## Enhancement Levels

### Level 1: Zero Config (First Run)

```bash
mmm improve
# Just works. No questions asked.
```

**Features:**
- Auto-detect everything
- Use sensible defaults  
- Fix obvious issues
- Show clear progress

**Hidden:**
- Configuration options
- Advanced commands
- Complex workflows
- Technical details

### Level 2: Focused Improvements (After 5+ Runs)

```bash
mmm improve --focus tests
mmm improve --target 9.0
```

**Revealed Features:**
- Focus flags for targeted improvement
- Quality score targets
- Preview mode
- Basic configuration

**Gentle Introduction:**
```
💡 Tip: You've run MMM 5 times! Did you know you can focus on specific areas?
   Try: mmm improve --focus tests
```

### Level 3: Power User (After 20+ Runs)

```bash
mmm improve --recipe performance-audit
mmm improve --parallel --aggressive
```

**Advanced Features:**
- Custom improvement recipes
- Parallel processing
- Aggressive refactoring
- Custom rules

**Discovery:**
```
🎓 You're becoming an MMM expert! Unlock power features:
   • Custom recipes: mmm recipes list
   • Parallel mode: mmm improve --parallel
   • See all options: mmm improve --help-advanced
```

### Level 4: Team/Enterprise (On Demand)

```bash
mmm team setup
mmm improve --team-config
```

**Team Features:**
- Shared configurations
- Team dashboards
- CI/CD integration
- Compliance rules

## Progressive Feature Revelation

### Smart Tips System

```rust
pub struct TipEngine {
    usage_stats: UsageStats,
}

impl TipEngine {
    pub fn get_contextual_tip(&self) -> Option<String> {
        match self.usage_stats {
            // Beginner tips
            UsageStats { total_runs: 1..=3, .. } => {
                Some("💡 MMM works best when run regularly. Try daily!")
            }
            
            // Intermediate tips
            UsageStats { total_runs: 5..=10, errors_fixed: 20.., .. } => {
                Some("💡 Great error fixing! Try 'mmm improve --focus tests' next")
            }
            
            // Advanced tips
            UsageStats { total_runs: 20.., .. } => {
                Some("💡 Power user tip: Create custom recipes with 'mmm recipe new'")
            }
            
            _ => None
        }
    }
}
```

### Adaptive Interface

```rust
pub struct AdaptiveUI {
    user_level: UserLevel,
}

impl AdaptiveUI {
    pub fn show_results(&self, results: &Results) {
        match self.user_level {
            UserLevel::Beginner => self.show_simple_results(results),
            UserLevel::Intermediate => self.show_detailed_results(results),
            UserLevel::Advanced => self.show_full_analytics(results),
        }
    }
    
    fn show_simple_results(&self, results: &Results) {
        println!("✨ Your code is {} better!", results.improvement_percent());
        println!("🎯 {} improvements made", results.total_improvements());
    }
    
    fn show_detailed_results(&self, results: &Results) {
        // Show breakdown by category
        println!("📊 Improvements by type:");
        for (category, count) in results.by_category() {
            println!("  {} {}: {}", 
                category.emoji(), 
                category.name(), 
                count
            );
        }
    }
}
```

## Feature Discovery Mechanisms

### 1. Help System Evolution

```bash
# Beginner (default)
$ mmm --help
mmm - Make your code better

Usage: mmm improve

That's it! Just run 'mmm improve' in your project.

# Intermediate (after 10 runs)
$ mmm --help
mmm - Make your code better

Commands:
  improve    Improve code quality (default)
  status     Show improvement history
  undo       Undo last improvement

Options:
  --focus    Target specific improvements
  --preview  Preview changes before applying

# Advanced (after 30 runs or --help-advanced)
$ mmm --help-advanced
[Full command documentation with all options]
```

### 2. Interactive Learning

```bash
$ mmm improve

🔧 Applying improvements...

[After first error fix]
ℹ️  I just fixed an error handling issue. These improvements
   make your code more robust and prevent crashes.
   
   Learn more: mmm explain error-handling

Continue? [Y/n]: 
```

### 3. Configuration Discovery

```toml
# Auto-generated mmm.toml after user uses --focus multiple times
# with helpful comments

# MMM Configuration (auto-generated)
# Uncomment lines to customize behavior

# Default improvement targets
# focus = ["errors", "tests"]

# Quality score target (default: 8.0)
# target_score = 9.0

# Advanced settings (be careful!)
# [advanced]
# parallel = true
# aggressive = false
```

## Recipe System (Advanced Feature)

### Built-in Recipes

```bash
# Discovered after 20+ runs
$ mmm recipes list

📚 Available Improvement Recipes:

quick-fix         Fast improvements (< 1 min)
test-boost        Maximize test coverage
perf-audit        Performance optimization
security-scan     Security-focused improvements
pre-release       Comprehensive pre-release prep
tech-debt         Technical debt reduction
```

### Custom Recipes

```yaml
# .mmm/recipes/my-recipe.yaml
name: api-hardening
description: Harden API endpoints
steps:
  - focus: error-handling
    target: "src/api/**"
  - focus: validation
    target: "src/api/**"  
  - focus: tests
    target: "tests/api/**"
  - validate: security-scan
```

## Learning Paths

### Path 1: Quality Improver
```
Basic → Focus on errors → Add tests → Improve docs
```

### Path 2: Performance Optimizer
```
Basic → Profile code → Focus on performance → Parallel processing
```

### Path 3: Team Lead
```
Basic → Team setup → Shared configs → CI integration
```

## Progressive Configuration

### Stage 1: No Config
Everything works with zero configuration.

### Stage 2: Simple Config
```toml
# mmm.toml
target_score = 9.0
focus = ["tests", "docs"]
```

### Stage 3: Advanced Config
```toml
# mmm.toml
[improve]
target_score = 9.5
focus = ["tests", "errors", "performance"]
exclude = ["generated/", "vendor/"]

[advanced]
parallel = true
max_file_size = "100KB"
custom_rules = "team-rules.yaml"

[claude]
model = "claude-3-opus"
temperature = 0.3
```

### Stage 4: Team Config
```toml
# team.mmm.toml (shared)
extends = "https://company.com/mmm-standards.toml"

[compliance]
require_tests = true
min_coverage = 80
doc_public_apis = true
```

## Gradual Complexity Revelation

### Commands
```
Level 1: mmm improve
Level 2: mmm improve --focus tests
Level 3: mmm recipe security-audit
Level 4: mmm team sync
```

### Options
```
Level 1: (none)
Level 2: --focus, --target, --preview
Level 3: --parallel, --recipe, --aggressive
Level 4: --team-config, --compliance
```

### Configuration
```
Level 1: Zero config
Level 2: Basic mmm.toml
Level 3: Advanced settings
Level 4: Team policies
```

## Implementation Strategy

### Usage Tracking

```rust
#[derive(Serialize, Deserialize)]
struct UsageStats {
    total_runs: u32,
    unique_commands: HashSet<String>,
    features_used: HashSet<String>,
    last_tip_shown: Option<String>,
    user_level: UserLevel,
}

impl UsageStats {
    fn update(&mut self, command: &Command) {
        self.total_runs += 1;
        self.unique_commands.insert(command.name());
        self.features_used.extend(command.features_used());
        self.update_user_level();
    }
    
    fn update_user_level(&mut self) {
        self.user_level = match self.total_runs {
            0..=5 => UserLevel::Beginner,
            6..=20 => UserLevel::Intermediate,
            21..=50 => UserLevel::Advanced,
            _ => UserLevel::Expert,
        };
    }
}
```

### Feature Gates

```rust
pub trait Feature {
    fn min_level(&self) -> UserLevel;
    fn is_available(&self, stats: &UsageStats) -> bool {
        stats.user_level >= self.min_level()
    }
}

impl Feature for ParallelMode {
    fn min_level(&self) -> UserLevel {
        UserLevel::Advanced
    }
}
```

## Success Metrics

1. **Adoption**: 90% of users successfully complete first run
2. **Progression**: 50% discover Level 2 features within a week  
3. **Retention**: 70% still using after 30 days
4. **Satisfaction**: No "too complex" complaints from beginners
5. **Power Usage**: 20% eventually use advanced features