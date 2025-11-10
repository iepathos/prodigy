## Troubleshooting

Common configuration issues and their solutions.

### Configuration File Issues

#### "Configuration file not found"

**Symptoms**:
```
Error: Configuration file not found at ~/.prodigy/config.yml
```

**Causes**:
- Config file doesn't exist
- Wrong file extension (`.yaml` instead of `.yml`, or `.toml` instead of `.yml`)
- File is in the wrong location

**Solutions**:
1. Create the file:
   ```bash
   mkdir -p ~/.prodigy
   cat > ~/.prodigy/config.yml << 'EOF'
   log_level: info
   auto_commit: true
   EOF
   ```

2. Check file extension:
   ```bash
   ls -la ~/.prodigy/config.*
   # Should show config.yml, not config.yaml or config.toml
   ```

3. Verify location:
   ```bash
   # Global config
   ls ~/.prodigy/config.yml

   # Project config
   ls .prodigy/config.yml
   ```

#### "Invalid YAML syntax"

**Symptoms**:
```
Error: Failed to parse configuration: invalid YAML syntax at line 10
```

**Causes**:
- Incorrect indentation (must use 2 spaces, no tabs)
- Missing space after colon (`key:value` instead of `key: value`)
- Unquoted strings with special characters
- Mixing TOML and YAML syntax

**Solutions**:
1. Check indentation (must be 2 spaces):
   ```yaml
   # ✗ Wrong (tabs or 4 spaces)
   storage:
       backend: file

   # ✓ Correct (2 spaces)
   storage:
     backend: file
   ```

2. Add space after colons:
   ```yaml
   # ✗ Wrong
   log_level:debug

   # ✓ Correct
   log_level: debug
   ```

3. Quote strings with special characters:
   ```yaml
   # ✗ Wrong
   message: Error: something failed

   # ✓ Correct
   message: "Error: something failed"
   ```

4. Use a YAML validator:
   ```bash
   yamllint ~/.prodigy/config.yml
   ```

#### "Unknown field in configuration"

**Symptoms**:
```
Warning: Unknown field 'unknown_setting' in configuration
```

**Causes**:
- Typo in field name
- Using deprecated field name
- Field from old TOML format

**Solutions**:
1. Check spelling against [Global Configuration Structure](global-configuration-structure.md) or [Project Configuration Structure](project-configuration-structure.md)

2. Remove deprecated fields:
   ```yaml
   # ✗ Deprecated TOML-style
   [storage]
   backend = "file"

   # ✓ Correct YAML
   storage:
     backend: file
   ```

3. Update field names from old versions

### Environment Variable Issues

#### "Environment variable not resolving"

**Symptoms**:
```
Error: Variable 'API_KEY' not found
```

**Causes**:
- Variable not defined in any configuration source
- Incorrect variable syntax in workflow
- Profile not activated

**Solutions**:
1. Check variable is defined:
   ```bash
   # System env
   echo $PRODIGY_API_KEY

   # Workflow env (check workflow.yml)
   grep API_KEY workflow.yml
   ```

2. Use correct syntax:
   ```yaml
   # ✗ Wrong
   command: "curl $API_KEY"

   # ✓ Correct
   command: "curl ${API_KEY}"
   ```

3. Activate profile if using profile-specific values:
   ```bash
   prodigy run workflow.yml --profile prod
   ```

4. Check precedence chain: Step env > Profile env > Workflow env > System env

#### "Secret not being masked in logs"

**Symptoms**:
```
Output: curl -H 'Authorization: Bearer sk-abc123...'
```

**Causes**:
- Secret not marked with `secret: true` in workflow env block
- Using system env vars (not masked automatically)

**Solutions**:
```yaml
# Mark as secret in workflow
env:
  API_KEY:
    secret: true
    value: "${PROD_API_KEY}"  # From system env
```

**Note**: Only workflow env vars marked as `secret: true` are masked. System environment variables are not automatically masked.

### Storage Issues

#### "Storage directory not writable"

**Symptoms**:
```
Error: Failed to write to storage: Permission denied
```

