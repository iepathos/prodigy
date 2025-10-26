#!/usr/bin/env python3
"""Analyze technical debt improvements between before and after debtmap results."""

import json
import sys
from collections import defaultdict


def load_json(path):
    """Load and parse JSON file."""
    with open(path, 'r') as f:
        return json.load(f)


def get_item_key(item_type, item_data):
    """Create a unique key for a debt item."""
    if item_type == 'File':
        path = item_data.get('metrics', {}).get('path', '')
        return ('File', path)
    elif item_type == 'Function':
        file_path = item_data.get('file', '')
        function_name = item_data.get('function_name', '')
        return ('Function', file_path, function_name)
    return ('Unknown', str(item_data))


def get_debt_score(item_type, item_data):
    """Extract debt score from item."""
    if item_type == 'File':
        return item_data.get('metrics', {}).get('total_complexity', 0)
    elif item_type == 'Function':
        return item_data.get('complexity', 0)
    return 0


def analyze_debt_changes(before_data, after_data, successful, failed, total):
    """Analyze changes between before and after debt data."""

    # Get top-level metrics
    total_before_score = before_data.get('total_debt_score', 0)
    total_after_score = after_data.get('total_debt_score', 0)

    # Extract items
    before_items = before_data.get('items', [])
    after_items = after_data.get('items', [])

    # Create lookup maps
    before_map = {}
    for item in before_items:
        item_type = list(item.keys())[0]
        item_data = item[item_type]
        key = get_item_key(item_type, item_data)
        before_map[key] = (item_type, item_data)

    after_map = {}
    for item in after_items:
        item_type = list(item.keys())[0]
        item_data = item[item_type]
        key = get_item_key(item_type, item_data)
        after_map[key] = (item_type, item_data)

    # Find resolved items
    resolved = []
    for key in before_map:
        if key not in after_map:
            item_type, item_data = before_map[key]
            score = get_debt_score(item_type, item_data)
            resolved.append((key, item_type, item_data, score))

    # Find improved items
    improved = []
    regressed = []
    for key in set(before_map.keys()) & set(after_map.keys()):
        before_type, before_data = before_map[key]
        after_type, after_data = after_map[key]

        before_score = get_debt_score(before_type, before_data)
        after_score = get_debt_score(after_type, after_data)

        if after_score < before_score:
            improved.append((key, before_score, after_score, before_type, before_data))
        elif after_score > before_score:
            regressed.append((key, before_score, after_score, after_type, after_data))

    # Find new items
    new_items = []
    for key in after_map:
        if key not in before_map:
            item_type, item_data = after_map[key]
            score = get_debt_score(item_type, item_data)
            new_items.append((key, item_type, item_data, score))

    return {
        'total_before': total_before_score,
        'total_after': total_after_score,
        'items_before': len(before_items),
        'items_after': len(after_items),
        'resolved': resolved,
        'improved': improved,
        'regressed': regressed,
        'new_items': new_items,
        'successful': successful,
        'failed': failed,
        'total': total,
        'before_data': before_data,
        'after_data': after_data
    }


def format_item_name(key, item_type, item_data):
    """Format a readable item name."""
    if item_type == 'File':
        return item_data.get('metrics', {}).get('path', str(key))
    elif item_type == 'Function':
        file_path = item_data.get('file', '')
        function_name = item_data.get('function_name', '')
        return f"{file_path}::{function_name}"
    return str(key)


def format_summary(analysis):
    """Format analysis results into a commit message summary."""
    lines = []

    # Overall metrics
    total_before = analysis['total_before']
    total_after = analysis['total_after']
    improvement = total_before - total_after
    improvement_pct = (improvement / total_before * 100) if total_before > 0 else 0

    lines.append("Technical Debt Improvements:")
    lines.append(f"- Total debt score: {total_before:.0f} → {total_after:.0f} ({improvement:+.0f}, {improvement_pct:+.1f}%)")
    lines.append(f"- Items resolved: {len(analysis['resolved'])} items completely removed")
    lines.append(f"- Items improved: {len(analysis['improved'])} items with reduced scores")
    lines.append(f"- Overall items: {analysis['items_before']} → {analysis['items_after']} ({analysis['items_after'] - analysis['items_before']:+d})")
    
    # Coverage improvement
    before_cov = analysis['before_data'].get('overall_coverage', 0) * 100
    after_cov = analysis['after_data'].get('overall_coverage', 0) * 100
    if before_cov > 0 or after_cov > 0:
        lines.append(f"- Test coverage: {before_cov:.1f}% → {after_cov:.1f}% ({after_cov - before_cov:+.1f}%)")
    
    lines.append("")

    # Top improvements (resolved items)
    if analysis['resolved']:
        lines.append("Top improvements (resolved):")
        sorted_resolved = sorted(
            analysis['resolved'],
            key=lambda x: x[3],  # score
            reverse=True
        )[:5]

        for i, (key, item_type, item_data, score) in enumerate(sorted_resolved, 1):
            name = format_item_name(key, item_type, item_data)
            lines.append(f"{i}. {name}: complexity score {score:.0f} → 0 (resolved)")
        lines.append("")

    # Top improvements (score reductions)
    if analysis['improved']:
        lines.append("Top improvements (score reductions):")
        sorted_improved = sorted(
            analysis['improved'],
            key=lambda x: x[1] - x[2],  # before_score - after_score
            reverse=True
        )[:5]

        for i, (key, before_score, after_score, item_type, item_data) in enumerate(sorted_improved, 1):
            name = format_item_name(key, item_type, item_data)
            reduction = (before_score - after_score) / before_score * 100 if before_score > 0 else 0
            lines.append(f"{i}. {name}: score {before_score:.0f} → {after_score:.0f} (-{reduction:.0f}%)")
        lines.append("")

    # Regressions
    if analysis['regressed']:
        lines.append("⚠️ Regressions detected:")
        for key, before_score, after_score, item_type, item_data in analysis['regressed'][:5]:
            name = format_item_name(key, item_type, item_data)
            increase = (after_score - before_score) / before_score * 100 if before_score > 0 else 0
            lines.append(f"- {name}: score {before_score:.0f} → {after_score:.0f} (+{increase:.0f}%)")
        lines.append("")

    # New items
    if analysis['new_items']:
        high_score_new = [item for item in analysis['new_items'] if item[3] > 10]
        if high_score_new:
            lines.append("⚠️ New high-complexity items introduced:")
            sorted_new = sorted(high_score_new, key=lambda x: x[3], reverse=True)[:5]

            for key, item_type, item_data, score in sorted_new:
                name = format_item_name(key, item_type, item_data)
                lines.append(f"- NEW: {name}: score {score:.0f}")
            lines.append("")

    return '\n'.join(lines)


def main():
    """Main entry point."""
    if len(sys.argv) < 6:
        print("Usage: analyze_debt.py <before.json> <after.json> <successful> <failed> <total>")
        sys.exit(1)

    before_path = sys.argv[1]
    after_path = sys.argv[2]
    successful = int(sys.argv[3])
    failed = int(sys.argv[4])
    total = int(sys.argv[5])

    # Load data
    before_data = load_json(before_path)
    after_data = load_json(after_path)

    # Analyze changes
    analysis = analyze_debt_changes(before_data, after_data, successful, failed, total)

    # Format and print summary
    summary = format_summary(analysis)
    print(summary)


if __name__ == '__main__':
    main()
