#!/usr/bin/env bash
# auto-implement-specs.sh - Automated spec implementation loop for Claude Code
# This script enables Claude to work through specs autonomously with validation gates

set -euo pipefail

# Configuration
SPEC_DIR="specs"
PROGRESS_FILE=".spec-progress.json"
LOG_DIR="logs/auto-implement"
CHECKPOINT_DIR=".checkpoints"
MAX_RETRIES=3
CLAUDE_CMD="${CLAUDE_CMD:-claude}"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Initialize directories
mkdir -p "$LOG_DIR" "$CHECKPOINT_DIR"

# Progress tracking functions
init_progress() {
    if [[ ! -f "$PROGRESS_FILE" ]]; then
        echo '{
  "current_spec": null,
  "completed_specs": [],
  "failed_specs": [],
  "skipped_specs": [],
  "total_specs": 0,
  "start_time": "'$(date -u +"%Y-%m-%dT%H:%M:%SZ")'"
}' > "$PROGRESS_FILE"
    fi
}

update_progress() {
    local spec="$1"
    local status="$2"
    local temp_file=$(mktemp)
    
    jq --arg spec "$spec" --arg status "$status" '
        if $status == "completed" then
            .completed_specs += [$spec]
        elif $status == "failed" then
            .failed_specs += [$spec]
        elif $status == "skipped" then
            .skipped_specs += [$spec]
        elif $status == "current" then
            .current_spec = $spec
        end
    ' "$PROGRESS_FILE" > "$temp_file" && mv "$temp_file" "$PROGRESS_FILE"
}

# Git checkpoint functions
create_checkpoint() {
    local spec="$1"
    local checkpoint_name="checkpoint-$(basename "$spec" .md)-$(date +%s)"
    
    echo -e "${BLUE}Creating checkpoint: $checkpoint_name${NC}"
    git add -A
    git commit -m "Checkpoint before implementing $spec" || true
    git tag "$checkpoint_name"
    
    echo "$checkpoint_name" > "$CHECKPOINT_DIR/latest"
}

restore_checkpoint() {
    local checkpoint=$(cat "$CHECKPOINT_DIR/latest" 2>/dev/null || echo "")
    
    if [[ -n "$checkpoint" ]]; then
        echo -e "${YELLOW}Restoring to checkpoint: $checkpoint${NC}"
        git reset --hard "$checkpoint"
    else
        echo -e "${RED}No checkpoint found${NC}"
        return 1
    fi
}

# Validation functions
run_build() {
    echo -e "${BLUE}Running build...${NC}"
    just build 2>&1 | tee "$LOG_DIR/build-$(date +%s).log"
}

run_tests() {
    echo -e "${BLUE}Running tests...${NC}"
    just test 2>&1 | tee "$LOG_DIR/test-$(date +%s).log"
}

run_lint() {
    echo -e "${BLUE}Running lint...${NC}"
    just lint 2>&1 | tee "$LOG_DIR/lint-$(date +%s).log"
}

run_compatibility_check() {
    echo -e "${BLUE}Running Git compatibility check...${NC}"
    if [[ -f "scripts/check-git-compatibility.sh" ]]; then
        ./scripts/check-git-compatibility.sh 2>&1 | tee "$LOG_DIR/compat-$(date +%s).log"
    else
        echo -e "${YELLOW}No compatibility check script found, skipping${NC}"
        return 0
    fi
}

# Main implementation function
implement_spec() {
    local spec="$1"
    local attempt=1
    
    echo -e "\n${GREEN}=== Implementing spec: $spec ===${NC}"
    update_progress "$spec" "current"
    
    while [[ $attempt -le $MAX_RETRIES ]]; do
        echo -e "${BLUE}Attempt $attempt of $MAX_RETRIES${NC}"
        
        # Create checkpoint before implementation
        create_checkpoint "$spec"
        
        # Run Claude implementation
        echo -e "${BLUE}Running Claude implementation...${NC}"
        if $CLAUDE_CMD "/implement-spec $spec" 2>&1 | tee "$LOG_DIR/implement-$spec-$attempt.log"; then
            
            # Run validation pipeline
            if run_build && run_tests && run_lint && run_compatibility_check; then
                echo -e "${GREEN}✓ Spec implemented successfully!${NC}"
                update_progress "$spec" "completed"
                
                # Commit successful implementation
                git add -A
                git commit -m "Implement $spec (automated)" || true
                
                return 0
            else
                echo -e "${RED}✗ Validation failed${NC}"
                
                if [[ $attempt -lt $MAX_RETRIES ]]; then
                    echo -e "${YELLOW}Attempting to fix issues...${NC}"
                    
                    # Let Claude try to fix the issues
                    $CLAUDE_CMD "The previous implementation failed validation. Please review the errors in the logs and fix them:
                    - Build log: $LOG_DIR/build-*.log
                    - Test log: $LOG_DIR/test-*.log
                    - Lint log: $LOG_DIR/lint-*.log
                    Fix these issues while maintaining the spec requirements." 2>&1 | tee "$LOG_DIR/fix-$spec-$attempt.log"
                fi
            fi
        else
            echo -e "${RED}✗ Implementation command failed${NC}"
        fi
        
        # If we're here, attempt failed
        if [[ $attempt -lt $MAX_RETRIES ]]; then
            restore_checkpoint
        fi
        
        ((attempt++))
    done
    
    # All attempts failed
    echo -e "${RED}Failed to implement $spec after $MAX_RETRIES attempts${NC}"
    update_progress "$spec" "failed"
    restore_checkpoint
    return 1
}

