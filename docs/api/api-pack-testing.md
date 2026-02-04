# Pack Testing API Endpoints

**API endpoints for executing and retrieving pack test results**

---

## Overview

The Pack Testing API enables programmatic execution of pack tests and retrieval of test history. Tests are executed using the pack's `pack.yaml` configuration and results are stored in the database for audit and monitoring purposes.

**Base Path**: `/api/v1/packs/{ref}/`

---

## Endpoints

### 1. Execute Pack Tests

Execute all tests defined in a pack's `pack.yaml` configuration.

**Endpoint**: `POST /api/v1/packs/{ref}/test`

**Authentication**: Required (Bearer token)

**Path Parameters**:
- `ref` (string, required): Pack reference identifier

**Request Body**: None

**Response**: `200 OK`

```json
{
  "data": {
    "packRef": "core",
    "packVersion": "1.0.0",
    "executionTime": "2026-01-22T03:30:00Z",
    "totalTests": 2,
    "passed": 2,
    "failed": 0,
    "skipped": 0,
    "passRate": 1.0,
    "durationMs": 25542,
    "testSuites": [
      {
        "name": "shell",
        "runnerType": "script",
        "total": 1,
        "passed": 1,
        "failed": 0,
        "skipped": 0,
        "durationMs": 12305,
        "testCases": [
          {
            "name": "shell_suite",
            "status": "passed",
            "durationMs": 12305,
            "errorMessage": null,
            "stdout": "...",
            "stderr": null
          }
        ]
      },
      {
        "name": "python",
        "runnerType": "unittest",
        "total": 1,
        "passed": 1,
        "failed": 0,
        "skipped": 0,
        "durationMs": 13235,
        "testCases": [
          {
            "name": "python_suite",
            "status": "passed",
            "durationMs": 13235,
            "errorMessage": null,
            "stdout": null,
            "stderr": "..."
          }
        ]
      }
    ]
  },
  "message": "Pack tests executed successfully"
}
```

**Error Responses**:

- `400 Bad Request`: Testing not enabled or no test configuration found
  ```json
  {
    "error": "No testing configuration found in pack.yaml"
  }
  ```

- `404 Not Found`: Pack not found
  ```json
  {
    "error": "Pack 'my_pack' not found"
  }
  ```

- `500 Internal Server Error`: Test execution failed
  ```json
  {
    "error": "Test execution failed: timeout after 120s"
  }
  ```

**Example**:

```bash
# Execute tests for core pack
curl -X POST http://localhost:8080/api/v1/packs/core/test \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json"
```

**Behavior**:
1. Loads pack from database
2. Reads `pack.yaml` from filesystem
3. Parses test configuration
4. Executes test suites (shell, python, etc.)
5. Stores results in database
6. Returns structured test results

**Notes**:
- Test results are stored with `trigger_reason: "manual"`
- Tests run synchronously (blocking request)
- Large test suites may timeout (consider async execution in future)

---

### 2. Get Pack Test History

Retrieve paginated test execution history for a pack.

**Endpoint**: `GET /api/v1/packs/{ref}/tests`

**Authentication**: Required (Bearer token)

**Path Parameters**:
- `ref` (string, required): Pack reference identifier

**Query Parameters**:
- `page` (integer, optional, default: 1): Page number (1-based)
- `limit` (integer, optional, default: 20, max: 100): Items per page

**Response**: `200 OK`

```json
{
  "data": [
    {
      "id": 123,
      "packId": 1,
      "packVersion": "1.0.0",
      "executionTime": "2026-01-22T03:30:00Z",
      "triggerReason": "manual",
      "totalTests": 74,
      "passed": 74,
      "failed": 0,
      "skipped": 0,
      "passRate": 1.0,
      "durationMs": 25542,
      "result": { ... },
      "created": "2026-01-22T03:30:00Z"
    },
    {
      "id": 122,
      "packId": 1,
      "packVersion": "1.0.0",
      "executionTime": "2026-01-21T10:15:00Z",
      "triggerReason": "install",
      "totalTests": 74,
      "passed": 73,
      "failed": 1,
      "skipped": 0,
      "passRate": 0.986,
      "durationMs": 28100,
      "result": { ... },
      "created": "2026-01-21T10:15:00Z"
    }
  ],
  "pagination": {
    "page": 1,
    "limit": 20,
    "total": 45,
    "totalPages": 3
  }
}
```

