#!/usr/bin/env python3
"""
Validate debtmap improvements by comparing before/after states.
"""
import json
import sys
from pathlib import Path
from typing import Dict, List, Any, Optional

def load_debtmap(path: str) -> Optional[Dict]:
    """Load and parse debtmap JSON file."""
    try:
        with open(path, 'r') as f:
            return json.load(f)
    except Exception as e:
        print(f"Error loading {path}: {e}", file=sys.stderr)
        return None

def get_item_key(item: Dict) -> str:
    """Generate unique key for a debt item."""
    location = item.get('location', {})
    return f"{location.get('file', '')}:{location.get('function', '')}:{location.get('line', 0)}"

def get_score(item: Dict) -> Optional[float]:
    """Extract unified score from item."""
    unified_score = item.get('unified_score')
    if unified_score and isinstance(unified_score, dict):
        return unified_score.get('final_score')
    return None

def calculate_metrics(debtmap: Dict) -> Dict:
    """Calculate summary metrics from debtmap."""
    items = debtmap.get('items', [])

    # Filter items with valid scores
    scored_items = [i for i in items if get_score(i) is not None]

    # Count high priority items (score >= 80 for unified_score)
    high_priority = len([i for i in scored_items if get_score(i) >= 80])

    # Calculate average score
    avg_score = sum(get_score(i) for i in scored_items) / len(scored_items) if scored_items else 0

    # Calculate average complexity
    complexity_items = [i for i in items if i.get('cyclomatic_complexity') is not None]
    avg_complexity = sum(i['cyclomatic_complexity'] for i in complexity_items) / len(complexity_items) if complexity_items else 0

    return {
        'total_items': len(items),
        'scored_items': len(scored_items),
        'high_priority_items': high_priority,
        'average_score': round(avg_score, 2),
        'average_complexity': round(avg_complexity, 2)
    }

def compare_debtmaps(before: Dict, after: Dict) -> Dict:
    """Compare two debtmap states and calculate improvement."""
    before_metrics = calculate_metrics(before)
    after_metrics = calculate_metrics(after)

    # Build item lookup tables
    before_items = {get_item_key(item): item for item in before.get('items', [])}
    after_items = {get_item_key(item): item for item in after.get('items', [])}

    # Find resolved, improved, and new items
    resolved_keys = set(before_items.keys()) - set(after_items.keys())
    new_keys = set(after_items.keys()) - set(before_items.keys())
    common_keys = set(before_items.keys()) & set(after_items.keys())

    # Analyze resolved items
    resolved_high_priority = sum(
        1 for key in resolved_keys
        if get_score(before_items[key]) and get_score(before_items[key]) >= 80
    )

    # Analyze improved items
    improved_items = []
    for key in common_keys:
        before_score = get_score(before_items[key])
        after_score = get_score(after_items[key])

        if before_score and after_score and after_score < before_score:
            improved_items.append({
                'key': key,
                'before_score': before_score,
                'after_score': after_score,
                'improvement': before_score - after_score
            })

    # Analyze new critical items (regression)
    new_critical = sum(
        1 for key in new_keys
        if get_score(after_items[key]) and get_score(after_items[key]) >= 80
    )

    # Calculate improvement score
    total_high_priority_before = before_metrics['high_priority_items']
    resolved_high_priority_pct = (resolved_high_priority / total_high_priority_before * 100) if total_high_priority_before > 0 else 0

    overall_score_improvement = ((before_metrics['average_score'] - after_metrics['average_score']) / before_metrics['average_score'] * 100) if before_metrics['average_score'] > 0 else 0

    complexity_reduction = ((before_metrics['average_complexity'] - after_metrics['average_complexity']) / before_metrics['average_complexity'] * 100) if before_metrics['average_complexity'] > 0 else 0

    no_new_critical = 100 if new_critical == 0 else max(0, 100 - (new_critical * 25))

    # Weighted average
    improvement_score = (
        resolved_high_priority_pct * 0.4 +
        max(0, overall_score_improvement) * 0.3 +
        max(0, complexity_reduction) * 0.2 +
        no_new_critical * 0.1
    )

    return {
        'improvement_score': round(improvement_score, 1),
        'before_metrics': before_metrics,
        'after_metrics': after_metrics,
        'resolved_items': len(resolved_keys),
        'resolved_high_priority': resolved_high_priority,
        'improved_items': len(improved_items),
        'new_items': len(new_keys),
        'new_critical': new_critical,
        'top_improvements': sorted(improved_items, key=lambda x: x['improvement'], reverse=True)[:5]
    }