# Find specs to implement
find_specs() {
    local spec_pattern="${1:-*.md}"
    find "$SPEC_DIR" -name "$spec_pattern" -type f | sort
}

# Generate progress report
generate_report() {
    local report_file="$LOG_DIR/report-$(date +%Y%m%d-%H%M%S).md"
    
    echo "# Automated Implementation Report" > "$report_file"
    echo "Generated: $(date)" >> "$report_file"
    echo "" >> "$report_file"
    
    local completed=$(jq -r '.completed_specs | length' "$PROGRESS_FILE")
    local failed=$(jq -r '.failed_specs | length' "$PROGRESS_FILE")
    local total=$(jq -r '.total_specs' "$PROGRESS_FILE")
    
    echo "## Summary" >> "$report_file"
    echo "- Total specs: $total" >> "$report_file"
    echo "- Completed: $completed" >> "$report_file"
    echo "- Failed: $failed" >> "$report_file"
    echo "- Success rate: $(( completed * 100 / total ))%" >> "$report_file"
    echo "" >> "$report_file"
    
    echo "## Completed Specs" >> "$report_file"
    jq -r '.completed_specs[] | "- \(.)"' "$PROGRESS_FILE" >> "$report_file"
    echo "" >> "$report_file"
    
    echo "## Failed Specs" >> "$report_file"
    jq -r '.failed_specs[] | "- \(.)"' "$PROGRESS_FILE" >> "$report_file"
    
    echo -e "${GREEN}Report generated: $report_file${NC}"
}

# Main execution
main() {
    echo -e "${GREEN}=== Git Good Automated Spec Implementation ===${NC}"
    echo -e "${BLUE}This script will autonomously implement specs with validation loops${NC}"
    echo ""
    
    # Initialize progress tracking
    init_progress
    
    # Get list of specs
    local specs=()
    if [[ $# -gt 0 ]]; then
        # Use provided spec patterns
        for pattern in "$@"; do
            while IFS= read -r spec; do
                specs+=("$spec")
            done < <(find_specs "$pattern")
        done
    else
        # Find all specs
        while IFS= read -r spec; do
            specs+=("$spec")
        done < <(find_specs)
    fi
    
    # Update total count
    local temp_file=$(mktemp)
    jq --arg total "${#specs[@]}" '.total_specs = ($total | tonumber)' "$PROGRESS_FILE" > "$temp_file" && mv "$temp_file" "$PROGRESS_FILE"
    
    echo -e "${BLUE}Found ${#specs[@]} specs to implement${NC}"
    echo -e "${YELLOW}Starting in 5 seconds... (Ctrl+C to cancel)${NC}"
    sleep 5
    
    # Process each spec
    local failed_count=0
    for spec in "${specs[@]}"; do
        # Skip if already completed
        if jq -e --arg spec "$spec" '.completed_specs | contains([$spec])' "$PROGRESS_FILE" > /dev/null; then
            echo -e "${YELLOW}Skipping already completed: $spec${NC}"
            continue
        fi
        
        # Skip if in failed list (unless --retry flag)
        if [[ ! "${RETRY_FAILED:-}" == "true" ]] && jq -e --arg spec "$spec" '.failed_specs | contains([$spec])' "$PROGRESS_FILE" > /dev/null; then
            echo -e "${YELLOW}Skipping previously failed: $spec${NC}"
            continue
        fi
        
        # Implement the spec
        if implement_spec "$spec"; then
            echo -e "${GREEN}✓ Successfully implemented: $spec${NC}"
        else
            echo -e "${RED}✗ Failed to implement: $spec${NC}"
            ((failed_count++))
            
            # Optional: Stop on first failure
            if [[ "${STOP_ON_FAILURE:-}" == "true" ]]; then
                echo -e "${RED}Stopping due to failure (STOP_ON_FAILURE=true)${NC}"
                break
            fi
        fi
        
        # Brief pause between specs
        echo -e "${BLUE}Pausing before next spec...${NC}"
        sleep 3
    done
    
    # Generate final report
    generate_report
    
    # Show summary
    echo ""
    echo -e "${GREEN}=== Implementation Complete ===${NC}"
    echo -e "Failed specs: $failed_count"
    echo -e "Check logs in: $LOG_DIR"
    
    # Exit with error if any specs failed
    [[ $failed_count -eq 0 ]]
}

# Handle script arguments
case "${1:-}" in
    --help|-h)
        echo "Usage: $0 [spec-patterns...]"
        echo ""
        echo "Options:"
        echo "  --help, -h       Show this help message"
        echo "  --retry-failed   Retry previously failed specs"
        echo "  --stop-on-fail   Stop on first failure"
        echo ""
        echo "Environment variables:"
        echo "  CLAUDE_CMD       Claude command (default: claude)"
        echo "  RETRY_FAILED     Retry failed specs (true/false)"
        echo "  STOP_ON_FAILURE  Stop on first failure (true/false)"
        echo ""
        echo "Examples:"
        echo "  $0                          # Implement all specs"
        echo "  $0 'foundation/*.md'        # Implement foundation specs"
        echo "  $0 'storage/*' 'perf/*'     # Implement multiple patterns"
        echo "  RETRY_FAILED=true $0        # Retry failed specs"
        exit 0
        ;;
    --retry-failed)
        export RETRY_FAILED=true
        shift
        main "$@"
        ;;
    --stop-on-fail)
        export STOP_ON_FAILURE=true
        shift
        main "$@"
        ;;
    *)
        main "$@"
        ;;
esac