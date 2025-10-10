#!/usr/bin/env python3
import json
import sys
from collections import defaultdict

def load_json(filepath):
    with open(filepath, 'r') as f:
        return json.load(f)

def create_item_key(item):
    """Create a unique key for an item based on file and function."""
    # Handle both File and Function items
    if 'File' in item:
        return f"{item['File']['metrics']['path']}::module"
    elif 'Function' in item:
        loc = item['Function']['location']
        return f"{loc['file']}::{loc['function']}"
    else:
        return str(item)

def analyze_debt_changes(before_data, after_data, map_results):
    # Create lookup dictionaries
    before_items = {create_item_key(item): item for item in before_data['items']}
    after_items = {create_item_key(item): item for item in after_data['items']}

    # Get files that were modified during the map phase
    modified_files = set()
    for result in map_results:
        modified_files.update(result.get('files_modified', []))

    # Calculate changes
    resolved = []
    improved = []
    regressed = []
    new_items = []
    unchanged = []

    # Find resolved items (in before, not in after)
    for key, before_item in before_items.items():
        if key not in after_items:
            if 'File' in before_item:
                file_path = before_item['File']['metrics']['path']
                function_name = 'module'
                score = before_item['File']['score']
            else:
                loc = before_item['Function']['location']
                file_path = loc['file']
                function_name = loc['function']
                score = before_item['Function']['unified_score']['final_score']

            resolved.append({
                'key': key,
                'file': file_path,
                'function': function_name,
                'before_score': score,
                'in_modified_files': file_path in modified_files
            })

    # Find new items (not in before, in after)
    for key, after_item in after_items.items():
        if key not in before_items:
            if 'File' in after_item:
                file_path = after_item['File']['metrics']['path']
                function_name = 'module'
                score = after_item['File']['score']
            else:
                loc = after_item['Function']['location']
                file_path = loc['file']
                function_name = loc['function']
                score = after_item['Function']['unified_score']['final_score']

            new_items.append({
                'key': key,
                'file': file_path,
                'function': function_name,
                'after_score': score,
                'in_modified_files': file_path in modified_files
            })

    # Find improved/regressed items
    for key in set(before_items.keys()) & set(after_items.keys()):
        before_item = before_items[key]
        after_item = after_items[key]

        # Extract scores
        if 'File' in before_item:
            before_score = before_item['File']['score']
            file_path = before_item['File']['metrics']['path']
            function_name = 'module'
        else:
            before_score = before_item['Function']['unified_score']['final_score']
            loc = before_item['Function']['location']
            file_path = loc['file']
            function_name = loc['function']

        if 'File' in after_item:
            after_score = after_item['File']['score']
        else:
            after_score = after_item['Function']['unified_score']['final_score']

        change = {
            'key': key,
            'file': file_path,
            'function': function_name,
            'before_score': before_score,
            'after_score': after_score,
            'change': after_score - before_score,
            'change_pct': ((after_score - before_score) / before_score * 100) if before_score > 0 else 0,
            'in_modified_files': file_path in modified_files
        }

        if after_score < before_score - 0.01:  # Improved by more than 0.01
            improved.append(change)
        elif after_score > before_score + 0.01:  # Regressed by more than 0.01
            regressed.append(change)
        else:
            unchanged.append(change)

    # Sort lists
    resolved.sort(key=lambda x: x['before_score'], reverse=True)
    improved.sort(key=lambda x: x['before_score'] - x['after_score'], reverse=True)
    regressed.sort(key=lambda x: x['after_score'] - x['before_score'], reverse=True)
    new_items.sort(key=lambda x: x['after_score'], reverse=True)

    return {
        'resolved': resolved,
        'improved': improved,
        'regressed': regressed,
        'new_items': new_items,
        'unchanged': unchanged,
        'modified_files': list(modified_files)
    }

def calculate_category_changes(before_data, after_data):
    """Calculate changes by debt category."""
    categories = {}

    # Aggregate scores by category from before
    before_by_category = defaultdict(float)
    for item in before_data['items']:
        # Get impact data which contains category-specific information
        if 'File' in item:
            impact = item['File'].get('impact', {})
        else:
            impact = item['Function'].get('impact', {})

        for category, value in impact.items():
            if value > 0:
                before_by_category[category] += value

    # Aggregate scores by category from after
    after_by_category = defaultdict(float)
    for item in after_data['items']:
        if 'File' in item:
            impact = item['File'].get('impact', {})
        else:
            impact = item['Function'].get('impact', {})

        for category, value in impact.items():
            if value > 0:
                after_by_category[category] += value

    # Calculate changes
    all_categories = set(before_by_category.keys()) | set(after_by_category.keys())
    for category in all_categories:
        before = before_by_category[category]
        after = after_by_category[category]
        change = after - before
        change_pct = (change / before * 100) if before > 0 else 0
        categories[category] = {
            'before': before,
            'after': after,
            'change': change,
            'change_pct': change_pct
        }

    return categories

