# Core Pack Unit Tests

This directory contains comprehensive unit tests for the Attune Core Pack actions.

> **Note**: These tests can be run manually (as documented below) or programmatically during pack installation via the Pack Testing Framework. See [`docs/pack-testing-framework.md`](../../../docs/pack-testing-framework.md) for details on automatic test execution during pack installation.

## Overview

The test suite validates that all core pack actions work correctly with:
- Valid inputs
- Invalid inputs (error handling)
- Edge cases
- Default values
- Various parameter combinations

## Test Files

- **`run_tests.sh`** - Bash-based test runner with colored output
- **`test_actions.py`** - Python unittest/pytest suite for comprehensive testing
- **`README.md`** - This file

## Running Tests

### Quick Test (Bash Runner)

```bash
cd packs/core/tests
chmod +x run_tests.sh
./run_tests.sh
```

**Features:**
- Color-coded output (green = pass, red = fail)
- Fast execution
- No dependencies beyond bash and python3
- Tests all actions automatically
- Validates YAML schemas
- Checks file permissions

### Comprehensive Tests (Python)

```bash
cd packs/core/tests

# Using unittest
python3 test_actions.py

# Using pytest (recommended)
pytest test_actions.py -v

# Run specific test class
pytest test_actions.py::TestEchoAction -v

# Run specific test
pytest test_actions.py::TestEchoAction::test_basic_echo -v
```

**Features:**
- Structured test cases with setUp/tearDown
- Detailed assertions and error messages
- Subtest support for parameterized tests
- Better integration with CI/CD
- Test discovery and filtering

## Prerequisites

### Required
- Bash (for shell action tests)
- Python 3.8+ (for Python action tests)

### Optional
- `pytest` for better test output: `pip install pytest`
- `PyYAML` for YAML validation: `pip install pyyaml`
- `requests` for HTTP tests: `pip install requests>=2.28.0`

## Test Coverage

### core.echo

- ✅ Basic echo with custom message
- ✅ Default message when none provided
- ✅ Uppercase conversion (true/false)
- ✅ Empty messages
- ✅ Special characters
- ✅ Multiline messages
- ✅ Exit code validation

**Total: 7 tests**

### core.noop

- ✅ Basic no-op execution
- ✅ Custom message logging
- ✅ Exit code 0 (success)
- ✅ Custom exit codes (1-255)
- ✅ Invalid negative exit codes (error)
- ✅ Invalid large exit codes (error)
- ✅ Invalid non-numeric exit codes (error)
- ✅ Maximum valid exit code (255)

**Total: 8 tests**

### core.sleep

- ✅ Basic sleep (1 second)
- ✅ Zero seconds sleep
- ✅ Custom message display
- ✅ Default duration (1 second)
- ✅ Multi-second sleep (timing validation)
- ✅ Invalid negative seconds (error)
- ✅ Invalid large seconds >3600 (error)
- ✅ Invalid non-numeric seconds (error)

**Total: 8 tests**

### core.http_request

- ✅ Simple GET request
- ✅ Missing required URL (error)
- ✅ POST with JSON body
- ✅ Custom headers
- ✅ Query parameters
- ✅ Timeout handling
- ✅ 404 status code handling
- ✅ Different HTTP methods (PUT, PATCH, DELETE, HEAD, OPTIONS)
- ✅ Elapsed time reporting
- ✅ Response parsing (JSON/text)

**Total: 10+ tests**

### Additional Tests

- ✅ File permissions (all scripts executable)
- ✅ YAML schema validation
- ✅ pack.yaml structure
- ✅ Action YAML schemas

**Total: 4+ tests**

## Test Results

When all tests pass, you should see output like:

```
========================================
Core Pack Unit Tests
========================================

Testing core.echo
  [1] echo: basic message ... PASS
  [2] echo: default message ... PASS
  [3] echo: uppercase conversion ... PASS
  [4] echo: uppercase false ... PASS
  [5] echo: exit code 0 ... PASS

Testing core.noop
  [6] noop: basic execution ... PASS
  [7] noop: with message ... PASS
  ...

========================================
Test Results
========================================

Total Tests:  37
Passed:       37
Failed:       0

✓ All tests passed!
```

## Adding New Tests

### Adding to Bash Test Runner

Edit `run_tests.sh` and add new test cases:

