# Pack Testing Framework

**Complete guide to testing Attune packs programmatically**

---

## Overview

The Pack Testing Framework enables automatic validation of packs during installation and development. Tests are defined in `pack.yaml` and executed by the worker service or CLI tool.

**Benefits:**
- ✅ Fail-fast pack installation (catch issues before deployment)
- ✅ Validate dependencies in target environment
- ✅ Audit trail of test results
- ✅ Quality assurance for pack ecosystem
- ✅ CI/CD integration ready

---

## Quick Start

### 1. Add Testing Configuration to pack.yaml

```yaml
testing:
  enabled: true
  
  discovery:
    method: "directory"
    path: "tests"
  
  runners:
    shell:
      type: "script"
      entry_point: "tests/run_tests.sh"
      timeout: 60
      result_format: "simple"
    
    python:
      type: "unittest"
      entry_point: "tests/test_actions.py"
      timeout: 120
      result_format: "simple"
  
  min_pass_rate: 1.0
  on_failure: "block"
```

### 2. Create Test Files

**Shell Test Runner** (`tests/run_tests.sh`):
```bash
#!/bin/bash
set -e

PASSED=0
FAILED=0
TOTAL=0

# Run your tests here
./actions/my_action.sh --test
if [ $? -eq 0 ]; then
    PASSED=$((PASSED + 1))
else
    FAILED=$((FAILED + 1))
fi
TOTAL=$((TOTAL + 1))

# Output results (required format)
echo "Total Tests: $TOTAL"
echo "Passed: $PASSED"
echo "Failed: $FAILED"

exit $FAILED
```

**Python Test Runner** (`tests/test_actions.py`):
```python
import unittest
from actions import my_action

class TestMyAction(unittest.TestCase):
    def test_basic_execution(self):
        result = my_action.run({"param": "value"})
        self.assertEqual(result["status"], "success")
    
    def test_error_handling(self):
        with self.assertRaises(ValueError):
            my_action.run({"invalid": "params"})

if __name__ == '__main__':
    unittest.main()
```

### 3. Run Tests

```bash
# Test a pack before installation
attune pack test ./packs/my_pack

# Test an installed pack
attune pack test my_pack

# Verbose output
attune pack test my_pack --verbose

# JSON output for CI/CD
attune pack test my_pack --output json
```

---

## Testing Configuration

### Pack.yaml Testing Section

```yaml
testing:
  # Enable/disable testing
  enabled: true
  
  # Test discovery configuration
  discovery:
    method: "directory"  # or "manifest", "executable"
    path: "tests"        # relative to pack root
  
  # Test runners by runtime type
  runners:
    shell:
      type: "script"
      entry_point: "tests/run_tests.sh"
      timeout: 60         # seconds
      result_format: "simple"
    
    python:
      type: "unittest"    # or "pytest"
      entry_point: "tests/test_actions.py"
      timeout: 120
      result_format: "simple"
    
    node:
      type: "jest"
      entry_point: "tests/actions.test.js"
      timeout: 90
      result_format: "json"
  
  # Test result expectations
  result_path: "tests/results/"
  min_pass_rate: 1.0      # 100% tests must pass
  on_failure: "block"     # or "warn"
```

### Configuration Options

#### `enabled` (boolean)
- `true`: Tests will be executed
- `false`: Tests will be skipped

#### `discovery.method` (string)
- `"directory"`: Discover tests in specified directory (recommended)
- `"manifest"`: List tests explicitly in pack.yaml
- `"executable"`: Run a single test discovery command

#### `runners.<name>.type` (string)
- `"script"`: Shell script execution
- `"unittest"`: Python unittest framework
- `"pytest"`: Python pytest framework
- `"jest"`: JavaScript Jest framework

#### `runners.<name>.result_format` (string)
- `"simple"`: Parse "Total Tests: X, Passed: Y, Failed: Z" format
- `"json"`: Parse structured JSON output
- `"junit-xml"`: Parse JUnit XML format (pytest, Jest)
- `"tap"`: Parse Test Anything Protocol format

#### `on_failure` (string)
- `"block"`: Prevent pack installation if tests fail
- `"warn"`: Allow installation but show warning

---

## Test Output Formats

### Simple Format (Default)

Your test runner must output these lines:

```
Total Tests: 36
Passed: 35
Failed: 1
Skipped: 0
```

The executor will:
1. Parse these counts from stdout/stderr
2. Use exit code to determine success/failure
3. Exit code 0 = success, non-zero = failure

### JSON Format (Advanced)

Output structured JSON:

```json
{
  "total": 36,
  "passed": 35,
  "failed": 1,
  "skipped": 0,
  "duration_ms": 12345,
  "tests": [
    {
      "name": "test_basic_execution",
      "status": "passed",
      "duration_ms": 123,
      "output": "..."
    }
  ]
}
```

### JUnit XML Format (Future)

