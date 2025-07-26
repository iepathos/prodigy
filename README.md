# Memento Mori (mmm)

A dead simple CLI tool that makes your code better through Claude CLI integration.

## What It Does

Run `mmm improve` and it automatically:
1. Analyzes your project (language, framework, code quality)
2. Calls Claude CLI to get improvement suggestions
3. Applies the improvements to your code
4. Repeats until your code reaches the target quality score

That's it. No configuration, no complex workflows, no learning curve.

## Installation

```bash
# Clone and build
git clone <repo-url>
cd mmm
cargo build --release

# Add to PATH or use directly
./target/release/mmm improve
```

## Usage

### Basic Usage
```bash
# Improve your code to a quality score of 8.0 (default)
mmm improve

# Set a custom target score
mmm improve --target 9.0

# See detailed progress
mmm improve --verbose
```

### What Happens
1. **Project Analysis**: Detects your language (Rust, Python, JS, etc.) and framework
2. **Quality Assessment**: Gives your current code a health score (0-10)
3. **Claude Integration**: Calls Claude CLI with smart context about your project
4. **File Modification**: Applies Claude's suggestions directly to your files
5. **Progress Tracking**: Repeats until target score is reached

### Requirements
- [Claude CLI](https://claude.ai/cli) installed and configured
- A project with recognizable code (Rust, Python, JavaScript, TypeScript, etc.)

## Examples

```bash
# Basic improvement run
$ mmm improve
🔍 Analyzing project...
📊 Current health score: 6.2/10
🤖 Calling Claude CLI for improvements...
✅ Applied 3 improvements to 2 files
📊 New health score: 7.1/10
🤖 Calling Claude CLI for improvements...
✅ Applied 2 improvements to 1 file
📊 Final health score: 8.1/10 ✨
🎉 Target reached! Your code is now better.

# Verbose output
$ mmm improve --verbose
🔍 Analyzing project...
   Language: Rust
   Framework: None detected
   Files analyzed: 12
   Issues found: 8
📊 Current health score: 6.2/10
   - Error handling: 5/10
   - Documentation: 4/10
   - Testing: 8/10
🤖 Calling Claude CLI for improvements...
   Command: claude improve --focus error-handling src/main.rs src/lib.rs
   Response: 3 improvements suggested
✅ Applied 3 improvements to 2 files:
   - Added Result<> returns to 2 functions
   - Replaced unwrap() with proper error handling
📊 New health score: 7.1/10
... (continues until target reached)
```

## How It Works

### Architecture
```
mmm improve
    ↓
Analyze Project (language, framework, health)
    ↓
Build Context for Claude CLI
    ↓
Call Claude CLI with improvement request
    ↓
Parse response and apply changes
    ↓
Update state and check if target reached
    ↓
Repeat until done
```

### State Management
- Creates `.mmm/` directory in your project
- Tracks improvement sessions in JSON files
- Caches project analysis for faster subsequent runs
- All human-readable, git-friendly

### Supported Languages
- **Rust**: Full support with Cargo integration
- **Python**: Basic support with pip/requirements detection
- **JavaScript/TypeScript**: Basic support with npm/package.json
- **Others**: Generic improvements (error handling, documentation, etc.)

## Safety

- **Backup**: Never modifies files without backup
- **Validation**: Checks that changes compile/parse before applying
- **Rollback**: Can undo changes if something goes wrong
- **Incremental**: Makes small, focused improvements rather than large changes

## Configuration

None required! The tool works out of the box with smart defaults.

Optional `.mmm/config.toml` for advanced users:
```toml
[improve]
default_target = 8.5
claude_args = ["--no-preamble", "--format=code"]
```

## Project Structure

```
mmm/
├── src/
│   ├── main.rs           # CLI entry point
│   ├── improve/          # Core improvement logic
│   ├── analyzer/         # Project analysis
│   └── simple_state/     # Minimal state management
├── .mmm/                 # Project context and state
└── README.md            # This file
```

## Development

```bash
# Run tests
cargo test

# Format code
cargo fmt

# Check for issues
cargo clippy

# Run on sample project
cargo run -- improve --verbose
```

## Philosophy

1. **Dead Simple**: One command, minimal options, works immediately
2. **Actually Functional**: Real Claude integration, real file changes
3. **Minimal State**: Track only what's needed for the improvement loop
4. **Clear Code**: Straightforward logic, easy to understand and modify
5. **Self-Sufficient**: Hands-off improvement cycles without manual intervention

## Limitations

- Requires Claude CLI to be installed and configured
- Works best with well-structured projects
- Limited to improvements Claude CLI can suggest
- No complex workflow or plugin system (by design)

## License

[Add your license here]

## Contributing

Focus on making the core `mmm improve` command more robust:
- Better language support
- Improved Claude context building
- Enhanced error handling
- Clearer progress feedback

Keep it simple. Keep it working.