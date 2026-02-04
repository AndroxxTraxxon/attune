# Pack Testing Framework

**Status**: 🔄 Design Document  
**Created**: 2026-01-20  
**Purpose**: Define how packs are tested programmatically during installation and validation

---

## Overview

The Pack Testing Framework enables automatic discovery and execution of pack tests during:
- Pack installation/loading
- Pack updates
- System validation
- CI/CD pipelines

This ensures that packs work correctly in the target environment before they're activated.

---

## Design Principles

1. **Runtime-Aware Testing**: Tests execute in the same runtime as actions
2. **Fail-Fast Installation**: Packs don't activate if tests fail (unless forced)
3. **Dependency Validation**: Tests verify all dependencies are satisfied
4. **Standardized Results**: Common test result format across all runner types
5. **Optional but Recommended**: Tests are optional but strongly encouraged
6. **Self-Documenting**: Test results stored for auditing and troubleshooting

---

## Pack Manifest Extension

### pack.yaml Schema Addition

```yaml
# Pack Testing Configuration
testing:
  # Enable/disable testing during installation
  enabled: true
  
  # Test discovery method
  discovery:
    method: "directory"  # directory, manifest, executable
    path: "tests"        # relative to pack root
  
  # Test runners by runtime type
  runners:
    shell:
      type: "script"
      entry_point: "tests/run_tests.sh"
      timeout: 60  # seconds
      
    python:
      type: "pytest"
      entry_point: "tests/test_actions.py"
      requirements: "tests/requirements-test.txt"  # optional
      timeout: 120
      
    node:
      type: "jest"
      entry_point: "tests/"
      config: "tests/jest.config.js"
      timeout: 90
  
  # Test result expectations
  result_format: "junit-xml"  # junit-xml, tap, json
  result_path: "tests/results/"  # where to find test results
  
  # Minimum passing criteria
  min_pass_rate: 1.0  # 100% tests must pass (0.0-1.0)
  
  # What to do on test failure
  on_failure: "block"  # block, warn, ignore
```

### Example: Core Pack

```yaml
# packs/core/pack.yaml
ref: core
label: "Core Pack"
version: "1.0.0"

# ... existing config ...

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
      
    python:
      type: "pytest"
      entry_point: "tests/test_actions.py"
      timeout: 120
  
  result_format: "junit-xml"
  result_path: "tests/results/"
  min_pass_rate: 1.0
  on_failure: "block"
```

---

## Test Discovery Methods

### Method 1: Directory-Based (Recommended)

**Convention**:
```
pack_name/
├── actions/
├── sensors/
├── tests/              # Test directory
│   ├── run_tests.sh    # Shell test runner
│   ├── test_*.py       # Python tests
│   ├── test_*.js       # Node.js tests
│   └── results/        # Test output directory
└── pack.yaml
```

**Discovery Logic**:
1. Check if `tests/` directory exists
2. Look for test runners matching pack's runtime types
3. Execute all discovered test runners
4. Aggregate results

### Method 2: Manifest-Based

**Explicit test listing in pack.yaml**:
```yaml
testing:
  enabled: true
  discovery:
    method: "manifest"
    tests:
      - name: "Action Tests"
        runner: "python"
        command: "pytest tests/test_actions.py -v"
        timeout: 60
        
      - name: "Integration Tests"
        runner: "shell"
        command: "bash tests/integration_tests.sh"
        timeout: 120
```

### Method 3: Executable-Based

**Single test executable**:
```yaml
testing:
  enabled: true
  discovery:
    method: "executable"
    command: "make test"
    timeout: 180
```

---

## Test Execution Workflow

### 1. Pack Installation Flow

```
User: attune pack install ./packs/my_pack
    ↓
CLI validates pack structure
    ↓
CLI reads pack.yaml → testing section
    ↓
CLI discovers test runners
    ↓
For each runtime type in pack:
    ↓
    Worker Service executes tests
    ↓
    Collect test results
    ↓
    Parse results (JUnit XML, JSON, etc.)
    ↓
All tests pass?
    ↓ YES              ↓ NO
    ↓                  ↓
Activate pack      on_failure = "block"?
                       ↓ YES          ↓ NO
                       ↓              ↓
                   Abort install   Show warning,
                   Show errors     allow install
```

