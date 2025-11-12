#!/usr/bin/env python3
"""
Generate mkdocs.yml navigation from mdbook's SUMMARY.md

This keeps both navigation structures in sync automatically.
Usage: python scripts/sync-mkdocs-nav.py
"""

import re
import yaml
from pathlib import Path


def parse_summary_md(summary_path: Path) -> list:
    """
    Parse mdbook's SUMMARY.md and convert to mkdocs nav structure.

    Returns a list of nav items in mkdocs format.
    """
    nav = []
    stack = [(nav, -1)]  # (current_list, indent_level)

    with open(summary_path, 'r') as f:
        for line in f:
            # Skip empty lines and comments
            line = line.rstrip()
            if not line or line.startswith('#'):
                continue

            # Match mdbook list items: - [Title](path.md)
            match = re.match(r'^(\s*)- \[(.+?)\]\((.+?)\)', line)
            if not match:
                continue

            indent_str, title, path = match.groups()
            indent_level = len(indent_str) // 2  # 2 spaces per level

            # Adjust path for mkdocs if needed
            # intro.md should become index.md for home page
            if path == 'intro.md':
                # Use as secondary home or getting started
                pass  # Keep as is, mkdocs.yml will handle

            # Create nav entry
            # If it's a directory index, use dict format
            if path.endswith('/index.md'):
                # Section with index
                nav_item = {title: path}
            else:
                # Simple page
                nav_item = {title: path}

            # Find the right parent level in the stack
            while stack and stack[-1][1] >= indent_level:
                stack.pop()

            # Add to current level
            current_list = stack[-1][0]

            # If this looks like it might have children (ends with index.md),
            # prepare for nested items
            if path.endswith('/index.md') or indent_level < 2:
                # This might be a section
                current_list.append(nav_item)
                # Don't create nested list yet, mkdocs handles this differently
            else:
                current_list.append(nav_item)

    return nav


def update_mkdocs_nav(mkdocs_path: Path, nav: list):
    """Update the nav section in mkdocs.yml while preserving other config."""

    # Read current config
    with open(mkdocs_path, 'r') as f:
        config = yaml.safe_load(f)

    # Update nav section
    config['nav'] = nav

    # Write back with nice formatting
    with open(mkdocs_path, 'w') as f:
        yaml.dump(config, f, default_flow_style=False, sort_keys=False, allow_unicode=True)

    print(f"✓ Updated {mkdocs_path} with {len(nav)} top-level navigation items")


def main():
    # Paths relative to project root
    project_root = Path(__file__).parent.parent
    summary_path = project_root / 'book' / 'src' / 'SUMMARY.md'
    mkdocs_path = project_root / 'mkdocs.yml'

    # Verify files exist
    if not summary_path.exists():
        print(f"✗ Error: {summary_path} not found")
        return 1

    if not mkdocs_path.exists():
        print(f"✗ Error: {mkdocs_path} not found")
        return 1

    print(f"📖 Parsing {summary_path}...")
    nav = parse_summary_md(summary_path)

    print(f"📝 Updating {mkdocs_path}...")
    update_mkdocs_nav(mkdocs_path, nav)

    print("\n✅ Navigation sync complete!")
    print(f"   mdbook uses: {summary_path}")
    print(f"   mkdocs uses: {mkdocs_path}")
    print("\nBoth navigation structures now reference the same source files in book/src/")

    return 0


if __name__ == '__main__':
    exit(main())