For pytest and Jest, use built-in JUnit reporters:

```bash
# pytest
pytest --junit-xml=results.xml

# Jest
jest --reporters=jest-junit
```

---

## CLI Commands

### Test a Pack

```bash
# Basic usage
attune pack test <pack>

# Local pack directory
attune pack test ./packs/my_pack

# Installed pack
attune pack test my_pack

# From pack root directory
cd packs/my_pack
attune pack test .
```

### Output Formats

```bash
# Human-readable table (default)
attune pack test my_pack

# Verbose with test case details
attune pack test my_pack --verbose

# Detailed with stdout/stderr
attune pack test my_pack --detailed

# JSON for scripting
attune pack test my_pack --output json

# YAML output
attune pack test my_pack --output yaml
```

### Exit Codes

- `0`: All tests passed
- `1`: One or more tests failed
- `2`: Test execution error (timeout, missing config, etc.)

Perfect for CI/CD pipelines:

```bash
#!/bin/bash
if attune pack test my_pack; then
    echo "✅ Tests passed, deploying..."
    attune pack install ./packs/my_pack
else
    echo "❌ Tests failed, aborting deployment"
    exit 1
fi
```

---

## Examples

### Example 1: Core Pack (Complete)

See `packs/core/` for a complete example:

- **Configuration**: `packs/core/pack.yaml` (testing section)
- **Shell Tests**: `packs/core/tests/run_tests.sh` (36 tests)
- **Python Tests**: `packs/core/tests/test_actions.py` (38 tests)
- **Documentation**: `packs/core/tests/README.md`

Test execution:

```bash
$ attune pack test packs/core

🧪 Testing Pack: core v1.0.0

Test Results:
─────────────────────────────────────────────
  Total Tests:  2
  ✓ Passed:     2
  ✗ Failed:     0
  ○ Skipped:    0
  Pass Rate:    100.0%
  Duration:     25542ms
─────────────────────────────────────────────

✓ ✅ All tests passed: 2/2
```

### Example 2: Python Pack with pytest

```yaml
# pack.yaml
testing:
  enabled: true
  runners:
    python:
      type: "pytest"
      entry_point: "tests/"
      timeout: 180
      result_format: "simple"
```

```python
# tests/test_mypack.py
import pytest
from actions.my_action import execute

def test_success():
    result = execute({"input": "value"})
    assert result["status"] == "success"

def test_validation():
    with pytest.raises(ValueError):
        execute({"invalid": None})

@pytest.mark.skip(reason="Not implemented yet")
def test_future_feature():
    pass
```

### Example 3: Shell Script Tests

```bash
#!/bin/bash
# tests/run_tests.sh

set -e

TOTAL=0
PASSED=0
FAILED=0

test_action() {
    local name="$1"
    local command="$2"
    local expected_exit="$3"
    
    TOTAL=$((TOTAL + 1))
    echo -n "Testing $name... "
    
    if eval "$command"; then
        actual_exit=$?
    else
        actual_exit=$?
    fi
    
    if [ "$actual_exit" -eq "$expected_exit" ]; then
        echo "PASS"
        PASSED=$((PASSED + 1))
    else
        echo "FAIL (exit: $actual_exit, expected: $expected_exit)"
        FAILED=$((FAILED + 1))
    fi
}

# Run tests
test_action "basic_echo" "./actions/echo.sh 'Hello'" 0
test_action "invalid_param" "./actions/echo.sh" 1
test_action "http_request" "./actions/http.py --url=https://httpbin.org/get" 0

# Output results
echo ""
echo "Total Tests: $TOTAL"
echo "Passed: $PASSED"
echo "Failed: $FAILED"

exit $FAILED
```

---

## Best Practices

### 1. Always Include Tests

Every pack should have tests. Minimum recommended:
- Test each action's success path
- Test error handling (invalid inputs)
- Test dependencies are available

### 2. Use Descriptive Test Names

```python
# Good
def test_http_request_returns_json_on_success(self):
    pass

# Bad
def test1(self):
    pass
```

### 3. Test Exit Codes

Ensure your tests return proper exit codes:
- `0` = success
- Non-zero = failure

```bash
#!/bin/bash
# tests/run_tests.sh

# Run tests
python -m unittest discover -s tests

# Capture exit code
TEST_EXIT=$?

# Output required format
echo "Total Tests: 10"
echo "Passed: 9"
echo "Failed: 1"

# Exit with test result
exit $TEST_EXIT
```

### 4. Test Dependencies

Validate required libraries are available:

```python
def test_dependencies(self):
    """Test required libraries are installed"""
    try:
        import requests
        import croniter
    except ImportError as e:
        self.fail(f"Missing dependency: {e}")
```

### 5. Use Timeouts

Set realistic timeouts for test execution:

```yaml
runners:
  python:
    timeout: 120  # 2 minutes max
```

### 6. Mock External Services

Don't rely on external services in tests:

