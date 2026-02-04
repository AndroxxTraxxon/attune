# Core Pack Setup Guide

This guide explains how to set up and load the Attune core pack into your database.

## Overview

The **core pack** is Attune's built-in system pack that provides essential automation components including:

- **Timer Triggers**: Interval-based, cron-based, and datetime triggers
- **Basic Actions**: Echo, sleep, noop, and HTTP request actions
- **Built-in Sensors**: Interval timer sensor for time-based automation

The core pack must be loaded into the database before it can be used in rules and workflows.

## Prerequisites

Before loading the core pack, ensure:

1. **PostgreSQL is running** and accessible
2. **Database migrations are applied**: `sqlx migrate run`
3. **Python 3.8+** is installed (for the loader script)
4. **Required Python packages** are installed:
   ```bash
   pip install psycopg2-binary pyyaml
   ```

## Loading Methods

### Method 1: Python Loader Script (Recommended)

The Python loader script reads the pack YAML files and creates database entries automatically.

**Usage:**
```bash
# From the project root
python3 scripts/load_core_pack.py

# With custom database URL
python3 scripts/load_core_pack.py --database-url "postgresql://user:pass@localhost:5432/attune"

# With custom pack directory
python3 scripts/load_core_pack.py --pack-dir ./packs
```

**What it does:**
- Reads `pack.yaml` for pack metadata
- Loads all trigger definitions from `triggers/*.yaml`
- Loads all action definitions from `actions/*.yaml`
- Loads all sensor definitions from `sensors/*.yaml`
- Creates or updates database entries (idempotent)
- Uses transactions (all-or-nothing)

**Output:**
```
============================================================
Core Pack Loader
============================================================

→ Loading pack metadata...
✓ Pack 'core' loaded (ID: 1)

→ Loading triggers...
  ✓ Trigger 'core.intervaltimer' (ID: 1)
  ✓ Trigger 'core.crontimer' (ID: 2)
  ✓ Trigger 'core.datetimetimer' (ID: 3)

→ Loading actions...
  ✓ Action 'core.echo' (ID: 1)
  ✓ Action 'core.sleep' (ID: 2)
  ✓ Action 'core.noop' (ID: 3)
  ✓ Action 'core.http_request' (ID: 4)

→ Loading sensors...
  ✓ Sensor 'core.interval_timer_sensor' (ID: 1)

============================================================
✓ Core pack loaded successfully!
============================================================
  Pack ID: 1
  Triggers: 3
  Actions: 4
  Sensors: 1
```

### Method 2: SQL Seed Script

For simpler setups or CI/CD, you can use the SQL seed script directly.

**Usage:**
```bash
psql $DATABASE_URL -f scripts/seed_core_pack.sql
```

**Note:** The SQL script may not include all pack metadata and is less flexible than the Python loader.

### Method 3: CLI (Future)

Once the CLI pack management commands are fully implemented:

```bash
attune pack register ./packs/core
```

## Verification

After loading, verify the core pack is available:

### Using CLI
```bash
# List all packs
attune pack list

# Show core pack details
attune pack show core

# List core pack actions
attune action list --pack core

# List core pack triggers
attune trigger list --pack core
```

### Using API
```bash
# Get pack info
curl http://localhost:8080/api/v1/packs/core | jq

# List actions
curl http://localhost:8080/api/v1/packs/core/actions | jq

# List triggers
curl http://localhost:8080/api/v1/packs/core/triggers | jq
```

### Using Database
```sql
-- Check pack exists
SELECT * FROM attune.pack WHERE ref = 'core';

-- Count components
SELECT 
    (SELECT COUNT(*) FROM attune.trigger WHERE pack_ref = 'core') as triggers,
    (SELECT COUNT(*) FROM attune.action WHERE pack_ref = 'core') as actions,
    (SELECT COUNT(*) FROM attune.sensor WHERE pack_ref = 'core') as sensors;
```

## Testing the Core Pack

### 1. Test Actions Directly

Test actions using environment variables:

```bash
# Test echo action
export ATTUNE_ACTION_MESSAGE="Hello, Attune!"
export ATTUNE_ACTION_UPPERCASE=false
./packs/core/actions/echo.sh

# Test sleep action
export ATTUNE_ACTION_SECONDS=2
export ATTUNE_ACTION_MESSAGE="Sleeping..."
./packs/core/actions/sleep.sh

# Test HTTP request action
export ATTUNE_ACTION_URL="https://httpbin.org/get"
export ATTUNE_ACTION_METHOD="GET"
python3 packs/core/actions/http_request.py
```

