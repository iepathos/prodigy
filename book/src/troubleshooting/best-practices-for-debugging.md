## Best Practices for Debugging

1. **Start simple**: Test commands individually before adding to workflow
2. **Use verbosity flags**: Use `-v` to see Claude interactions, `-vv` for debug logs, `-vvv` for trace
3. **Debug variable interpolation**:
   - Use echo statements to verify variable values
   - Capture command outputs with `capture_output` for later use
   - Enable verbose mode (`-v`) to see variable interpolation in real-time
   - Check variable scope (step vs workflow level)
4. **Check logs and state**:
   - Review event logs: `~/.prodigy/events/{repo_name}/{job_id}/`
   - Check unified sessions: `~/.prodigy/sessions/{session-id}.json`
   - Inspect checkpoints: `~/.prodigy/state/{repo_name}/mapreduce/jobs/{job_id}/`
5. **View Claude JSON logs**:
   - Use `prodigy logs --latest` to see the most recent Claude execution
   - Check `~/.local/state/claude/logs/` for detailed Claude interaction logs
   - Review tool invocations, token usage, and error details
   - Use `prodigy logs --latest --tail` to watch live Claude execution
6. **Review DLQ for MapReduce failures**:
   - Use `prodigy dlq show <job_id>` to see failed items with error details
   - Check `json_log_location` in DLQ entries for Claude execution logs
   - Retry failed items with `prodigy dlq retry <job_id>`
7. **Test incrementally**: Add commands one at a time and test after each
8. **Validate input data**: Ensure JSON files and data formats are correct before MapReduce
9. **Monitor resources**: Check disk space, memory, and CPU during execution
10. **Version control**: Commit working workflows before making changes
11. **Read error messages carefully**: MapReduceError types indicate specific failure modes
12. **Inspect checkpoint state**: Check `~/.prodigy/state/{repo}/mapreduce/jobs/{job_id}/` when resume fails
13. **Examine worktree history**: Use `cd ~/.prodigy/worktrees/{repo}/{session}/ && git log` to see all commits
14. **Ask for help**: Include full error messages, workflow config, and verbosity output when seeking support
