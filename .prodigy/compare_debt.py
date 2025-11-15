#!/usr/bin/env python3
"""
Compare technical debt before and after fixes.
"""
import json
import sys
from collections import defaultdict


def load_json(path):
    """Load and parse a JSON file."""
    with open(path, 'r') as f:
        return json.load(f)


def extract_item_info(item_wrapper):
    """Extract information from an item wrapper (File/Function/etc)."""
    item_type = list(item_wrapper.keys())[0]
    item = item_wrapper[item_type]

    if item_type == "File":
        path = item['metrics']['path']
        return {
            'type': 'File',
            'path': path,
            'key': f"File::{path}",
            'name': path,
            'metrics': item['metrics']
        }
    elif item_type == "Function":
        path = item['location']['file']
        name = item['location']['function']
        line = item['location'].get('line', item['location'].get('start_line', 0))
        return {
            'type': 'Function',
            'path': path,
            'name': name,
            'line': line,
            'key': f"Function::{path}::{name}::{line}",
            'metrics': item.get('metrics', {}),
            'location': item['location'],
            'unified_score': item.get('unified_score', {})
        }
    elif item_type == "DuplicateCode":
        locations = item.get('locations', [])
        if locations:
            first_loc = locations[0]
            key = f"DuplicateCode::{first_loc['file']}::{first_loc.get('start_line', 0)}"
        else:
            key = f"DuplicateCode::unknown"
        return {
            'type': 'DuplicateCode',
            'locations': locations,
            'key': key,
            'metrics': item.get('metrics', {})
        }
    elif item_type == "Dependency":
        source = item.get('source', {})
        target = item.get('target', {})
        key = f"Dependency::{source.get('name', 'unknown')}::{target.get('name', 'unknown')}"
        return {
            'type': 'Dependency',
            'source': source,
            'target': target,
            'key': key,
            'metrics': item.get('metrics', {})
        }
    else:
        return {
            'type': item_type,
            'key': f"{item_type}::unknown",
            'metrics': item.get('metrics', {})
        }


def get_debt_score(item_info):
    """Get the debt score from item info."""
    # Functions have unified_score at top level
    if 'unified_score' in item_info:
        unified_score = item_info['unified_score']
        if isinstance(unified_score, dict) and 'final_score' in unified_score:
            return unified_score['final_score']

    # Check metrics
    metrics = item_info.get('metrics', {})

    # Try different possible score fields
    if 'total_debt_score' in metrics:
        return metrics['total_debt_score']
    elif 'debt_score' in metrics:
        return metrics['debt_score']
    elif 'duplication_score' in metrics:
        return metrics['duplication_score']
    elif 'coupling_score' in metrics:
        return metrics['coupling_score']

    return 0


def calculate_metrics(before_data, after_data):
    """Calculate overall debt metrics."""
    total_before = before_data.get('total_debt_score', 0)
    total_after = after_data.get('total_debt_score', 0)
    items_before = len(before_data.get('items', []))
    items_after = len(after_data.get('items', []))

    return {
        'total_before': total_before,
        'total_after': total_after,
        'items_before': items_before,
        'items_after': items_after,
        'score_reduction': total_before - total_after,
        'score_reduction_pct': ((total_before - total_after) / total_before * 100) if total_before > 0 else 0,
        'items_reduction': items_before - items_after,
        'items_reduction_pct': ((items_before - items_after) / items_before * 100) if items_before > 0 else 0,
    }


def analyze_changes(before_data, after_data):
    """Analyze item-level changes."""
    before_items = before_data.get('items', [])
    after_items = after_data.get('items', [])

    # Extract and index items
    before_map = {}
    for item_wrapper in before_items:
        item_info = extract_item_info(item_wrapper)
        before_map[item_info['key']] = item_info

    after_map = {}
    for item_wrapper in after_items:
        item_info = extract_item_info(item_wrapper)
        after_map[item_info['key']] = item_info

    # Find changes
    resolved = []
    improved = []
    regressed = []
    unchanged = []
    new_items = []

    for key, before_item in before_map.items():
        if key not in after_map:
            resolved.append((key, before_item))
        else:
            before_score = get_debt_score(before_item)
            after_score = get_debt_score(after_map[key])

            if after_score < before_score:
                improved.append((key, before_item, after_map[key], before_score - after_score))
            elif after_score > before_score:
                regressed.append((key, before_item, after_map[key], after_score - before_score))
            else:
                unchanged.append((key, before_item))

    for key, after_item in after_map.items():
        if key not in before_map:
            new_items.append((key, after_item))

    return {
        'resolved': resolved,
        'improved': improved,
        'regressed': regressed,
        'unchanged': unchanged,
        'new_items': new_items,
    }


def analyze_by_category(before_data, after_data):
    """Analyze improvements by item type."""
    before_items = before_data.get('items', [])
    after_items = after_data.get('items', [])

    # Group by type
    before_by_type = defaultdict(lambda: {'count': 0, 'score': 0})
    after_by_type = defaultdict(lambda: {'count': 0, 'score': 0})

    for item_wrapper in before_items:
        item_info = extract_item_info(item_wrapper)
        item_type = item_info['type']
        score = get_debt_score(item_info)
        before_by_type[item_type]['count'] += 1
        before_by_type[item_type]['score'] += score

    for item_wrapper in after_items:
        item_info = extract_item_info(item_wrapper)
        item_type = item_info['type']
        score = get_debt_score(item_info)
        after_by_type[item_type]['count'] += 1
        after_by_type[item_type]['score'] += score

    # Calculate improvements
    type_stats = {}
    all_types = set(list(before_by_type.keys()) + list(after_by_type.keys()))

    for item_type in all_types:
        before = before_by_type[item_type]
        after = after_by_type[item_type]

        reduction = before['score'] - after['score']
        reduction_pct = (reduction / before['score'] * 100) if before['score'] > 0 else 0

        type_stats[item_type] = {
            'before_total': before['score'],
            'after_total': after['score'],
            'before_count': before['count'],
            'after_count': after['count'],
            'reduction': reduction,
            'reduction_pct': reduction_pct,
        }

    return type_stats