**Error Responses**:

- `404 Not Found`: Pack not found
  ```json
  {
    "error": "Pack 'my_pack' not found"
  }
  ```

**Example**:

```bash
# Get first page of test history
curl -X GET "http://localhost:8080/api/v1/packs/core/tests?page=1&limit=10" \
  -H "Authorization: Bearer $TOKEN"

# Get second page
curl -X GET "http://localhost:8080/api/v1/packs/core/tests?page=2&limit=10" \
  -H "Authorization: Bearer $TOKEN"
```

**Notes**:
- Results are ordered by execution time (newest first)
- Full test result JSON is included in `result` field
- Trigger reasons: `manual`, `install`, `update`, `validation`

---

### 3. Get Latest Pack Test Result

Retrieve the most recent test execution for a pack.

**Endpoint**: `GET /api/v1/packs/{ref}/tests/latest`

**Authentication**: Required (Bearer token)

**Path Parameters**:
- `ref` (string, required): Pack reference identifier

**Response**: `200 OK`

```json
{
  "data": {
    "id": 123,
    "packId": 1,
    "packVersion": "1.0.0",
    "executionTime": "2026-01-22T03:30:00Z",
    "triggerReason": "manual",
    "totalTests": 74,
    "passed": 74,
    "failed": 0,
    "skipped": 0,
    "passRate": 1.0,
    "durationMs": 25542,
    "result": {
      "packRef": "core",
      "packVersion": "1.0.0",
      "executionTime": "2026-01-22T03:30:00Z",
      "totalTests": 2,
      "passed": 2,
      "failed": 0,
      "skipped": 0,
      "passRate": 1.0,
      "durationMs": 25542,
      "testSuites": [ ... ]
    },
    "created": "2026-01-22T03:30:00Z"
  }
}
```

**Error Responses**:

- `404 Not Found`: Pack not found or no tests available
  ```json
  {
    "error": "No test results found for pack 'my_pack'"
  }
  ```

**Example**:

```bash
# Get latest test result for core pack
curl -X GET http://localhost:8080/api/v1/packs/core/tests/latest \
  -H "Authorization: Bearer $TOKEN"

# Check if tests are passing
curl -s -X GET http://localhost:8080/api/v1/packs/core/tests/latest \
  -H "Authorization: Bearer $TOKEN" | jq '.data.passRate'
```

**Use Cases**:
- Health monitoring dashboards
- CI/CD pipeline validation
- Pack quality badges
- Automated alerts on test failures

---

## Data Models

### PackTestResult

Main test execution result structure.

```typescript
{
  packRef: string;           // Pack reference identifier
  packVersion: string;       // Pack version tested
  executionTime: string;     // ISO 8601 timestamp
  totalTests: number;        // Total number of tests
  passed: number;            // Number of passed tests
  failed: number;            // Number of failed tests
  skipped: number;           // Number of skipped tests
  passRate: number;          // Pass rate (0.0 to 1.0)
  durationMs: number;        // Total duration in milliseconds
  testSuites: TestSuiteResult[];  // Test suites executed
}
```

### TestSuiteResult

Result for a single test suite (e.g., shell, python).

```typescript
{
  name: string;              // Suite name (shell, python, etc.)
  runnerType: string;        // Runner type (script, unittest, pytest)
  total: number;             // Total tests in suite
  passed: number;            // Passed tests
  failed: number;            // Failed tests
  skipped: number;           // Skipped tests
  durationMs: number;        // Suite duration in milliseconds
  testCases: TestCaseResult[];  // Individual test cases
}
```

