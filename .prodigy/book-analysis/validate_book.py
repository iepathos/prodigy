#!/usr/bin/env python3
"""
Holistic book validation script for detecting cross-cutting issues.
"""

import json
import os
import re
from pathlib import Path
from typing import Dict, List, Tuple, Any
from collections import defaultdict

BOOK_DIR = Path("/Users/glen/memento-mori/prodigy/book/src")

def find_files_with_best_practices() -> List[Path]:
    """Find all markdown files with Best Practices sections."""
    files = []
    for md_file in BOOK_DIR.rglob("*.md"):
        content = md_file.read_text()
        if re.search(r'^##+ Best Practices', content, re.MULTILINE):
            files.append(md_file.relative_to(BOOK_DIR))
    return sorted(files)

def get_bp_line_range(file_path: Path) -> Tuple[int, int]:
    """Get the line range of the Best Practices section."""
    content = (BOOK_DIR / file_path).read_text()
    lines = content.split('\n')

    start_line = None
    for i, line in enumerate(lines, 1):
        if re.match(r'^##+ Best Practices', line):
            start_line = i
            break

    if not start_line:
        return (0, 0)

    # Find end (next ## or end of file)
    end_line = len(lines)
    for i in range(start_line, len(lines)):
        if re.match(r'^##? ', lines[i]):
            end_line = i
            break

    return (start_line, end_line)

def detect_redundant_best_practices(bp_files: List[Path]) -> List[Dict[str, Any]]:
    """Detect redundant Best Practices sections in subsections."""
    issues = []

    for file_path in bp_files:
        # Skip dedicated best-practices.md files
        if file_path.name == "best-practices.md":
            continue

        # Skip index.md files (they can have BP sections)
        if file_path.name == "index.md":
            continue

        # Check if this is a subsection with a parent directory
        if file_path.parent != Path("."):
            # Check if chapter has dedicated best-practices.md
            chapter_bp = file_path.parent / "best-practices.md"
            if (BOOK_DIR / chapter_bp).exists():
                start, end = get_bp_line_range(file_path)
                issues.append({
                    "file": str(file_path),
                    "lines": [start, end],
                    "redundant_with": str(chapter_bp),
                    "recommendation": "Remove section, content covered in dedicated file"
                })

    return issues

def detect_bp_in_reference_pages(bp_files: List[Path]) -> List[Dict[str, Any]]:
    """Detect Best Practices in technical reference pages."""
    issues = []

    reference_indicators = [
        "syntax", "reference", "configuration", "fields", "options",
        "parameters", "structure", "format", "available"
    ]

    for file_path in bp_files:
        # Skip dedicated best-practices.md files
        if file_path.name == "best-practices.md":
            continue

        content = (BOOK_DIR / file_path).read_text()
        first_500_chars = content[:500].lower()

        # Check for reference page indicators
        ref_score = sum(1 for indicator in reference_indicators if indicator in first_500_chars)

        # Check for guide indicators (make it less likely to be a reference page)
        guide_indicators = ["tutorial", "guide", "walkthrough", "example", "how to"]
        guide_score = sum(1 for indicator in guide_indicators if indicator in first_500_chars)

        # If more reference indicators than guide indicators, it's likely a reference page
        if ref_score >= 2 and guide_score < 2:
            start, end = get_bp_line_range(file_path)
            issues.append({
                "file": str(file_path),
                "lines": [start, end],
                "file_type": "technical_reference",
                "recommendation": "Remove Best Practices section - this is technical documentation"
            })

    return issues

def extract_see_also_links(file_path: Path) -> List[str]:
    """Extract links from See Also section."""
    content = (BOOK_DIR / file_path).read_text()
    lines = content.split('\n')

    in_see_also = False
    links = []

    for line in lines:
        if re.match(r'^## See Also', line):
            in_see_also = True
            continue

        if in_see_also:
            if re.match(r'^##? ', line):
                break

            # Extract markdown links
            link_matches = re.findall(r'\[([^\]]+)\]\(([^\)]+)\)', line)
            for _, link in link_matches:
                links.append(link)

    return links

