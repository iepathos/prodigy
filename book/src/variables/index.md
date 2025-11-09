# Variable Interpolation

## Overview

Prodigy provides two complementary variable systems:

1. **Built-in Variables**: Automatically available based on workflow context (workflow state, step info, work items, etc.)
2. **Custom Captured Variables**: User-defined variables created via the `capture:` field in commands

Both systems use the same `${variable.name}` interpolation syntax and can be freely mixed in your workflows.

## Variable Availability by Phase

| Variable Category | Setup | Map | Reduce | Merge |
|------------------|-------|-----|--------|-------|
| Standard Variables | ✓ | ✓ | ✓ | ✓ |
| Output Variables | ✓ | ✓ | ✓ | ✓ |
| Item Variables (`${item.*}`) | ✗ | ✓ | ✗ | ✗ |
| Map Aggregation (`${map.total}`, etc.) | ✗ | ✗ | ✓ | ✗ |
| Merge Variables | ✗ | ✗ | ✗ | ✓ |
| Custom Captured Variables | ✓ | ✓ | ✓ | ✓ |

**Note**: Using phase-specific variables outside their designated phase (e.g., `${item}` in reduce phase, `${map.results}` in map phase) will result in interpolation errors or empty values. Always verify variable availability matches your workflow phase.

**Reduce Phase Access to Item Data**: In reduce phase, individual item variables (`${item.*}`) are not directly available, but you can access all item data through `${map.results}` which contains the aggregated results from all map agents. This allows you to process item-level information during aggregation.


## Additional Topics

See also:
- [Available Variables](available-variables.md)
- [Custom Variable Capture](custom-variable-capture.md)
- [Troubleshooting Variable Interpolation](troubleshooting-variable-interpolation.md)
- [See Also](see-also.md)
