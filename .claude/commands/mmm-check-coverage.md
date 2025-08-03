# /mmm-check-coverage

Verify that test coverage tools are working correctly before running other commands in the workflow.

## Execute

### Phase 1: Check Dependencies

**Verify cargo-tarpaulin is installed**:
```bash
if ! command -v cargo-tarpaulin &> /dev/null; then
    echo "⚠️  cargo-tarpaulin not installed. Installing..."
    cargo install cargo-tarpaulin
fi
```

### Phase 2: Run Coverage Check

**Run a quick coverage test to ensure it compiles**:
```bash
echo "🔍 Checking if coverage compilation works..."

# Run tarpaulin with minimal settings to check compilation
if cargo tarpaulin --skip-clean --engine llvm --no-run --timeout 60 2>&1; then
    echo "✅ Coverage compilation successful"
else
    echo "❌ Coverage compilation failed"
    
    # Check for common issues
    if cargo tarpaulin --skip-clean --engine llvm --no-run --timeout 60 2>&1 | grep -q "serde_toml"; then
        echo "⚠️  Found unresolved serde_toml reference"
        echo "💡 This might be from a recent merge - checking for alternatives..."
        
        # Suggest fix
        echo "🔧 Attempting to identify the issue..."
        grep -r "serde_toml::" . --include="*.rs" || true
    fi
    
    echo "❌ Coverage check failed. Fix the issues before proceeding."
fi
```

### Phase 3: Quick Validation

**If compilation succeeds, run a minimal coverage check**:
```bash
echo "🧪 Running minimal coverage test..."

# Run coverage on a small subset to verify functionality
if cargo tarpaulin --skip-clean --engine llvm --timeout 30 --exclude-files "tests/*" 2>&1 | head -20; then
    echo "✅ Coverage tools are working correctly"
else
    echo "❌ Coverage run failed"
fi
```

## Success Criteria & Output

**Success Output**:
```
✅ Coverage compilation successful
🧪 Running minimal coverage test...
✅ Coverage tools are working correctly
```

**Failure Output**:
```
❌ Coverage compilation failed
⚠️  Found unresolved serde_toml reference
💡 This might be from a recent merge - checking for alternatives...
❌ Coverage check failed. Fix the issues before proceeding.
```

## Command Integration

**Usage in workflows**:
- Run before `mmm-coverage` to ensure tools work
- Run before `mmm-lint` in tech debt workflows

**Early failure detection**:
- Catches compilation issues before lengthy operations
- Identifies missing dependencies or broken imports
- Provides helpful diagnostics for common issues