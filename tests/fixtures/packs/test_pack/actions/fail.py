#!/usr/bin/env python3
"""Failing action — always exits with code 1."""
import json, sys
params = json.loads(sys.stdin.read()) if not sys.stdin.isatty() else {}
msg = params.get("message", "Action intentionally failed")
print(json.dumps({"success": False, "error": msg}))
sys.exit(1)
