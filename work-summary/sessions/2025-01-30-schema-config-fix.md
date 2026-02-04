# Schema Configuration Fix for E2E Services

**Date**: 2025-01-30  
**Status**: ✅ Complete  
**Branch**: main

## Summary

Fixed an issue where the `start-e2e-services.sh` script was ignoring the database schema configuration from custom config files and always defaulting to the `attune` schema.

## Problem

When launching services with a custom config file:

```bash
CONFIG_FILE=config.development.yaml ./scripts/start-e2e-services.sh
```

The services would ignore the `schema: "public"` setting in `config.development.yaml` and always use the `attune` schema instead.

### Root Cause

The configuration loading system has the following priority order:

1. Base config file (e.g., `config.yaml` from `ATTUNE_CONFIG`)
2. Environment-specific config file (e.g., `config.{environment}.yaml`)
3. Environment variables (highest priority)

The `start-e2e-services.sh` script was setting `ATTUNE__ENVIRONMENT=e2e`, which caused the config loader to look for and load `config.e2e.yaml` **after** loading the custom config file. Since environment-specific files have higher priority, any settings in `config.e2e.yaml` (including the default `schema: "attune"`) would override the custom config.

### Code Flow

```rust
// In crates/common/src/config.rs::Config::load()

// 1. Load base config (from ATTUNE_CONFIG)
builder = builder.add_source(config_crate::File::with_name(&config_path));

// 2. Load environment-specific config (PROBLEM: this overrides step 1)
let environment = std::env::var("ATTUNE__ENVIRONMENT").unwrap_or_else(...);
let env_config_path = format!("config.{}.yaml", environment);
builder = builder.add_source(config_crate::File::with_name(&env_config_path));

// 3. Load environment variables (highest priority)
builder = builder.add_source(config_crate::Environment::with_prefix("ATTUNE")...);
```

When `ATTUNE__ENVIRONMENT=e2e` was set, it loaded `config.e2e.yaml` which contained `schema: "attune"`, overriding the `schema: "public"` from `config.development.yaml`.

## Solution

Modified `scripts/start-e2e-services.sh` to only set `ATTUNE__ENVIRONMENT=e2e` when using the default E2E config file, allowing custom config files to determine their own environment:

```bash
# Only set ATTUNE__ENVIRONMENT if using default e2e config
# Otherwise, let the config file determine the environment
if [ "$CONFIG_FILE" = "config.e2e.yaml" ]; then
    ATTUNE__ENVIRONMENT=e2e ATTUNE_CONFIG="$CONFIG_FILE" ./target/debug/$binary_name > "$log_file" 2>&1 &
else
    ATTUNE_CONFIG="$CONFIG_FILE" ./target/debug/$binary_name > "$log_file" 2>&1 &
fi
```

## Changes Made

### Modified Files

**`scripts/start-e2e-services.sh`**:
1. Added schema detection from config file to display in startup banner
2. Modified service startup to conditionally set `ATTUNE__ENVIRONMENT` only for default E2E config
3. Added configuration info display showing config file and detected schema

### Enhanced User Experience

Added informative banner showing configuration being used:

```
Configuration:
  • Config file: config.development.yaml
  • Database schema: public
```

This helps users verify the correct configuration is loaded before services start.

## Verification

### Before Fix

```bash
$ CONFIG_FILE=config.development.yaml ./scripts/start-e2e-services.sh
# Services would show:
INFO Using production schema: attune
```

### After Fix

```bash
$ CONFIG_FILE=config.development.yaml ./scripts/start-e2e-services.sh

Configuration:
  • Config file: config.development.yaml
  • Database schema: public

# Services now show:
WARN Using non-standard schema: 'public'. Production should use 'attune'
INFO Connecting to database with max_connections=50, schema=public
```

All services (api, executor, worker, sensor) correctly use the `public` schema.

## Usage

### Default E2E Environment
```bash
# Uses config.e2e.yaml with attune schema (default)
./scripts/start-e2e-services.sh
```

### Development Environment
```bash
# Uses config.development.yaml with public schema
CONFIG_FILE=config.development.yaml ./scripts/start-e2e-services.sh
```

### Custom Configuration
```bash
# Uses any custom config file with its specified schema
CONFIG_FILE=my-custom-config.yaml ./scripts/start-e2e-services.sh
```

## Related Files

- `scripts/start-e2e-services.sh` - Service startup script (modified)
- `crates/common/src/config.rs` - Configuration loading logic (reference)
- `crates/common/src/db.rs` - Database connection with schema support (reference)
- `config.development.yaml` - Development config with `schema: "public"`
- `config.e2e.yaml` - E2E test config with default `schema: "attune"`

## Configuration Priority Reference

For future reference, Attune's configuration loading priority is:

1. **Base config file** (lowest priority)
   - `config.yaml` or file specified by `ATTUNE_CONFIG` env var

2. **Environment-specific config file**
   - `config.{ATTUNE__ENVIRONMENT}.yaml`
   - Only loaded if `ATTUNE__ENVIRONMENT` is set or determined from base config

3. **Environment variables** (highest priority)
   - Prefix: `ATTUNE__`
   - Separator: `__` (double underscore)
   - Example: `ATTUNE__DATABASE__SCHEMA=public`

### Schema Configuration

The database schema can be configured via:

1. Config file: `database.schema: "public"`
2. Environment variable: `ATTUNE__DATABASE__SCHEMA=public`
3. Default if not specified: `"attune"`

## Lessons Learned

1. **Environment variable overrides can be subtle**: Setting `ATTUNE__ENVIRONMENT` has cascading effects by triggering additional config file loads
2. **Configuration priority matters**: Higher priority sources always override lower priority ones
3. **Scripts should respect user intent**: When users provide custom config, don't override their choices
4. **Visibility helps debugging**: Showing the detected configuration at startup makes issues immediately visible

## Testing

Tested with:
- ✅ Default E2E config (uses `attune` schema)
- ✅ Development config (uses `public` schema)
- ✅ All services (api, executor, worker, sensor) respect the config
- ✅ Schema detection and display in startup banner

## Impact

**Positive**:
- Users can now easily switch between development and E2E environments
- Schema isolation works correctly for parallel testing
- Configuration behavior is more intuitive and predictable

**No Breaking Changes**:
- Default behavior unchanged (E2E config still uses `attune` schema)
- Existing scripts and workflows continue to work
- Only affects behavior when custom `CONFIG_FILE` is provided

## Future Improvements

Consider adding:
1. Validation warning if config file doesn't exist
2. Display full resolved configuration on startup (with sensitive values masked)
3. Config validation command: `attune config validate --file config.yaml`
4. Environment variable to completely disable environment-specific config loading