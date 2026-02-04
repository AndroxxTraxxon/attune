# Pack Installation with Testing Integration

## Overview

Pack installation and registration now includes automatic test execution to validate that packs work correctly in the target environment. This provides fail-fast validation and ensures quality across the pack ecosystem.

## Features

- **Automatic Test Execution**: Tests run automatically during pack installation/registration
- **Fail-Fast Validation**: Installation fails if tests don't pass (unless forced)
- **Test Result Storage**: All test results are stored in the database for audit trails
- **Flexible Control**: Skip tests or force installation with command-line flags
- **Test Result Display**: CLI shows test results with pass/fail status

## Installation Methods

### 1. Register Pack (Local Directory)

Register a pack from a local filesystem directory:

```bash
# Basic registration with automatic testing
attune pack register /path/to/pack

# Force registration even if pack already exists
attune pack register /path/to/pack --force

# Skip tests during registration
attune pack register /path/to/pack --skip-tests

# Combine flags
attune pack register /path/to/pack --force --skip-tests
```

**How it works:**
1. Reads `pack.yaml` from the directory
2. Creates pack record in database
3. Syncs workflows from pack directory
4. **Runs pack tests** (if not skipped)
5. **Fails registration if tests fail** (unless `--force` is used)
6. Stores test results in database

### 2. Install Pack (Remote Source)

**Status**: Not yet implemented

Install a pack from a git repository or remote source:

```bash
# Install from git repository (future)
attune pack install https://github.com/attune/pack-slack.git

# Install specific version (future)
attune pack install https://github.com/attune/pack-slack.git --ref v1.0.0
```

This feature will be implemented in a future release.

## API Endpoints

### Register Pack

**Endpoint**: `POST /api/v1/packs/register`

**Request Body**:
```json
{
  "path": "/path/to/pack/directory",
  "force": false,
  "skip_tests": false
}
```

**Response** (201 Created):
```json
{
  "data": {
    "pack": {
      "id": 1,
      "ref": "mypack",
      "label": "My Pack",
      "version": "1.0.0",
      ...
    },
    "test_result": {
      "pack_ref": "mypack",
      "pack_version": "1.0.0",
      "status": "passed",
      "total_tests": 10,
      "passed": 10,
      "failed": 0,
      "skipped": 0,
      "pass_rate": 1.0,
      "duration_ms": 1234,
      "test_suites": [...]
    },
    "tests_skipped": false
  },
  "message": "Pack registered successfully"
}
```

**Error Response** (400 Bad Request) - Tests Failed:
```json
{
  "error": "Pack registration failed: tests did not pass. Use force=true to register anyway.",
  "code": "BAD_REQUEST"
}
```

### Install Pack

**Endpoint**: `POST /api/v1/packs/install`

**Status**: Returns 501 Not Implemented

**Request Body**:
```json
{
  "source": "https://github.com/attune/pack-slack.git",
  "ref_spec": "main",
  "force": false,
  "skip_tests": false
}
```

This endpoint will be implemented in a future release.

## Test Execution Behavior

### Default Behavior (Tests Enabled)

When registering a pack with tests configured in `pack.yaml`:

1. **Tests are automatically executed** after pack creation
2. **Test results are stored** in the `pack_test_execution` table
3. **Registration fails** if any test fails
4. **Pack record is rolled back** if tests fail (unless `--force` is used)

### Skip Tests (`--skip-tests` flag)

Use this flag when:
- Tests are known to be slow or flaky
- Testing in a non-standard environment
- Manually verifying pack functionality later

```bash
attune pack register /path/to/pack --skip-tests
```

**Behavior**:
- Tests are not executed
- No test results are stored
- Registration always succeeds (no validation)
- Response includes `"tests_skipped": true`

### Force Registration (`--force` flag)

Use this flag when:
- Pack already exists and you want to reinstall
- Tests are failing but you need to proceed anyway
- Developing and iterating on pack tests

```bash
attune pack register /path/to/pack --force
```

**Behavior**:
- Deletes existing pack if it exists
- Tests still run (unless `--skip-tests` is also used)
- **Registration succeeds even if tests fail**
- Warning logged if tests fail

### Combined Flags

```bash
# Force reinstall and skip tests entirely
attune pack register /path/to/pack --force --skip-tests
```

## CLI Output Examples

### Successful Registration with Tests

