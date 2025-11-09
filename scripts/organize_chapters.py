#!/usr/bin/env python3
"""
Automatically organize large mdBook chapters into subsections.

This script analyzes markdown files in an mdBook and splits large chapters
into logical subsections based on H2 headings.
"""

import argparse
import json
import os
import re
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import List, Tuple, Optional


@dataclass
class Section:
    """Represents a section in a markdown file."""
    title: str
    level: int  # 2 for H2, 3 for H3, etc.
    start_line: int
    end_line: int
    content: List[str]


@dataclass
class ChapterAnalysis:
    """Analysis result for a chapter."""
    file_path: Path
    line_count: int
    h2_sections: List[Section]
    should_split: bool
    reason: str


def parse_markdown_sections(lines: List[str]) -> List[Section]:
    """Parse markdown file into sections based on headings."""
    sections = []
    current_section = None

    for i, line in enumerate(lines):
        # Check if this is a heading
        heading_match = re.match(r'^(#{2,6})\s+(.+)$', line)

        if heading_match:
            # Save previous section
            if current_section:
                current_section.end_line = i - 1
                sections.append(current_section)

            # Start new section
            level = len(heading_match.group(1))
            title = heading_match.group(2).strip()
            current_section = Section(
                title=title,
                level=level,
                start_line=i,
                end_line=i,
                content=[line]
            )
        elif current_section:
            current_section.content.append(line)

    # Save last section
    if current_section:
        current_section.end_line = len(lines) - 1
        sections.append(current_section)

    return sections


def analyze_chapter(file_path: Path, min_lines: int, min_h2_sections: int) -> ChapterAnalysis:
    """Analyze a chapter file to determine if it should be split."""
    with open(file_path, 'r', encoding='utf-8') as f:
        lines = f.readlines()

    line_count = len(lines)
    sections = parse_markdown_sections(lines)
    h2_sections = [s for s in sections if s.level == 2]
    h2_count = len(h2_sections)

    # Apply decision matrix
    should_split = False
    reason = ""

    if line_count > 800:
        should_split = True
        reason = f"Always split (>{800} lines)"
    elif line_count > 600:
        should_split = True
        reason = f"Large file (>{600} lines)"
    elif line_count >= min_lines and h2_count >= min_h2_sections:
        should_split = True
        reason = f">={min_lines} lines and >={min_h2_sections} H2 sections"
    else:
        reason = f"Keep as single file ({line_count} lines, {h2_count} H2s)"

    return ChapterAnalysis(
        file_path=file_path,
        line_count=line_count,
        h2_sections=h2_sections,
        should_split=should_split,
        reason=reason
    )


def generate_filename(title: str) -> str:
    """Convert section title to filename."""
    # Lowercase, replace spaces with hyphens, remove special chars
    filename = title.lower()
    filename = re.sub(r'\s+', '-', filename)
    filename = re.sub(r'[^a-z0-9-]', '', filename)
    return f"{filename}.md"


def split_chapter(analysis: ChapterAnalysis, preserve_index_sections: int, book_dir: Path) -> dict:
    """Split a chapter into subdirectory with index.md and subsection files."""
    chapter_name = analysis.file_path.stem
    chapter_dir = book_dir / "src" / chapter_name

    # Read original file
    with open(analysis.file_path, 'r', encoding='utf-8') as f:
        lines = f.readlines()

    # Create subdirectory
    chapter_dir.mkdir(exist_ok=True)

    # Find intro content (before first H2)
    intro_end = 0
    for i, line in enumerate(lines):
        if re.match(r'^##\s+', line):
            intro_end = i
            break

    intro_content = lines[:intro_end]

    # Split H2 sections into index vs subsections
    index_sections = analysis.h2_sections[:preserve_index_sections]
    subsection_sections = analysis.h2_sections[preserve_index_sections:]

    # Generate index.md
    index_content = intro_content.copy()

    # Add preserved sections to index
    for section in index_sections:
        index_content.extend(section.content)

    # Add navigation links to subsections
    if subsection_sections:
        index_content.append("\n## Additional Topics\n\n")
        index_content.append("See also:\n")
        for section in subsection_sections:
            filename = generate_filename(section.title)
            index_content.append(f"- [{section.title}]({filename})\n")

    # Write index.md
    with open(chapter_dir / "index.md", 'w', encoding='utf-8') as f:
        f.writelines(index_content)

    # Generate subsection files
    subsection_files = []
    for section in subsection_sections:
        filename = generate_filename(section.title)
        filepath = chapter_dir / filename

        with open(filepath, 'w', encoding='utf-8') as f:
            f.writelines(section.content)

        subsection_files.append({
            'filename': filename,
            'title': section.title
        })

    return {
        'chapter_name': chapter_name,
        'subsection_count': len(subsection_files),
        'subsection_files': subsection_files,
        'index_sections': [s.title for s in index_sections]
    }


