#!/usr/bin/env python3
import json
import os

# Get parameters from environment
name = os.environ.get('ATTUNE_ACTION_name', 'Python User')
count = int(os.environ.get('ATTUNE_ACTION_count', '1'))

# Generate greetings
greetings = [f"Hello, {name}! (greeting {i+1})" for i in range(count)]

# Output result as JSON
result = {
    "greetings": greetings,
    "total_count": len(greetings)
}

print(json.dumps(result))
