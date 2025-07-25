#!/usr/bin/env bash
# monitor-implementation.sh - Real-time monitoring for automated spec implementation

set -euo pipefail

PROGRESS_FILE=".spec-progress.json"
LOG_DIR="logs/auto-implement"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

# Clear screen and move cursor to top
clear_screen() {
    printf '\033[2J\033[H'
}

# Display progress bar
progress_bar() {
    local current=$1
    local total=$2
    local width=50
    
    if [[ $total -eq 0 ]]; then
        echo -n "["
        printf '%*s' $width | tr ' ' '-'
        echo "] 0%"
        return
    fi
    
    local progress=$((current * 100 / total))
    local filled=$((progress * width / 100))
    
    echo -n "["
    printf '%*s' $filled | tr ' ' '█'
    printf '%*s' $((width - filled)) | tr ' ' '-'
    echo "] $progress%"
}

# Format duration
format_duration() {
    local seconds=$1
    local hours=$((seconds / 3600))
    local minutes=$(((seconds % 3600) / 60))
    local secs=$((seconds % 60))
    
    if [[ $hours -gt 0 ]]; then
        printf "%dh %dm %ds" $hours $minutes $secs
    elif [[ $minutes -gt 0 ]]; then
        printf "%dm %ds" $minutes $secs
    else
        printf "%ds" $secs
    fi
}

# Get latest log entry
get_latest_log() {
    local pattern="${1:-*.log}"
    find "$LOG_DIR" -name "$pattern" -type f -printf '%T@ %p\n' 2>/dev/null | \
        sort -n | tail -1 | cut -d' ' -f2-
}

# Monitor loop
monitor() {
    local refresh_rate=${1:-2}
    
    while true; do
        clear_screen
        
        if [[ ! -f "$PROGRESS_FILE" ]]; then
            echo -e "${RED}No implementation in progress${NC}"
            echo -e "${YELLOW}Run: ./scripts/auto-implement-specs.sh${NC}"
            sleep $refresh_rate
            continue
        fi
        
        # Read progress data
        local current_spec=$(jq -r '.current_spec // "None"' "$PROGRESS_FILE")
        local completed=$(jq -r '.completed_specs | length' "$PROGRESS_FILE")
        local failed=$(jq -r '.failed_specs | length' "$PROGRESS_FILE")
        local total=$(jq -r '.total_specs // 0' "$PROGRESS_FILE")
        local start_time=$(jq -r '.start_time // ""' "$PROGRESS_FILE")
        
        # Calculate duration
        local duration=0
        if [[ -n "$start_time" ]]; then
            local start_epoch=$(date -d "$start_time" +%s 2>/dev/null || date -j -f "%Y-%m-%dT%H:%M:%SZ" "$start_time" +%s 2>/dev/null || echo 0)
            local current_epoch=$(date +%s)
            duration=$((current_epoch - start_epoch))
        fi
        
        # Display header
        echo -e "${GREEN}=== Git Good Spec Implementation Monitor ===${NC}"
        echo -e "Runtime: $(format_duration $duration)"
        echo ""
        
        # Progress overview
        echo -e "${BLUE}Progress:${NC}"
        progress_bar $completed $total
        echo -e "Completed: ${GREEN}$completed${NC} | Failed: ${RED}$failed${NC} | Total: $total"
        echo ""
        
        # Current spec
        echo -e "${BLUE}Current Spec:${NC} $current_spec"
        echo ""
        
        # Recent activity
        echo -e "${BLUE}Recent Activity:${NC}"
        
        # Show latest build status
        local latest_build=$(get_latest_log "build-*.log")
        if [[ -n "$latest_build" ]]; then
            if tail -5 "$latest_build" 2>/dev/null | grep -q "error\|failed"; then
                echo -e "  Build: ${RED}✗ Failed${NC}"
            else
                echo -e "  Build: ${GREEN}✓ Passed${NC}"
            fi
        fi
        
        # Show latest test status
        local latest_test=$(get_latest_log "test-*.log")
        if [[ -n "$latest_test" ]]; then
            if tail -10 "$latest_test" 2>/dev/null | grep -q "FAILED\|error"; then
                echo -e "  Tests: ${RED}✗ Failed${NC}"
            else
                echo -e "  Tests: ${GREEN}✓ Passed${NC}"
            fi
        fi
        
        # Show latest lint status
        local latest_lint=$(get_latest_log "lint-*.log")
        if [[ -n "$latest_lint" ]]; then
            if tail -5 "$latest_lint" 2>/dev/null | grep -q "warning\|error"; then
                echo -e "  Lint:  ${RED}✗ Failed${NC}"
            else
                echo -e "  Lint:  ${GREEN}✓ Passed${NC}"
            fi
        fi
        
        echo ""
        
        # Recently completed specs
        if [[ $completed -gt 0 ]]; then
            echo -e "${BLUE}Recently Completed:${NC}"
            jq -r '.completed_specs[-3:] | reverse | .[] | "  ✓ \(.)"' "$PROGRESS_FILE" 2>/dev/null || true
            echo ""
        fi
        
        # Failed specs
        if [[ $failed -gt 0 ]]; then
            echo -e "${BLUE}Failed Specs:${NC}"
            jq -r '.failed_specs[-3:] | reverse | .[] | "  ✗ \(.)"' "$PROGRESS_FILE" 2>/dev/null || true
            echo ""
        fi
        
        # Performance metrics
        if [[ $completed -gt 0 && $duration -gt 0 ]]; then
            local avg_time=$((duration / completed))
            local eta=$((avg_time * (total - completed)))
            
            echo -e "${BLUE}Performance:${NC}"
            echo -e "  Avg time per spec: $(format_duration $avg_time)"
            echo -e "  Estimated remaining: $(format_duration $eta)"
            echo ""
        fi
        
        # Footer
        echo -e "${CYAN}Press Ctrl+C to exit monitor${NC}"
        
        sleep $refresh_rate
    done
}

# Main
case "${1:-}" in
    --help|-h)
        echo "Usage: $0 [refresh-rate]"
        echo ""
        echo "Monitor the automated spec implementation progress"
        echo ""
        echo "Arguments:"
        echo "  refresh-rate    Seconds between updates (default: 2)"
        echo ""
        echo "Example:"
        echo "  $0        # Update every 2 seconds"
        echo "  $0 5      # Update every 5 seconds"
        exit 0
        ;;
    *)
        monitor "${1:-2}"
        ;;
esac