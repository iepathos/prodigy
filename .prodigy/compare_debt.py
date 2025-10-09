#!/usr/bin/env python3
"""Compare before and after debtmap results to analyze technical debt improvements."""

import json
import sys
from collections import defaultdict
from pathlib import Path

def load_json(path):
    """Load and parse a JSON file."""
    with open(path, 'r') as f:
        return json.load(f)

def get_item_key(item):
    """Generate a unique key for a debt item."""
    # Handle both 'File' and 'Function' item types
    if 'File' in item:
        file_data = item['File']
        path = file_data.get('metrics', {}).get('path', '')
        return ('file', path, 0)
    elif 'Function' in item:
        func_data = item['Function']
        location = func_data.get('location', {})
        return ('function', location.get('file', ''), location.get('name', ''))
    return ('unknown', '', '')

def get_score(item):
    """Extract score from either File or Function format."""
    if 'File' in item:
        return item['File'].get('score', 0)
    elif 'Function' in item:
        return item['Function'].get('score', 0)
    return 0

def get_category(item):
    """Extract category from either File or Function format."""
    if 'File' in item:
        return 'File-level'
    elif 'Function' in item:
        func_data = item['Function']
        # debt_type is a dict with category names as keys
        debt_type = func_data.get('debt_type', {})
        if isinstance(debt_type, dict) and debt_type:
            # Return the first category name
            return list(debt_type.keys())[0]
        return 'Function-level'
    return 'unknown'

def get_item_desc(item):
    """Get a human-readable description of the item."""
    if 'File' in item:
        path = item['File'].get('metrics', {}).get('path', 'unknown')
        return path
    elif 'Function' in item:
        loc = item['Function'].get('location', {})
        file = loc.get('file', 'unknown')
        name = loc.get('name', 'unknown')
        return f"{file}::{name}"
    return 'unknown'

