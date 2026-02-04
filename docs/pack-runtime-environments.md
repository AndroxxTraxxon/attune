# Pack Runtime Environments

**Status:** Production-ready  
**Version:** 1.0  
**Last Updated:** 2024-02-03

---

## Overview

Pack runtime environments provide **isolated dependency management** for each pack. When a pack declares runtime dependencies (e.g., Python packages, npm modules), these are installed in a pack-specific environment rather than system-wide. This prevents conflicts between packs that use the same runtime but require different dependency versions.

---

## Architecture

### Directory Structure

```
/opt/attune/
├── packs/                    # Pack code and metadata
│   ├── mycompany.monitoring/
│   │   ├── pack.yaml
│   │   ├── requirements.txt  # Python dependencies
│   │   └── actions/
│   └── core/
│       └── ...
└── packenvs/                 # Isolated runtime environments
    ├── mycompany.monitoring/
    │   ├── python/           # Python venv for this pack
    │   │   ├── bin/
    │   │   │   ├── python
    │   │   │   └── pip
    │   │   └── lib/
    │   └── nodejs/           # Node.js modules for this pack
    │       └── node_modules/
    └── core/
        └── python/
```

### Database Schema

#### `runtime` Table Enhancement

New field: `installers` (JSONB)

Stores instructions for creating pack-specific environments:

```json
{
  "base_path_template": "/opt/attune/packenvs/{pack_ref}/{runtime_name_lower}",
  "installers": [
    {
      "name": "create_venv",
      "description": "Create Python virtual environment",
      "command": "python3",
      "args": ["-m", "venv", "{env_path}"],
      "cwd": "{pack_path}",
      "env": {},
      "order": 1,
      "optional": false
    },
    {
      "name": "install_requirements",
      "description": "Install pack Python dependencies",
      "command": "{env_path}/bin/pip",
      "args": ["install", "-r", "{pack_path}/requirements.txt"],
      "cwd": "{pack_path}",
      "env": {},
      "order": 2,
      "optional": false,
      "condition": {
        "file_exists": "{pack_path}/requirements.txt"
      }
    }
  ],
  "executable_templates": {
    "python": "{env_path}/bin/python",
    "pip": "{env_path}/bin/pip"
  },
  "requires_environment": true
}
```

#### `pack_environment` Table

Tracks installed environments:

```sql
CREATE TABLE pack_environment (
    id BIGSERIAL PRIMARY KEY,
    pack BIGINT NOT NULL REFERENCES pack(id),
    pack_ref TEXT NOT NULL,
    runtime BIGINT NOT NULL REFERENCES runtime(id),
    runtime_ref TEXT NOT NULL,
    env_path TEXT NOT NULL,
    status pack_environment_status_enum NOT NULL,
    installed_at TIMESTAMPTZ,
    last_verified TIMESTAMPTZ,
    install_log TEXT,
    install_error TEXT,
    metadata JSONB,
    UNIQUE(pack, runtime)
);
```

**Status Values:**
- `pending` - Environment creation scheduled
- `installing` - Currently installing
- `ready` - Environment ready for use
- `failed` - Installation failed
- `outdated` - Pack updated, needs rebuild

---

## Template Variables

Installer commands support template variable substitution:

| Variable | Description | Example |
|----------|-------------|---------|
| `{env_path}` | Full path to environment | `/opt/attune/packenvs/mypack/python` |
| `{pack_path}` | Full path to pack directory | `/opt/attune/packs/mypack` |
| `{pack_ref}` | Pack reference | `mycompany.monitoring` |
| `{runtime_ref}` | Runtime reference | `core.python` |
| `{runtime_name_lower}` | Lowercase runtime name | `python` |

---

## Runtime-Specific Configuration

### Python

**Installer Actions:**
1. Create virtual environment: `python3 -m venv {env_path}`
2. Upgrade pip: `{env_path}/bin/pip install --upgrade pip`
3. Install requirements: `{env_path}/bin/pip install -r {pack_path}/requirements.txt`

**Pack Requirements File:** `requirements.txt` (optional)

**Executable Templates:**
- `python`: `{env_path}/bin/python`
- `pip`: `{env_path}/bin/pip`