```bash
# Test new action
echo -e "${BLUE}Testing core.my_action${NC}"

check_output \
    "my_action: basic test" \
    "cd '$ACTIONS_DIR' && ATTUNE_ACTION_PARAM='value' ./my_action.sh" \
    "Expected output"

run_test_expect_fail \
    "my_action: invalid input" \
    "cd '$ACTIONS_DIR' && ATTUNE_ACTION_PARAM='invalid' ./my_action.sh"
```

### Adding to Python Test Suite

Add a new test class to `test_actions.py`:

```python
class TestMyAction(CorePackTestCase):
    """Tests for core.my_action"""

    def test_basic_functionality(self):
        """Test basic functionality"""
        stdout, stderr, code = self.run_action(
            "my_action.sh",
            {"ATTUNE_ACTION_PARAM": "value"}
        )
        self.assertEqual(code, 0)
        self.assertIn("expected output", stdout)

    def test_error_handling(self):
        """Test error handling"""
        stdout, stderr, code = self.run_action(
            "my_action.sh",
            {"ATTUNE_ACTION_PARAM": "invalid"},
            expect_failure=True
        )
        self.assertNotEqual(code, 0)
        self.assertIn("ERROR", stderr)
```

## Continuous Integration

### GitHub Actions Example

```yaml
name: Core Pack Tests

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions/setup-python@v4
        with:
          python-version: '3.10'
      
      - name: Install dependencies
        run: pip install pytest pyyaml requests
      
      - name: Run bash tests
        run: |
          cd packs/core/tests
          chmod +x run_tests.sh
          ./run_tests.sh
      
      - name: Run python tests
        run: |
          cd packs/core/tests
          pytest test_actions.py -v
```

## Troubleshooting

### Tests fail with "Permission denied"

```bash
chmod +x packs/core/actions/*.sh
chmod +x packs/core/actions/*.py
```

### Python import errors

```bash
# Install required libraries
pip install requests>=2.28.0 pyyaml
```

### HTTP tests timing out

The `httpbin.org` service may be slow or unavailable. Try:
- Increasing timeout in tests
- Running tests again later
- Using a local httpbin instance

### YAML validation fails

Ensure PyYAML is installed:
```bash
pip install pyyaml
```

## Best Practices

1. **Test both success and failure cases** - Don't just test the happy path
2. **Use descriptive test names** - Make it clear what each test validates
3. **Test edge cases** - Empty strings, zero values, boundary conditions
4. **Validate error messages** - Ensure helpful errors are returned
5. **Keep tests fast** - Use minimal sleep times, short timeouts
6. **Make tests independent** - Each test should work in isolation
7. **Document expected behavior** - Add comments for complex tests

## Performance

Expected test execution times:

- **Bash runner**: ~15-30 seconds (with HTTP tests)
- **Python suite**: ~20-40 seconds (with HTTP tests)
- **Without HTTP tests**: ~5-10 seconds

Slowest tests:
- `core.sleep` timing validation tests (intentional delays)
- `core.http_request` network requests

## Future Improvements

- [ ] Add integration tests with Attune services
- [ ] Add performance benchmarks
- [ ] Test concurrent action execution
- [ ] Mock HTTP requests for faster tests
- [ ] Add property-based testing (hypothesis)
- [ ] Test sensor functionality
- [ ] Test trigger functionality
- [ ] Add coverage reporting

## Programmatic Test Execution

The Core Pack includes a `testing` section in `pack.yaml` that enables automatic test execution during pack installation:

```yaml
testing:
  enabled: true
  runners:
    shell:
      entry_point: "tests/run_tests.sh"
      timeout: 60
    python:
      entry_point: "tests/test_actions.py"
      timeout: 120
  min_pass_rate: 1.0
  on_failure: "block"
```

When installing the pack with `attune pack install`, these tests will run automatically to verify the pack works in the target environment.

## Resources

- [Core Pack Documentation](../README.md)
- [Testing Guide](../TESTING.md)
- [Pack Testing Framework](../../../docs/pack-testing-framework.md) - Programmatic test execution
- [Action Development Guide](../../../docs/action-development.md)
- [Python unittest docs](https://docs.python.org/3/library/unittest.html)
- [pytest docs](https://docs.pytest.org/)

---

**Last Updated**: 2024-01-20  
**Maintainer**: Attune Team