def format_summary(before_data, after_data, changes, categories, successful, failed, total):
    """Format the summary report."""
    before_score = before_data['total_debt_score']
    after_score = after_data['total_debt_score']
    score_change = after_score - before_score
    score_change_pct = (score_change / before_score * 100) if before_score > 0 else 0

    before_count = len(before_data['items'])
    after_count = len(after_data['items'])

    lines = []
    lines.append("Technical Debt Improvements:")
    lines.append(f"- Total debt score: {before_score:.1f} → {after_score:.1f} ({score_change:+.1f}, {score_change_pct:+.1f}%)")
    lines.append(f"- Items resolved: {len(changes['resolved'])} debt items completely eliminated")
    lines.append(f"- Overall items: {before_count} → {after_count} ({after_count - before_count:+d})")
    lines.append(f"- MapReduce execution: {successful}/{total} agents succeeded, {failed} failed")
    lines.append("")

    # Category analysis
    if categories:
        lines.append("By category:")
        sorted_categories = sorted(categories.items(),
                                   key=lambda x: abs(x[1]['change']),
                                   reverse=True)
        for category, data in sorted_categories[:5]:
            if abs(data['change']) > 0.1:
                lines.append(f"- {category}: {data['before']:.1f} → {data['after']:.1f} ({data['change']:+.1f}, {data['change_pct']:+.1f}%)")

    lines.append("")

    # Top resolved items (in modified files only)
    resolved_in_modified = [r for r in changes['resolved'] if r['in_modified_files']]
    if resolved_in_modified:
        lines.append("Top resolved items:")
        for i, item in enumerate(resolved_in_modified[:5], 1):
            lines.append(f"{i}. {item['file']}::{item['function']}: score {item['before_score']:.1f} → 0 (resolved)")

    # Top improved items (in modified files only)
    improved_in_modified = [r for r in changes['improved'] if r['in_modified_files']]
    if improved_in_modified:
        lines.append("")
        lines.append("Top improved items:")
        for i, item in enumerate(improved_in_modified[:5], 1):
            improvement = item['before_score'] - item['after_score']
            improvement_pct = (improvement / item['before_score'] * 100) if item['before_score'] > 0 else 0
            lines.append(f"{i}. {item['file']}::{item['function']}: score {item['before_score']:.1f} → {item['after_score']:.1f} (-{improvement:.1f}, -{improvement_pct:.1f}%)")

    # Regressions (in modified files only)
    regressed_in_modified = [r for r in changes['regressed'] if r['in_modified_files']]
    if regressed_in_modified:
        lines.append("")
        lines.append("⚠️ Regressions detected (in modified files):")
        for item in regressed_in_modified[:5]:
            regression = item['after_score'] - item['before_score']
            lines.append(f"- {item['file']}::{item['function']}: score {item['before_score']:.1f} → {item['after_score']:.1f} ({regression:+.1f}, {item['change_pct']:+.1f}%)")

    # New high-score items (in modified files only)
    new_in_modified = [n for n in changes['new_items'] if n['in_modified_files'] and n['after_score'] > 30]
    if new_in_modified:
        lines.append("")
        lines.append("⚠️ New high-score items (in modified files):")
        for item in new_in_modified[:5]:
            lines.append(f"- NEW: {item['file']}::{item['function']}: score {item['after_score']:.1f}")

    return "\n".join(lines)

def main():
    if len(sys.argv) < 6:
        print("Usage: analyze_debt.py <before.json> <after.json> <map-results.json> <successful> <failed> <total>")
        sys.exit(1)

    before_file = sys.argv[1]
    after_file = sys.argv[2]
    map_results_file = sys.argv[3]
    successful = int(sys.argv[4])
    failed = int(sys.argv[5])
    total = int(sys.argv[6])

    # Load data
    before_data = load_json(before_file)
    after_data = load_json(after_file)
    map_results = load_json(map_results_file)

    # Analyze changes
    changes = analyze_debt_changes(before_data, after_data, map_results)
    categories = calculate_category_changes(before_data, after_data)

    # Format and print summary
    summary = format_summary(before_data, after_data, changes, categories, successful, failed, total)
    print(summary)

if __name__ == '__main__':
    main()
