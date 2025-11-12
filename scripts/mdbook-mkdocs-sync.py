#!/usr/bin/env python3
"""
mdbook-mkdocs-sync: Sync mdbook SUMMARY.md to mkdocs.yml navigation

A general-purpose tool for maintaining dual mdbook/mkdocs documentation
from a single source.

Features:
- Converts SUMMARY.md structure to mkdocs.yml nav
- Handles intro.md → index.md mapping automatically
- Optionally renames files for mkdocs compatibility
- Preserves existing mkdocs.yml configuration
- Works with any mdbook project

Usage:
    python mdbook-mkdocs-sync.py [OPTIONS]

Options:
    --summary PATH      Path to SUMMARY.md (default: book/src/SUMMARY.md)
    --mkdocs PATH       Path to mkdocs.yml (default: mkdocs.yml)
    --rename-files      Actually rename intro.md to index.md (default: just map in nav)
    --dry-run           Show what would be done without making changes
    --help              Show this help message

Examples:
    # Basic usage (from project root)
    python mdbook-mkdocs-sync.py

    # With custom paths
    python mdbook-mkdocs-sync.py --summary docs/SUMMARY.md --mkdocs config.yml

    # Rename files and update SUMMARY.md
    python mdbook-mkdocs-sync.py --rename-files

    # Preview changes without applying
    python mdbook-mkdocs-sync.py --dry-run

GitHub: https://github.com/iepathos/mdbook-mkdocs-sync (future)
License: MIT
"""

import re
import sys
import argparse
from pathlib import Path
from typing import List, Dict, Any, Optional, Tuple, Set

try:
    import yaml
except ImportError:
    print("Error: PyYAML is required. Install with: pip install pyyaml")
    sys.exit(1)


