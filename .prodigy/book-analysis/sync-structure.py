#!/usr/bin/env python3
"""
Sync chapter structure in chapters.json with actual file structure.
This ensures chapters.json accurately reflects whether chapters are single-file or multi-subsection.
"""

import json
import os
import re
from pathlib import Path
from typing import Dict, List, Set

BOOK_SRC = "book/src"
CHAPTERS_FILE = "workflows/data/prodigy-chapters.json"

def find_multi_subsection_chapters() -> Dict[str, List[str]]:
    """Find all directories with index.md and their subsection files."""
    multi_subsection = {}

    book_path = Path(BOOK_SRC)
    for item in book_path.iterdir():
        if item.is_dir():
            index_file = item / "index.md"
            if index_file.exists():
                # This is a multi-subsection chapter
                subsections = []
                for md_file in item.glob("*.md"):
                    if md_file.name != "index.md":
                        subsections.append(md_file.stem)

                if subsections:  # Only if there are actual subsections
                    multi_subsection[item.name] = sorted(subsections)

    return multi_subsection

def extract_title_from_markdown(file_path: Path) -> str:
    """Extract title from markdown file (first H1 or H2)."""
    try:
        with open(file_path, 'r', encoding='utf-8') as f:
            for line in f:
                # Match H1 or H2
                match = re.match(r'^#{1,2}\s+(.+)$', line.strip())
                if match:
                    return match.group(1)
    except Exception as e:
        print(f"Warning: Could not read {file_path}: {e}")

    # Fallback: convert filename to title
    return file_path.stem.replace('-', ' ').title()

def extract_topics_from_markdown(file_path: Path) -> List[str]:
    """Extract section headings as topics from markdown."""
    topics = []
    try:
        with open(file_path, 'r', encoding='utf-8') as f:
            for line in f:
                # Match H2 or H3
                match = re.match(r'^#{2,3}\s+(.+)$', line.strip())
                if match:
                    topic = match.group(1)
                    # Skip common generic headings
                    if topic.lower() not in ['overview', 'introduction', 'see also', 'related', 'next steps']:
                        topics.append(topic)
    except Exception as e:
        print(f"Warning: Could not read {file_path}: {e}")

    return topics[:5]  # Limit to 5 topics

def title_to_id(title: str) -> str:
    """Convert title to kebab-case ID."""
    return title.lower().replace(' ', '-').replace('_', '-')

def create_subsection_definition(chapter_id: str, subsection_id: str, file_path: Path) -> Dict:
    """Create a subsection definition from a markdown file."""
    title = extract_title_from_markdown(file_path)
    topics = extract_topics_from_markdown(file_path)

    return {
        "id": subsection_id,
        "title": title,
        "file": f"book/src/{chapter_id}/{subsection_id}.md",
        "topics": topics if topics else [title.lower()],
        "validation": f"Check {title.lower()} documentation matches implementation"
    }

def sync_chapter_structure():
    """Main function to sync chapter structure."""
    print("🔍 Scanning for multi-subsection chapters in book/src/...")

    # Find actual multi-subsection chapters
    actual_structure = find_multi_subsection_chapters()
    print(f"Found {len(actual_structure)} multi-subsection chapters")

    # Load existing chapters.json
    print(f"\n📖 Loading {CHAPTERS_FILE}...")
    with open(CHAPTERS_FILE, 'r') as f:
        chapters_data = json.load(f)

    chapters = chapters_data['chapters']

    # Track mismatches and migrations
    mismatches = []
    migrations = []

    print("\n🔄 Comparing against chapters.json definitions...")

    # Check each actual multi-subsection chapter
    for chapter_id, subsection_files in actual_structure.items():
        # Find this chapter in chapters.json
        chapter_def = next((c for c in chapters if c['id'] == chapter_id), None)

        if not chapter_def:
            print(f"⚠️  Chapter '{chapter_id}' exists as directory but not in chapters.json")
            continue

        chapter_type = chapter_def.get('type', 'single-file')

        if chapter_type == 'single-file':
            print(f"\n🔧 MISMATCH: '{chapter_id}' marked as single-file but has {len(subsection_files)} subsections")
            mismatches.append({
                "chapter_id": chapter_id,
                "expected_type": "single-file",
                "actual_type": "multi-subsection",
                "subsection_count": len(subsection_files)
            })

            # Auto-migrate: build subsections array
            print(f"   Discovering subsections from files...")
            subsections = []
            for subsection_id in subsection_files:
                file_path = Path(BOOK_SRC) / chapter_id / f"{subsection_id}.md"
                if file_path.exists():
                    subsection_def = create_subsection_definition(chapter_id, subsection_id, file_path)
                    subsections.append(subsection_def)
                    print(f"   ✓ Found: {subsection_def['title']}")

            # Update chapter definition
            chapter_def['type'] = 'multi-subsection'
            chapter_def['index_file'] = f"book/src/{chapter_id}/index.md"

            # Remove 'file' field if it exists (multi-subsection chapters use index_file)
            if 'file' in chapter_def:
                del chapter_def['file']

            chapter_def['subsections'] = subsections

            migrations.append({
                "chapter_id": chapter_id,
                "action": "migrated_to_multi_subsection",
                "subsections_added": len(subsections)
            })

            print(f"   ✅ Migrated '{chapter_id}' to multi-subsection with {len(subsections)} subsections")

        elif chapter_type == 'multi-subsection':
            # Verify subsection count matches
            existing_subsections = chapter_def.get('subsections', [])
            if len(existing_subsections) != len(subsection_files):
                print(f"⚠️  '{chapter_id}' subsection count mismatch: {len(existing_subsections)} in JSON vs {len(subsection_files)} files")

    # Save results
    if migrations:
        print(f"\n💾 Saving updated chapters.json...")
        with open(CHAPTERS_FILE, 'w') as f:
            json.dump(chapters_data, f, indent=2)
        print(f"✅ Updated {CHAPTERS_FILE}")
    else:
        print("\n✅ No mismatches found - structure is already in sync")

    # Save sync report
    sync_report = {
        "sync_timestamp": "2025-11-09T00:00:00Z",
        "multi_subsection_chapters_found": len(actual_structure),
        "mismatches_found": len(mismatches),
        "mismatches": mismatches,
        "migrations_performed": len(migrations),
        "migrations": migrations
    }

    sync_report_path = ".prodigy/book-analysis/structure-sync.json"
    with open(sync_report_path, 'w') as f:
        json.dump(sync_report, f, indent=2)
    print(f"📊 Saved sync report to {sync_report_path}")

    return len(migrations) > 0

if __name__ == "__main__":
    changed = sync_chapter_structure()
    exit(0 if changed else 0)  # Always exit 0 for success
