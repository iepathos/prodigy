#!/bin/bash
# Test script to verify the fix for "Argument list too long" error

echo "Creating test data with large content..."

# Create a test workflow that generates large environment variables
cat > test_large_env.yml << 'EOF'
name: test-large-env
mode: mapreduce

env:
  PROJECT_NAME: "TestProject"
  MAX_PARALLEL: "2"

map:
  input: test_items.json
  json_path: "$.items[*]"

  agent_template:
    - shell: "echo Processing ${item.id}"
    - shell: "echo '${item.data}' | wc -c"

  max_parallel: 2

reduce:
  - shell: "echo Reduce phase with ${map.total} items"
  - shell: "echo Map results size: $(echo '${map.results}' | wc -c) bytes"
  - shell: "echo Success!"
EOF

# Create test items with large data
cat > test_items.json << 'EOF'
{
  "items": [
    {
      "id": "item1",
      "data": "AAAAAAAAAA"
    },
    {
      "id": "item2",
      "data": "BBBBBBBBBB"
    }
  ]
}
EOF

# Add 100 more items with 10KB of data each to create a large map.results
echo "Generating items with large data..."
python3 -c "
import json
items = []
for i in range(100):
    items.append({
        'id': f'item{i+3}',
        'data': 'X' * 10000  # 10KB per item
    })
with open('test_items_large.json', 'w') as f:
    json.dump({'items': items}, f)
"

# Update workflow to use the large items file
sed -i '' 's/test_items.json/test_items_large.json/' test_large_env.yml

echo "Running test workflow with large environment variables..."
echo "This should NOT fail with 'Argument list too long' error"
echo

# Run with verbosity to see the env var size logging
target/release/prodigy run test_large_env.yml -v 2>&1 | grep -E "(Environment variables count:|Argument list too long|E2BIG|error 7|✓ Reduce phase completed|SUCCESS)"

# Check exit code
if [ $? -eq 0 ]; then
    echo
    echo "✅ Test PASSED: No 'Argument list too long' error detected"
else
    echo
    echo "❌ Test FAILED: Error occurred during execution"
fi

# Clean up
rm -f test_large_env.yml test_items.json test_items_large.json