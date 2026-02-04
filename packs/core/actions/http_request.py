#!/usr/bin/env python3
"""
HTTP Request Action - Core Pack
Make HTTP requests to external APIs with support for various methods, headers, and authentication
"""

import json
import os
import sys
import time
from typing import Any, Dict, Optional

try:
    import requests
    from requests.auth import HTTPBasicAuth
except ImportError:
    print(
        "ERROR: requests library not installed. Run: pip install requests>=2.28.0",
        file=sys.stderr,
    )
    sys.exit(1)


def get_env_param(name: str, default: Any = None, required: bool = False) -> Any:
    """Get action parameter from environment variable."""
    env_key = f"ATTUNE_ACTION_{name.upper()}"
    value = os.environ.get(env_key, default)

    if required and value is None:
        raise ValueError(f"Required parameter '{name}' not provided")

    return value


def parse_json_param(name: str, default: Any = None) -> Any:
    """Parse JSON parameter from environment variable."""
    value = get_env_param(name)
    if value is None:
        return default

    try:
        return json.loads(value)
    except json.JSONDecodeError as e:
        raise ValueError(f"Invalid JSON for parameter '{name}': {e}")


def parse_bool_param(name: str, default: bool = False) -> bool:
    """Parse boolean parameter from environment variable."""
    value = get_env_param(name)
    if value is None:
        return default

    if isinstance(value, bool):
        return value

    return str(value).lower() in ("true", "1", "yes", "on")


def parse_int_param(name: str, default: int = 0) -> int:
    """Parse integer parameter from environment variable."""
    value = get_env_param(name)
    if value is None:
        return default

    try:
        return int(value)
    except (ValueError, TypeError):
        raise ValueError(f"Invalid integer for parameter '{name}': {value}")


def make_http_request() -> Dict[str, Any]:
    """Execute HTTP request with provided parameters."""

    # Parse required parameters
    url = get_env_param("url", required=True)

    # Parse optional parameters
    method = get_env_param("method", "GET").upper()
    headers = parse_json_param("headers", {})
    body = get_env_param("body")
    json_body = parse_json_param("json_body")
    query_params = parse_json_param("query_params", {})
    timeout = parse_int_param("timeout", 30)
    verify_ssl = parse_bool_param("verify_ssl", True)
    auth_type = get_env_param("auth_type", "none")
    follow_redirects = parse_bool_param("follow_redirects", True)
    max_redirects = parse_int_param("max_redirects", 10)

    # Prepare request kwargs
    request_kwargs = {
        "method": method,
        "url": url,
        "headers": headers,
        "params": query_params,
        "timeout": timeout,
        "verify": verify_ssl,
        "allow_redirects": follow_redirects,
    }

    # Handle authentication
    if auth_type == "basic":
        username = get_env_param("auth_username")
        password = get_env_param("auth_password")
        if username and password:
            request_kwargs["auth"] = HTTPBasicAuth(username, password)
    elif auth_type == "bearer":
        token = get_env_param("auth_token")
        if token:
            request_kwargs["headers"]["Authorization"] = f"Bearer {token}"

    # Handle request body
    if json_body is not None:
        request_kwargs["json"] = json_body
    elif body is not None:
        request_kwargs["data"] = body

    # Make the request
    start_time = time.time()

    try:
        response = requests.request(**request_kwargs)
        elapsed_ms = int((time.time() - start_time) * 1000)

        # Parse response
        result = {
            "status_code": response.status_code,
            "headers": dict(response.headers),
            "body": response.text,
            "elapsed_ms": elapsed_ms,
            "url": response.url,
            "success": 200 <= response.status_code < 300,
        }

        # Try to parse JSON response
        try:
            result["json"] = response.json()
        except (json.JSONDecodeError, ValueError):
            result["json"] = None

        return result

    except requests.exceptions.Timeout:
        return {
            "status_code": 0,
            "headers": {},
            "body": "",
            "json": None,
            "elapsed_ms": int((time.time() - start_time) * 1000),
            "url": url,
            "success": False,
            "error": "Request timeout",
        }
    except requests.exceptions.ConnectionError as e:
        return {
            "status_code": 0,
            "headers": {},
            "body": "",
            "json": None,
            "elapsed_ms": int((time.time() - start_time) * 1000),
            "url": url,
            "success": False,
            "error": f"Connection error: {str(e)}",
        }
    except requests.exceptions.RequestException as e:
        return {
            "status_code": 0,
            "headers": {},
            "body": "",
            "json": None,
            "elapsed_ms": int((time.time() - start_time) * 1000),
            "url": url,
            "success": False,
            "error": f"Request error: {str(e)}",
        }


def main():
    """Main entry point for the action."""
    try:
        result = make_http_request()

        # Output result as JSON
        print(json.dumps(result, indent=2))

        # Exit with success/failure based on HTTP status
        if result.get("success", False):
            sys.exit(0)
        else:
            # Non-2xx status code or error
            error = result.get("error")
            if error:
                print(f"ERROR: {error}", file=sys.stderr)
            else:
                print(
                    f"ERROR: HTTP request failed with status {result.get('status_code')}",
                    file=sys.stderr,
                )
            sys.exit(1)

    except Exception as e:
        print(f"ERROR: {str(e)}", file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()
