#!/usr/bin/env python3
"""
Validate Debtmap Improvement Script

Compares before and after debtmap JSON output to validate that technical debt
improvements have been made. Outputs validation result as JSON.
"""

import json
import sys
from pathlib import Path
from typing import Dict, List, Tuple, Any

def parse_arguments(args: List[str]) -> Tuple[str, str, str]:
    """Parse command line arguments."""
    before_file = ""
    after_file = ""
    output_file = ".prodigy/debtmap-validation.json"

    i = 0
    while i < len(args):
        if args[i] == "--before" and i + 1 < len(args):
            before_file = args[i + 1]
            i += 2
        elif args[i] == "--after" and i + 1 < len(args):
            after_file = args[i + 1]
            i += 2
        elif args[i] == "--output" and i + 1 < len(args):
            output_file = args[i + 1]
            i += 2
        else:
            print(f"Unknown parameter: {args[i]}", file=sys.stderr)
            i += 1

    if not before_file or not after_file:
        print("Error: --before and --after parameters are required", file=sys.stderr)
        print("Usage: validate_debtmap_improvement.py --before <before-json> --after <after-json> [--output <output-json>]", file=sys.stderr)
        sys.exit(1)

    return before_file, after_file, output_file

def load_debtmap(filepath: str) -> Dict:
    """Load and parse debtmap JSON file."""
    try:
        with open(filepath, 'r') as f:
            return json.load(f)
    except FileNotFoundError:
        print(f"Error: File not found: {filepath}", file=sys.stderr)
        return {"items": []}
    except json.JSONDecodeError as e:
        print(f"Error: Invalid JSON in {filepath}: {e}", file=sys.stderr)
        return {"items": []}

def get_item_key(item: Dict) -> Tuple[str, str, int]:
    """Extract unique key for a debt item."""
    loc = item["location"]
    return (loc["file"], loc["function"], loc["line"])

def calculate_metrics(before_data: Dict, after_data: Dict) -> Dict:
    """Calculate improvement metrics between before and after states."""
    before_items = {get_item_key(item): item for item in before_data.get("items", [])}
    after_items = {get_item_key(item): item for item in after_data.get("items", [])}

    # Identify changes
    resolved_keys = set(before_items.keys()) - set(after_items.keys())
    new_keys = set(after_items.keys()) - set(before_items.keys())
    common_keys = set(before_items.keys()) & set(after_items.keys())

    resolved_items = [before_items[k] for k in resolved_keys]
    new_items = [after_items[k] for k in new_keys]

    # Calculate improvements and regressions
    improved_items = []
    regressed_items = []
    unchanged_critical = []

    for key in common_keys:
        before_score = before_items[key]["unified_score"]["final_score"]
        after_score = after_items[key]["unified_score"]["final_score"]

        if after_score < before_score - 0.1:  # Improved (with small tolerance)
            improved_items.append((key, before_items[key], after_items[key], before_score, after_score))
        elif after_score > before_score + 0.1:  # Regressed
            regressed_items.append((key, before_items[key], after_items[key], before_score, after_score))
        elif before_score >= 80:  # Unchanged critical item
            unchanged_critical.append((key, before_items[key], after_items[key]))

    # Count by priority
    before_critical = sum(1 for item in before_items.values() if item["unified_score"]["final_score"] >= 80)
    after_critical = sum(1 for item in after_items.values() if item["unified_score"]["final_score"] >= 80)

    resolved_critical = sum(1 for item in resolved_items if item["unified_score"]["final_score"] >= 80)
    new_critical = sum(1 for item in new_items if item["unified_score"]["final_score"] >= 80)

    # Calculate average scores
    before_avg = sum(item["unified_score"]["final_score"] for item in before_items.values()) / len(before_items) if before_items else 0
    after_avg = sum(item["unified_score"]["final_score"] for item in after_items.values()) / len(after_items) if after_items else 0

    return {
        "before_items": before_items,
        "after_items": after_items,
        "resolved_items": resolved_items,
        "resolved_critical": resolved_critical,
        "new_items": new_items,
        "new_critical": new_critical,
        "improved_items": improved_items,
        "regressed_items": regressed_items,
        "unchanged_critical": unchanged_critical,
        "before_critical": before_critical,
        "after_critical": after_critical,
        "before_avg": before_avg,
        "after_avg": after_avg,
    }

