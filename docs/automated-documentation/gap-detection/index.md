# Automatic Gap Detection

Automatic gap detection is a critical component of Prodigy's documentation workflow that identifies undocumented features and automatically creates chapter/subsection definitions with stub markdown files. This ensures comprehensive documentation coverage and prevents features from being implemented without corresponding user guidance.

!!! note "Why Gap Detection Matters"
    Without gap detection, new features can silently go undocumented. Gap detection acts as a safety net, automatically flagging when code features outpace their documentation.

**Source**: Implemented in `.claude/commands/prodigy-detect-documentation-gaps.md:1-509` and tested in `tests/documentation_gap_detection_test.rs:1-677`

## How Gap Detection Works

```mermaid
flowchart LR
    subgraph Input["Input Analysis"]
        direction TB
        Features["features.json
        from feature analysis"]
        Chapters["chapters.json
        existing definitions"]
    end

    subgraph Process["Gap Detection Process"]
        direction TB
        Compare["Compare Features
        vs Documentation"]
        Validate["Validate Content
        Sufficiency"]
        Classify["Classify Gaps
        by Severity"]
    end

    subgraph Output["Outputs"]
        direction TB
        Report["gap-report.json"]
        Stubs["Stub markdown files"]
        Flat["flattened-items.json
        for map phase"]
    end

    Features --> Compare
    Chapters --> Compare
    Compare --> Validate
    Validate --> Classify
    Classify --> Report
    Classify --> Stubs
    Classify --> Flat

    style Input fill:#e1f5ff
    style Process fill:#fff3e0
    style Output fill:#e8f5e9
```

**Figure**: Gap detection analyzes features against existing documentation, validates content sufficiency, and generates outputs for the map phase.

## Subpages

<div class="grid cards" markdown>

-   :material-eye-outline: **[Overview](overview.md)**

    ---

    Command usage and gap severity classification

-   :material-check-decagram: **[Validation Phases](validation-phases.md)**

    ---

    Content sufficiency validation, structure validation, and flattened items generation

-   :material-format-list-checks: **[Topic Normalization](topic-normalization.md)**

    ---

    Topic normalization and idempotence guarantees

-   :material-file-document-alert-outline: **[Gap Report](gap-report.md)**

    ---

    Gap report structure, execution progress, error handling, and testing

</div>
