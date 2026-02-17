# Database-Driven Runtime Detection

**Version:** 1.0  
**Last Updated:** 2026-02-02

> **⚠️ Note:** This document was written before the `runtime_type` column was removed from the runtime table. SQL examples referencing `WHERE runtime_type = 'sensor'`, `INSERT ... runtime_type`, and 3-part refs like `core.sensor.python` are outdated. The current architecture uses unified runtimes with 2-part refs (`core.python`, `core.shell`) and determines executability by the presence of `execution_config`. See `docs/QUICKREF-unified-runtime-detection.md` for the current model.

---

## Overview

The sensor service uses **database-driven runtime detection** instead of hardcoded checks. Runtime availability verification is configured in the `runtime` table, making the sensor service independent and self-configuring. Adding new runtimes requires no code changes—just database configuration.

---

## Architecture

### How It Works

```
Sensor Service Startup
    ↓
Query runtime table for sensor runtimes
    ↓
For each runtime:
  - Check verification metadata
  - If "always_available": mark as available
  - If verification commands exist: try each in priority order
  - If any command succeeds: mark runtime as available
    ↓
Register sensor worker with detected runtimes
    ↓
Store capabilities in worker table
```

### Benefits

- ✅ **No code changes needed** to add new runtimes
- ✅ **Centralized configuration** in database
- ✅ **Flexible verification** with multiple fallback commands
- ✅ **Pattern matching** for version validation
- ✅ **Priority ordering** for preferred verification methods
- ✅ **Override capability** via environment variables

---

## Runtime Table Schema

### Relevant Columns

```sql
CREATE TABLE runtime (
    id BIGSERIAL PRIMARY KEY,
    ref TEXT NOT NULL UNIQUE,
    runtime_type runtime_type_enum NOT NULL,  -- 'action' or 'sensor'
    name TEXT NOT NULL,
    distributions JSONB NOT NULL,             -- Contains verification metadata
    installation JSONB,
    ...
);
```

### Verification Metadata Structure

Located in `distributions->verification`:

```json
{
  "verification": {
    "always_available": false,
    "check_required": true,
    "commands": [
      {
        "binary": "python3",
        "args": ["--version"],
        "exit_code": 0,
        "pattern": "Python 3\\.",
        "priority": 1,
        "optional": false
      },
      {
        "binary": "python",
        "args": ["--version"],
        "exit_code": 0,
        "pattern": "Python 3\\.",
        "priority": 2,
        "optional": false
      }
    ]
  }
}
```

### Field Definitions

| Field | Type | Description |
|-------|------|-------------|
| `always_available` | boolean | If true, skip verification (e.g., shell, native) |
| `check_required` | boolean | If false, assume available without checking |
| `commands` | array | List of verification commands to try |
| `commands[].binary` | string | Binary/executable name to run |
| `commands[].args` | array | Arguments to pass to binary |
| `commands[].exit_code` | integer | Expected exit code (default: 0) |
| `commands[].pattern` | string | Regex pattern to match in stdout/stderr |
| `commands[].priority` | integer | Lower number = higher priority (try first) |
| `commands[].optional` | boolean | If true, failure doesn't mean unavailable |

---

## Configured Sensor Runtimes

### Python Runtime

**Reference:** `core.sensor.python`

```json
{
  "verification": {
    "commands": [
      {
        "binary": "python3",
        "args": ["--version"],
        "exit_code": 0,
        "pattern": "Python 3\\.",
        "priority": 1
      },
      {
        "binary": "python",
        "args": ["--version"],
        "exit_code": 0,
        "pattern": "Python 3\\.",
        "priority": 2
      }
    ]
  },
  "min_version": "3.8",
  "recommended_version": "3.11"
}
```

**Verification Logic:**
1. Try `python3 --version` (priority 1)
2. If fails, try `python --version` (priority 2)
3. Check output matches regex `Python 3\.`
4. If any succeeds, mark Python as available

### Node.js Runtime

**Reference:** `core.sensor.nodejs`

