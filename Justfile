# Run all CI tasks
ci: lint test build

# Build the project
build:
    cargo build --release

# Run tests
test:
    cargo test

# Run linting checks
lint:
    cargo fmt -- --check
    cargo clippy -- -D warnings

# Format code
fmt:
    cargo fmt

# Run the application
run *ARGS:
    cargo run -- {{ARGS}}

# Clean build artifacts
clean:
    cargo clean

# Check for outdated dependencies
outdated:
    cargo outdated

# Update dependencies
update:
    cargo update

# Watch for changes and rebuild
watch:
    cargo watch -x build -x test