def format_item_name(key, item):
    """Format an item name for display."""
    item_type = item['type']

    if item_type == 'File':
        return item['path']
    elif item_type == 'Function':
        return f"{item['path']}::{item['name']}:{item.get('line', 0)}"
    elif item_type == 'DuplicateCode':
        locations = item.get('locations', [])
        if locations:
            return f"{locations[0]['file']}:{locations[0].get('start_line', 0)}"
        return "duplicate code"
    elif item_type == 'Dependency':
        source = item.get('source', {}).get('name', 'unknown')
        target = item.get('target', {}).get('name', 'unknown')
        return f"{source} → {target}"
    else:
        return item.get('name', key)


def generate_report(metrics, changes, type_stats, successful, failed, total):
    """Generate a formatted report for the commit message."""
    lines = []

    # Overall summary
    lines.append("Technical Debt Improvements:")
    lines.append(f"- Total debt score: {metrics['total_before']:.1f} → {metrics['total_after']:.1f} (-{metrics['score_reduction_pct']:.1f}%)")
    lines.append(f"- Items resolved: {len(changes['resolved'])} of {total} targeted")
    lines.append(f"- Overall items: {metrics['items_before']} → {metrics['items_after']} ({metrics['items_reduction']:+d}, {metrics['items_reduction_pct']:+.1f}%)")
    lines.append("")

    # Type breakdown
    if type_stats:
        lines.append("By type:")
        for item_type, stats in sorted(type_stats.items(), key=lambda x: x[1]['reduction'], reverse=True):
            if stats['reduction'] > 0:
                count_change = stats['before_count'] - stats['after_count']
                lines.append(f"- {item_type}: -{stats['reduction_pct']:.0f}% ({count_change:+d} items)")
        lines.append("")

    # Top improvements
    if changes['resolved'] or changes['improved']:
        lines.append("Top improvements:")

        # Combine and sort by impact
        all_improvements = []

        for key, item in changes['resolved']:
            score = get_debt_score(item)
            all_improvements.append((score, key, item, None, True))

        for key, before_item, after_item, reduction in changes['improved']:
            before_score = get_debt_score(before_item)
            all_improvements.append((reduction, key, before_item, after_item, False))

        all_improvements.sort(reverse=True, key=lambda x: x[0])

        count = 1
        for impact, key, before_item, after_item, is_resolved in all_improvements[:5]:
            if is_resolved:
                lines.append(f"{count}. {format_item_name(key, before_item)}: score {impact:.1f} → 0 (resolved)")
            else:
                before_score = get_debt_score(before_item)
                after_score = get_debt_score(after_item)
                pct = (impact / before_score * 100) if before_score > 0 else 0
                lines.append(f"{count}. {format_item_name(key, before_item)}: score {before_score:.1f} → {after_score:.1f} (-{pct:.0f}%)")
            count += 1
        lines.append("")

    # Regressions
    if changes['regressed'] or changes['new_items']:
        lines.append("⚠️ Regressions detected:")

        for key, before_item, after_item, increase in changes['regressed'][:3]:
            before_score = get_debt_score(before_item)
            after_score = get_debt_score(after_item)
            pct = (increase / before_score * 100) if before_score > 0 else 0
            lines.append(f"- {format_item_name(key, before_item)}: score {before_score:.1f} → {after_score:.1f} (+{pct:.0f}%)")

        for key, item in changes['new_items'][:3]:
            score = get_debt_score(item)
            if score > 0:
                lines.append(f"- NEW: {format_item_name(key, item)}: score {score:.1f}")

        lines.append("")

    return '\n'.join(lines)


def main():
    # Parse arguments
    before_path = None
    after_path = None
    successful = 0
    failed = 0
    total = 0

    i = 1
    while i < len(sys.argv):
        if sys.argv[i] == '--before':
            before_path = sys.argv[i + 1]
            i += 2
        elif sys.argv[i] == '--after':
            after_path = sys.argv[i + 1]
            i += 2
        elif sys.argv[i] == '--map-results-file':
            # Skip for now
            i += 2
        elif sys.argv[i] == '--successful':
            successful = int(sys.argv[i + 1])
            i += 2
        elif sys.argv[i] == '--failed':
            failed = int(sys.argv[i + 1])
            i += 2
        elif sys.argv[i] == '--total':
            total = int(sys.argv[i + 1])
            i += 2
        else:
            i += 1

    if not before_path or not after_path:
        print("Error: --before and --after are required", file=sys.stderr)
        sys.exit(1)

    # Load data
    before_data = load_json(before_path)
    after_data = load_json(after_path)

    # Calculate metrics
    metrics = calculate_metrics(before_data, after_data)
    changes = analyze_changes(before_data, after_data)
    type_stats = analyze_by_category(before_data, after_data)

    # Generate report
    report = generate_report(metrics, changes, type_stats, successful, failed, total)

    # Print report
    print(report)


if __name__ == '__main__':
    main()