```json
{
  "verification": {
    "commands": [
      {
        "binary": "node",
        "args": ["--version"],
        "exit_code": 0,
        "pattern": "v\\d+\\.\\d+\\.\\d+",
        "priority": 1
      }
    ]
  },
  "min_version": "16.0.0",
  "recommended_version": "20.0.0"
}
```

**Verification Logic:**
1. Run `node --version`
2. Check output matches version pattern (e.g., `v20.10.0`)
3. If succeeds, mark Node.js as available

### Shell Runtime

**Reference:** `core.sensor.shell`

```json
{
  "verification": {
    "commands": [
      {
        "binary": "sh",
        "args": ["--version"],
        "exit_code": 0,
        "optional": true,
        "priority": 1
      },
      {
        "binary": "bash",
        "args": ["--version"],
        "exit_code": 0,
        "optional": true,
        "priority": 2
      }
    ],
    "always_available": true
  }
}
```

**Verification Logic:**
- Marked as `always_available: true`
- Verification skipped, always reports as available
- Shell is assumed to be present on all systems

### Native Runtime

**Reference:** `core.sensor.native`

```json
{
  "verification": {
    "always_available": true,
    "check_required": false
  },
  "languages": ["rust", "go", "c", "c++"]
}
```

**Verification Logic:**
- Marked as `always_available: true`
- No verification needed
- Native compiled executables always supported

### Built-in Runtime

**Reference:** `core.sensor.builtin`

```json
{
  "verification": {
    "always_available": true,
    "check_required": false
  },
  "type": "builtin"
}
```

**Verification Logic:**
- Built-in sensors (like timer) always available
- Part of sensor service itself

---

## Adding New Runtimes

### Example: Adding Ruby Runtime

```sql
INSERT INTO runtime (ref, pack, pack_ref, description, runtime_type, name, distributions)
VALUES (
    'core.sensor.ruby',
    (SELECT id FROM pack WHERE ref = 'core'),
    'core',
    'Ruby sensor runtime',
    'sensor',
    'Ruby',
    jsonb_build_object(
        'verification', jsonb_build_object(
            'commands', jsonb_build_array(
                jsonb_build_object(
                    'binary', 'ruby',
                    'args', jsonb_build_array('--version'),
                    'exit_code', 0,
                    'pattern', 'ruby \\d+\\.\\d+',
                    'priority', 1
                )
            )
        ),
        'min_version', '3.0'
    )
);
```

**No code changes required!** The sensor service will automatically:
1. Discover the new runtime on next startup
2. Verify if `ruby` is available
3. Include it in reported capabilities if found

### Example: Adding Perl Runtime with Multiple Checks

```sql
INSERT INTO runtime (ref, pack, pack_ref, description, runtime_type, name, distributions)
VALUES (
    'core.sensor.perl',
    (SELECT id FROM pack WHERE ref = 'core'),
    'core',
    'Perl sensor runtime',
    'sensor',
    'Perl',
    jsonb_build_object(
        'verification', jsonb_build_object(
            'commands', jsonb_build_array(
                -- Try perl6 first (Raku)
                jsonb_build_object(
                    'binary', 'perl6',
                    'args', jsonb_build_array('--version'),
                    'exit_code', 0,
                    'priority', 1,
                    'optional', true
                ),
                -- Fall back to perl5
                jsonb_build_object(
                    'binary', 'perl',
                    'args', jsonb_build_array('--version'),
                    'exit_code', 0,
                    'pattern', 'perl',
                    'priority', 2
                )
            )
        )
    )
);
```

---

## Configuration Override

### Priority System

1. **Environment Variable** (highest priority)
   ```bash
   export ATTUNE_SENSOR_RUNTIMES="python,shell"
   ```
   Skips database detection entirely.

2. **Config File** (medium priority)
   ```yaml
   sensor:
     capabilities:
       runtimes: ["python", "shell"]
   ```
   Uses specified runtimes without verification.

3. **Database Detection** (lowest priority)
   Queries runtime table and verifies each runtime.

### Use Cases

