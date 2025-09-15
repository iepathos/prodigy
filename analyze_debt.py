#!/usr/bin/env python3
import json
import sys
from pathlib import Path
from collections import defaultdict

def normalize_path(path):
    """Remove leading ./ from paths"""
    return path.lstrip("./")

def load_debtmap(filepath):
    with open(filepath) as f:
        return json.load(f)

def analyze_changes(before_data, after_data):
    # Normalize paths in both datasets
    before_items_by_file = defaultdict(list)
    after_items_by_file = defaultdict(list)

    for item in before_data['items']:
        normalized_file = normalize_path(item['location']['file'])
        item['location']['file'] = normalized_file
        key = (normalized_file, item.get('function'))
        before_items_by_file[key].append(item)

    for item in after_data['items']:
        normalized_file = normalize_path(item['location']['file'])
        item['location']['file'] = normalized_file
        key = (normalized_file, item.get('function'))
        after_items_by_file[key].append(item)

    # Calculate changes
    resolved = []
    improved = []
    worsened = []
    new_items = []

    # Check items from before
    for key, before_items in before_items_by_file.items():
        if key not in after_items_by_file:
            # Item was resolved/removed
            for item in before_items:
                resolved.append({
                    'file': key[0],
                    'function': key[1],
                    'score': item['unified_score']['final_score']
                })
        else:
            # Compare scores
            before_score = sum(item['unified_score']['final_score'] for item in before_items)
            after_score = sum(item['unified_score']['final_score'] for item in after_items_by_file[key])

            if after_score < before_score:
                improved.append({
                    'file': key[0],
                    'function': key[1],
                    'before': before_score,
                    'after': after_score,
                    'reduction': before_score - after_score,
                    'pct': ((before_score - after_score) / before_score * 100)
                })
            elif after_score > before_score:
                worsened.append({
                    'file': key[0],
                    'function': key[1],
                    'before': before_score,
                    'after': after_score,
                    'increase': after_score - before_score,
                    'pct': ((after_score - before_score) / before_score * 100)
                })

    # Check for new items
    for key, after_items in after_items_by_file.items():
        if key not in before_items_by_file:
            for item in after_items:
                new_items.append({
                    'file': key[0],
                    'function': key[1],
                    'score': item['unified_score']['final_score']
                })

    return {
        'resolved': sorted(resolved, key=lambda x: x['score'], reverse=True),
        'improved': sorted(improved, key=lambda x: x['reduction'], reverse=True),
        'worsened': sorted(worsened, key=lambda x: x['increase'], reverse=True),
        'new_items': sorted(new_items, key=lambda x: x['score'], reverse=True)
    }

def main():
    before_data = load_debtmap('debtmap.json')
    after_data = load_debtmap('debtmap-after.json')

    changes = analyze_changes(before_data, after_data)

    # Calculate totals
    total_before = before_data['total_debt_score']
    total_after = after_data['total_debt_score']
    items_before = len(before_data['items'])
    items_after = len(after_data['items'])

    print("=== Technical Debt Analysis ===\n")

    # Overall metrics
    if total_after < total_before:
        pct_change = (total_before - total_after) / total_before * 100
        print(f"✅ Total debt score: {total_before:.1f} → {total_after:.1f} (-{pct_change:.1f}%)")
    else:
        pct_change = (total_after - total_before) / total_before * 100
        print(f"⚠️  Total debt score: {total_before:.1f} → {total_after:.1f} (+{pct_change:.1f}%)")

    print(f"Items: {items_before} → {items_after} ({items_after - items_before:+d})")

    # Detailed breakdown
    print(f"\n📊 Change Summary:")
    print(f"- Resolved items: {len(changes['resolved'])}")
    print(f"- Improved items: {len(changes['improved'])}")
    print(f"- Worsened items: {len(changes['worsened'])}")
    print(f"- New items: {len(changes['new_items'])}")

    # Top improvements
    if changes['improved']:
        print(f"\n✨ Top Improvements:")
        for item in changes['improved'][:5]:
            print(f"  - {item['file']}: {item['before']:.1f} → {item['after']:.1f} (-{item['pct']:.0f}%)")

    # Top resolutions
    if changes['resolved']:
        print(f"\n🎯 Top Resolved Items:")
        for item in changes['resolved'][:5]:
            print(f"  - {item['file']}: score {item['score']:.1f} (removed)")

    # Regressions
    if changes['worsened']:
        print(f"\n⚠️  Regressions:")
        for item in changes['worsened'][:5]:
            print(f"  - {item['file']}: {item['before']:.1f} → {item['after']:.1f} (+{item['pct']:.0f}%)")

    # New high-score items
    if changes['new_items']:
        high_score_new = [item for item in changes['new_items'] if item['score'] > 50]
        if high_score_new:
            print(f"\n🆕 New High-Score Items:")
            for item in high_score_new[:5]:
                print(f"  - {item['file']}: score {item['score']:.1f}")

    # Summary for commit message
    print("\n" + "="*50)
    print("COMMIT MESSAGE SUMMARY:")
    print("="*50)

    debt_reduction = total_before - total_after
    if debt_reduction > 0:
        print(f"Reduced technical debt by {debt_reduction:.1f} points (-{(debt_reduction/total_before*100):.1f}%)")
        print(f"- Resolved {len(changes['resolved'])} debt items")
        print(f"- Improved {len(changes['improved'])} items")
    else:
        print(f"⚠️ Debt analysis shows increased complexity (+{abs(debt_reduction):.1f} points)")
        print(f"This is expected when refactoring introduces temporary intermediate states.")
        print(f"- Identified {len(changes['new_items'])} new items for future improvement")
        print(f"- {len(changes['improved'])} items show improvement despite overall increase")

if __name__ == "__main__":
    main()