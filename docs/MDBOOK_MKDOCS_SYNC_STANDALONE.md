# mdbook-mkdocs-sync: Standalone Tool Plan

## Overview

`mdbook-mkdocs-sync` solves a universal problem: maintaining dual mdbook/mkdocs documentation from a single source. This should be its own standalone tool/repository.

## Why Separate It?

### Universal Problem
- Every project with both mdbook and mkdocs faces this
- Not specific to Prodigy
- Reusable across the Rust/Python ecosystem

### Benefits of Standalone Tool
- ✅ **Reusable**: Any project can use it
- ✅ **Better Testing**: Dedicated CI/CD
- ✅ **Community**: Accept contributions
- ✅ **Discoverability**: PyPI, crates.io, npm (choose one)
- ✅ **Portfolio**: Demonstrates tooling expertise
- ✅ **Integration**: Can be used in Prodigy workflows as a dependency

### Showcases Prodigy
- "Built by the Prodigy team" branding
- Example of tools created to solve real problems
- Demonstrates ecosystem building
- Can reference in Prodigy docs as companion tool

## Proposed Repository Structure

```
mdbook-mkdocs-sync/
├── README.md                   # Main documentation
├── LICENSE                     # MIT License
├── pyproject.toml             # Python packaging (modern)
├── setup.py                   # Python packaging (legacy compat)
├── requirements.txt           # Dependencies
├── requirements-dev.txt       # Dev dependencies
│
├── mdbook_mkdocs_sync/        # Python package
│   ├── __init__.py
│   ├── __main__.py            # CLI entry point
│   ├── cli.py                 # CLI argument parsing
│   ├── parser.py              # SUMMARY.md parser
│   ├── converter.py           # mkdocs nav converter
│   └── utils.py               # Utilities
│
├── tests/                     # Test suite
│   ├── test_parser.py
│   ├── test_converter.py
│   ├── fixtures/
│   │   ├── example_summary.md
│   │   └── expected_nav.yml
│   └── integration/
│       └── test_full_sync.py
│
├── examples/                  # Example projects
│   ├── simple/
│   │   ├── SUMMARY.md
│   │   └── mkdocs.yml
│   └── complex/
│       ├── SUMMARY.md
│       └── mkdocs.yml
│
├── docs/                      # Tool documentation (dogfooding!)
│   ├── index.md
│   ├── installation.md
│   ├── usage.md
│   ├── configuration.md
│   └── examples.md
│
├── .github/
│   └── workflows/
│       ├── test.yml           # Run tests on PR
│       ├── publish.yml        # Publish to PyPI
│       └── docs.yml           # Deploy docs
│
└── CHANGELOG.md               # Version history
```

## Package Metadata (pyproject.toml)