**Example `requirements.txt`:**
```
requests>=2.28.0
pyyaml>=6.0
psutil>=5.9.0
```

### Node.js

**Installer Actions:**
1. Install dependencies: `npm install --prefix {env_path}`

**Pack Requirements File:** `package.json` (optional)

**Environment Variables:**
- `NODE_PATH`: `{env_path}/node_modules`

**Example `package.json`:**
```json
{
  "name": "mypack",
  "dependencies": {
    "axios": "^1.6.0",
    "dotenv": "^16.0.0"
  }
}
```

### Shell

**No environment needed** - Uses system shell directly.

**Installer Actions:** None

**`requires_environment`:** `false`

### Native (Compiled Binaries)

**No environment needed** - Binaries are standalone executables.

**Installer Actions:** None

**`requires_environment`:** `false`

---

## Pack Environment Manager API

### Module Location

`attune_common::pack_environment::PackEnvironmentManager`

### Methods

```rust
use attune_common::pack_environment::PackEnvironmentManager;
use std::path::Path;

// Create manager
let manager = PackEnvironmentManager::new(pool.clone(), &config);

// Ensure environment exists (creates if needed)
let env = manager.ensure_environment(
    pack_id,
    "mycompany.monitoring",
    runtime_id,
    "core.python",
    Path::new("/opt/attune/packs/mycompany.monitoring")
).await?;

// Get environment details
let env = manager.get_environment(pack_id, runtime_id).await?;

// Get executable path
let python_path = manager.get_executable_path(
    pack_id,
    runtime_id,
    "python"
).await?;

// Verify environment is functional
let is_valid = manager.verify_environment(pack_id, runtime_id).await?;

// Delete environment
manager.delete_environment(pack_id, runtime_id).await?;

// List all environments for a pack
let envs = manager.list_pack_environments(pack_id).await?;
```

---

## Pack Installation Flow

### 1. Pack Installation Triggered

When a pack is installed via:
- CLI: `attune pack install mypack.tar.gz`
- API: `POST /api/packs/install`

### 2. Pack Metadata Parsed

System reads `pack.yaml` to determine runtime dependencies:

```yaml
name: mycompany.monitoring
runtime_dependencies:
  - python: ">=3.8"
```

### 3. Environments Created

For each runtime dependency:

1. **Check if environment exists**
   - Query `pack_environment` table
   - If `status = 'ready'`, skip creation

2. **Create environment record**
   - Insert into `pack_environment` with `status = 'pending'`
   - Calculate `env_path` using template

3. **Execute installer actions**
   - Update `status = 'installing'`
   - Run each installer in `order` sequence
   - Capture logs in `install_log`
   - On success: `status = 'ready'`, `installed_at = NOW()`
   - On failure: `status = 'failed'`, error in `install_error`

4. **Record completion**
   - Update `last_verified` timestamp
   - Store metadata (installed packages, versions)

### 4. Pack Ready

Once all environments are `ready`, pack is available for use.

---

## Action/Sensor Execution Flow

### 1. Execution Request Received

Worker receives execution request for an action.

### 2. Pack Environment Lookup

```rust
// Get pack environment for this runtime
let env = pack_env_manager
    .get_environment(pack_id, runtime_id)
    .await?;

if env.status != PackEnvironmentStatus::Ready {
    return Err("Pack environment not ready");
}
```

### 3. Executable Path Resolution

```rust
// Get pack-specific Python interpreter
let python_path = pack_env_manager
    .get_executable_path(pack_id, runtime_id, "python")
    .await?
    .unwrap_or_else(|| "python3".to_string()); // Fallback to system

// Execute action with pack environment
let mut cmd = Command::new(&python_path);
cmd.arg(action_script_path);
cmd.current_dir(pack_path);
// ... rest of execution
```

### 4. Environment Variables

Set runtime-specific environment variables:

**Python:**
```rust
cmd.env("VIRTUAL_ENV", env_path);
cmd.env("PATH", format!("{}/bin:{}", env_path, std::env::var("PATH")?));
```

**Node.js:**
```rust
cmd.env("NODE_PATH", format!("{}/node_modules", env_path));
```

---

## Installer Conditions

Installers can specify conditions to control execution:

### File Exists

