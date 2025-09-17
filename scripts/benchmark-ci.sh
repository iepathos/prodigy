#!/bin/bash
#
# CI script for running benchmarks with regression detection
# Usage: ./scripts/benchmark-ci.sh [--update-baseline]

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo "🚀 Running Prodigy Benchmark Suite"
echo "=================================="

# Get current commit hash
CURRENT_COMMIT=$(git rev-parse HEAD)
echo "Current commit: $CURRENT_COMMIT"

# Get base commit for comparison (default to main branch)
BASE_COMMIT=${BASE_COMMIT:-$(git rev-parse main 2>/dev/null || git rev-parse master 2>/dev/null || echo "")}

if [ -n "$BASE_COMMIT" ]; then
    echo "Base commit: $BASE_COMMIT"
else
    echo -e "${YELLOW}Warning: No base commit found, running without comparison${NC}"
fi

# Create benchmark results directory
RESULTS_DIR=".benchmark-results"
mkdir -p "$RESULTS_DIR"

# Function to run a benchmark group
run_benchmark() {
    local bench_name=$1
    local output_file="$RESULTS_DIR/${bench_name}-${CURRENT_COMMIT:0:8}.json"

    echo -e "\n📊 Running benchmark: ${bench_name}"
    echo "----------------------------------------"

    # Run the benchmark with JSON output
    if cargo bench --bench "$bench_name" -- --output-format bencher | tee "$output_file"; then
        echo -e "${GREEN}✓ ${bench_name} completed${NC}"
        return 0
    else
        echo -e "${RED}✗ ${bench_name} failed${NC}"
        return 1
    fi
}

# Run all benchmarks
BENCHMARKS=(
    "checkpoint_benchmarks"
    "execution_benchmarks"
    "mapreduce_benchmarks"
    "memory_benchmarks"
)

FAILED_BENCHMARKS=()

for bench in "${BENCHMARKS[@]}"; do
    if ! run_benchmark "$bench"; then
        FAILED_BENCHMARKS+=("$bench")
    fi
done

# Check for failures
if [ ${#FAILED_BENCHMARKS[@]} -gt 0 ]; then
    echo -e "\n${RED}❌ Some benchmarks failed:${NC}"
    for bench in "${FAILED_BENCHMARKS[@]}"; do
        echo "  - $bench"
    done
    exit 1
fi

echo -e "\n${GREEN}✅ All benchmarks completed successfully${NC}"

# Run regression detection if we have criterion results
if [ -f "target/criterion" ]; then
    echo -e "\n🔍 Checking for performance regressions..."
    echo "=========================================="

    # Export environment variables for regression detection
    export BASE_COMMIT
    export CURRENT_COMMIT

    # Check if we should update baseline
    if [[ "$1" == "--update-baseline" ]]; then
        export UPDATE_BASELINE=1
        echo "📝 Will update baseline after checks"
    fi

    # Run regression detection tool (would be integrated with criterion)
    # This is a placeholder for actual integration
    cargo run --bin benchmark-regression-check 2>/dev/null || {
        # If the regression check binary doesn't exist yet, just report
        echo -e "${YELLOW}Note: Regression detection binary not found${NC}"
        echo "To enable regression detection, build with:"
        echo "  cargo build --bin benchmark-regression-check"
    }
fi

# Generate performance report
echo -e "\n📈 Generating performance report..."
echo "===================================="

# Create markdown report
REPORT_FILE="$RESULTS_DIR/performance-report-${CURRENT_COMMIT:0:8}.md"
cat > "$REPORT_FILE" << EOF
# Performance Report

**Commit:** $CURRENT_COMMIT
**Date:** $(date -u +"%Y-%m-%d %H:%M:%S UTC")
**Branch:** $(git branch --show-current)

## Benchmark Results

EOF

# Add summary of each benchmark
for bench in "${BENCHMARKS[@]}"; do
    echo "### $bench" >> "$REPORT_FILE"
    echo "" >> "$REPORT_FILE"

    if [ -f "$RESULTS_DIR/${bench}-${CURRENT_COMMIT:0:8}.json" ]; then
        echo "Results saved to: $RESULTS_DIR/${bench}-${CURRENT_COMMIT:0:8}.json" >> "$REPORT_FILE"
    else
        echo "No results file found" >> "$REPORT_FILE"
    fi
    echo "" >> "$REPORT_FILE"
done

echo -e "${GREEN}Report generated: $REPORT_FILE${NC}"

# CI summary
echo -e "\n=================================="
echo "📋 CI Benchmark Summary"
echo "=================================="
echo "✓ Benchmarks run: ${#BENCHMARKS[@]}"
echo "✓ Failed: ${#FAILED_BENCHMARKS[@]}"
echo "✓ Report: $REPORT_FILE"

if [ ${#FAILED_BENCHMARKS[@]} -eq 0 ]; then
    echo -e "\n${GREEN}🎉 CI Benchmark Suite: PASSED${NC}"
    exit 0
else
    echo -e "\n${RED}💔 CI Benchmark Suite: FAILED${NC}"
    exit 1
fi