def detect_circular_see_also() -> List[Dict[str, Any]]:
    """Detect circular See Also references."""
    # Build graph of See Also links
    graph = defaultdict(set)

    for md_file in BOOK_DIR.rglob("*.md"):
        file_path = md_file.relative_to(BOOK_DIR)
        links = extract_see_also_links(file_path)

        for link in links:
            # Resolve relative link
            try:
                target = (md_file.parent / link).relative_to(BOOK_DIR)
                graph[str(file_path)].add(str(target))
            except:
                pass

    # Find mutual references (A -> B and B -> A)
    circular = []
    seen = set()

    for file_a, targets in graph.items():
        for file_b in targets:
            if file_a in graph.get(file_b, set()):
                pair = tuple(sorted([file_a, file_b]))
                if pair not in seen:
                    seen.add(pair)
                    circular.append({
                        "files": list(pair),
                        "description": "Mutual references without explaining specific relationship"
                    })

    return circular

def detect_generic_see_also() -> List[Dict[str, Any]]:
    """Detect generic See Also lists with many unexplained links."""
    issues = []

    for md_file in BOOK_DIR.rglob("*.md"):
        file_path = md_file.relative_to(BOOK_DIR)
        content = md_file.read_text()
        lines = content.split('\n')

        in_see_also = False
        link_lines = []

        for line in lines:
            if re.match(r'^## See Also', line):
                in_see_also = True
                continue

            if in_see_also:
                if re.match(r'^##? ', line):
                    break

                if re.search(r'\[([^\]]+)\]\(([^\)]+)\)', line):
                    link_lines.append(line)

        if len(link_lines) > 5:
            # Check how many have explanations (text after the link)
            explained = sum(1 for line in link_lines if re.search(r'\).*\w{3,}', line))

            if explained < len(link_lines) / 2:
                issues.append({
                    "file": str(file_path),
                    "link_count": len(link_lines),
                    "explained_count": explained,
                    "recommendation": "Reduce to 3-4 most relevant links with specific relationships"
                })

    return issues

def detect_over_fragmented_chapters() -> List[Dict[str, Any]]:
    """Detect chapters with too many subsections or subsections that are too small."""
    issues = []

    for chapter_dir in BOOK_DIR.iterdir():
        if not chapter_dir.is_dir():
            continue

        # Count subsection files (exclude index.md)
        subsections = [f for f in chapter_dir.glob("*.md") if f.name != "index.md"]

        if len(subsections) > 10:
            # Check average file size
            total_lines = sum(len(f.read_text().split('\n')) for f in subsections)
            avg_lines = total_lines / len(subsections) if subsections else 0

            if avg_lines < 100:
                issues.append({
                    "chapter": str(chapter_dir.relative_to(BOOK_DIR)),
                    "subsection_count": len(subsections),
                    "average_lines": int(avg_lines),
                    "recommendation": f"Consolidate related subsections - target 6-8 focused subsections"
                })

    return issues

def detect_stub_navigation_files() -> List[Dict[str, Any]]:
    """Detect files that are mostly navigation boilerplate."""
    issues = []

    for md_file in BOOK_DIR.rglob("*.md"):
        content = md_file.read_text()
        lines = content.split('\n')
        line_count = len(lines)

        if line_count < 50:
            # Count links
            link_count = len(re.findall(r'^\s*-\s*\[.*\]\(', content, re.MULTILINE))

            # If more than 50% links, it's a navigation stub
            if line_count > 0 and (link_count * 2) > line_count:
                file_path = md_file.relative_to(BOOK_DIR)
                issues.append({
                    "file": str(file_path),
                    "lines": line_count,
                    "link_percentage": int((link_count / line_count) * 100),
                    "recommendation": f"Consolidate into {file_path.parent}/index.md"
                })

    return issues

