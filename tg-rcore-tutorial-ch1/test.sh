#!/bin/bash

OUTPUT=$(cargo run 2>&1)

if echo "$OUTPUT" | grep -q "Hello, world!"; then
    echo "Test PASSED: Found 'Hello, world!' in output"
    exit 0
else
    echo "Test FAILED: 'Hello, world!' not found in output"
    echo "Actual output:"
    echo "$OUTPUT"
    exit 1
fi