class MdBookMkDocsSync:
    """Sync mdbook SUMMARY.md to mkdocs.yml navigation."""

    def __init__(
        self,
        summary_path: Path,
        mkdocs_path: Path,
        rename_files: bool = False,
        dry_run: bool = False
    ):
        self.summary_path = summary_path
        self.mkdocs_path = mkdocs_path
        self.rename_files = rename_files
        self.dry_run = dry_run
        self.docs_dir = self._detect_docs_dir()
        self.changes = []

    def _detect_docs_dir(self) -> Path:
        """Detect the docs_dir from mkdocs.yml or use default."""
        if not self.mkdocs_path.exists():
            # Default to mdbook structure
            return self.summary_path.parent

        # Simple text parsing to avoid YAML tag issues
        with open(self.mkdocs_path, 'r') as f:
            for line in f:
                if line.startswith('docs_dir:'):
                    docs_dir = line.split(':', 1)[1].strip()
                    # Remove quotes if present
                    docs_dir = docs_dir.strip('"').strip("'")
                    return Path(docs_dir)

        # Default if not found
        return Path('docs')

    def parse_summary(self) -> List[Dict[str, Any]]:
        """
        Parse mdbook's SUMMARY.md and convert to mkdocs nav structure.

        Handles:
        - Section headers (# User Guide, # Advanced Topics, etc.)
        - Nested navigation (indentation)
        - intro.md → index.md mapping
        - Chapter index pages
        - MkDocs Material tab structure

        Returns a list of nav items in mkdocs format optimized for Material theme.
        """
        nav = []
        current_section = None
        current_section_name = None
        section_items = []
        home_page_found = False
        processed_lines = set()  # Track which lines we've already processed

        with open(self.summary_path, 'r') as f:
            lines = f.readlines()

        line_num = 0
        while line_num < len(lines):
            line = lines[line_num].rstrip()
            line_num += 1

            if not line:
                continue

            # Skip if already processed (as child of parent item)
            if line_num in processed_lines:
                continue

            # Check for section headers (# User Guide, # Advanced Topics, etc.)
            if line.startswith('#'):
                # Save previous section if exists
                if current_section_name and section_items:
                    nav.append({current_section_name: section_items})
                    section_items = []

                # Extract new section name
                section_header = line.lstrip('#').strip()

                # Skip "Summary" header
                if section_header.lower() == 'summary':
                    continue

                # Normalize section name for MkDocs Material
                current_section_name = self._normalize_section_name(section_header)
                current_section = section_header
                self.changes.append(f"Found section: {section_header} → {current_section_name}")
                continue

            # Match mdbook list items: - [Title](path.md) or [Title](path.md) (for first entry)
            match = re.match(r'^(\s*)(?:- )?\[(.+?)\]\((.+?)\)', line)
            if not match:
                continue

            indent_str, title, path = match.groups()
            indent_level = len(indent_str) // 2  # 2 spaces per level in mdbook

            # Only process top-level items (indent 0) - skip children, they'll be collected by parent
            if indent_level > 0:
                continue

            # Handle intro.md → index.md conversion
            original_path = path
            if self._is_home_page(path):
                path = 'index.md'
                home_page_found = True
                if original_path != 'index.md':
                    self.changes.append(f"Map home page: {original_path} → {path}")

                # Introduction goes in Home tab (before sections)
                nav.insert(0, {'Home': path})
                continue

            # Parse this item and its children
            nav_item, consumed_lines = self._parse_nav_item_with_tracking(
                lines, line_num - 1, title, path, indent_level
            )

            # Mark consumed lines as processed
            processed_lines.update(consumed_lines)

            # Add to current section or top-level
            if current_section_name:
                section_items.append(nav_item)
            else:
                # Items before first section go to top-level
                nav.append(nav_item)

        # Add final section if exists
        if current_section_name and section_items:
            nav.append({current_section_name: section_items})

        if not home_page_found:
            self.changes.append("Warning: No home page found (intro.md or index.md)")

        return nav

    def _normalize_section_name(self, section: str) -> str:
        """Normalize mdbook section names to MkDocs Material tab names."""
        # Mapping from mdbook section names to cleaner MkDocs tab names
        mapping = {
            'User Guide': 'User Guide',
            'Advanced Topics': 'Advanced',
            'Reference': 'Reference',
            'API Reference': 'API',
            'Developer Guide': 'Development',
        }
        return mapping.get(section, section)

    def _parse_nav_item_with_tracking(
        self,
        lines: List[str],
        current_line: int,
        title: str,
        path: str,
        indent_level: int
    ) -> Tuple[Dict[str, Any], Set[int]]:
        """
        Parse a navigation item and its children, tracking which lines were consumed.

        Returns: (nav_item, set of line numbers that were processed)
        """
        consumed_lines = set()

        # Look ahead to see if this item has children
        has_children = False
        if current_line + 1 < len(lines):
            next_line = lines[current_line + 1]
            next_match = re.match(r'^(\s*)- \[', next_line)
            if next_match:
                next_indent = len(next_match.group(1)) // 2
                has_children = next_indent > indent_level

        # Create nav item
        if has_children or path.endswith('/index.md'):
            # This is a parent item with children
            children = []

            # Collect all children at the next indent level
            i = current_line + 1
            while i < len(lines):
                child_line = lines[i].rstrip()

                # Stop at section headers
                if child_line.startswith('#'):
                    break

                # Skip empty lines
                if not child_line:
                    i += 1
                    continue

                child_match = re.match(r'^(\s*)- \[(.+?)\]\((.+?)\)', child_line)
                if child_match:
                    child_indent_str, child_title, child_path = child_match.groups()
                    child_indent = len(child_indent_str) // 2

                    # Stop if we've gone back to same or lower indent
                    if child_indent <= indent_level:
                        break

                    # Only process direct children (one level deeper)
                    if child_indent == indent_level + 1:
                        children.append({child_title: child_path})
                        consumed_lines.add(i + 1)  # +1 because line numbers are 1-indexed

                i += 1

            # Return section with index and children
            if path and not path.endswith('/'):
                return {title: [path] + children}, consumed_lines
            else:
                return {title: children}, consumed_lines
        else:
            # Simple page without children
            return {title: path}, consumed_lines

    def _is_home_page(self, path: str) -> bool:
        """Check if path is a home page (intro.md, README.md, etc.)."""
        home_pages = ['intro.md', 'readme.md', 'introduction.md']
        return path.lower() in home_pages or path == 'index.md'

    def update_mkdocs_nav(self, nav: List[Dict[str, Any]]):
        """Update the nav section in mkdocs.yml while preserving other config."""
        if not self.mkdocs_path.exists():
            print(f"✗ Error: {self.mkdocs_path} not found")
            return False

        # Read the full file to preserve Python-specific YAML tags
        with open(self.mkdocs_path, 'r') as f:
            content = f.read()

        # Count old nav items for reporting
        old_nav_count = content.count('\n  - ') + content.count('\n    - ')

        if self.dry_run:
            print(f"\n[DRY RUN] Would update {self.mkdocs_path}")
            print(f"  Old nav items: ~{old_nav_count}")
            print(f"  New nav items: {len(nav)}")
            return True

        # Use regex-based replacement to preserve all other YAML structure
        return self._update_nav_in_place(content, nav)

    def _update_nav_in_place(self, content: str, nav: List[Dict[str, Any]]) -> bool:
        """Update only the nav section in the YAML file, preserving everything else."""
        # Generate the new nav YAML
        nav_yaml = yaml.dump(nav, default_flow_style=False, sort_keys=False, allow_unicode=True, width=1000)

        # Indent each line of nav_yaml by 2 spaces (to fit under 'nav:')
        nav_lines = nav_yaml.rstrip().split('\n')
        indented_nav = '\n'.join('  ' + line if line else '' for line in nav_lines)

        # Find and replace the nav section using regex
        # Match from 'nav:' until the next top-level key (starts with letter, not indented) or EOF
        # This pattern matches the nav: line, then any indented lines or blank lines,
        # until we hit a line that starts with a letter (next YAML key) or end of file
        pattern = r'^nav:.*?\n((?:[ \t]+.*\n|\s*\n)*?)(?=^[a-zA-Z]|\Z)'

        def replace_nav(match):
            return f'nav:\n{indented_nav}\n\n'

        new_content, count = re.subn(pattern, replace_nav, content, flags=re.MULTILINE | re.DOTALL)

        if count == 0:
            # No existing nav section found, append it
            new_content = content.rstrip() + '\n\n# Navigation\nnav:\n' + indented_nav + '\n'

        # Write the updated content
        with open(self.mkdocs_path, 'w') as f:
            f.write(new_content)

        print(f"✓ Updated {self.mkdocs_path}")
        print(f"  Navigation items: {len(nav)}")
        return True

    def rename_home_page(self):
        """Rename intro.md to index.md if needed."""
        intro_path = self.docs_dir / 'intro.md'
        index_path = self.docs_dir / 'index.md'

        if not intro_path.exists():
            return  # Nothing to rename

        if index_path.exists():
            print(f"ℹ️  {index_path} already exists, skipping rename")
            return

        if self.dry_run:
            print(f"\n[DRY RUN] Would rename {intro_path} → {index_path}")
            self._show_summary_updates('intro.md', 'index.md')
            return

        # Rename the file
        intro_path.rename(index_path)
        print(f"✓ Renamed {intro_path} → {index_path}")
        self.changes.append(f"Renamed: {intro_path.name} → {index_path.name}")

        # Update SUMMARY.md references
        self._update_summary_references('intro.md', 'index.md')

    def _update_summary_references(self, old_name: str, new_name: str):
        """Update SUMMARY.md to reference new filename."""
        with open(self.summary_path, 'r') as f:
            content = f.read()

        # Replace references
        pattern = r'\((' + re.escape(old_name) + r')\)'
        updated_content = re.sub(pattern, f'({new_name})', content)

        if content == updated_content:
            return  # No changes needed

        with open(self.summary_path, 'w') as f:
            f.write(updated_content)

        print(f"✓ Updated {self.summary_path} references")

    def _show_summary_updates(self, old_name: str, new_name: str):
        """Show what would be updated in SUMMARY.md."""
        with open(self.summary_path, 'r') as f:
            content = f.read()

        pattern = r'\((' + re.escape(old_name) + r')\)'
        matches = re.findall(pattern, content)

        if matches:
            print(f"  Would update {len(matches)} reference(s) in {self.summary_path}")

    def run(self) -> int:
        """Run the sync process."""
        print("🔄 mdbook → mkdocs Navigation Sync")
        print("=" * 50)

        # Verify files exist
        if not self.summary_path.exists():
            print(f"✗ Error: {self.summary_path} not found")
            return 1

        if not self.mkdocs_path.exists():
            print(f"✗ Error: {self.mkdocs_path} not found")
            print(f"  Create a basic mkdocs.yml first with:")
            print(f"    docs_dir: {self.docs_dir}")
            return 1

        # Parse SUMMARY.md
        print(f"\n📖 Parsing {self.summary_path}...")
        nav = self.parse_summary()

        # Show detected changes
        if self.changes:
            print(f"\n📝 Detected changes:")
            for change in self.changes:
                print(f"  • {change}")

        # Rename files if requested
        if self.rename_files:
            print(f"\n📁 Renaming files...")
            self.rename_home_page()

        # Update mkdocs.yml
        print(f"\n📝 Updating {self.mkdocs_path}...")
        if not self.update_mkdocs_nav(nav):
            return 1

        # Summary
        print(f"\n✅ Sync complete!")
        print(f"   mdbook uses: {self.summary_path}")
        print(f"   mkdocs uses: {self.mkdocs_path}")
        print(f"\nBoth navigation structures now reference the same source files in {self.docs_dir}/")

        if self.dry_run:
            print("\n💡 This was a dry run. Use without --dry-run to apply changes.")

        return 0