**Development:** Override for faster startup
```bash
export ATTUNE_SENSOR_RUNTIMES="shell,python"
cargo run --bin attune-sensor
```

**Production:** Let database drive detection
```yaml
# No sensor.capabilities.runtimes specified
# Service auto-detects from database
```

**Restricted Environment:** Limit to available runtimes
```yaml
sensor:
  capabilities:
    runtimes: ["shell", "native"]  # Only these two
```

---

## Verification Process

### Step-by-Step

```rust
// 1. Query sensor runtimes from database
let runtimes = query_sensor_runtimes(&pool).await?;

// 2. For each runtime
for runtime in runtimes {
    // 3. Check if always available
    if runtime.always_available {
        available.push(runtime.name);
        continue;
    }
    
    // 4. Try verification commands in priority order
    for cmd in runtime.commands.sorted_by_priority() {
        // 5. Execute command
        let output = Command::new(cmd.binary)
            .args(&cmd.args)
            .output()?;
        
        // 6. Check exit code
        if output.status.code() != cmd.exit_code {
            continue;  // Try next command
        }
        
        // 7. Check pattern if specified
        if let Some(pattern) = cmd.pattern {
            let output_text = String::from_utf8_lossy(&output.stdout);
            if !Regex::new(pattern)?.is_match(&output_text) {
                continue;  // Try next command
            }
        }
        
        // 8. Success! Runtime is available
        available.push(runtime.name);
        break;
    }
}

// 9. Register with detected runtimes
register_worker(available).await?;
```

### Example: Python Verification

```
Query: SELECT * FROM runtime WHERE ref = 'core.sensor.python'

Retrieved verification commands:
  1. python3 --version (priority 1)
  2. python --version (priority 2)

Try command 1:
  $ python3 --version
  Output: "Python 3.11.6"
  Exit code: 0
  Pattern match: "Python 3\." ✓
  
Result: Python runtime AVAILABLE ✓
```

### Example: Haskell Verification (Not Installed)

```
Query: SELECT * FROM runtime WHERE ref = 'test.sensor.haskell'

Retrieved verification commands:
  1. ghc --version (priority 1)

Try command 1:
  $ ghc --version
  Error: Command not found
  
Result: Haskell runtime NOT AVAILABLE ✗
```

---

## Querying Available Runtimes

### View All Sensor Runtimes

```sql
SELECT ref, name, 
       distributions->'verification'->'always_available' as always_avail,
       distributions->'verification'->'commands' as verify_commands
FROM runtime 
WHERE runtime_type = 'sensor'
ORDER BY ref;
```

### Check Specific Runtime Verification

```sql
SELECT name,
       distributions->'verification' as verification_config
FROM runtime
WHERE ref = 'core.sensor.python';
```

### Find Runtimes by Verification Type

```sql
-- Always available runtimes
SELECT name FROM runtime
WHERE runtime_type = 'sensor'
  AND distributions->'verification'->>'always_available' = 'true';

-- Runtimes requiring verification
SELECT name FROM runtime
WHERE runtime_type = 'sensor'
  AND distributions->'verification'->>'check_required' = 'true';
```

---

## Troubleshooting

### Runtime Not Detected

**Symptom:** Expected runtime not in sensor worker capabilities

**Diagnosis:**
```bash
# Check if runtime in database
psql $DATABASE_URL -c "SELECT ref, name FROM runtime WHERE runtime_type = 'sensor';"

# Check verification metadata
psql $DATABASE_URL -c "SELECT distributions->'verification' FROM runtime WHERE ref = 'core.sensor.python';" -x

# Test verification command manually
python3 --version
```

**Solution:**
```sql
-- Fix verification command
UPDATE runtime
SET distributions = jsonb_set(
    distributions,
    '{verification,commands,0,binary}',
    '"python3"'
)
WHERE ref = 'core.sensor.python';
```

### All Runtimes Showing as Available (Incorrectly)

**Symptom:** Runtime reports as available but binary not installed

