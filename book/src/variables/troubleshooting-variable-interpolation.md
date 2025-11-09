## Troubleshooting Variable Interpolation

This guide helps you diagnose and fix common variable interpolation issues in Prodigy workflows.

### Issue: Variables Not Interpolating

**Symptom:** Literal `${var}` or `$VAR` appears in output instead of the value.

**Common Causes:**

1. **Variable name typo or case mismatch**
   ```yaml
   # Wrong
   - shell: "echo ${Item.path}"  # Should be lowercase: ${item.path}

   # Correct
   - shell: "echo ${item.path}"
   ```

2. **Variable doesn't exist in current scope**
   ```yaml
   # Wrong - using item variable in reduce phase
   reduce:
     - shell: "echo ${item.name}"  # ${item.*} not available here!

   # Correct - access via map.results
   reduce:
     - shell: "echo '${map.results}' | jq -r '.[].item.name'"
   ```

3. **Command failed before setting variable**
   ```yaml
   - shell: "cargo test"  # If this fails...
     capture_output: "tests"
   - shell: "echo ${tests}"  # ...this might be empty

   # Better: Check success first
   - shell: |
       if ${tests.success}; then
         echo "Tests: ${tests}"
       else
         echo "Tests failed: ${tests.stderr}"
       fi
   ```

**Solutions:**
- Check spelling and case sensitivity
- Verify variable exists in current phase (see phase availability table)
- Use verbose mode (`-v`) to see variable values during execution
- Echo variables to debug: `shell: "echo 'DEBUG: var=${my_var}'"`

### Issue: Variable Empty or Undefined

**Symptom:** Variable interpolates but contains empty string or null.

**Common Causes:**

1. **Variable used before being set**
   ```yaml
   # Wrong - using before capture
   - shell: "echo ${count}"
   - shell: "wc -l file.txt"
     capture_output: "count"

   # Correct - capture first, use second
   - shell: "wc -l file.txt"
     capture_output: "count"
   - shell: "echo ${count}"
   ```

2. **Command produced no output**
   ```yaml
   - shell: "find . -name 'nonexistent.txt'"
     capture_output: "result"  # Will be empty if no matches
   - shell: "echo ${result}"  # Empty string
   ```

3. **Capture not configured**
   ```yaml
   # Wrong - forgot capture_output
   - shell: "cargo --version"
   - shell: "echo ${shell.output}"  # Empty unless capture_output: true

   # Correct
   - shell: "cargo --version"
     capture_output: true  # or capture_output: "cargo_version"
   ```

**Solutions:**
- Ensure capture_output is set when you need to save output
- Check command actually produces output
- Use verbose mode to see when variables are set
- Provide defaults: `${var:-default_value}` (shell syntax)

### Issue: Phase-Specific Variable Not Available

**Symptom:** Error about undefined variable or empty value when using phase-specific variables.

**Common Causes:**

| Variable | Wrong Phase | Correct Phase | Fix |
|----------|-------------|---------------|-----|
| `${item.*}` | Reduce, Setup, Merge | Map only | Use `${map.results}` in reduce |
| `${map.*}` | Setup, Map, Merge | Reduce only | Move logic to reduce phase |
| `${merge.*}` | Setup, Map, Reduce | Merge only | Only use in merge commands |

**Example Problem:**
```yaml
# Wrong - can't use ${item} in reduce
reduce:
  - shell: "process ${item.name}"  # ERROR!

# Correct - iterate through map.results
reduce:
  - shell: "echo '${map.results}' | jq -r '.[] | .item.name' | while read name; do process \"$name\"; done"
```

**Solutions:**
- Review phase availability table in main Variables documentation
- Move variable usage to appropriate phase
- In reduce, access item data through `${map.results}`
- Restructure workflow if necessary

### Issue: Nested Field Access Fails

**Symptom:** Can't access nested JSON fields like `${var.field.nested}`.

**Common Causes:**

1. **Format not specified as JSON**
   ```yaml
   # Wrong - no format specification
   - shell: "cargo metadata --format-version 1"
     capture_output: "metadata"
   - shell: "echo ${metadata.workspace_root}"  # Won't work!

   # Correct - specify JSON format
   - shell: "cargo metadata --format-version 1"
     capture_output: "metadata"
     capture_format: "json"
   - shell: "echo ${metadata.workspace_root}"  # Works!
   ```

2. **Field doesn't exist in JSON**
   ```yaml
   - shell: "echo ${item.nonexistent_field}"  # Empty if field missing
   ```

3. **JSON is invalid**
   ```yaml
   # Command produces malformed JSON
   - shell: "echo '{incomplete json'"
     capture_output: "data"
     capture_format: "json"  # Will fail to parse
   ```

**Solutions:**
- Always use `capture_format: "json"` for JSON output
- Verify JSON structure with `jq`: `echo '${var}' | jq .`
- Check field exists: `echo '${var}' | jq -r '.field // "default"'`
- Validate JSON before capture

### Issue: Git Context Variables Empty

**Symptom:** Git variables like `${step.files_added}` are empty.

**Common Causes:**