def calculate_improvement_score(metrics: Dict) -> float:
    """Calculate overall improvement score based on weighted factors."""
    before_critical = metrics["before_critical"]
    resolved_critical = metrics["resolved_critical"]
    new_critical = metrics["new_critical"]
    before_total = len(metrics["before_items"])
    resolved_total = len(metrics["resolved_items"])
    before_avg = metrics["before_avg"]
    after_avg = metrics["after_avg"]

    # Factor 1: Resolved high-priority items (40% weight)
    if before_critical > 0:
        resolved_high_priority_pct = (resolved_critical / before_critical) * 100
    else:
        resolved_high_priority_pct = 100 if new_critical == 0 else 0

    # Factor 2: Overall score improvement (30% weight)
    if before_avg > 0:
        overall_improvement_pct = max(0, ((before_avg - after_avg) / before_avg) * 100)
    else:
        overall_improvement_pct = 100

    # Factor 3: Items resolved (20% weight)
    if before_total > 0:
        items_resolved_pct = (resolved_total / before_total) * 100
    else:
        items_resolved_pct = 100

    # Factor 4: No new critical debt (10% weight)
    no_new_critical_pct = max(0, 100 - (new_critical * 25))  # Each new critical item costs 25%

    # Weighted average
    improvement_score = (
        resolved_high_priority_pct * 0.4 +
        overall_improvement_pct * 0.3 +
        items_resolved_pct * 0.2 +
        no_new_critical_pct * 0.1
    )

    return min(100, max(0, improvement_score))

def identify_gaps(metrics: Dict, improvement_score: float) -> Dict:
    """Identify specific gaps in the improvement."""
    gaps = {}

    # Gap 1: Critical debt remaining
    if metrics["unchanged_critical"]:
        key, before_item, after_item = metrics["unchanged_critical"][0]
        loc = before_item["location"]
        gaps["critical_debt_remaining"] = {
            "description": f"High-priority debt item still present: {loc['function']}",
            "location": f"{loc['file']}:{loc['function']}:{loc['line']}",
            "severity": "high",
            "suggested_fix": before_item["recommendation"]["primary_action"],
            "current_score": after_item["unified_score"]["final_score"]
        }

    # Gap 2: Regression detected
    if metrics["new_critical"] > 0:
        gaps["regression_detected"] = {
            "description": f"New critical issues introduced: {metrics['new_critical']} items",
            "severity": "critical",
            "suggested_fix": "Review and simplify recently added complexity"
        }

    # Gap 3: Insufficient refactoring - check if improved items still have high complexity
    high_complexity_items = []
    for key, before_item, after_item, before_score, after_score in metrics["improved_items"]:
        if after_score >= 60:  # Still high score after improvement
            high_complexity_items.append((key, before_item, after_item, before_score, after_score))

    if high_complexity_items:
        key, before_item, after_item, before_score, after_score = high_complexity_items[0]
        loc = after_item["location"]

        # Extract complexity from debt_type
        current_complexity = 0
        if "TestingGap" in after_item.get("debt_type", {}):
            current_complexity = after_item["debt_type"]["TestingGap"].get("cyclomatic", 0)
        elif "ComplexityDebt" in after_item.get("debt_type", {}):
            current_complexity = after_item["debt_type"]["ComplexityDebt"].get("cyclomatic", 0)

        gaps["insufficient_refactoring"] = {
            "description": f"Function complexity still above threshold: {loc['function']}",
            "location": f"{loc['file']}:{loc['function']}:{loc['line']}",
            "severity": "medium",
            "suggested_fix": "Extract helper functions using pure functional patterns",
            "current_complexity": current_complexity,
            "target_complexity": 8
        }

    return gaps

