# Memento Mori (mmm)

A dead simple CLI tool that makes your code better through Claude CLI integration.

## What It Does

Run `mmm improve` and it automatically:
1. **Analyzes** your project (language, framework, code quality)
2. **Reviews** code with Claude CLI and creates improvement specs
3. **Implements** the improvements by applying fixes to your code
4. **Lints** and formats the code with automated tools
5. **Repeats** until your code reaches the target quality score

All changes are committed to git with clear audit trails. No configuration, no complex workflows, no learning curve.

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

### What Happens (Git-Native Flow)
1. **Project Analysis**: Detects your language (Rust, Python, JS, etc.) and framework
2. **Quality Assessment**: Gives your current code a health score (0-10)
3. **Code Review**: Claude analyzes code and generates improvement specs
4. **Spec Commit**: Creates `specs/temp/iteration-*-improvements.md` and commits it
5. **Implementation**: Applies fixes from the spec and commits changes
6. **Linting**: Runs `cargo fmt`, `clippy --fix`, and `test`, commits if changes
7. **Progress Tracking**: Re-analyzes and repeats until target score is reached

Each step creates git commits for complete auditability.

### Requirements
- [Claude CLI](https://claude.ai/cli) installed and configured
- A project with recognizable code (Rust, Python, JavaScript, TypeScript, etc.)

## Examples

```bash
# Basic improvement run
$ mmm improve
🔍 Analyzing project...
Current score: 6.2/10
🔄 Iteration 1/10...
✅ Review completed: Found 3 issues
✅ Generated spec: iteration-1708123456-improvements.md  
✅ Implementation completed: 2 files modified
✅ Linting completed: formatting applied
Score: 6.2 → 7.1
🔄 Iteration 2/10...
✅ Review completed: Found 1 issue
✅ Generated spec: iteration-1708123789-improvements.md
✅ Implementation completed: 1 file modified  
✅ Linting completed: no changes needed
Score: 7.1 → 8.1
✅ Complete! Final score: 8.1/10
Files changed: 3
Iterations: 2

# Verbose output shows detailed git flow
$ mmm improve --verbose
🔍 Analyzing project...
Current score: 6.2/10
🔄 Iteration 1/10...
Calling Claude CLI for code review...
Extracting spec ID from git history...
Calling Claude CLI to implement spec: iteration-1708123456-improvements
Calling Claude CLI for linting...
Score: 6.2 → 7.1
... (continues until target reached)

# Check the git history to see what happened
$ git log --oneline -10
a1b2c3d style: apply automated formatting and lint fixes
b2c3d4e fix: apply improvements from spec iteration-1708123789-improvements
c3d4e5f review: generate improvement spec for iteration-1708123789-improvements
d4e5f6g style: apply automated formatting and lint fixes  
e5f6g7h fix: apply improvements from spec iteration-1708123456-improvements
f6g7h8i review: generate improvement spec for iteration-1708123456-improvements
```

## How It Works

### Git-Native Architecture
```
mmm improve
    ↓
Analyze Project (language, framework, health score)
    ↓
┌─────────────────── IMPROVEMENT LOOP ──────────────────┐
│  Call claude /mmm-code-review                         │
│      ↓                                                │
│  Generate specs/temp/iteration-*-improvements.md      │
│      ↓                                                │
│  Commit: "review: generate improvement spec..."       │
│      ↓                                                │
│  Extract spec ID from git log                         │
│      ↓                                                │
│  Call claude /mmm-implement-spec {spec-id}            │
│      ↓                                                │
│  Apply fixes and commit: "fix: apply improvements..." │
│      ↓                                                │
│  Call claude /mmm-lint                                │
│      ↓                                                │
│  Format/lint and commit: "style: apply automated..."  │
│      ↓                                                │
│  Re-analyze → Check target → Continue or Exit         │
└───────────────────────────────────────────────────────┘
```

### State Management
- **Git History**: Complete audit trail of all changes through commits
- **Temporary Specs**: `specs/temp/iteration-*-improvements.md` contain exact fixes applied  
- **Simple State**: `.mmm/state.json` tracks basic session info (current score, run count)
- **Project Context**: `.mmm/PROJECT.md`, `ARCHITECTURE.md` provide Claude with project understanding
- All human-readable, git-friendly, no complex databases

### Supported Languages
- **Rust**: Full support with Cargo integration
- **Python**: Basic support with pip/requirements detection
- **JavaScript/TypeScript**: Basic support with npm/package.json
- **Others**: Generic improvements (error handling, documentation, etc.)

## Safety

- **Git-Native**: Every change is a git commit - easy to inspect and revert
- **Automated Testing**: Each iteration runs tests to ensure nothing breaks
- **Incremental**: Makes small, focused improvements rather than large changes
- **Auditable**: Complete paper trail of what was changed and why
- **Validation**: Code is linted and formatted after each change

## Configuration

None required! The tool works out of the box with smart defaults.

### Configurable Workflows (Optional)

Create a `.mmm/workflow.toml` file to customize the improvement workflow:

```toml
# .mmm/workflow.toml
# List of Claude commands to execute in order
commands = [
    "mmm-code-review",
    "mmm-implement-spec",
    "mmm-lint"
]

# Optional: Maximum iterations (default: 10)
max_iterations = 10
```

The default workflow runs these three commands in order:
1. `mmm-code-review` - Analyzes code and generates improvement specs
2. `mmm-implement-spec` - Implements the improvements (spec ID extracted automatically)
3. `mmm-lint` - Runs formatting and linting

#### Alternative Workflow Examples

**Security-Focused Workflow:**
```toml
commands = [
    "mmm-security-scan",
    "mmm-fix-vulnerabilities",
    "mmm-lint",
    "mmm-security-verify"
]
```

**Test-Driven Workflow:**
```toml
commands = [
    "mmm-coverage",
    "mmm-add-tests",
    "mmm-code-review",
    "mmm-implement-spec"
]
```

**Documentation Workflow:**
```toml
commands = [
    "mmm-doc-check",
    "mmm-generate-docs",
    "mmm-lint-docs"
]
```

### Advanced Configuration

Optional `.mmm/config.toml` for advanced users:
```toml
[improve]
default_target = 8.5
claude_args = ["--no-preamble", "--format=code"]
```

### Parallel Sessions with Git Worktrees

Run multiple improvement sessions concurrently without conflicts:

```bash
# Enable worktree mode for this session
MMM_USE_WORKTREE=true mmm improve --focus "performance"

# In another terminal, run a different improvement focus
MMM_USE_WORKTREE=true mmm improve --focus "security"

# List active worktree sessions
mmm worktree list

# Merge improvements back to main branch
mmm worktree merge mmm-performance-1234567890

# Clean up completed worktrees
mmm worktree clean mmm-performance-1234567890
# Or clean all worktrees
mmm worktree clean --all
```

Each session runs in its own git worktree with an isolated branch, allowing multiple improvement efforts to proceed without interfering with each other. Worktrees are preserved on failure for debugging and automatically suggested for cleanup on success.

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
2. **Actually Functional**: Real Claude integration, real file changes, real git commits
3. **Git-Native**: Use git as the communication layer - simple, reliable, auditable
4. **Self-Sufficient**: Fully automated improvement cycles with complete logging
5. **Clear & Minimal**: Focus on the core loop, avoid over-engineering

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