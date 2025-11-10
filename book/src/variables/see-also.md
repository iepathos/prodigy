## See Also

### Related Documentation

- **[Environment Variables](../environment/)** - Configure environment variables, secrets, and profiles for workflow parameterization
- **[Git Context Advanced](../git-context-advanced.md)** - Deep dive into git variable formats, filtering, and advanced use cases
- **[MapReduce Workflows](../mapreduce/)** - Using item.* and map.* variables in distributed parallel processing
- **[Commands](../commands.md)** - Command-specific capture_output configuration and command types
- **[Examples](../examples.md)** - Real-world variable usage patterns and complete workflows
- **[Troubleshooting](../troubleshooting/)** - Debugging workflows with variable-related issues

### Key Concepts

- **Built-in vs Custom Variables**: Built-in variables are automatic (workflow.*, item.*, etc.), while custom variables require explicit capture_output configuration
- **Phase Availability**: Variables have different availability depending on workflow phase (setup, map, reduce, merge)
- **Variable Scoping**: Setup captures are workflow-wide, map captures are agent-local, reduce captures are step-forward
- **Format Modifiers**: Git context variables support format modifiers (:json, :lines, :csv, :*.ext), while custom captures use capture_format
- **Metadata Fields**: All captured variables include .success, .exit_code, .stderr, and .duration fields

### External Resources

- **[JSONPath Syntax](https://goessner.net/articles/JsonPath/)** - For json_path in MapReduce input extraction
- **[Glob Pattern Syntax](https://en.wikipedia.org/wiki/Glob_(programming))** - For git context filtering with `:*.ext` modifiers
- **[jq Manual](https://stedolan.github.io/jq/manual/)** - Essential tool for working with JSON variables in workflows

### Quick Reference

**Phase-Specific Variables:**
- Setup: All standard variables, custom captures (workflow-wide scope)
- Map: item.*, item_index, item_total + inherited setup captures
- Reduce: map.total, map.successful, map.failed, map.results
- Merge: merge.worktree, merge.source_branch, merge.target_branch

**Git Context Variables:**
- step.files_added, step.files_modified, step.files_deleted, step.files_changed
- step.commits, step.commit_count, step.insertions, step.deletions
- workflow.commits, workflow.commit_count
- Format modifiers: :json, :lines, :csv, :*.ext

**Custom Capture:**
- capture_output: "var_name" - Creates ${var_name}
- capture_format: "json|string|lines|number|boolean"
- Metadata: ${var.success}, ${var.exit_code}, ${var.stderr}, ${var.duration}