def update_summary_md(book_dir: Path, split_results: List[dict], dry_run: bool):
    """Update SUMMARY.md with nested structure for split chapters."""
    summary_path = book_dir / "src" / "SUMMARY.md"

    with open(summary_path, 'r', encoding='utf-8') as f:
        lines = f.readlines()

    new_lines = []
    for line in lines:
        modified = False

        # Check if this line references a split chapter
        for result in split_results:
            chapter_name = result['chapter_name']
            pattern = rf'\[([^\]]+)\]\({re.escape(chapter_name)}\.md\)'
            match = re.search(pattern, line)

            if match:
                # Found a split chapter reference
                title = match.group(1)
                indent = len(line) - len(line.lstrip())
                indent_str = ' ' * indent

                # Replace with nested structure
                new_lines.append(f"{indent_str}- [{title}]({chapter_name}/index.md)\n")

                # Add subsections
                for subsection in result['subsection_files']:
                    new_lines.append(f"{indent_str}  - [{subsection['title']}]({chapter_name}/{subsection['filename']})\n")

                modified = True
                break

        if not modified:
            new_lines.append(line)

    # Write updated SUMMARY.md
    if not dry_run:
        with open(summary_path, 'w', encoding='utf-8') as f:
            f.writelines(new_lines)


def main():
    parser = argparse.ArgumentParser(description='Organize large mdBook chapters into subsections')
    parser.add_argument('--book-dir', default='book', help='Path to mdBook root directory')
    parser.add_argument('--min-h2-sections', type=int, default=6, help='Minimum H2 sections to trigger split')
    parser.add_argument('--min-lines', type=int, default=400, help='Minimum lines to trigger split')
    parser.add_argument('--preserve-index-sections', type=int, default=2, help='Number of H2 sections to keep in index.md')
    parser.add_argument('--dry-run', action='store_true', help='Preview changes without applying them')

    args = parser.parse_args()

    book_dir = Path(args.book_dir)
    src_dir = book_dir / "src"

    # Validate book directory
    if not src_dir.exists():
        print(f"Error: Book directory '{src_dir}' not found", file=sys.stderr)
        sys.exit(1)

    if not (src_dir / "SUMMARY.md").exists():
        print(f"Error: SUMMARY.md not found in '{src_dir}'", file=sys.stderr)
        sys.exit(1)

    # Analyze all chapter files
    print(f"Analyzing chapters in {src_dir}...\n")

    analyses = []
    for md_file in sorted(src_dir.glob("*.md")):
        if md_file.name == "SUMMARY.md":
            continue

        # Skip if already has subdirectory (manually organized)
        chapter_dir = src_dir / md_file.stem
        if chapter_dir.exists() and chapter_dir.is_dir():
            print(f"✓ {md_file.name}: Already organized (skipping)")
            continue

        analysis = analyze_chapter(md_file, args.min_lines, args.min_h2_sections)
        analyses.append(analysis)

        status = "→ Would split" if analysis.should_split else "→ Keep as-is"
        print(f"✓ {md_file.name} ({analysis.line_count} lines, {len(analysis.h2_sections)} H2s) {status}")
        if analysis.should_split:
            print(f"  Reason: {analysis.reason}")

    # Filter to chapters that should be split
    chapters_to_split = [a for a in analyses if a.should_split]

    if not chapters_to_split:
        print("\nNo chapters need to be split.")
        return

    print(f"\n{'Preview of' if args.dry_run else 'Applying'} changes:\n")

    split_results = []
    for analysis in chapters_to_split:
        if args.dry_run:
            print(f"Would split: {analysis.file_path.name}")
            print(f"  → {analysis.file_path.stem}/index.md (Introduction + {args.preserve_index_sections} sections)")
            for i, section in enumerate(analysis.h2_sections[args.preserve_index_sections:]):
                filename = generate_filename(section.title)
                print(f"  → {analysis.file_path.stem}/{filename} ({section.title})")
        else:
            result = split_chapter(analysis, args.preserve_index_sections, book_dir)
            split_results.append(result)

            print(f"✓ Split {analysis.file_path.name} → {result['chapter_name']}/ ({result['subsection_count']} subsections)")
            print(f"  - index.md ({', '.join(result['index_sections'])})")
            for subsection in result['subsection_files']:
                print(f"  - {subsection['filename']}")

    if split_results:
        print(f"\n✓ Updated SUMMARY.md with nested structure")
        update_summary_md(book_dir, split_results, args.dry_run)

        # Delete original chapter files
        for analysis in chapters_to_split:
            if not args.dry_run:
                analysis.file_path.unlink()
                print(f"✓ Deleted {analysis.file_path.name}")

    print(f"\nSummary:")
    print(f"  - {len(chapters_to_split)} chapters {'would be' if args.dry_run else ''} split")
    total_subsections = sum(len(a.h2_sections) - args.preserve_index_sections for a in chapters_to_split)
    print(f"  - {total_subsections} subsection files {'would be' if args.dry_run else ''} created")
    print(f"  - SUMMARY.md {'would be' if args.dry_run else ''} updated")

    if args.dry_run:
        print(f"\nRun without --dry-run to apply changes.")


if __name__ == '__main__':
    main()