```python
from unittest.mock import patch, MagicMock

@patch('requests.get')
def test_http_request(self, mock_get):
    mock_get.return_value = MagicMock(
        status_code=200,
        json=lambda: {"status": "ok"}
    )
    result = my_action.execute()
    self.assertEqual(result["status"], "success")
```

---

## Troubleshooting

### Tests Fail with "Entry point not found"

**Problem**: Test file doesn't exist or path is wrong

**Solution**:
```bash
# Check file exists
ls -la packs/my_pack/tests/

# Verify path in pack.yaml is relative to pack root
entry_point: "tests/run_tests.sh"  # ✓ Correct
entry_point: "run_tests.sh"         # ✗ Wrong
```

### Tests Timeout

**Problem**: Tests take too long

**Solutions**:
1. Increase timeout in pack.yaml
2. Optimize slow tests
3. Mock external dependencies
4. Split into separate test suites

```yaml
runners:
  quick:
    timeout: 30
  integration:
    timeout: 300  # Longer for integration tests
```

### Parse Errors

**Problem**: Test output format not recognized

**Solution**: Ensure output includes required lines:

```bash
# Required output format
echo "Total Tests: $TOTAL"
echo "Passed: $PASSED"
echo "Failed: $FAILED"
```

### Exit Code 127 (Command not found)

**Problem**: Test runner executable not found

**Solutions**:
1. Make test script executable: `chmod +x tests/run_tests.sh`
2. Use full interpreter path: `/bin/bash tests/run_tests.sh`
3. Check shebang line: `#!/bin/bash`

---

## Architecture

### Components

```
CLI (attune pack test)
    ↓
Worker Test Executor
    ↓
Runtime Manager (shell, python, node)
    ↓
Test Runners (unittest, pytest, jest)
    ↓
Output Parser (simple, json, junit, tap)
    ↓
Test Results (structured data)
    ↓
Database (pack_test_execution table)
```

### Data Flow

```
pack.yaml (testing config)
    ↓
TestConfig (parsed)
    ↓
TestExecutor.execute_pack_tests()
    ├─ execute_test_suite(shell)
    │   └─ parse_simple_output()
    └─ execute_test_suite(python)
        └─ parse_simple_output()
    ↓
PackTestResult (aggregated)
    ↓
CLI display / JSON output / Database storage
```

### Database Schema

Tests are stored in `pack_test_execution` table:

```sql
CREATE TABLE attune.pack_test_execution (
    id BIGSERIAL PRIMARY KEY,
    pack_id BIGINT NOT NULL REFERENCES attune.pack(id),
    pack_version TEXT NOT NULL,
    execution_time TIMESTAMPTZ NOT NULL,
    trigger_reason TEXT NOT NULL,
    total_tests INT NOT NULL,
    passed INT NOT NULL,
    failed INT NOT NULL,
    skipped INT NOT NULL,
    pass_rate DOUBLE PRECISION NOT NULL,
    duration_ms BIGINT NOT NULL,
    result JSONB NOT NULL
);
```

---

## API (Future)

### Test Execution Endpoint

```http
POST /api/v1/packs/{pack_ref}/test
```

Response:
```json
{
  "data": {
    "id": 123,
    "packRef": "core",
    "packVersion": "1.0.0",
    "totalTests": 74,
    "passed": 74,
    "failed": 0,
    "passRate": 1.0,
    "durationMs": 25000
  }
}
```

### Test History Endpoint

```http
GET /api/v1/packs/{pack_ref}/tests?limit=10
```

### Latest Test Result

```http
GET /api/v1/packs/{pack_ref}/tests/latest
```

---

## CI/CD Integration

### GitHub Actions

```yaml
name: Test Pack
on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      
      - name: Install Attune CLI
        run: |
          curl -L https://get.attune.io | sh
          export PATH="$HOME/.attune/bin:$PATH"
      
      - name: Test Pack
        run: |
          attune pack test ./packs/my_pack --output json > results.json
          
      - name: Upload Results
        uses: actions/upload-artifact@v3
        with:
          name: test-results
          path: results.json
```

### GitLab CI

```yaml
test-pack:
  stage: test
  script:
    - attune pack test ./packs/my_pack
  artifacts:
    reports:
      junit: test-results.xml
```

---

## Related Documentation

- **Design Document**: `docs/pack-testing-framework.md`
- **Core Pack Tests**: `packs/core/tests/README.md`
- **Database Schema**: `migrations/012_add_pack_test_results.sql`
- **API Documentation**: `docs/api-packs.md`

---

## Changelog

- **2026-01-22**: Initial implementation (Phases 1 & 2)
  - Worker test executor
  - CLI pack test command
  - Simple output parser
  - Core pack validation (76 tests)

---

## Support

For issues or questions:
- GitHub Issues: https://github.com/attune-io/attune/issues
- Documentation: https://docs.attune.io/packs/testing
- Community: https://community.attune.io