def analyze_improvements(before_data, after_data, map_results, successful, failed, total):
    """Analyze debt improvements between before and after."""

    # Extract items from both datasets
    before_items = {get_item_key(item): item for item in before_data.get('items', [])}
    after_items = {get_item_key(item): item for item in after_data.get('items', [])}

    # Calculate overall metrics
    total_before = sum(get_score(item) for item in before_items.values())
    total_after = sum(get_score(item) for item in after_items.values())

    # Analyze changes
    resolved = set(before_items.keys()) - set(after_items.keys())
    new_items = set(after_items.keys()) - set(before_items.keys())
    improved = []
    regressed = []

    for key in set(before_items.keys()) & set(after_items.keys()):
        before_score = get_score(before_items[key])
        after_score = get_score(after_items[key])

        if after_score < before_score:
            improved.append((key, before_items[key], after_items[key], before_score, after_score))
        elif after_score > before_score:
            regressed.append((key, before_items[key], after_items[key], before_score, after_score))

    # Category analysis
    category_improvements = defaultdict(lambda: {'before': 0, 'after': 0, 'items': 0})

    for key in resolved:
        item = before_items[key]
        category = get_category(item)
        score = get_score(item)
        category_improvements[category]['before'] += score
        category_improvements[category]['items'] += 1

    for key, before_item, after_item, before_score, after_score in improved:
        category = get_category(before_item)
        category_improvements[category]['before'] += before_score
        category_improvements[category]['after'] += after_score
        category_improvements[category]['items'] += 1

    # Generate report
    report_lines = []
    report_lines.append("Technical Debt Improvements:")

    if total_before > 0:
        change_pct = ((total_after - total_before) / total_before) * 100
        report_lines.append(f"- Total debt score: {total_before:.0f} → {total_after:.0f} ({change_pct:+.1f}%)")
    else:
        report_lines.append(f"- Total debt score: {total_before:.0f} → {total_after:.0f}")

    report_lines.append(f"- Items resolved: {len(resolved)} items completely eliminated")
    report_lines.append(f"- Items improved: {len(improved)} items with reduced scores")
    report_lines.append(f"- Overall items: {len(before_items)} → {len(after_items)} ({len(after_items) - len(before_items):+d})")
    report_lines.append("")

    # Category breakdown
    if category_improvements:
        report_lines.append("By category:")
        for category, stats in sorted(category_improvements.items(), key=lambda x: x[1]['before'] - x[1]['after'], reverse=True):
            before = stats['before']
            after = stats['after']
            items = stats['items']
            if before > 0:
                change_pct = ((after - before) / before) * 100
                report_lines.append(f"- {category}: {before:.0f} → {after:.0f} ({change_pct:+.1f}%, {items} items)")
        report_lines.append("")

    # Top improvements
    top_improvements = []

    # Add resolved items
    for key in resolved:
        item = before_items[key]
        score = get_score(item)
        desc = get_item_desc(item)
        top_improvements.append((score, f"{desc}: score {score:.0f} → 0 (resolved)"))

    # Add improved items
    for key, before_item, after_item, before_score, after_score in improved:
        desc = get_item_desc(before_item)
        reduction = before_score - after_score
        pct = -((after_score - before_score) / before_score) * 100 if before_score > 0 else 0
        top_improvements.append((reduction, f"{desc}: score {before_score:.0f} → {after_score:.0f} (-{pct:.1f}%)"))

    # Sort by score reduction and show top 10
    top_improvements.sort(reverse=True, key=lambda x: x[0])
    if top_improvements:
        report_lines.append("Top improvements:")
        for i, (_, desc) in enumerate(top_improvements[:10], 1):
            report_lines.append(f"{i}. {desc}")
        report_lines.append("")

    # Report regressions
    if regressed:
        report_lines.append("⚠️ Regressions detected:")
        for key, before_item, after_item, before_score, after_score in regressed[:5]:
            desc = get_item_desc(before_item)
            pct = ((after_score - before_score) / before_score) * 100 if before_score > 0 else 0
            report_lines.append(f"- {desc}: score {before_score:.0f} → {after_score:.0f} ({pct:+.1f}%)")
        if len(regressed) > 5:
            report_lines.append(f"- ... and {len(regressed) - 5} more regressions")
        report_lines.append("")

    # Report new high-score items
    high_score_new = [(key, after_items[key]) for key in new_items
                      if get_score(after_items[key]) > 50]
    if high_score_new:
        report_lines.append("⚠️ New high-score items introduced:")
        for key, item in high_score_new[:5]:
            desc = get_item_desc(item)
            score = get_score(item)
            report_lines.append(f"- NEW: {desc}: score {score:.0f}")
        if len(high_score_new) > 5:
            report_lines.append(f"- ... and {len(high_score_new) - 5} more new high-score items")
        report_lines.append("")

    return '\n'.join(report_lines)

def main():
    if len(sys.argv) < 7:
        print("Usage: compare_debt.py --before <path> --after <path> --map-results-file <path> --successful <n> --failed <n> --total <n>")
        sys.exit(1)

    # Parse arguments
    args = {}
    i = 1
    while i < len(sys.argv):
        if sys.argv[i].startswith('--'):
            key = sys.argv[i][2:]
            if i + 1 < len(sys.argv):
                args[key] = sys.argv[i + 1]
                i += 2
            else:
                i += 1
        else:
            i += 1

    before_path = args.get('before')
    after_path = args.get('after')
    map_results_path = args.get('map-results-file')
    successful = int(args.get('successful', 0))
    failed = int(args.get('failed', 0))
    total = int(args.get('total', 0))

    # Load data
    before_data = load_json(before_path)
    after_data = load_json(after_path)
    map_results = load_json(map_results_path) if map_results_path and Path(map_results_path).exists() else None

    # Analyze and generate report
    report = analyze_improvements(before_data, after_data, map_results, successful, failed, total)

    # Generate commit message
    commit_msg = f"""fix: eliminate {successful} technical debt items via MapReduce

Processed {total} debt items in parallel:
- Successfully fixed: {successful} items
- Failed to fix: {failed} items

{report}

This commit represents the aggregated work of multiple parallel agents.
"""

    print(commit_msg)

if __name__ == '__main__':
    main()
