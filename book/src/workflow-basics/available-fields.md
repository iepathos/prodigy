## Available Fields

Standard workflows support these top-level fields:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `commands` | Array | Yes* | List of commands to execute sequentially |
| `env` | Map | No | Global environment variables |
| `secrets` | Map | No | Secret environment variables (masked in logs) |
| `env_files` | Array | No | Paths to .env files to load |
| `profiles` | Map | No | Named environment profiles |
| `merge` | Object | No | Custom merge workflow for worktree integration |

**Note:** `commands` is only required in the full format. Simple array format doesn't use the `commands` key.

