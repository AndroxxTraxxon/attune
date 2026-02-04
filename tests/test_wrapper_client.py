#!/usr/bin/env python3
"""
Test Wrapper Client Validation

Simple test script to validate that the wrapper client works correctly
with the generated API client.
"""

import os
import sys

# Add tests directory to path
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))


def test_imports():
    """Test that all imports work"""
    print("Testing imports...")
    try:
        from generated_client import AuthenticatedClient, Client

        print("  ✓ Generated client imports")
    except ImportError as e:
        print(f"  ✗ Failed to import generated client: {e}")
        return False

    try:
        from helpers import AttuneClient

        print("  ✓ Wrapper client imports")
    except ImportError as e:
        print(f"  ✗ Failed to import wrapper client: {e}")
        return False

    return True


def test_client_initialization():
    """Test client initialization"""
    print("\nTesting client initialization...")
    try:
        from helpers import AttuneClient

        # Test without auto-login
        client = AttuneClient(
            base_url="http://localhost:8080", timeout=30, auto_login=False
        )
        print("  ✓ Client initialized without auto-login")

        # Check client properties
        assert client.base_url == "http://localhost:8080"
        assert client.timeout == 30
        assert client.auth_client is None
        print("  ✓ Client properties correct")

        return True
    except Exception as e:
        print(f"  ✗ Client initialization failed: {e}")
        import traceback

        traceback.print_exc()
        return False


def test_models():
    """Test Pydantic model construction"""
    print("\nTesting Pydantic models...")
    try:
        from generated_client.models.login_request import LoginRequest

        request = LoginRequest(login="test@example.com", password="password123")
        print("  ✓ LoginRequest model created")

        # Test to_dict
        data = request.to_dict()
        assert data["login"] == "test@example.com"
        assert data["password"] == "password123"
        print("  ✓ Model to_dict() works")

        return True
    except Exception as e:
        print(f"  ✗ Model construction failed: {e}")
        import traceback

        traceback.print_exc()
        return False


def test_health_check(api_url="http://localhost:8080"):
    """Test health check endpoint (doesn't require auth)"""
    print(f"\nTesting health check against {api_url}...")
    try:
        from helpers import AttuneClient

        client = AttuneClient(base_url=api_url, timeout=5, auto_login=False)

        health = client.health()
        print(f"  ✓ Health check successful: {health}")
        return True
    except Exception as e:
        print(f"  ✗ Health check failed: {e}")
        print(f"     (This is expected if API is not running)")
        return False


def test_to_dict_helper():
    """Test the to_dict conversion helper"""
    print("\nTesting to_dict helper...")
    try:
        from helpers.client_wrapper import to_dict

        # Test with dict
        d = {"key": "value"}
        assert to_dict(d) == d
        print("  ✓ Dict passthrough works")

        # Test with None
        assert to_dict(None) is None
        print("  ✓ None handling works")

        # Test with list
        lst = [{"a": 1}, {"b": 2}]
        result = to_dict(lst)
        assert result == lst
        print("  ✓ List conversion works")

        return True
    except Exception as e:
        print(f"  ✗ to_dict helper failed: {e}")
        import traceback

        traceback.print_exc()
        return False


def main():
    """Run all tests"""
    print("=" * 60)
    print("Wrapper Client Validation Tests")
    print("=" * 60)

    results = []

    # Run tests
    results.append(("Imports", test_imports()))
    results.append(("Client Init", test_client_initialization()))
    results.append(("Models", test_models()))
    results.append(("to_dict Helper", test_to_dict_helper()))

    # Only test health if API URL is provided
    api_url = os.getenv("ATTUNE_API_URL", "http://localhost:8080")
    if api_url:
        results.append(("Health Check", test_health_check(api_url)))

    # Summary
    print("\n" + "=" * 60)
    print("Test Summary")
    print("=" * 60)

    passed = sum(1 for _, result in results if result)
    total = len(results)

    for name, result in results:
        status = "✓ PASS" if result else "✗ FAIL"
        print(f"{status}: {name}")

    print(f"\nResults: {passed}/{total} tests passed")

    if passed == total:
        print("\n✓ All tests passed!")
        return 0
    else:
        print(f"\n✗ {total - passed} test(s) failed")
        return 1


if __name__ == "__main__":
    sys.exit(main())