**Causes**:
- Insufficient permissions on `~/.prodigy` directory
- Directory owned by different user
- Disk full

**Solutions**:
1. Check permissions:
   ```bash
   ls -ld ~/.prodigy
   # Should show: drwxr-xr-x username username
   ```

2. Fix ownership:
   ```bash
   sudo chown -R $USER:$USER ~/.prodigy
   chmod -R u+rwX ~/.prodigy
   ```

3. Check disk space:
   ```bash
   df -h ~/.prodigy
   ```

#### "Failed to acquire storage lock"

**Symptoms**:
```
Error: Failed to acquire storage lock after 30s
```

**Causes**:
- Another Prodigy process holding the lock
- Stale lock from crashed process
- File locking disabled but concurrent access occurring

**Solutions**:
1. Check for running processes:
   ```bash
   ps aux | grep prodigy
   ```

2. Remove stale lock (if no processes running):
   ```bash
   rm ~/.prodigy/storage.lock
   ```

3. Wait for lock release (if process is running)

4. Disable locking temporarily (not recommended for production):
   ```yaml
   storage:
     enable_locking: false
   ```

### Workflow Configuration Issues

#### "Workflow variables not interpolating"

**Symptoms**:
```
Output: Deploying ${PROJECT_NAME} to ${ENVIRONMENT}
```

**Causes**:
- Incorrect variable syntax
- Variable not defined in workflow or config
- Using project config variables instead of workflow env

**Solutions**:
1. Use correct syntax:
   ```yaml
   # ✓ Workflow env vars
   env:
     PROJECT_NAME: myapp
   commands:
     - shell: "echo ${PROJECT_NAME}"

   # ✗ Project config variables (different namespace)
   # .prodigy/config.yml
   variables:
     PROJECT_NAME: myapp  # Not available in workflows
   ```

2. Define variable in workflow `env:` block or as system env

3. Check variable exists:
   ```bash
   prodigy run workflow.yml -vv  # Verbose mode shows variable resolution
   ```

#### "MapReduce items not found"

**Symptoms**:
```
Error: No items found at JSONPath: $.items[*]
```

**Causes**:
- Incorrect JSONPath expression
- Input file not generated in setup phase
- JSON structure doesn't match path

**Solutions**:
1. Validate JSONPath:
   ```bash
   cat items.json | jq '.items[0]'
   ```

2. Check setup phase output:
   ```yaml
   setup:
     - shell: "generate-items.sh > items.json"
     - shell: "cat items.json"  # Verify file exists and has content
   ```

3. Test JSONPath expression:
   ```bash
   # Use jq to test
   cat items.json | jq '$[*]'  # Root array
   cat items.json | jq '.items[*]'  # Nested array
   ```

#### "Validation always fails"

**Symptoms**:
```
Error: Validation failed: completion_percentage 85 below threshold 100
```

**Causes**:
- Threshold set too high (default: 100)
- Validation command not returning expected format
- Expected schema mismatch

**Solutions**:
1. Adjust threshold:
   ```yaml
   validate:
     threshold: 80  # Accept 80% instead of 100%
   ```

2. Check validation output format:
   ```bash
   # Validation must output:
   {
     "completion_percentage": 85,
     "status": "incomplete",
     "gaps": ["Missing feature X", "Incomplete test Y"]
   }
   ```

3. Test validation command manually:
   ```bash
   # Run validation command outside workflow
   ./validate-script.sh
   ```

### API and Authentication Issues

#### "Claude API key not recognized"

**Symptoms**:
```
Error: Invalid Claude API key
```

**Causes**:
- API key not set
- Key set in wrong location
- Invalid key format
- Key expired or revoked

**Solutions**:
1. Check key is set (precedence order):
   ```bash
   # Highest precedence: Environment variable
   echo $PRODIGY_CLAUDE_API_KEY

   # Project config
   grep claude_api_key .prodigy/config.yml

   # Global config
   grep claude_api_key ~/.prodigy/config.yml
   ```