**Diagnosis:**
```bash
# Check if marked as always_available
psql $DATABASE_URL -c "SELECT ref, distributions->'verification'->>'always_available' FROM runtime WHERE runtime_type = 'sensor';"
```

**Solution:**
```sql
-- Remove always_available flag
UPDATE runtime
SET distributions = distributions - 'verification' || jsonb_build_object(
    'verification', jsonb_build_object(
        'commands', jsonb_build_array(
            jsonb_build_object(
                'binary', 'ruby',
                'args', jsonb_build_array('--version'),
                'exit_code', 0,
                'priority', 1
            )
        )
    )
)
WHERE ref = 'core.sensor.ruby';
```

### Pattern Matching Fails

**Symptom:** Verification command succeeds but runtime not detected

**Diagnosis:**
```bash
# Run verification command manually
python3 --version

# Check pattern in database
psql $DATABASE_URL -c "SELECT distributions->'verification'->'commands'->0->>'pattern' FROM runtime WHERE ref = 'core.sensor.python';"

# Test regex pattern
echo "Python 3.11.6" | grep -E "Python 3\."
```

**Solution:**
```sql
-- Fix regex pattern (use proper escaping)
UPDATE runtime
SET distributions = jsonb_set(
    distributions,
    '{verification,commands,0,pattern}',
    '"Python 3\\."'
)
WHERE ref = 'core.sensor.python';
```

---

## Performance Considerations

### Startup Time

- **Database Query:** ~10-20ms for 5-10 runtimes
- **Verification Per Runtime:** ~10-50ms depending on command
- **Total Startup Overhead:** ~100-300ms

### Optimization Tips

1. **Use always_available:** Skip verification for guaranteed runtimes
2. **Limit verification commands:** Fewer fallbacks = faster verification
3. **Cache results:** Future enhancement to cache verification results

### Comparison

```
Hardcoded detection: ~50-100ms (all checks in code)
Database-driven:     ~100-300ms (query + verify)

Trade-off: Slight startup delay for significantly better maintainability
```

---

## Security Considerations

### Command Injection

✅ **Safe:** Command and args are separate parameters, not shell-interpreted

```rust
// Safe: No shell interpretation
Command::new("python3")
    .args(&["--version"])
    .output()
```

❌ **Unsafe (Not Used):**
```rust
// Unsafe: Shell interpretation (NOT USED)
Command::new("sh")
    .arg("-c")
    .arg("python3 --version")  // Could be exploited
    .output()
```

### Malicious Runtime Entries

**Risk:** Database compromise could inject malicious verification commands

**Mitigations:**
- Database access control (restricted to svc_attune user)
- No shell interpretation of commands
- Verification runs with sensor service privileges (not root)
- Timeout protection (commands timeout after 10 seconds)

### Best Practices

1. **Restrict database access** to runtime table
2. **Validate patterns** before inserting (ensure valid regex)
3. **Audit changes** to runtime verification metadata
4. **Use specific binaries** (e.g., `/usr/bin/python3` instead of `python3`)

---

## Migration: 20260202000001

**File:** `migrations/20260202000001_add_sensor_runtimes.sql`

**Purpose:** Adds sensor runtimes with verification metadata

**Runtimes Added:**
- `core.sensor.python` - Python 3 with python3/python fallback
- `core.sensor.nodejs` - Node.js runtime
- `core.sensor.shell` - Shell (always available)
- `core.sensor.native` - Native compiled (always available)
- Updates `core.sensor.builtin` with metadata

**Apply:**
```bash
export DATABASE_URL="postgresql://attune:attune@localhost:5432/attune"
psql $DATABASE_URL < migrations/20260202000001_add_sensor_runtimes.sql
```

---

## See Also

- [Sensor Worker Registration](sensor-worker-registration.md)
- [Sensor Runtime Execution](sensor-runtime.md)
- [Runtime Table Schema](../database-schema.md)
- [Configuration Guide](../configuration/configuration.md)

---

**Status:** ✅ Implemented  
**Version:** 1.0  
**Requires:** PostgreSQL with runtime table, sensor service v0.1.0+