1. **No commits created**
   ```yaml
   - shell: "echo 'hello' > file.txt"
   - shell: "echo ${step.files_added}"  # Empty - no commit yet!

   # Correct - ensure command creates commit
   - shell: "echo 'hello' > file.txt"
     commit_required: true  # Forces commit
   - shell: "echo ${step.files_added}"  # Now has value
   ```

2. **Not in a git repository**
   ```yaml
   # Git variables require git repo
   - shell: "echo ${step.files_added}"  # Empty if not in git repo
   ```

3. **No files changed in step**
   ```yaml
   - shell: "cargo check"  # Doesn't modify files
   - shell: "echo ${step.files_added}"  # Empty - no files added
   ```

**Solutions:**
- Ensure commands that modify files use `commit_required: true`
- Verify you're in a git repository
- Check that commands actually modify files
- Use `git status` to verify changes exist

### Issue: Format Modifiers Not Working

**Symptom:** Format modifiers like `:json` or `:*.rs` don't apply.

**Common Causes:**

1. **Wrong variable type**
   ```yaml
   # Wrong - not a git context variable
   - shell: "ls"
     capture_output: "files"
   - shell: "echo ${files:json}"  # Format modifiers only work on git vars!

   # Correct - use capture_format instead
   - shell: "ls"
     capture_output: "files"
     capture_format: "json"
   ```

2. **Syntax error**
   ```yaml
   # Wrong syntax
   - shell: "echo ${step.files_added:.rs}"  # Missing * in glob

   # Correct
   - shell: "echo ${step.files_added:*.rs}"
   ```

**Solutions:**
- Format modifiers (`:json`, `:newline`, `:*.ext`) only work on git context variables
- For custom captures, use `capture_format` instead
- Check glob pattern syntax

### Debugging Techniques

#### 1. Use Verbose Mode

```bash
# Run with verbose flag to see variable resolution
prodigy run workflow.yml -v
```

Verbose mode shows:
- Variable values at each step
- When variables are captured
- Interpolated command strings before execution

#### 2. Echo Variables for Debugging

```yaml
- shell: "echo 'DEBUG: item=${item}'"
- shell: "echo 'DEBUG: item.path=${item.path}'"
- shell: "echo 'DEBUG: item_index=${item_index}'"
```

#### 3. Check Claude JSON Logs

Claude command logs contain variable interpolation details:

```bash
# View most recent Claude command log
cat ~/.claude/projects/*/latest.jsonl | jq -c 'select(.type == "assistant")'
```

#### 4. Verify Variables in Checkpoint Files

For resume issues, check checkpoint files:

```bash
# View checkpoint variables
cat ~/.prodigy/state/*/checkpoints/latest.json | jq .variables
```

#### 5. Use jq to Explore JSON Variables

```yaml
# Explore structure
- shell: "echo '${map.results}' | jq ."

# List available keys
- shell: "echo '${metadata}' | jq 'keys'"

# Pretty print
- shell: "echo '${item}' | jq -C ."
```

### Common Syntax Issues

#### Issue: Special Characters in Variables

**Problem:**
```yaml
# Variable contains spaces or special chars
- shell: echo ${item.name}  # Breaks if name has spaces!
```

**Solution:**
```yaml
# Always quote variables in shell commands
- shell: "echo \"${item.name}\""
```

#### Issue: YAML String Escaping

**Problem:**
```yaml
# Single quotes prevent interpolation
- shell: 'echo ${item.name}'  # Literal ${item.name} printed!
```

**Solution:**
```yaml
# Use double quotes for interpolation
- shell: "echo ${item.name}"
```

#### Issue: Combining Variables with Text

**Problem:**
```yaml
# Ambiguous variable name
- shell: "echo $item_path"  # Is it ${item_path} or ${item}_path?
```

**Solution:**
```yaml
# Use ${} syntax to clarify boundaries
- shell: "echo ${item}_path"
- shell: "echo prefix_${item}_suffix"
```

### Best Practices for Avoiding Issues

1. **Always use `${VAR}` syntax** - More reliable than `$VAR`
2. **Check phase availability** - Review phase table before using variables
3. **Quote shell variables** - Use `"${var}"` in shell commands
4. **Capture before use** - Set `capture_output` before referencing
5. **Specify JSON format** - Use `capture_format: "json"` for structured data
6. **Use verbose mode** - Debug with `-v` flag
7. **Validate JSON** - Test with `jq` before using in workflow
8. **Document assumptions** - Comment expected variable structure
9. **Provide fallbacks** - Handle empty variables gracefully
10. **Test incrementally** - Add variables one at a time

### Getting Help

If you're still stuck after trying these debugging techniques:

1. **Check logs**: Review Claude JSON logs for variable resolution
2. **Inspect checkpoints**: Look at stored variable values
3. **Simplify workflow**: Remove complexity to isolate issue
4. **Review examples**: Check working examples in documentation
5. **Verify phase**: Double-check variable is available in current phase

**See Also:**
- [Available Variables](available-variables.md) - Full variable reference with phase availability
- [Custom Variable Capture](custom-variable-capture.md) - Capture configuration and formats
- [Examples](../examples.md) - Working examples of variable usage