def main():
    print("=== Holistic Book Validation ===\n")

    # Find all files with Best Practices sections
    print("Scanning for Best Practices sections...")
    bp_files = find_files_with_best_practices()
    print(f"Found {len(bp_files)} files with Best Practices sections\n")

    # Detect anti-patterns
    print("Detecting anti-patterns...\n")

    redundant_bp = detect_redundant_best_practices(bp_files)
    print(f"✓ Redundant Best Practices sections: {len(redundant_bp)}")

    bp_in_ref = detect_bp_in_reference_pages(bp_files)
    print(f"✓ Best Practices in reference pages: {len(bp_in_ref)}")

    circular_see_also = detect_circular_see_also()
    print(f"✓ Circular See Also references: {len(circular_see_also)}")

    generic_see_also = detect_generic_see_also()
    print(f"✓ Generic See Also lists: {len(generic_see_also)}")

    over_fragmented = detect_over_fragmented_chapters()
    print(f"✓ Over-fragmented chapters: {len(over_fragmented)}")

    stub_files = detect_stub_navigation_files()
    print(f"✓ Stub navigation files: {len(stub_files)}")

    # Generate report
    total_issues = (
        len(redundant_bp) + len(bp_in_ref) + len(circular_see_also) +
        len(generic_see_also) + len(over_fragmented) + len(stub_files)
    )

    report = {
        "validation_timestamp": "2025-01-10T15:30:00Z",
        "project": "Prodigy",
        "book_dir": str(BOOK_DIR),
        "total_files": len(list(BOOK_DIR.rglob("*.md"))),
        "total_chapters": len([d for d in BOOK_DIR.iterdir() if d.is_dir()]),
        "issues_found": [
            {
                "type": "redundant_best_practices",
                "severity": "high",
                "files": redundant_bp
            },
            {
                "type": "best_practices_in_reference",
                "severity": "medium",
                "files": bp_in_ref
            },
            {
                "type": "circular_see_also",
                "severity": "low",
                "patterns": circular_see_also
            },
            {
                "type": "generic_see_also",
                "severity": "low",
                "files": generic_see_also
            },
            {
                "type": "over_fragmented_chapter",
                "severity": "medium",
                "chapters": over_fragmented
            },
            {
                "type": "stub_navigation_file",
                "severity": "medium",
                "files": stub_files
            }
        ],
        "summary": {
            "redundant_best_practices": len(redundant_bp),
            "best_practices_in_reference": len(bp_in_ref),
            "circular_see_also": len(circular_see_also),
            "generic_see_also": len(generic_see_also),
            "over_fragmented_chapters": len(over_fragmented),
            "stub_navigation_files": len(stub_files),
            "total_issues": total_issues
        },
        "recommendations": [
            f"Remove {len(redundant_bp)} redundant Best Practices sections",
            f"Remove {len(bp_in_ref)} Best Practices sections from technical reference pages",
            f"Consolidate {len(over_fragmented)} over-fragmented chapters",
            f"Merge {len(stub_files)} stub navigation files into chapter indexes"
        ]
    }

    # Write report
    output_path = Path("/Users/glen/memento-mori/prodigy/.prodigy/book-analysis/validation.json")
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.write_text(json.dumps(report, indent=2))

    print(f"\n✓ Holistic validation complete")
    print(f"  Total issues found: {total_issues}")
    print(f"  Report written to: {output_path}")

    # Print summary
    print("\n=== Summary ===")
    print(f"\nHigh Priority ({len(redundant_bp)}):")
    print(f"  • {len(redundant_bp)} redundant Best Practices sections")

    print(f"\nMedium Priority ({len(bp_in_ref) + len(over_fragmented) + len(stub_files)}):")
    print(f"  • {len(bp_in_ref)} Best Practices in technical reference pages")
    print(f"  • {len(over_fragmented)} over-fragmented chapters")
    print(f"  • {len(stub_files)} stub navigation files")

    print(f"\nLow Priority ({len(circular_see_also) + len(generic_see_also)}):")
    print(f"  • {len(circular_see_also)} circular See Also references")
    print(f"  • {len(generic_see_also)} generic See Also lists")

if __name__ == "__main__":
    main()
