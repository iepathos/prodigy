# Memento Mori (mmm)

A Rust CLI tool for implementing self-sufficient loops with Claude CLI, enabling automated specification processing and iterative development.

## Installation

```bash
just build
```

## Usage

### Basic Commands

```bash
# Run all specifications
mmm

# Run a specific specification
mmm run --spec feature-name

# Run with a custom command (e.g., /implement-spec, /lint)
mmm run --command /implement-spec

# List available specifications
mmm list

# Create a new specification template
mmm init --name my-feature

# Add a new specification interactively
mmm add --name my-feature

# Add a new specification with Claude generation
mmm add --name my-feature --generate
```

### Options

- `-s, --specs-dir <DIR>`: Directory containing specifications (default: "specs")
- `-m, --max-iterations <N>`: Maximum iterations per spec (default: 3)
- `-v, --verbose`: Enable verbose logging

## How It Works

1. **Specification Loading**: Reads all `.md` files from the specs directory
2. **Iterative Processing**: For each spec, calls Claude CLI with context
3. **Progress Tracking**: Maintains state between iterations
4. **Completion Detection**: Specs marked complete when Claude responds with "COMPLETED:"

## Custom Commands

The tool supports custom Claude commands for different development phases:

- `/implement-spec`: Initial implementation
- `/lint`: Code quality checks
- `/test`: Test creation and execution
- `/review`: Final review and approval

Configure commands in `mmm.toml`.

## Configuration

Create a `mmm.toml` file to customize behavior:

```toml
[claude]
default_args = ["--no-preamble"]

[commands]
implement = "/implement-spec"
lint = "/lint"
test = "/test"
review = "/review"
```

## Project Structure

```
mmm/
├── Justfile           # Build and development commands
├── mmm.toml           # Configuration file
├── specs/             # Specification files
│   └── README.md      # Spec format documentation
├── src/
│   └── main.rs        # Main CLI implementation
└── target/            # Build artifacts
```

## Development

```bash
# Run all CI checks
just ci

# Format code
just fmt

# Run tests
just test

# Run linting
just lint
```