### 2. Test Execution Process

```rust
// Pseudocode for test execution

async fn execute_pack_tests(pack: &Pack) -> TestResults {
    let test_config = pack.testing.unwrap_or_default();
    
    if !test_config.enabled {
        return TestResults::Skipped;
    }
    
    let mut results = Vec::new();
    
    // Discover tests based on method
    let tests = discover_tests(&pack, &test_config)?;
    
    // Execute each test suite
    for test_suite in tests {
        let runtime = get_runtime_for_test(test_suite.runner)?;
        
        let result = runtime.execute_test(
            test_suite.command,
            test_suite.timeout,
            test_suite.env_vars
        ).await?;
        
        results.push(result);
    }
    
    // Parse and aggregate results
    let aggregate = aggregate_test_results(results, test_config.result_format)?;
    
    // Store in database
    store_test_results(&pack, &aggregate).await?;
    
    // Check pass criteria
    if aggregate.pass_rate < test_config.min_pass_rate {
        return TestResults::Failed(aggregate);
    }
    
    TestResults::Passed(aggregate)
}
```

---

## Test Result Format

### Standardized Test Result Structure

```rust
// Common library: models.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackTestResult {
    pub pack_ref: String,
    pub pack_version: String,
    pub execution_time: DateTime<Utc>,
    pub total_tests: i32,
    pub passed: i32,
    pub failed: i32,
    pub skipped: i32,
    pub pass_rate: f64,
    pub duration_ms: i64,
    pub test_suites: Vec<TestSuiteResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestSuiteResult {
    pub name: String,
    pub runner_type: String,  // shell, python, node
    pub total: i32,
    pub passed: i32,
    pub failed: i32,
    pub skipped: i32,
    pub duration_ms: i64,
    pub test_cases: Vec<TestCaseResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCaseResult {
    pub name: String,
    pub status: TestStatus,
    pub duration_ms: i64,
    pub error_message: Option<String>,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TestStatus {
    Passed,
    Failed,
    Skipped,
    Error,
}
```

### Example JSON Output

```json
{
  "pack_ref": "core",
  "pack_version": "1.0.0",
  "execution_time": "2026-01-20T10:30:00Z",
  "total_tests": 36,
  "passed": 36,
  "failed": 0,
  "skipped": 0,
  "pass_rate": 1.0,
  "duration_ms": 20145,
  "test_suites": [
    {
      "name": "Bash Test Runner",
      "runner_type": "shell",
      "total": 36,
      "passed": 36,
      "failed": 0,
      "skipped": 0,
      "duration_ms": 20145,
      "test_cases": [
        {
          "name": "echo: basic message",
          "status": "Passed",
          "duration_ms": 245,
          "error_message": null,
          "stdout": "Hello, Attune!\n",
          "stderr": null
        },
        {
          "name": "noop: invalid exit code",
          "status": "Passed",
          "duration_ms": 189,
          "error_message": null,
          "stdout": "",
          "stderr": "ERROR: exit_code must be between 0 and 255\n"
        }
      ]
    }
  ]
}
```

---

## Database Schema

### Migration: `add_pack_test_results.sql`

