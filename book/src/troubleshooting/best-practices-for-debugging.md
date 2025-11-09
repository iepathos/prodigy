## Best Practices for Debugging

1. **Start simple**: Test commands individually before adding to workflow
2. **Use verbosity flags**: Use `-v` to see Claude interactions, `-vv` for debug logs, `-vvv` for trace
3. **Use echo liberally**: Debug variable values with echo statements
4. **Check logs and state**: Review event logs (`~/.prodigy/events/`) and session state (`.prodigy/session_state.json`)
5. **Test incrementally**: Add commands one at a time and test after each
6. **Validate input data**: Ensure JSON files and data formats are correct before MapReduce
7. **Check DLQ regularly**: Monitor failed items with `prodigy dlq list` and retry when appropriate
8. **Monitor resources**: Check disk space, memory, and CPU during execution
9. **Version control**: Commit working workflows before making changes
10. **Read error messages carefully**: MapReduceError types indicate specific failure modes
11. **Ask for help**: Include full error messages, workflow config, and verbosity output when seeking support
