#!/usr/bin/env python3
"""
Auto-fix script for holistic book validation issues.
"""

import json
import sys
from pathlib import Path

BOOK_DIR = Path("/Users/glen/memento-mori/prodigy/book/src")
VALIDATION_REPORT = Path("/Users/glen/memento-mori/prodigy/.prodigy/book-analysis/validation.json")

def remove_lines_from_file(file_path: Path, start_line: int, end_line: int):
    """Remove lines from start_line to end_line (inclusive, 1-indexed)."""
    lines = file_path.read_text().split('\n')

    # Remove the lines (convert to 0-indexed)
    new_lines = lines[:start_line-1] + lines[end_line:]

    # Write back
    file_path.write_text('\n'.join(new_lines))

def fix_redundant_best_practices(issues):
    """Remove redundant Best Practices sections."""
    fixed_count = 0

    for issue in issues:
        file_path = BOOK_DIR / issue['file']
        start, end = issue['lines']

        print(f"  Removing redundant Best Practices from {issue['file']} (lines {start}-{end})")
        remove_lines_from_file(file_path, start, end)
        fixed_count += 1

    return fixed_count

def fix_bp_in_reference(issues):
    """Remove Best Practices sections from reference pages."""
    fixed_count = 0

    for issue in issues:
        file_path = BOOK_DIR / issue['file']
        start, end = issue['lines']

        print(f"  Removing Best Practices from reference page {issue['file']} (lines {start}-{end})")
        remove_lines_from_file(file_path, start, end)
        fixed_count += 1

    return fixed_count

def fix_stub_files(issues):
    """Consolidate stub navigation files into their parent index.md."""
    fixed_count = 0

    for issue in issues:
        stub_path = BOOK_DIR / issue['file']
        chapter_dir = stub_path.parent
        index_path = chapter_dir / "index.md"

        print(f"  Consolidating {issue['file']} into {chapter_dir.name}/index.md")

        # Read stub content
        stub_content = stub_path.read_text()

        # Append to index.md
        index_content = index_path.read_text()
        index_path.write_text(f"{index_content}\n\n{stub_content}")

        # Remove stub file
        stub_path.unlink()

        # Update SUMMARY.md to remove stub reference
        summary_path = BOOK_DIR / "SUMMARY.md"
        summary_content = summary_path.read_text()

        # Remove the line referencing this file
        lines = summary_content.split('\n')
        new_lines = [line for line in lines if stub_path.name not in line]
        summary_path.write_text('\n'.join(new_lines))

        fixed_count += 1

    return fixed_count

def main():
    print("=== Auto-Fix Mode ===\n")

    # Load validation report
    if not VALIDATION_REPORT.exists():
        print(f"Error: Validation report not found: {VALIDATION_REPORT}")
        sys.exit(1)

    report = json.loads(VALIDATION_REPORT.read_text())

    # Track fixes
    total_fixed = 0

    # Fix redundant Best Practices sections
    print("1. Removing redundant Best Practices sections...")
    redundant_issues = next(
        (item['files'] for item in report['issues_found'] if item['type'] == 'redundant_best_practices'),
        []
    )
    redundant_count = fix_redundant_best_practices(redundant_issues)
    total_fixed += redundant_count
    print(f"   ✓ Fixed {redundant_count} redundant sections\n")

    # Fix Best Practices in reference pages
    print("2. Removing Best Practices from reference pages...")
    reference_issues = next(
        (item['files'] for item in report['issues_found'] if item['type'] == 'best_practices_in_reference'),
        []
    )
    reference_count = fix_bp_in_reference(reference_issues)
    total_fixed += reference_count
    print(f"   ✓ Fixed {reference_count} reference pages\n")

    # Fix stub navigation files
    print("3. Consolidating stub navigation files...")
    stub_issues = next(
        (item['files'] for item in report['issues_found'] if item['type'] == 'stub_navigation_file'),
        []
    )
    stub_count = fix_stub_files(stub_issues)
    total_fixed += stub_count
    print(f"   ✓ Fixed {stub_count} stub files\n")

    # Summary
    print("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━")
    print(f"Total fixes applied: {total_fixed}")
    print(f"  • {redundant_count} redundant Best Practices sections removed")
    print(f"  • {reference_count} Best Practices removed from reference pages")
    print(f"  • {stub_count} stub navigation files consolidated")

    # Note about manual review
    manual_issues = report['summary']['circular_see_also']
    if manual_issues > 0:
        print(f"\nManual Review Required:")
        print(f"  • {manual_issues} circular See Also references (need context-specific fixes)")

    return total_fixed

if __name__ == "__main__":
    try:
        fixes = main()
        sys.exit(0 if fixes > 0 else 1)
    except Exception as e:
        print(f"\nError during auto-fix: {e}")
        import traceback
        traceback.print_exc()
        sys.exit(1)