```toml
[build-system]
requires = ["setuptools>=61.0", "wheel"]
build-backend = "setuptools.build_meta"

[project]
name = "mdbook-mkdocs-sync"
version = "0.1.0"
description = "Sync mdbook SUMMARY.md to mkdocs.yml navigation"
readme = "README.md"
authors = [
    {name = "Glen Baker", email = "iepathos@gmail.com"}
]
license = {text = "MIT"}
keywords = ["mdbook", "mkdocs", "documentation", "sync", "conversion"]
classifiers = [
    "Development Status :: 4 - Beta",
    "Intended Audience :: Developers",
    "License :: OSI Approved :: MIT License",
    "Programming Language :: Python :: 3",
    "Programming Language :: Python :: 3.8",
    "Programming Language :: Python :: 3.9",
    "Programming Language :: Python :: 3.10",
    "Programming Language :: Python :: 3.11",
    "Programming Language :: Python :: 3.12",
    "Topic :: Documentation",
    "Topic :: Software Development :: Documentation",
    "Topic :: Utilities",
]
requires-python = ">=3.8"
dependencies = [
    "pyyaml>=6.0",
]

[project.optional-dependencies]
dev = [
    "pytest>=7.0",
    "pytest-cov>=4.0",
    "black>=23.0",
    "mypy>=1.0",
    "types-PyYAML",
]

[project.urls]
Homepage = "https://github.com/iepathos/mdbook-mkdocs-sync"
Documentation = "https://iepathos.github.io/mdbook-mkdocs-sync"
Repository = "https://github.com/iepathos/mdbook-mkdocs-sync"
Issues = "https://github.com/iepathos/mdbook-mkdocs-sync/issues"
Changelog = "https://github.com/iepathos/mdbook-mkdocs-sync/blob/main/CHANGELOG.md"

[project.scripts]
mdbook-mkdocs-sync = "mdbook_mkdocs_sync.__main__:main"

[tool.setuptools.packages.find]
where = ["."]
include = ["mdbook_mkdocs_sync*"]

[tool.pytest.ini_options]
testpaths = ["tests"]
python_files = ["test_*.py"]
python_classes = ["Test*"]
python_functions = ["test_*"]

[tool.mypy]
python_version = "3.8"
warn_return_any = true
warn_unused_configs = true
disallow_untyped_defs = true

[tool.black]
line-length = 88
target-version = ['py38', 'py39', 'py310', 'py311']
```

## Installation (After Publishing)

### From PyPI
```bash
pip install mdbook-mkdocs-sync
```

### From GitHub (Latest)
```bash
pip install git+https://github.com/iepathos/mdbook-mkdocs-sync.git
```

### For Development
```bash
git clone https://github.com/iepathos/mdbook-mkdocs-sync.git
cd mdbook-mkdocs-sync
pip install -e ".[dev]"
```

## Usage

### As CLI Tool
```bash
# Basic usage
mdbook-mkdocs-sync

# With options
mdbook-mkdocs-sync --summary docs/SUMMARY.md --mkdocs config.yml

# Rename intro.md to index.md
mdbook-mkdocs-sync --rename-files

# Preview changes
mdbook-mkdocs-sync --dry-run
```

### As Python Library
```python
from mdbook_mkdocs_sync import MdBookMkDocsSync
from pathlib import Path

syncer = MdBookMkDocsSync(
    summary_path=Path('book/src/SUMMARY.md'),
    mkdocs_path=Path('mkdocs.yml'),
    rename_files=True
)
syncer.run()
```

### In GitHub Actions
```yaml
# .github/workflows/sync-docs.yml
name: Sync Documentation Navigation

on:
  push:
    paths:
      - 'book/src/SUMMARY.md'

jobs:
  sync:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3

      - uses: actions/setup-python@v4
        with:
          python-version: '3.11'

      - name: Install mdbook-mkdocs-sync
        run: pip install mdbook-mkdocs-sync

      - name: Sync navigation
        run: mdbook-mkdocs-sync

      - name: Commit changes
        run: |
          git config user.name "GitHub Actions"
          git config user.email "actions@github.com"
          git add mkdocs.yml
          git commit -m "docs: sync mkdocs nav from SUMMARY.md" || true
          git push
```

### In Prodigy Workflows
```yaml
# workflows/sync-docs-nav.yml
name: sync-docs-navigation
mode: standard

steps:
  - shell: "pip install mdbook-mkdocs-sync"
  - shell: "mdbook-mkdocs-sync --rename-files"
  - shell: "git add mkdocs.yml book/src/SUMMARY.md book/src/index.md"
  - shell: "git commit -m 'docs: sync navigation structures' || true"
```

## Features to Add

### V0.1.0 (MVP)
- [x] Parse SUMMARY.md
- [x] Generate mkdocs nav structure
- [x] Handle intro.md → index.md
- [x] CLI interface
- [x] Dry-run mode
- [ ] Basic tests
- [ ] README with examples
- [ ] PyPI publishing

### V0.2.0
- [ ] Reverse sync (mkdocs.yml → SUMMARY.md)
- [ ] Watch mode (auto-sync on changes)
- [ ] Configuration file support
- [ ] Custom mappings (e.g., README.md → index.md)
- [ ] Validation (detect broken links)