```json
{
  "condition": {
    "file_exists": "{pack_path}/requirements.txt"
  }
}
```

Only runs if the specified file exists.

### Future Conditions

Planned support for:
- `command_exists`: Check if command is available
- `env_var_set`: Check if environment variable is set
- `platform`: Run only on specific platforms (Linux, Darwin, etc.)

---

## Error Handling

### Installation Failures

When an installer fails:

1. **Non-optional installer fails** → Environment marked `failed`
2. **Optional installer fails** → Log warning, continue
3. **Installation log** → Stored in `pack_environment.install_log`
4. **Error message** → Stored in `pack_environment.install_error`

**Example Error:**

```
Installer 'install_requirements' failed: Command failed with exit code Some(1)
STDOUT:
Collecting requests>=2.28.0
  ERROR: Could not find a version that satisfies the requirement requests>=2.28.0

STDERR:
ERROR: No matching distribution found for requests>=2.28.0
```

### Recovery

**Retry Installation:**

```sql
-- Reset environment to pending
UPDATE pack_environment
SET status = 'pending', install_error = NULL, install_log = NULL
WHERE pack = $pack_id AND runtime = $runtime_id;
```

Then trigger re-installation via pack manager.

**Manual Fix:**

```bash
# Fix dependencies manually
cd /opt/attune/packenvs/mypack/python
source bin/activate
pip install requests==2.31.0

# Mark as ready
psql -U attune -d attune -c \
  "UPDATE pack_environment SET status = 'ready', installed_at = NOW() WHERE id = $env_id;"
```

---

## Monitoring & Verification

### Check Environment Status

```sql
-- View all environments with health status
SELECT * FROM v_pack_environment_status;

-- Check specific pack
SELECT
    pack_ref,
    runtime_ref,
    status,
    installed_at,
    last_verified
FROM v_pack_environment_status
WHERE pack_ref = 'mycompany.monitoring';
```

### Periodic Verification

**Recommended:** Verify environments weekly.

```rust
use attune_common::pack_environment::PackEnvironmentManager;

async fn verify_all_environments(manager: &PackEnvironmentManager) -> Result<()> {
    let envs = sqlx::query("SELECT pack, runtime FROM pack_environment WHERE status = 'ready'")
        .fetch_all(&manager.pool)
        .await?;

    for env in envs {
        let pack_id: i64 = env.get("pack");
        let runtime_id: i64 = env.get("runtime");
        
        if !manager.verify_environment(pack_id, runtime_id).await? {
            warn!("Environment verification failed: pack={} runtime={}", pack_id, runtime_id);
        }
    }

    Ok(())
}
```

### Cleanup Orphaned Environments

```sql
-- Find environments for deleted packs
SELECT pe.*
FROM pack_environment pe
LEFT JOIN pack p ON pe.pack = p.id
WHERE p.id IS NULL;

-- Delete orphaned environments
DELETE FROM pack_environment
WHERE pack NOT IN (SELECT id FROM pack);
```

---

## Database Functions

### `get_pack_environment_path(pack_ref, runtime_ref)`

Calculate filesystem path for an environment.

```sql
SELECT get_pack_environment_path('mycompany.monitoring', 'core.python');
-- Returns: /opt/attune/packenvs/mycompany.monitoring/python
```

### `runtime_requires_environment(runtime_ref)`

Check if a runtime needs a pack-specific environment.

```sql
SELECT runtime_requires_environment('core.python');  -- true
SELECT runtime_requires_environment('core.shell');   -- false
```

---

## Best Practices

### Pack Authors

1. **Specify Dependencies Explicitly**
   - Include `requirements.txt` or `package.json`
   - Pin versions or use version ranges

2. **Minimal Dependencies**
   - Only include required packages
   - Avoid large frameworks if not needed

3. **Test Locally**
   - Create test environment: `python3 -m venv testenv`
   - Install and test: `testenv/bin/pip install -r requirements.txt`

4. **Document Requirements**
   - List dependencies in pack README
   - Note any system-level requirements

### Operators

1. **Monitor Environment Health**
   - Check `v_pack_environment_status` regularly
   - Set up alerts for `failed` status

2. **Disk Space Management**
   - Environments can be large (100MB+ for Python)
   - Monitor `/opt/attune/packenvs` disk usage

