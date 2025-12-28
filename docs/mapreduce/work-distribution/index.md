# Work Distribution

Work distribution is the process of extracting, filtering, sorting, and distributing work items to parallel agents in MapReduce workflows. The data pipeline provides powerful capabilities for selecting and organizing work items from various input sources.

## Overview

The work distribution system processes data through a multi-stage pipeline:

1. **Input Source** - Load data from JSON files or command output
2. **JSONPath Extraction** - Extract work items from nested structures
3. **Filtering** - Select items matching criteria
4. **Sorting** - Order items by priority or other fields
5. **Deduplication** - Remove duplicate items
6. **Pagination** - Apply offset and limit for testing or batching

Each stage is optional and can be configured independently to build the exact work distribution strategy you need.

```mermaid
flowchart LR
    Input[Input Source<br/>JSON file or command] --> JSONPath[JSONPath Extraction<br/>$.items[*]]
    JSONPath --> Filter[Filtering<br/>score >= 5]
    Filter --> Sort[Sorting<br/>priority DESC]
    Sort --> Dedup[Deduplication<br/>distinct: id]
    Dedup --> Offset[Offset<br/>skip first N]
    Offset --> Limit[Limit<br/>take M items]
    Limit --> Agents[Distribute to<br/>Parallel Agents]

    style Input fill:#e1f5ff
    style JSONPath fill:#fff3e0
    style Filter fill:#f3e5f5
    style Sort fill:#e8f5e9
    style Dedup fill:#fff3e0
    style Offset fill:#f3e5f5
    style Limit fill:#e1f5ff
    style Agents fill:#ffebee
```

**Figure**: Work distribution pipeline showing data flow from input source through transformation stages to parallel agents.

## Subpages

This section is organized into the following pages:

- **[Input Sources](input-sources.md)** - Loading data from JSON files and command output, plus JSONPath extraction patterns
- **[Filtering & Sorting](filtering-sorting.md)** - Selecting and ordering work items with filter expressions, sort specifications, and deduplication
- **[Pagination](pagination.md)** - Controlling batch sizes with offset/limit and understanding the processing pipeline order
- **[Examples](examples.md)** - Complete workflow examples, map phase integration, and troubleshooting tips
