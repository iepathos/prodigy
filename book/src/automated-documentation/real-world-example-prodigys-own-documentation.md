## Real-World Example: Prodigy's Own Documentation

This documentation you're reading is maintained by the same workflow described here. You can examine the configuration:

**Configuration**: `.prodigy/book-config.json`
```json
{
  "project_name": "Prodigy",
  "project_type": "cli_tool",
  "analysis_targets": [
    {
      "area": "workflow_execution",
      "source_files": ["src/workflow/", "src/orchestrator/"],
      "feature_categories": ["workflow_types", "execution_modes", "lifecycle"]
    },
    {
      "area": "mapreduce",
      "source_files": ["src/mapreduce/"],
      "feature_categories": ["map_phase", "reduce_phase", "parallelism"]
    }
  ]
}
```

**Chapters**: `workflows/data/prodigy-chapters.json`
```json
{
  "chapters": [
    {
      "id": "workflow-basics",
      "title": "Workflow Basics",
      "file": "book/src/workflow-basics.md",
      "topics": ["Standard workflows", "Basic structure"],
      "validation": "Check workflow syntax matches current implementation"
    }
  ]
}
```

**Workflow**: `workflows/book-docs-drift.yml`

Study these files for a complete working example.

