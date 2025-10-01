#!/bin/bash
# Validate Debtmap Improvement Command
# Compares before and after debtmap JSON to validate technical debt improvements

set -euo pipefail

# Run the validation script with all provided arguments
python3 .claude/scripts/validate_debtmap_improvement.py "$@"