def main():
    parser = argparse.ArgumentParser(
        description='Sync mdbook SUMMARY.md to mkdocs.yml navigation',
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  %(prog)s                                    # Basic usage
  %(prog)s --rename-files                     # Rename intro.md to index.md
  %(prog)s --dry-run                          # Preview changes
  %(prog)s --summary docs/SUMMARY.md          # Custom paths

For more info: https://github.com/iepathos/mdbook-mkdocs-sync
        """
    )

    parser.add_argument(
        '--summary',
        type=Path,
        default=Path('book/src/SUMMARY.md'),
        help='Path to SUMMARY.md (default: book/src/SUMMARY.md)'
    )

    parser.add_argument(
        '--mkdocs',
        type=Path,
        default=Path('mkdocs.yml'),
        help='Path to mkdocs.yml (default: mkdocs.yml)'
    )

    parser.add_argument(
        '--rename-files',
        action='store_true',
        help='Rename intro.md to index.md and update SUMMARY.md'
    )

    parser.add_argument(
        '--dry-run',
        action='store_true',
        help='Show what would be done without making changes'
    )

    parser.add_argument(
        '--version',
        action='version',
        version='%(prog)s 0.1.0'
    )

    args = parser.parse_args()

    syncer = MdBookMkDocsSync(
        summary_path=args.summary,
        mkdocs_path=args.mkdocs,
        rename_files=args.rename_files,
        dry_run=args.dry_run
    )

    return syncer.run()


if __name__ == '__main__':
    sys.exit(main())