```sql
-- Pack test execution tracking
CREATE TABLE attune.pack_test_execution (
    id BIGSERIAL PRIMARY KEY,
    pack_id BIGINT NOT NULL REFERENCES attune.pack(id) ON DELETE CASCADE,
    pack_version VARCHAR(50) NOT NULL,
    execution_time TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    trigger_reason VARCHAR(50) NOT NULL, -- 'install', 'update', 'manual', 'validation'
    total_tests INT NOT NULL,
    passed INT NOT NULL,
    failed INT NOT NULL,
    skipped INT NOT NULL,
    pass_rate DECIMAL(5,4) NOT NULL, -- 0.0000 to 1.0000
    duration_ms BIGINT NOT NULL,
    result JSONB NOT NULL, -- Full test result structure
    created TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_pack_test_execution_pack_id ON attune.pack_test_execution(pack_id);
CREATE INDEX idx_pack_test_execution_time ON attune.pack_test_execution(execution_time DESC);
CREATE INDEX idx_pack_test_execution_pass_rate ON attune.pack_test_execution(pass_rate);

-- Pack test result summary view
CREATE VIEW attune.pack_test_summary AS
SELECT 
    p.id AS pack_id,
    p.ref AS pack_ref,
    p.label AS pack_label,
    pte.pack_version,
    pte.execution_time AS last_test_time,
    pte.total_tests,
    pte.passed,
    pte.failed,
    pte.skipped,
    pte.pass_rate,
    pte.trigger_reason,
    ROW_NUMBER() OVER (PARTITION BY p.id ORDER BY pte.execution_time DESC) AS rn
FROM attune.pack p
LEFT JOIN attune.pack_test_execution pte ON p.id = pte.pack_id
WHERE pte.id IS NOT NULL;

-- Latest test results per pack
CREATE VIEW attune.pack_latest_test AS
SELECT 
    pack_id,
    pack_ref,
    pack_label,
    pack_version,
    last_test_time,
    total_tests,
    passed,
    failed,
    skipped,
    pass_rate,
    trigger_reason
FROM attune.pack_test_summary
WHERE rn = 1;
```

---

## Worker Service Integration

### Test Execution in Worker

```rust
// crates/worker/src/test_executor.rs

use attune_common::models::{PackTestResult, TestSuiteResult};
use std::path::PathBuf;
use std::time::Duration;

pub struct TestExecutor {
    runtime_manager: Arc<RuntimeManager>,
}

impl TestExecutor {
    pub async fn execute_pack_tests(
        &self,
        pack_dir: &PathBuf,
        test_config: &TestConfig,
    ) -> Result<PackTestResult> {
        let mut suites = Vec::new();
        
        // Execute tests for each runner type
        for (runner_type, runner_config) in &test_config.runners {
            let suite_result = self.execute_test_suite(
                pack_dir,
                runner_type,
                runner_config,
            ).await?;
            
            suites.push(suite_result);
        }
        
        // Aggregate results
        let total: i32 = suites.iter().map(|s| s.total).sum();
        let passed: i32 = suites.iter().map(|s| s.passed).sum();
        let failed: i32 = suites.iter().map(|s| s.failed).sum();
        let skipped: i32 = suites.iter().map(|s| s.skipped).sum();
        let duration_ms: i64 = suites.iter().map(|s| s.duration_ms).sum();
        
        Ok(PackTestResult {
            pack_ref: pack_dir.file_name().unwrap().to_string_lossy().to_string(),
            pack_version: "1.0.0".to_string(), // TODO: Get from pack.yaml
            execution_time: Utc::now(),
            total_tests: total,
            passed,
            failed,
            skipped,
            pass_rate: if total > 0 { passed as f64 / total as f64 } else { 0.0 },
            duration_ms,
            test_suites: suites,
        })
    }
    
    async fn execute_test_suite(
        &self,
        pack_dir: &PathBuf,
        runner_type: &str,
        runner_config: &RunnerConfig,
    ) -> Result<TestSuiteResult> {
        let runtime = self.runtime_manager.get_runtime(runner_type)?;
        
        // Build test command
        let test_script = pack_dir.join(&runner_config.entry_point);
        
        // Execute with timeout
        let timeout = Duration::from_secs(runner_config.timeout);
        
        let output = runtime.execute_with_timeout(
            &test_script,
            HashMap::new(), // env vars
            timeout,
        ).await?;
        
        // Parse test results based on format
        let test_result = match runner_config.result_format.as_str() {
            "junit-xml" => self.parse_junit_xml(&output.stdout)?,
            "json" => self.parse_json_results(&output.stdout)?,
            "tap" => self.parse_tap_results(&output.stdout)?,
            _ => self.parse_simple_output(&output)?,
        };
        
        Ok(test_result)
    }
    
    fn parse_simple_output(&self, output: &CommandOutput) -> Result<TestSuiteResult> {
        // Parse simple output format (what our bash runner uses)
        // Look for patterns like:
        // "Total Tests: 36"
        // "Passed: 36"
        // "Failed: 0"
        
        let stdout = String::from_utf8_lossy(&output.stdout);
        
        let total = self.extract_number(&stdout, "Total Tests:")?;
        let passed = self.extract_number(&stdout, "Passed:")?;
        let failed = self.extract_number(&stdout, "Failed:")?;
        
        Ok(TestSuiteResult {
            name: "Shell Test Runner".to_string(),
            runner_type: "shell".to_string(),
            total,
            passed,
            failed,
            skipped: 0,
            duration_ms: output.duration_ms,
            test_cases: vec![], // Could parse individual test lines
        })
    }
}
```

