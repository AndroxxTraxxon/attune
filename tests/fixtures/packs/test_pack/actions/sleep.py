#!/usr/bin/env python3
"""Sleep action — sleeps for the requested duration then succeeds."""
import json, sys, time
params = json.loads(sys.stdin.read())
duration = int(params.get("duration", 1))
time.sleep(duration)
print(json.dumps({"success": True, "slept": duration}))