### TestCaseResult

Individual test case result.

```typescript
{
  name: string;              // Test case name
  status: TestStatus;        // "passed" | "failed" | "skipped" | "error"
  durationMs: number;        // Test duration in milliseconds
  errorMessage?: string;     // Error message if failed
  stdout?: string;           // Standard output (if captured)
  stderr?: string;           // Standard error (if captured)
}
```

### PackTestExecution

Database record of test execution.

```typescript
{
  id: number;                // Execution ID
  packId: number;            // Pack ID (foreign key)
  packVersion: string;       // Pack version
  executionTime: string;     // When tests were run
  triggerReason: string;     // "manual" | "install" | "update" | "validation"
  totalTests: number;        // Total number of tests
  passed: number;            // Passed tests
  failed: number;            // Failed tests
  skipped: number;           // Skipped tests
  passRate: number;          // Pass rate (0.0 to 1.0)
  durationMs: number;        // Duration in milliseconds
  result: object;            // Full PackTestResult JSON
  created: string;           // Record creation timestamp
}
```

---

## Usage Examples

### 1. Run Tests Before Deployment

```bash
#!/bin/bash
# test-and-deploy.sh

PACK="my_pack"
API_URL="http://localhost:8080/api/v1"

# Execute tests
RESULT=$(curl -s -X POST "$API_URL/packs/$PACK/test" \
  -H "Authorization: Bearer $TOKEN")

# Check pass rate
PASS_RATE=$(echo $RESULT | jq -r '.data.passRate')

if (( $(echo "$PASS_RATE >= 1.0" | bc -l) )); then
  echo "✅ All tests passed, deploying..."
  # Deploy pack
else
  echo "❌ Tests failed (pass rate: $PASS_RATE)"
  exit 1
fi
```

### 2. Monitor Pack Quality

```bash
#!/bin/bash
# monitor-pack-quality.sh

PACKS=("core" "aws" "kubernetes")

for PACK in "${PACKS[@]}"; do
  LATEST=$(curl -s -X GET "$API_URL/packs/$PACK/tests/latest" \
    -H "Authorization: Bearer $TOKEN")
  
  PASS_RATE=$(echo $LATEST | jq -r '.data.passRate')
  FAILED=$(echo $LATEST | jq -r '.data.failed')
  
  if [ "$FAILED" -gt 0 ]; then
    echo "⚠️  $PACK has $FAILED failing tests (pass rate: $PASS_RATE)"
  else
    echo "✅ $PACK: All tests passing"
  fi
done
```

### 3. Get Test Trend Data

```bash
#!/bin/bash
# get-test-trend.sh

PACK="core"

# Get last 10 test executions
HISTORY=$(curl -s -X GET "$API_URL/packs/$PACK/tests?limit=10" \
  -H "Authorization: Bearer $TOKEN")

# Extract pass rates
echo $HISTORY | jq -r '.data[] | "\(.executionTime): \(.passRate)"'
```

### 4. JavaScript/TypeScript Integration

```typescript
// pack-test-client.ts

interface PackTestClient {
  async executeTests(packRef: string): Promise<PackTestResult>;
  async getTestHistory(packRef: string, page?: number): Promise<PaginatedResponse<PackTestExecution>>;
  async getLatestTest(packRef: string): Promise<PackTestExecution>;
}

class AttunePackTestClient implements PackTestClient {
  constructor(
    private apiUrl: string,
    private token: string
  ) {}

  async executeTests(packRef: string): Promise<PackTestResult> {
    const response = await fetch(
      `${this.apiUrl}/packs/${packRef}/test`,
      {
        method: 'POST',
        headers: {
          'Authorization': `Bearer ${this.token}`,
          'Content-Type': 'application/json'
        }
      }
    );
    
    if (!response.ok) {
      throw new Error(`Test execution failed: ${response.statusText}`);
    }
    
    const { data } = await response.json();
    return data;
  }

  async getLatestTest(packRef: string): Promise<PackTestExecution> {
    const response = await fetch(
      `${this.apiUrl}/packs/${packRef}/tests/latest`,
      {
        headers: {
          'Authorization': `Bearer ${this.token}`
        }
      }
    );
    
    if (!response.ok) {
      throw new Error(`Failed to get latest test: ${response.statusText}`);
    }
    
    const { data } = await response.json();
    return data;
  }
}

// Usage
const client = new AttunePackTestClient(
  'http://localhost:8080/api/v1',
  process.env.ATTUNE_TOKEN
);

const result = await client.executeTests('core');
console.log(`Pass rate: ${result.passRate * 100}%`);
```

