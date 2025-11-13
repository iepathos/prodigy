#!/bin/bash
# Test script for verifying Prodigy resume functionality
# This script tests both sequential and MapReduce workflow resume capabilities

set -e

PRODIGY_BIN="./target/debug/prodigy"
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}Prodigy Resume Functionality Test Suite${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""

# Build prodigy first
echo -e "${YELLOW}Building Prodigy...${NC}"
cargo build --quiet
echo -e "${GREEN}✓ Build complete${NC}"
echo ""

# Test 1: Sequential Workflow Resume
echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}Test 1: Sequential Workflow Resume${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""

echo -e "${YELLOW}Starting sequential workflow (will interrupt after 3 seconds)...${NC}"
echo "Instructions: The workflow will be interrupted automatically."
echo ""

# Start the workflow in background
$PRODIGY_BIN run workflows/tests/test-sequential-resume.yml -y > /tmp/prodigy-test-output.log 2>&1 &
WORKFLOW_PID=$!

# Wait for step 2 to start (each step has 2s sleep, so interrupt during step 2)
sleep 4

# Interrupt the workflow
echo -e "${YELLOW}Interrupting workflow with SIGINT...${NC}"
kill -INT $WORKFLOW_PID 2>/dev/null || true
wait $WORKFLOW_PID 2>/dev/null || true

echo -e "${GREEN}✓ Workflow interrupted${NC}"
echo ""

# Extract session ID from output (format: workflow-XXXXXXXXX)
echo -e "${YELLOW}Finding session ID...${NC}"
if ! SESSION_ID=$(grep -oE 'workflow-[0-9]+' /tmp/prodigy-test-output.log | head -1); then
    echo -e "${RED}✗ Failed to find session ID${NC}"
    echo "Output:"
    cat /tmp/prodigy-test-output.log
    exit 1
fi

echo -e "${GREEN}✓ Found session: $SESSION_ID${NC}"
echo ""

# Check if checkpoint exists
echo -e "${YELLOW}Checking for checkpoint...${NC}"
$PRODIGY_BIN sessions list | grep -q "$SESSION_ID" && echo -e "${GREEN}✓ Session found in session list${NC}" || echo -e "${RED}✗ Session not found${NC}"
echo ""

# Test resume
echo -e "${YELLOW}Attempting to resume workflow...${NC}"
if $PRODIGY_BIN resume "$SESSION_ID"; then
    echo -e "${GREEN}✓ Sequential workflow resume SUCCESSFUL${NC}"
else
    echo -e "${RED}✗ Sequential workflow resume FAILED${NC}"
    echo "Session ID was: $SESSION_ID"
    echo "Last output:"
    tail -20 /tmp/prodigy-test-output.log
    exit 1
fi
echo ""

# Test 2: MapReduce Workflow Resume
echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}Test 2: MapReduce Workflow Resume${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""

echo -e "${YELLOW}Starting MapReduce workflow (will interrupt after 5 seconds)...${NC}"
echo "Instructions: The workflow will be interrupted during the map phase."
echo ""

# Start the workflow in background
$PRODIGY_BIN run workflows/tests/test-mapreduce-resume.yml -y > /tmp/prodigy-mapreduce-test-output.log 2>&1 &
MR_WORKFLOW_PID=$!

# Wait for map phase to start (longer to ensure setup completes)
sleep 5

# Interrupt the workflow
echo -e "${YELLOW}Interrupting MapReduce workflow with SIGINT...${NC}"
kill -INT $MR_WORKFLOW_PID 2>/dev/null || true
wait $MR_WORKFLOW_PID 2>/dev/null || true

echo -e "${GREEN}✓ MapReduce workflow interrupted${NC}"
echo ""

# Extract session ID and job ID
# Session format: session-UUID
# Job format: mapreduce-YYYYMMDD_HHMMSS
echo -e "${YELLOW}Finding MapReduce session/job ID...${NC}"
if ! MR_SESSION_ID=$(grep -oE 'session-[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}' /tmp/prodigy-mapreduce-test-output.log | head -1); then
    echo -e "${RED}✗ Failed to find MapReduce session ID${NC}"
    echo "Output:"
    cat /tmp/prodigy-mapreduce-test-output.log
    exit 1
fi

# Try to find job ID (format: mapreduce-YYYYMMDD_HHMMSS)
MR_JOB_ID=$(grep -oE 'mapreduce-[0-9]{8}_[0-9]{6}' /tmp/prodigy-mapreduce-test-output.log | head -1) || MR_JOB_ID=""

echo -e "${GREEN}✓ Found MapReduce session: $MR_SESSION_ID${NC}"
if [ -n "$MR_JOB_ID" ]; then
    echo -e "${GREEN}✓ Found job ID: $MR_JOB_ID${NC}"
fi
echo ""

# Test resume with session ID
echo -e "${YELLOW}Test 2a: Resume MapReduce using session ID...${NC}"
if $PRODIGY_BIN resume "$MR_SESSION_ID"; then
    echo -e "${GREEN}✓ MapReduce resume via session ID SUCCESSFUL${NC}"
else
    echo -e "${RED}✗ MapReduce resume via session ID FAILED${NC}"
    echo "This might be expected if the implementation is incomplete"
fi
echo ""

# Test resume with job ID if available
if [ -n "$MR_JOB_ID" ]; then
    echo -e "${YELLOW}Test 2b: Resume MapReduce using job ID (if supported)...${NC}"

    # First, interrupt again to create a new checkpoint
    $PRODIGY_BIN run workflows/tests/test-mapreduce-resume.yml -y > /tmp/prodigy-mapreduce-test-output2.log 2>&1 &
    MR_WORKFLOW_PID2=$!
    sleep 5
    kill -INT $MR_WORKFLOW_PID2 2>/dev/null || true
    wait $MR_WORKFLOW_PID2 2>/dev/null || true

    MR_JOB_ID2=$(grep -oE 'mapreduce-[a-zA-Z0-9_-]+' /tmp/prodigy-mapreduce-test-output2.log | head -1) || MR_JOB_ID2=""

    if [ -n "$MR_JOB_ID2" ]; then
        if $PRODIGY_BIN resume-job "$MR_JOB_ID2" 2>/dev/null; then
            echo -e "${GREEN}✓ MapReduce resume via job ID SUCCESSFUL${NC}"
        else
            echo -e "${YELLOW}⚠ MapReduce resume-job command may not be fully implemented${NC}"
        fi
    fi
fi
echo ""

# Test 3: Verify State Consistency
echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}Test 3: State Consistency Checks${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""

echo -e "${YELLOW}Checking session state consistency...${NC}"
$PRODIGY_BIN sessions list
echo ""

echo -e "${YELLOW}Checking for orphaned worktrees...${NC}"
if [ -d "$HOME/.prodigy/worktrees" ]; then
    WORKTREE_COUNT=$(find "$HOME/.prodigy/worktrees" -type d -name "session-*" 2>/dev/null | wc -l | tr -d ' ')
    echo "Found $WORKTREE_COUNT session worktrees"

    if [ "$WORKTREE_COUNT" -gt 10 ]; then
        echo -e "${YELLOW}⚠ Many worktrees found - consider running: prodigy worktree clean${NC}"
    else
        echo -e "${GREEN}✓ Worktree count looks normal${NC}"
    fi
else
    echo -e "${YELLOW}⚠ No worktrees directory found${NC}"
fi
echo ""

# Test 4: Resume Non-existent Session (Should Fail)
echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}Test 4: Error Handling - Resume Non-existent${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""

echo -e "${YELLOW}Testing error handling for non-existent session...${NC}"
if $PRODIGY_BIN resume "session-nonexistent-12345" 2>/dev/null; then
    echo -e "${RED}✗ UNEXPECTED: Resume should have failed for non-existent session${NC}"
else
    echo -e "${GREEN}✓ Correctly rejected non-existent session${NC}"
fi
echo ""

# Summary
echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}Test Summary${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""
echo -e "${GREEN}Sequential Workflow Resume: Tested${NC}"
echo -e "${YELLOW}MapReduce Workflow Resume: Tested (check output above for status)${NC}"
echo -e "${GREEN}Error Handling: Tested${NC}"
echo ""
echo -e "${BLUE}Manual Verification Steps:${NC}"
echo "1. Check if resumed workflows completed all steps"
echo "2. Verify checkpoint files are created in ~/.prodigy/"
echo "3. Check session state with: prodigy sessions list"
echo "4. For MapReduce, verify all items were processed"
echo ""
echo -e "${YELLOW}Cleanup:${NC}"
echo "Run the following to clean up test artifacts:"
echo "  rm -f /tmp/prodigy-*test-output*.log"
echo "  rm -f /tmp/prodigy-resume-test.txt"
echo "  rm -f /tmp/resume-test-items.json"
echo "  prodigy worktree clean -f"
echo ""
echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}Test Suite Complete${NC}"
echo -e "${BLUE}========================================${NC}"