```
✓ Pack 'core' registered successfully
  Version: 0.1.0
  ID: 1
✓ All tests passed
  Tests: 76/76 passed
```

### Failed Registration (Tests Failed)

```
✗ Error: Pack registration failed: tests did not pass. Use force=true to register anyway.
```

### Registration with Skipped Tests

```
✓ Pack 'core' registered successfully
  Version: 0.1.0
  ID: 1
  Tests were skipped
```

### Forced Registration with Failed Tests

```
⚠ Pack 'mypack' registered successfully
  Version: 1.0.0
  ID: 2
✗ Some tests failed
  Tests: 8/10 passed
```

## Testing Requirements

For a pack to support automatic testing during installation:

1. **`pack.yaml` must include a `testing` section**:

```yaml
testing:
  enabled: true
  test_suites:
    - name: "Unit Tests"
      runner: "python_unittest"
      working_dir: "tests"
      test_files:
        - "test_*.py"
      timeout_seconds: 60
```

2. **Test files must exist** in the specified locations
3. **Tests must be executable** in the target environment

See [Pack Testing Framework](./PACK_TESTING.md) for detailed test configuration.

## Database Storage

All test executions are stored in the `attune.pack_test_execution` table:

- **pack_id**: Reference to the pack
- **pack_version**: Version tested
- **trigger_reason**: How tests were triggered (e.g., "register", "manual")
- **total_tests**, **passed**, **failed**, **skipped**: Test counts
- **pass_rate**: Percentage of tests passed
- **duration_ms**: Total execution time
- **result**: Full test results as JSON

Query test history:
```bash
attune pack test-history core
```

Or via API:
```bash
curl -H "Authorization: Bearer $TOKEN" \
  http://localhost:8080/api/v1/packs/core/tests
```

## Error Handling

### Pack Directory Not Found

```json
{
  "error": "Pack directory does not exist: /invalid/path",
  "code": "BAD_REQUEST"
}
```

### Missing pack.yaml

```json
{
  "error": "pack.yaml not found in directory: /path/to/pack",
  "code": "BAD_REQUEST"
}
```

### No Testing Configuration

```json
{
  "error": "No testing configuration found in pack.yaml for pack 'mypack'",
  "code": "BAD_REQUEST"
}
```

### Testing Disabled

```json
{
  "error": "Testing is disabled for pack 'mypack'",
  "code": "BAD_REQUEST"
}
```

### Pack Already Exists

```json
{
  "error": "Pack 'mypack' already exists. Use force=true to reinstall.",
  "code": "CONFLICT"
}
```

## Best Practices

### Development Workflow

1. **During active development**: Use `--skip-tests` for faster iteration
   ```bash
   attune pack register ./my-pack --force --skip-tests
   ```

2. **Before committing**: Run tests explicitly to validate
   ```bash
   attune pack test my-pack
   ```

3. **For production**: Let tests run automatically during registration
   ```bash
   attune pack register ./my-pack
   ```

### CI/CD Integration

In your CI/CD pipeline:

```bash
# Register pack (tests will run automatically)
attune pack register ./pack-directory

# Exit code will be non-zero if tests fail
echo $?  # 0 = success, 1 = failure
```

### Production Deployment

For production deployments:

1. **Never skip tests** unless you have a specific reason
2. **Use `--force`** only when redeploying a known-good version
3. **Monitor test results** via the API or database
4. **Set up alerts** for test failures

## Future Enhancements

Planned improvements to pack installation:

1. **Remote Pack Installation**: Install packs from git repositories
2. **Dependency Resolution**: Automatically install required packs
3. **Version Management**: Support multiple versions of the same pack
4. **Async Testing**: Return immediately and poll for test results
5. **Test Result Comparison**: Compare test results across versions
6. **Webhooks**: Notify external systems of test results
7. **Pack Registry**: Central repository for discovering and installing packs

## Related Documentation

- [Pack Testing Framework](./PACK_TESTING.md) - Complete testing guide
- [Pack Testing API Reference](./api-pack-testing.md) - API documentation
- [Pack Development Guide](./PACK_DEVELOPMENT.md) - Creating packs
- [Pack Structure](./pack-structure.md) - pack.yaml format

## Examples

See the `packs/core` directory for a complete example of a pack with testing enabled:

- `packs/core/pack.yaml` - Testing configuration
- `packs/core/tests/` - Test files
- `packs/core/actions/` - Actions being tested

Register the core pack:

```bash
attune pack register packs/core
```