---

## CLI Commands

### Pack Test Command

```bash
# Test a pack before installation
attune pack test ./packs/my_pack

# Test an installed pack
attune pack test core

# Test with verbose output
attune pack test core --verbose

# Test and show detailed results
attune pack test core --detailed

# Test specific runtime
attune pack test core --runtime python

# Force install even if tests fail
attune pack install ./packs/my_pack --skip-tests
attune pack install ./packs/my_pack --force
```

### CLI Implementation

```rust
// crates/cli/src/commands/pack.rs

pub async fn test_pack(pack_path: &str, options: TestOptions) -> Result<()> {
    println!("🧪 Testing pack: {}", pack_path);
    println!();
    
    // Load pack configuration
    let pack_yaml = load_pack_yaml(pack_path)?;
    let test_config = pack_yaml.testing.ok_or("No test configuration found")?;
    
    if !test_config.enabled {
        println!("⚠️  Testing disabled for this pack");
        return Ok(());
    }
    
    // Execute tests via worker
    let client = create_worker_client().await?;
    let result = client.execute_pack_tests(pack_path, test_config).await?;
    
    // Display results
    display_test_results(&result, options.verbose)?;
    
    // Exit with appropriate code
    if result.failed > 0 {
        println!();
        println!("❌ Tests failed: {}/{}", result.failed, result.total_tests);
        std::process::exit(1);
    } else {
        println!();
        println!("✅ All tests passed: {}/{}", result.passed, result.total_tests);
        Ok(())
    }
}
```

---

## Test Result Parsers

### JUnit XML Parser

```rust
// crates/worker/src/test_parsers/junit.rs

pub fn parse_junit_xml(xml: &str) -> Result<TestSuiteResult> {
    // Parse JUnit XML format (pytest --junit-xml, Jest, etc.)
    // <testsuite name="..." tests="36" failures="0" skipped="0" time="12.5">
    //   <testcase name="..." time="0.245" />
    //   <testcase name="..." time="0.189">
    //     <failure message="...">Stack trace</failure>
    //   </testcase>
    // </testsuite>
    
    // Implementation using quick-xml or roxmltree crate
}
```

### TAP Parser

```rust
// crates/worker/src/test_parsers/tap.rs

pub fn parse_tap(tap_output: &str) -> Result<TestSuiteResult> {
    // Parse TAP (Test Anything Protocol) format
    // 1..36
    // ok 1 - echo: basic message
    // ok 2 - echo: default message
    // not ok 3 - echo: invalid parameter
    //   ---
    //   message: 'Expected failure'
    //   ...
}
```

---

## Pack Installation Integration

### Modified Pack Load Workflow