def generate_improvements_list(metrics: Dict) -> List[str]:
    """Generate list of improvements made."""
    improvements = []

    if metrics["resolved_critical"] > 0:
        improvements.append(f"Resolved {metrics['resolved_critical']} high-priority debt items")

    if len(metrics["improved_items"]) > 0:
        improvements.append(f"Improved {len(metrics['improved_items'])} existing debt items")

    if metrics["before_avg"] > metrics["after_avg"]:
        pct_change = ((metrics["before_avg"] - metrics["after_avg"]) / metrics["before_avg"]) * 100
        improvements.append(f"Reduced average debt score by {pct_change:.1f}%")

    return improvements

def generate_remaining_issues(metrics: Dict) -> List[str]:
    """Generate list of remaining issues."""
    issues = []

    if metrics["after_critical"] > 0:
        issues.append(f"{metrics['after_critical']} critical debt items still present")

    if metrics["new_critical"] > 0:
        issues.append(f"{metrics['new_critical']} new critical issues introduced")

    if len(metrics["regressed_items"]) > 0:
        issues.append(f"{len(metrics['regressed_items'])} items got worse")

    return issues

def validate_improvement(before_file: str, after_file: str, output_file: str):
    """Main validation function."""
    import os
    is_automation = (
        os.environ.get("PRODIGY_AUTOMATION") == "true" or
        os.environ.get("PRODIGY_VALIDATION") == "true"
    )

    if not is_automation:
        print(f"Loading debtmap data...")
        print(f"  Before: {before_file}")
        print(f"  After: {after_file}")

    before_data = load_debtmap(before_file)
    after_data = load_debtmap(after_file)

    # Handle error cases
    if not before_data.get("items") and not after_data.get("items"):
        result = {
            "completion_percentage": 0.0,
            "status": "failed",
            "improvements": [],
            "remaining_issues": ["Unable to compare: both debtmap files are empty or invalid"],
            "gaps": {},
            "before_summary": {"total_items": 0, "high_priority_items": 0, "average_score": 0},
            "after_summary": {"total_items": 0, "high_priority_items": 0, "average_score": 0}
        }

        output_path = Path(output_file)
        output_path.parent.mkdir(parents=True, exist_ok=True)
        with open(output_file, 'w') as f:
            json.dump(result, f, indent=2)

        if not is_automation:
            print(f"\n✗ Validation failed - invalid input files")
            print(f"  Output written to: {output_file}")
        return result

    if not is_automation:
        print(f"Calculating improvement metrics...")
    metrics = calculate_metrics(before_data, after_data)

    if not is_automation:
        print(f"Computing improvement score...")
    improvement_score = calculate_improvement_score(metrics)

    if not is_automation:
        print(f"Identifying gaps...")
    gaps = identify_gaps(metrics, improvement_score)

    improvements = generate_improvements_list(metrics)
    remaining_issues = generate_remaining_issues(metrics)

    # Determine status
    if improvement_score >= 75:
        status = "complete"
    else:
        status = "incomplete"

    # Build validation result
    result = {
        "completion_percentage": round(improvement_score, 1),
        "status": status,
        "improvements": improvements,
        "remaining_issues": remaining_issues,
        "gaps": gaps,
        "before_summary": {
            "total_items": len(metrics["before_items"]),
            "high_priority_items": metrics["before_critical"],
            "average_score": round(metrics["before_avg"], 2)
        },
        "after_summary": {
            "total_items": len(metrics["after_items"]),
            "high_priority_items": metrics["after_critical"],
            "average_score": round(metrics["after_avg"], 2)
        }
    }

    # Write to output file
    output_path = Path(output_file)
    output_path.parent.mkdir(parents=True, exist_ok=True)

    with open(output_file, 'w') as f:
        json.dump(result, f, indent=2)

    if not is_automation:
        print(f"\n✓ Validation complete!")
        print(f"  Improvement score: {improvement_score:.1f}%")
        print(f"  Status: {status}")
        print(f"  Output written to: {output_file}")

    return result

# Main execution
if __name__ == "__main__":
    before_file, after_file, output_file = parse_arguments(sys.argv[1:])
    result = validate_improvement(before_file, after_file, output_file)

    # Exit with 0 always - the validation result is in the JSON file
    sys.exit(0)
