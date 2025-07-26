# PROJECT.md - Memento Mori (mmm)

## Overview

Memento Mori (mmm) is a dead simple Rust CLI tool that makes your code better through Claude CLI integration. Just run `mmm improve` and it automatically analyzes your project, calls Claude CLI for improvements, and applies the changes.

## Current State

- **Project Status**: Active Development - Core Working
- **Core Feature**: `mmm improve` command with Claude CLI integration
- **Latest Version**: 0.1.0
- **Implementation Status**: Real Claude CLI integration implemented with working self-sufficient loop, developer experience bloat removed, and state management simplified to essentials only

## What Exists

### Core Functionality
- **Simple CLI**: `mmm improve [--target 8.0] [--verbose]`
- **Claude Integration**: Real Claude CLI subprocess calls using /mmm-code-review and /mmm-implement-spec commands
- **Project Analysis**: Automatic language and framework detection
- **File Modification**: Real changes applied to your codebase
- **Minimal State**: Simplified JSON files tracking only essential data (score, runs, session history)

### Project Structure
```
mmm/
├── src/
│   ├── main.rs           # CLI entry point
│   ├── improve/          # Core improve command logic
│   ├── analyzer/         # Project analysis
│   ├── simple_state/     # Minimal state management
│   └── lib.rs           # Library exports
├── .mmm/                # Project context and state
└── README.md            # User documentation
```

## Key Capabilities

1. **Dead Simple Interface**
   - Single command: `mmm improve`
   - Optional target score and verbosity
   - Works out of the box

2. **Real Claude Integration**
   - Calls actual Claude CLI commands
   - Applies suggested improvements
   - Tracks changes and progress

3. **Minimal State Management**
   - Simplified JSON files for essential data only
   - Basic project analysis caching
   - Current score and run count tracking

4. **Working Self-Sufficient Loop**
   - Analyze → Call Claude Review → Call Claude Implement → Apply Changes → Re-analyze → Repeat
   - Automatic termination when target reached or no issues found
   - Real file modifications with backup and validation

## Technology Stack

- **Language**: Rust (2021 edition)
- **CLI Framework**: Clap v4
- **Async Runtime**: Tokio
- **State**: JSON files
- **Serialization**: Serde (JSON)
- **Claude Integration**: Direct CLI subprocess calls

## Development Philosophy

- **Dead Simple**: Single command interface, minimal options
- **Actually Works**: Real Claude integration, real file changes
- **Minimal State**: Track only what's needed for the loop
- **Clear Code**: Straightforward logic, single focused module
- **Self-Sufficient**: Genuine improvement cycles without manual intervention

## Next Steps

Focus on making the core `mmm improve` command robust and reliable:
- Better error handling
- More language support
- Improved Claude context building
- Enhanced progress feedback