### V0.3.0
- [ ] Pre-commit hook integration
- [ ] GitHub Action (standalone)
- [ ] VS Code extension
- [ ] GUI (optional)

### V1.0.0
- [ ] Full bidirectional sync
- [ ] Conflict resolution
- [ ] Comprehensive test suite
- [ ] Complete documentation
- [ ] Stable API

## Testing Strategy

### Unit Tests
```python
# tests/test_parser.py
def test_parse_simple_summary():
    summary = """
    # Summary
    - [Home](index.md)
    - [Chapter 1](chapter1.md)
    """
    parser = SummaryParser(summary)
    nav = parser.parse()
    assert len(nav) == 2
    assert nav[0] == {'Home': 'index.md'}

def test_intro_to_index_mapping():
    summary = "- [Introduction](intro.md)"
    parser = SummaryParser(summary)
    nav = parser.parse()
    assert nav[0] == {'Introduction': 'index.md'}
```

### Integration Tests
```python
# tests/integration/test_full_sync.py
def test_full_sync(tmp_path):
    # Setup
    summary_path = tmp_path / "SUMMARY.md"
    mkdocs_path = tmp_path / "mkdocs.yml"

    summary_path.write_text(SAMPLE_SUMMARY)
    mkdocs_path.write_text(SAMPLE_MKDOCS)

    # Run
    syncer = MdBookMkDocsSync(summary_path, mkdocs_path)
    result = syncer.run()

    # Verify
    assert result == 0
    with open(mkdocs_path) as f:
        config = yaml.safe_load(f)
    assert len(config['nav']) > 0
```

## Documentation

### README.md Structure
```markdown
# mdbook-mkdocs-sync

Sync mdbook SUMMARY.md to mkdocs.yml navigation

## Why?

When maintaining both mdbook and mkdocs documentation...

## Features

- ✅ Automatic navigation sync
- ✅ Handle intro.md → index.md
- ✅ Preserve mkdocs.yml config
- ✅ Dry-run mode
- ✅ CLI and Python API

## Installation

pip install mdbook-mkdocs-sync

## Quick Start

mdbook-mkdocs-sync --rename-files

## Documentation

Full docs: https://iepathos.github.io/mdbook-mkdocs-sync

## Examples

See examples/

## Contributing

See CONTRIBUTING.md

## License

MIT

## Credits

Created by the Prodigy team to solve documentation sync challenges.
Part of the Prodigy ecosystem: https://github.com/iepathos/prodigy
```

## Publishing Workflow

### To PyPI
```bash
# Build
python -m build

# Test upload
python -m twine upload --repository testpypi dist/*

# Production upload
python -m twine upload dist/*
```

### GitHub Actions (Auto-publish)
```yaml
# .github/workflows/publish.yml
name: Publish to PyPI

on:
  release:
    types: [published]

jobs:
  publish:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3

      - uses: actions/setup-python@v4
        with:
          python-version: '3.11'

      - name: Install build tools
        run: pip install build twine

      - name: Build package
        run: python -m build

      - name: Publish to PyPI
        env:
          TWINE_USERNAME: __token__
          TWINE_PASSWORD: ${{ secrets.PYPI_TOKEN }}
        run: python -m twine upload dist/*
```

## Integration with Prodigy

### Use in Prodigy Docs
Update `requirements.txt`:
```
mkdocs-material>=9.5.0
mdbook-mkdocs-sync>=0.1.0
```

### Prodigy Workflow
```yaml
# workflows/sync-docs-nav.yml
name: sync-documentation-navigation
mode: standard

env:
  SUMMARY_PATH: "book/src/SUMMARY.md"
  MKDOCS_PATH: "mkdocs.yml"

steps:
  - shell: "mdbook-mkdocs-sync --summary ${SUMMARY_PATH} --mkdocs ${MKDOCS_PATH} --rename-files"
    on_failure:
      claude: "/fix-navigation-sync-issues"
      commit_required: true

  - shell: "git add ${MKDOCS_PATH} ${SUMMARY_PATH} book/src/index.md"
  - shell: "git commit -m 'docs: sync navigation structures' || true"
```