```rust
// crates/api/src/services/pack_service.rs

pub async fn install_pack(
    pack_path: &Path,
    options: InstallOptions,
) -> Result<Pack> {
    // 1. Validate pack structure
    validate_pack_structure(pack_path)?;
    
    // 2. Load pack.yaml
    let pack_config = load_pack_yaml(pack_path)?;
    
    // 3. Check if testing is enabled
    if pack_config.testing.map(|t| t.enabled).unwrap_or(false) {
        if !options.skip_tests {
            println!("🧪 Running pack tests...");
            
            let test_result = execute_pack_tests(pack_path, &pack_config).await?;
            
            // Store test results
            store_test_results(&test_result).await?;
            
            // Check if tests passed
            if test_result.failed > 0 {
                let on_failure = pack_config.testing
                    .and_then(|t| t.on_failure)
                    .unwrap_or(OnFailure::Block);
                
                match on_failure {
                    OnFailure::Block => {
                        if !options.force {
                            return Err(Error::PackTestsFailed {
                                failed: test_result.failed,
                                total: test_result.total_tests,
                            });
                        }
                    }
                    OnFailure::Warn => {
                        eprintln!("⚠️  Warning: {} tests failed", test_result.failed);
                    }
                    OnFailure::Ignore => {
                        // Continue installation
                    }
                }
            } else {
                println!("✅ All tests passed!");
            }
        }
    }
    
    // 4. Register pack in database
    let pack = register_pack(&pack_config).await?;
    
    // 5. Register actions, sensors, triggers
    register_pack_components(&pack, pack_path).await?;
    
    // 6. Set up runtime environments
    setup_pack_environments(&pack, pack_path).await?;
    
    Ok(pack)
}
```

---

## API Endpoints

### Test Results API

```rust
// GET /api/v1/packs/:pack_ref/tests
// List test executions for a pack

// GET /api/v1/packs/:pack_ref/tests/latest
// Get latest test results for a pack

// GET /api/v1/packs/:pack_ref/tests/:execution_id
// Get specific test execution details

// POST /api/v1/packs/:pack_ref/test
// Manually trigger pack tests

// GET /api/v1/packs/tests
// List all pack test results (admin)
```

---

## Best Practices for Pack Authors

### 1. Always Include Tests

```yaml
# pack.yaml
testing:
  enabled: true
  runners:
    shell:
      entry_point: "tests/run_tests.sh"
```

### 2. Test All Actions

Every action should have at least:
- One successful execution test
- One error handling test
- Parameter validation tests

### 3. Use Exit Codes Correctly

```bash
# tests/run_tests.sh
if [ $FAILURES -gt 0 ]; then
    exit 1  # Non-zero exit = test failure
else
    exit 0  # Zero exit = success
fi
```

### 4. Output Parseable Results

```bash
# Simple format the worker can parse
echo "Total Tests: $TOTAL"
echo "Passed: $PASSED"
echo "Failed: $FAILED"
```

### 5. Test Dependencies

```python
# tests/test_dependencies.py
def test_required_libraries():
    """Verify all required libraries are available"""
    import requests
    import croniter
    assert True
```

---

## Implementation Phases

### Phase 1: Core Framework ✅ (Current)
- [x] Design document (this file)
- [x] Core pack tests implemented
- [x] Test infrastructure created
- [ ] Database schema for test results
- [ ] Worker test executor implementation

### Phase 2: Worker Integration
- [ ] Test executor in worker service
- [ ] Simple output parser
- [ ] Test result storage
- [ ] Error handling and timeouts

### Phase 3: CLI Integration
- [ ] `attune pack test` command
- [ ] Test result display
- [ ] Integration with pack install
- [ ] Force/skip test options

### Phase 4: Advanced Features
- [ ] JUnit XML parser
- [ ] TAP parser
- [ ] API endpoints for test results
- [ ] Web UI for test results
- [ ] Test history and trends

---

## Future Enhancements

- **Parallel Test Execution**: Run tests for different runtimes in parallel
- **Test Caching**: Cache test results for unchanged packs
- **Selective Testing**: Test only changed actions
- **Performance Benchmarks**: Track test execution time trends
- **Test Coverage Reports**: Integration with coverage tools
- **Remote Test Execution**: Distribute tests across workers
- **Test Environments**: Isolated test environments per pack

---

## Conclusion

The Pack Testing Framework provides a standardized way to validate packs during installation, ensuring reliability and catching issues early. By making tests a first-class feature of the pack system, we enable:

- **Confident Installation**: Know that packs will work before activating them
- **Dependency Validation**: Verify all required dependencies are present
- **Regression Prevention**: Detect breaking changes when updating packs
- **Quality Assurance**: Encourage pack authors to write comprehensive tests
- **Audit Trail**: Track test results over time for compliance and debugging

---

**Next Steps**: Implement Phase 1 (database schema and worker test executor) to enable programmatic test execution during pack installation.