2. Verify key format (should start with `sk-ant-`):
   ```bash
   echo $PRODIGY_CLAUDE_API_KEY | head -c 10
   # Should show: sk-ant-api
   ```

3. Use environment variable (recommended):
   ```bash
   export PRODIGY_CLAUDE_API_KEY="sk-ant-api03-..."
   ```

4. Verify key is valid at [Anthropic Console](https://console.anthropic.com/)

### Performance Issues

#### "Workflow running slowly"

**Causes**:
- Excessive parallelism exhausting resources
- Large work items in MapReduce
- Slow validation commands

**Solutions**:
1. Reduce parallelism:
   ```yaml
   map:
     max_parallel: 3  # Reduce from 10
   ```

2. Add timeout limits:
   ```yaml
   commands:
     - shell: "long-running-command"
       timeout: 300  # 5 minutes
   ```

3. Enable caching (if available):
   ```yaml
   storage:
     enable_cache: true
   ```

#### "Storage growing too large"

**Causes**:
- Old events and DLQ data accumulating
- Large checkpoint files
- No compression enabled

**Solutions**:
1. Clean up old data:
   ```bash
   # Remove old events (older than 30 days)
   find ~/.prodigy/events -type f -mtime +30 -delete

   # Clean DLQ for completed jobs
   prodigy dlq clean --completed
   ```

2. Enable compression:
   ```yaml
   storage:
     backend_config:
       enable_compression: true
   ```

3. Reduce file retention:
   ```yaml
   storage:
     backend_config:
       max_file_size: 52428800  # 50MB instead of 100MB
   ```

### Debugging Tools

#### Check Effective Configuration

View the merged configuration from all sources:

```bash
prodigy config show
```

#### Verbose Logging

Enable detailed logging:

```bash
# Verbose mode (shows Claude streaming)
prodigy run workflow.yml -v

# Debug mode (shows variable resolution)
prodigy run workflow.yml -vv

# Trace mode (shows all internal operations)
prodigy run workflow.yml -vvv
```

Or set log level in config:

```yaml
log_level: debug  # trace, debug, info, warn, error
```

#### Validate Configuration Files

```bash
# Validate YAML syntax
yamllint ~/.prodigy/config.yml
yamllint .prodigy/config.yml

# Validate workflow syntax
prodigy validate workflow.yml
```

#### Check Claude Logs

View Claude execution logs for debugging:

```bash
# Latest log
prodigy logs --latest

# Tail live log
prodigy logs --latest --tail

# View specific log
cat ~/.local/state/claude/logs/session-abc123.json | jq
```

### Common Error Messages

| Error Message | Likely Cause | Solution |
|--------------|-------------|----------|
| "Configuration file not found" | Missing config file | Create `~/.prodigy/config.yml` or `.prodigy/config.yml` |
| "Invalid YAML syntax" | Syntax error in YAML | Check indentation, colons, quotes |
| "Unknown field" | Typo or deprecated field | Check docs for correct field names |
| "Variable not found" | Undefined variable | Define in workflow `env:` or system env |
| "Storage lock timeout" | Concurrent access | Wait or remove stale lock |
| "Permission denied" | Insufficient permissions | Fix ownership/permissions on `~/.prodigy` |
| "JSONPath not found" | Wrong path or missing data | Verify JSON structure and path |
| "API key invalid" | Wrong or expired key | Check key format and validity |

### Getting Help

If you can't resolve an issue:

1. **Check logs**: Use `-vvv` for maximum verbosity
2. **Verify config**: Run `prodigy config show`
3. **Check docs**: See [Configuration Structure](global-configuration-structure.md) and [Workflow Basics](../workflow-basics.md)
4. **File an issue**: [Prodigy GitHub Issues](https://github.com/anthropics/prodigy/issues) with:
   - Error message (redact secrets)
   - Relevant config snippets
   - Prodigy version (`prodigy --version`)
   - Operating system

### See Also

- [Configuration Precedence Rules](configuration-precedence-rules.md)
- [Environment Variables](environment-variables.md)
- [Storage Configuration](storage-configuration.md)
- [Default Values Reference](default-values-reference.md)