### Reference in Prodigy Docs
```markdown
## Maintaining Dual Documentation

Prodigy supports both mdbook and MkDocs Material from a single source.
We use [`mdbook-mkdocs-sync`](https://github.com/iepathos/mdbook-mkdocs-sync)
to keep navigation in sync.

Install it:
```bash
pip install mdbook-mkdocs-sync
```

Then sync:
```bash
mdbook-mkdocs-sync --rename-files
```

Or use our Prodigy workflow:
```bash
prodigy run workflows/sync-docs-nav.yml
```
```

## Marketing/Positioning

### Tagline
"Bridge the gap between mdbook and MkDocs Material"

### Description
```
mdbook-mkdocs-sync automatically synchronizes navigation between
mdbook's SUMMARY.md and mkdocs.yml, enabling you to maintain dual
documentation systems from a single source.

Perfect for projects that want:
- Rust's mdbook for code documentation
- MkDocs Material for marketing/user docs
- Single source of truth for content
- Automatic navigation sync
```

### Use Cases
- Rust projects with dual docs (crates.io + readthedocs)
- Projects migrating from mdbook to mkdocs
- Teams that want format flexibility
- Documentation-as-code workflows

## Roadmap to Standalone Repo

### Phase 1: Extract (Week 1)
- [ ] Create new GitHub repo: `iepathos/mdbook-mkdocs-sync`
- [ ] Copy enhanced script to new repo
- [ ] Add tests, README, LICENSE
- [ ] Set up CI/CD
- [ ] Create initial release (v0.1.0)

### Phase 2: Publish (Week 2)
- [ ] Publish to PyPI
- [ ] Create documentation site (using MkDocs Material!)
- [ ] Write blog post announcing tool
- [ ] Submit to relevant communities (r/rust, etc.)

### Phase 3: Integrate Back (Week 3)
- [ ] Add as dependency in Prodigy
- [ ] Update Prodigy docs to reference tool
- [ ] Create Prodigy workflow examples
- [ ] Document in automated-documentation chapter

### Phase 4: Enhance (Ongoing)
- [ ] Add reverse sync (mkdocs → mdbook)
- [ ] Add watch mode
- [ ] Create GitHub Action
- [ ] Add more test coverage
- [ ] Accept community contributions

## Benefits Summary

### For Users
- ✅ Solve a real problem
- ✅ Easy to use
- ✅ Well tested
- ✅ Actively maintained
- ✅ Free and open source

### For Prodigy
- ✅ Demonstrates ecosystem building
- ✅ Shows tool creation capability
- ✅ Attracts developers
- ✅ Portfolio piece
- ✅ Community engagement

### For You (Glen)
- ✅ Another tool in portfolio
- ✅ PyPI publishing experience
- ✅ Community project
- ✅ Potential citations/stars
- ✅ Demonstrates problem-solving

## Next Steps

1. **Decide**: Should this be a separate repo?
2. **If yes**:
   - Create `iepathos/mdbook-mkdocs-sync` repo
   - Copy enhanced script
   - Add packaging files
   - Publish v0.1.0
3. **If no**:
   - Keep in Prodigy as script
   - Document in Prodigy docs
   - Consider extracting later

## Recommendation

**YES - Make it standalone**

Reasons:
1. **Solves universal problem** (not Prodigy-specific)
2. **Easy to maintain separately** (simple scope)
3. **Good marketing** for Prodigy ("we built this")
4. **Portfolio builder** (shows tool creation)
5. **Quick to publish** (MVP ready now)
6. **Low risk** (small, focused tool)

The tool is ready for standalone release. Just needs packaging and docs!
