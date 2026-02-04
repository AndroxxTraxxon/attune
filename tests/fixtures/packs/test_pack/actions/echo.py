#!/usr/bin/env python3
"""
Echo Action for E2E Testing
Echoes back the input message with timestamp and execution metrics
"""

import json
import sys
import time
from datetime import datetime


def main():
    """Main entry point for the echo action"""
    start_time = time.time()

    try:
        # Read parameters from stdin (Attune standard)
        input_data = json.loads(sys.stdin.read())

        # Extract parameters
        message = input_data.get("message", "Hello from Attune!")
        delay = input_data.get("delay", 0)
        should_fail = input_data.get("fail", False)

        # Validate parameters
        if not isinstance(message, str):
            raise ValueError(f"message must be a string, got {type(message).__name__}")

        if not isinstance(delay, int) or delay < 0 or delay > 30:
            raise ValueError(f"delay must be an integer between 0 and 30, got {delay}")

        # Simulate delay if requested
        if delay > 0:
            print(f"Delaying for {delay} seconds...", file=sys.stderr)
            time.sleep(delay)

        # Simulate failure if requested
        if should_fail:
            raise RuntimeError(f"Action intentionally failed as requested (fail=true)")

        # Calculate execution time
        execution_time = time.time() - start_time

        # Create output
        output = {
            "message": message,
            "timestamp": datetime.utcnow().isoformat() + "Z",
            "execution_time": round(execution_time, 3),
            "success": True,
        }

        # Write output to stdout
        print(json.dumps(output, indent=2))

        # Log to stderr for debugging
        print(
            f"Echo action completed successfully in {execution_time:.3f}s",
            file=sys.stderr,
        )

        return 0

    except json.JSONDecodeError as e:
        error_output = {
            "success": False,
            "error": "Invalid JSON input",
            "details": str(e),
        }
        print(json.dumps(error_output), file=sys.stdout)
        print(f"ERROR: Failed to parse JSON input: {e}", file=sys.stderr)
        return 1

    except Exception as e:
        execution_time = time.time() - start_time
        error_output = {
            "success": False,
            "error": str(e),
            "execution_time": round(execution_time, 3),
        }
        print(json.dumps(error_output), file=sys.stdout)
        print(f"ERROR: {e}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