def identify_gaps(comparison: Dict, after: Dict) -> Dict:
    """Identify specific improvement gaps."""
    gaps = {}
    after_items = after.get('items', [])

    # Find remaining critical items
    critical_items = [
        item for item in after_items
        if get_score(item) and get_score(item) >= 80
    ]

    if critical_items:
        # Pick the highest priority remaining item
        top_critical = max(critical_items, key=lambda x: get_score(x) or 0)
        location = top_critical.get('location', {})
        recommendation = top_critical.get('recommendation', {})

        gaps['critical_debt_remaining'] = {
            'description': f"High-priority debt item still present: {location.get('function', 'unknown')}",
            'location': f"{location.get('file', 'unknown')}:{location.get('function', 'unknown')}:{location.get('line', 0)}",
            'severity': 'high',
            'suggested_fix': recommendation.get('primary_action', 'Apply functional programming patterns to reduce complexity'),
            'current_score': get_score(top_critical)
        }

    # Check for insufficient refactoring (complex items still above threshold)
    complex_items = [
        item for item in after_items
        if item.get('cyclomatic_complexity') and item.get('cyclomatic_complexity') > 10
    ]

    if complex_items and comparison['improvement_score'] < 75:
        top_complex = max(complex_items, key=lambda x: x.get('cyclomatic_complexity', 0))
        location = top_complex.get('location', {})
        gaps['insufficient_refactoring'] = {
            'description': f"Function complexity still above threshold: {location.get('function', 'unknown')}",
            'location': f"{location.get('file', 'unknown')}:{location.get('function', 'unknown')}:{location.get('line', 0)}",
            'severity': 'medium',
            'suggested_fix': 'Extract helper functions using pure functional patterns',
            'current_complexity': top_complex.get('cyclomatic_complexity'),
            'target_complexity': 8
        }

    # Check for regression
    if comparison['new_critical'] > 0:
        gaps['regression_detected'] = {
            'description': f"New critical issues introduced: {comparison['new_critical']} items",
            'severity': 'critical',
            'suggested_fix': 'Review and simplify recently added complexity'
        }

    return gaps

def generate_validation_result(before_path: str, after_path: str, output_path: str) -> int:
    """Generate validation result comparing before/after debtmaps."""
    print("Loading debtmap files...")
    before = load_debtmap(before_path)
    after = load_debtmap(after_path)

    if not before:
        result = {
            'completion_percentage': 0.0,
            'status': 'failed',
            'improvements': [],
            'remaining_issues': [f'Failed to load before file: {before_path}'],
            'gaps': {},
            'raw_output': 'Unable to load before debtmap'
        }
    elif not after:
        result = {
            'completion_percentage': 0.0,
            'status': 'failed',
            'improvements': [],
            'remaining_issues': [f'Failed to load after file: {after_path}'],
            'gaps': {},
            'raw_output': 'Unable to load after debtmap'
        }
    else:
        print("Comparing debtmap states...")
        comparison = compare_debtmaps(before, after)
        gaps = identify_gaps(comparison, after)

        # Build improvements list
        improvements = []
        if comparison['resolved_high_priority'] > 0:
            improvements.append(f"Resolved {comparison['resolved_high_priority']} high-priority debt items")
        if comparison['improved_items'] > 0:
            improvements.append(f"Improved {comparison['improved_items']} existing debt items")

        before_m = comparison['before_metrics']
        after_m = comparison['after_metrics']

        if after_m['average_complexity'] < before_m['average_complexity']:
            reduction = round((1 - after_m['average_complexity'] / before_m['average_complexity']) * 100, 1)
            improvements.append(f"Reduced average complexity by {reduction}%")

        if after_m['average_score'] < before_m['average_score']:
            improvements.append(f"Reduced average debt score from {before_m['average_score']} to {after_m['average_score']}")

        # Build remaining issues list
        remaining_issues = []
        if after_m['high_priority_items'] > 0:
            remaining_issues.append(f"{after_m['high_priority_items']} critical debt items still present")

        if comparison['new_critical'] > 0:
            remaining_issues.append(f"{comparison['new_critical']} new critical issues introduced")

        # Determine status
        score = comparison['improvement_score']
        if score >= 75:
            status = 'complete'
        else:
            status = 'incomplete'

        result = {
            'completion_percentage': score,
            'status': status,
            'improvements': improvements,
            'remaining_issues': remaining_issues,
            'gaps': gaps,
            'before_summary': {
                'total_items': before_m['total_items'],
                'high_priority_items': before_m['high_priority_items'],
                'average_score': before_m['average_score']
            },
            'after_summary': {
                'total_items': after_m['total_items'],
                'high_priority_items': after_m['high_priority_items'],
                'average_score': after_m['average_score']
            }
        }

    # Write result to output file
    print(f"Writing validation result to {output_path}")
    output_file = Path(output_path)
    output_file.parent.mkdir(parents=True, exist_ok=True)

    with open(output_file, 'w') as f:
        json.dump(result, f, indent=2)

    print(f"Validation complete: {result['completion_percentage']:.1f}% improvement")
    print(f"Status: {result['status']}")

    return 0

def main():
    if len(sys.argv) < 5:
        print("Usage: validate_debtmap.py --before <before.json> --after <after.json> --output <output.json>", file=sys.stderr)
        return 1

    # Parse arguments
    args = {}
    i = 1
    while i < len(sys.argv):
        if sys.argv[i] in ['--before', '--after', '--output']:
            if i + 1 < len(sys.argv):
                args[sys.argv[i][2:]] = sys.argv[i + 1]
                i += 2
            else:
                print(f"Missing value for {sys.argv[i]}", file=sys.stderr)
                return 1
        else:
            i += 1

    before_path = args.get('before', '.prodigy/debtmap-before.json')
    after_path = args.get('after', '.prodigy/debtmap-after.json')
    output_path = args.get('output', '.prodigy/debtmap-validation.json')

    return generate_validation_result(before_path, after_path, output_path)

if __name__ == '__main__':
    sys.exit(main())