3. **Rebuild Outdated Environments**
   - When packs update, rebuild environments
   - Use `DELETE FROM pack_environment` and reinstall

4. **Backup Considerations**
   - Environments are **reproducible** - don't need backup
   - Backup pack files and database only

---

## Troubleshooting

### Environment Stuck in "installing"

**Cause:** Installation process crashed or was interrupted.

**Fix:**
```sql
UPDATE pack_environment SET status = 'pending' WHERE status = 'installing';
```

Then trigger re-installation.

### Python venv Creation Fails

**Error:** `python3: command not found`

**Cause:** Python 3 not installed on worker.

**Fix:**
```bash
# Install Python 3
apt-get install python3 python3-venv  # Debian/Ubuntu
yum install python3                    # RHEL/CentOS
```

### Dependency Installation Fails

**Error:** `ERROR: Could not find a version that satisfies the requirement ...`

**Cause:** Package not available or version conflict.

**Fix:**
1. Check package name and version in `requirements.txt`
2. Test installation manually: `pip install <package>`
3. Update pack dependencies to compatible versions

### Node.js npm install Fails

**Error:** `npm ERR! Cannot read property 'match' of undefined`

**Cause:** Corrupted `package-lock.json` or npm cache.

**Fix:**
```bash
cd /opt/attune/packs/mypack
rm package-lock.json
rm -rf node_modules
# Reinstall pack to trigger environment rebuild
```

---

## Migration Guide

### Existing Packs

For packs installed before pack environments:

1. **No changes required** - Packs without dependency files work as before
2. **Add dependency files** - Create `requirements.txt` or `package.json`
3. **Reinstall pack** - Trigger environment creation

```bash
# Reinstall pack
attune pack uninstall mypack
attune pack install mypack.tar.gz
```

### Database Migration

Run migration `20260203000002_add_pack_environments.sql`:

```bash
cd attune
sqlx migrate run
```

**Changes:**
- Adds `runtime.installers` column
- Creates `pack_environment` table
- Populates installer metadata for core runtimes

---

## Future Enhancements

### Planned Features

1. **Version Pinning**
   - Store detected runtime versions
   - Require specific versions per pack
   - Example: `python: "3.11.x"`

2. **Shared Base Environments**
   - Common dependencies in shared layer
   - Pack-specific on top
   - Reduces disk usage

3. **Container-Based Environments**
   - Run each pack in isolated container
   - Full OS-level isolation
   - Resource limits per pack

4. **Dependency Caching**
   - Cache downloaded packages
   - Faster environment creation
   - Offline installation support

5. **Environment Templates**
   - Pre-built environment images
   - Quick cloning for new packs
   - Standardized base environments

---

## API Reference

### Pack Environment Status Codes

| Status | Description | Recovery |
|--------|-------------|----------|
| `pending` | Not yet installed | Normal - will install |
| `installing` | Installation in progress | Wait or reset to pending |
| `ready` | Installed and verified | None needed |
| `failed` | Installation failed | Check logs, fix issues, reset |
| `outdated` | Pack updated | Reinstall environment |

### Common SQL Queries

**List all ready environments:**
```sql
SELECT pack_ref, runtime_ref, env_path
FROM pack_environment
WHERE status = 'ready';
```

**Find failed installations:**
```sql
SELECT pack_ref, runtime_ref, install_error
FROM pack_environment
WHERE status = 'failed';
```

**Recent installations:**
```sql
SELECT pack_ref, runtime_ref, installed_at
FROM pack_environment
WHERE installed_at > NOW() - INTERVAL '24 hours'
ORDER BY installed_at DESC;
```

---

## Summary

✅ **Isolated Environments** - Each pack gets its own dependencies  
✅ **Conflict Prevention** - No version conflicts between packs  
✅ **Database-Driven** - Installation instructions in database  
✅ **Automatic Setup** - Environments created on pack install  
✅ **Runtime Agnostic** - Supports Python, Node.js, and extensible to others  
✅ **Production Ready** - Includes monitoring, verification, and error handling  

**Migration Required:** Yes (`20260203000002_add_pack_environments.sql`)  
**Breaking Changes:** None (additive feature)  
**Production Ready:** ✅ Yes