### 2. Run Pack Test Suite

```bash
# Run comprehensive test suite
./packs/core/test_core_pack.sh
```

### 3. Create a Test Rule

Create a simple rule to test the core pack integration:

```bash
# Create a rule that echoes every 10 seconds
attune rule create \
  --name "test_timer_echo" \
  --trigger "core.intervaltimer" \
  --trigger-config '{"unit":"seconds","interval":10}' \
  --action "core.echo" \
  --action-params '{"message":"Timer triggered!"}' \
  --enabled
```

## Updating the Core Pack

To update the core pack after making changes:

1. Edit the relevant YAML files in `packs/core/`
2. Re-run the loader script:
   ```bash
   python3 scripts/load_core_pack.py
   ```
3. The loader will update existing entries (upsert)

## Troubleshooting

### "Failed to connect to database"
- Verify PostgreSQL is running: `pg_isready`
- Check `DATABASE_URL` environment variable
- Test connection: `psql $DATABASE_URL -c "SELECT 1"`

### "pack.yaml not found"
- Ensure you're running from the project root
- Check the `--pack-dir` argument points to the correct directory
- Verify `packs/core/pack.yaml` exists

### "ModuleNotFoundError: No module named 'psycopg2'"
```bash
pip install psycopg2-binary pyyaml
```

### "Pack loaded but not visible in API"
- Restart the API service to reload pack data
- Check pack is enabled: `SELECT enabled FROM attune.pack WHERE ref = 'core'`

### Actions not executing
- Verify action scripts are executable: `chmod +x packs/core/actions/*.sh`
- Check worker service is running and can access the packs directory
- Verify runtime configuration is correct

## Development Workflow

When developing new core pack components:

1. **Add new action:**
   - Create `actions/new_action.yaml` with metadata
   - Create `actions/new_action.sh` (or `.py`) with implementation
   - Make script executable: `chmod +x actions/new_action.sh`
   - Test locally: `export ATTUNE_ACTION_*=... && ./actions/new_action.sh`
   - Load into database: `python3 scripts/load_core_pack.py`

2. **Add new trigger:**
   - Create `triggers/new_trigger.yaml` with metadata
   - Load into database: `python3 scripts/load_core_pack.py`
   - Create sensor if needed

3. **Add new sensor:**
   - Create `sensors/new_sensor.yaml` with metadata
   - Create `sensors/new_sensor.py` with implementation
   - Load into database: `python3 scripts/load_core_pack.py`
   - Restart sensor service

## Environment Variables

The loader script supports the following environment variables:

- `DATABASE_URL` - PostgreSQL connection string
  - Default: `postgresql://postgres:postgres@localhost:5432/attune`
  - Example: `postgresql://user:pass@db.example.com:5432/attune`

- `ATTUNE_PACKS_DIR` - Base directory for packs
  - Default: `./packs`
  - Example: `/opt/attune/packs`

## CI/CD Integration

For automated deployments:

```yaml
# Example GitHub Actions workflow
- name: Load Core Pack
  run: |
    python3 scripts/load_core_pack.py \
      --database-url "${{ secrets.DATABASE_URL }}"
  env:
    DATABASE_URL: ${{ secrets.DATABASE_URL }}
```

## Next Steps

After loading the core pack:

1. **Create your first rule** using core triggers and actions
2. **Enable sensors** to start generating events
3. **Monitor executions** via the API or Web UI
4. **Explore pack documentation** in `README.md`

## Additional Resources

- **Pack README**: `packs/core/README.md` - Comprehensive component documentation
- **Testing Guide**: `packs/core/TESTING.md` - Testing procedures
- **API Documentation**: `docs/api-packs.md` - Pack management API
- **Action Development**: `docs/action-development.md` - Creating custom actions

## Support

If you encounter issues:

1. Check this troubleshooting section
2. Review logs from services (api, executor, worker, sensor)
3. Verify database state with SQL queries
4. File an issue with detailed error messages and logs

---

**Last Updated:** 2025-01-20  
**Core Pack Version:** 1.0.0