---

## Best Practices

### 1. Run Tests in CI/CD

Always run pack tests before deploying:

```yaml
# .github/workflows/test-pack.yml
name: Test Pack

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      
      - name: Test Pack
        run: |
          curl -X POST ${{ secrets.ATTUNE_API }}/packs/my_pack/test \
            -H "Authorization: Bearer ${{ secrets.ATTUNE_TOKEN }}" \
            -f  # Fail on HTTP errors
```

### 2. Monitor Test Trends

Track test pass rates over time:

```javascript
// Store metrics in monitoring system
const latest = await client.getLatestTest('core');

metrics.gauge('pack.test.pass_rate', latest.passRate, {
  pack: 'core',
  version: latest.packVersion
});

metrics.gauge('pack.test.duration_ms', latest.durationMs, {
  pack: 'core'
});
```

### 3. Alert on Test Failures

Set up alerts for failing tests:

```javascript
const latest = await client.getLatestTest('core');

if (latest.failed > 0) {
  await slack.sendMessage({
    channel: '#alerts',
    text: `⚠️ Pack 'core' has ${latest.failed} failing tests!`,
    attachments: [{
      color: 'danger',
      fields: [
        { title: 'Pass Rate', value: `${latest.passRate * 100}%` },
        { title: 'Failed', value: latest.failed },
        { title: 'Duration', value: `${latest.durationMs}ms` }
      ]
    }]
  });
}
```

### 4. Use Timeouts

Test execution can take time. Use appropriate timeouts:

```bash
# 5 minute timeout for test execution
timeout 300 curl -X POST "$API_URL/packs/my_pack/test" \
  -H "Authorization: Bearer $TOKEN"
```

---

## Troubleshooting

### Tests Always Fail

**Problem**: Tests fail even though they work locally

**Solutions**:
1. Check pack.yaml testing configuration is correct
2. Verify test files exist and are executable
3. Check dependencies are available in API environment
4. Review test logs in database `result` field

### Timeout Errors

**Problem**: Test execution times out

**Solutions**:
1. Increase timeout in pack.yaml runners
2. Split tests into multiple suites
3. Mock slow external dependencies
4. Consider async test execution (future feature)

### Missing Test Results

**Problem**: No test history available

**Solutions**:
1. Run tests at least once: `POST /packs/{ref}/test`
2. Check pack exists in database
3. Verify database migrations have run

---

## Related Documentation

- **Pack Testing Framework**: `docs/pack-testing-framework.md`
- **Pack Testing Guide**: `docs/PACK_TESTING.md`
- **Core Pack Tests**: `packs/core/tests/README.md`
- **API Reference**: `docs/api-reference.md`
- **Database Schema**: `migrations/012_add_pack_test_results.sql`

---

## Future Enhancements

- [ ] Async test execution (return job ID, poll for results)
- [ ] Webhooks for test completion
- [ ] Test result comparison (diff between runs)
- [ ] Test coverage metrics
- [ ] Performance regression detection
- [ ] Scheduled test execution
- [ ] Test result retention policies

---

## Changelog

- **2026-01-22**: Initial implementation
  - POST /packs/{ref}/test - Execute tests
  - GET /packs/{ref}/tests - Get test history
  - GET /packs/{ref}/tests/latest - Get latest test result