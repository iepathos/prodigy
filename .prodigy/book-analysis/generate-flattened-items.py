#!/usr/bin/env python3
"""
Generate flattened-items.json for the map phase.
This file contains all chapters and subsections in a flat array for parallel processing.
"""

import json
from pathlib import Path

CHAPTERS_FILE = "workflows/data/prodigy-chapters.json"
OUTPUT_FILE = ".prodigy/book-analysis/flattened-items.json"

def flatten_chapters():
    """Flatten chapters and subsections into a single array."""
    print(f"📖 Loading {CHAPTERS_FILE}...")
    with open(CHAPTERS_FILE, 'r') as f:
        chapters_data = json.load(f)

    flattened = []

    for chapter in chapters_data['chapters']:
        chapter_type = chapter.get('type', 'single-file')

        if chapter_type == 'multi-subsection':
            # Extract each subsection with parent metadata
            subsections = chapter.get('subsections', [])
            for subsection in subsections:
                flattened.append({
                    "id": subsection['id'],
                    "title": subsection['title'],
                    "file": subsection['file'],
                    "topics": subsection['topics'],
                    "validation": subsection.get('validation', ''),
                    "feature_mapping": subsection.get('feature_mapping', []),
                    "type": "subsection",
                    "parent_chapter_id": chapter['id'],
                    "parent_chapter_title": chapter['title']
                })
        else:
            # Single-file chapter
            flattened.append({
                "id": chapter['id'],
                "title": chapter['title'],
                "file": chapter['file'],
                "topics": chapter['topics'],
                "validation": chapter.get('validation', ''),
                "type": "single-file"
            })

    print(f"✅ Flattened {len(flattened)} items ({len([i for i in flattened if i['type'] == 'single-file'])} chapters, {len([i for i in flattened if i['type'] == 'subsection'])} subsections)")

    # Save flattened items
    print(f"💾 Saving to {OUTPUT_FILE}...")
    with open(OUTPUT_FILE, 'w') as f:
        json.dump(flattened, f, indent=2)

    print(f"✅ Saved {len(flattened)} items for map phase processing")
    return len(flattened)

if __name__ == "__main__":
    count = flatten_chapters()
    print(f"\n📊 Summary:")
    print(f"   Total items: {count}")
    print(f"   Ready for map phase: ✓")
