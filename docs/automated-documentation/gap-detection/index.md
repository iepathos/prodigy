## Automatic Gap Detection

Automatic gap detection is a critical component of Prodigy's documentation workflow that identifies undocumented features and automatically creates chapter/subsection definitions with stub markdown files. This ensures comprehensive documentation coverage and prevents features from being implemented without corresponding user guidance.

**Source**: Implemented in `.claude/commands/prodigy-detect-documentation-gaps.md:1-1048` and tested in `tests/documentation_gap_detection_test.rs:1-678`

## Subpages

- [Overview](overview.md) - Command usage and gap severity classification
- [Validation Phases](validation-phases.md) - Content sufficiency validation, structure validation, and flattened items generation
- [Topic Normalization](topic-normalization.md) - Topic normalization and idempotence guarantees
- [Gap Report](gap-report.md) - Gap report structure, execution progress, error handling, and testing
