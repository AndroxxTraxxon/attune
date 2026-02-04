# Configuration URL Parsing Fix

**Date:** 2026-01-15
**Status:** ✅ Completed

## Problem

The application was failing to start with the following error:

```
Error: Configuration error: invalid type: sequence, expected a string
for key `database.url` in the environment

Error: Configuration error: invalid type: sequence, expected a string
for key `message_queue.url` in the environment
```

This error occurred when loading configuration from YAML files that contained array values (like `cors_origins`).

## Root Cause

The configuration loader in `crates/common/src/config.rs` was using `.list_separator(",")` when loading environment variables. This caused the `config` crate to incorrectly attempt to parse URL strings (which contain special characters like `://`, `:`, `/`) as comma-separated lists/sequences.

The problematic code:
```rust
builder = builder.add_source(
    config_crate::Environment::with_prefix("ATTUNE")
        .separator("__")
        .try_parsing(true)
        .list_separator(","),  // <-- This was the problem
);
```

## Solution

Implemented a two-part fix:

### 1. Removed `.list_separator(",")`
Removed the problematic `.list_separator(",")` call from the environment variable configuration to prevent incorrect parsing of URL strings.

### 2. Custom Deserializer for Array Fields
Added a custom deserializer module `string_or_vec` that allows fields like `cors_origins` to accept both:
- **Array format** (YAML): `cors_origins: [http://localhost:3000, http://localhost:5173]`
- **Comma-separated string** (env var): `ATTUNE__SERVER__CORS_ORIGINS="http://localhost:3000,http://localhost:5173"`

The custom deserializer:
```rust
mod string_or_vec {
    use serde::{Deserialize, Deserializer};

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum StringOrVec {
            String(String),
            Vec(Vec<String>),
        }

        match StringOrVec::deserialize(deserializer)? {
            StringOrVec::String(s) => {
                Ok(s.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect())
            }
            StringOrVec::Vec(v) => Ok(v),
        }
    }
}
```

Applied to the `cors_origins` field:
```rust
#[serde(default, deserialize_with = "string_or_vec::deserialize")]
pub cors_origins: Vec<String>,
```

## Testing

Verified the fix works correctly in all scenarios:

1. ✅ **YAML array format**: Configuration loads successfully from `config.yaml`
2. ✅ **Environment variable URLs**: Database and message queue URLs parse correctly
   - `ATTUNE__DATABASE__URL="postgresql://user:pass@host:5432/db"`
   - `ATTUNE__MESSAGE_QUEUE__URL="amqp://user:pass@host:5672/%2f"`
3. ✅ **Environment variable arrays**: CORS origins parse correctly as comma-separated string
   - `ATTUNE__SERVER__CORS_ORIGINS="http://test1.com,http://test2.com,http://test3.com"`

## Files Modified

- `crates/common/src/config.rs`
  - Removed `.list_separator(",")` from environment variable loading
  - Added `string_or_vec` deserializer module
  - Updated `ServerConfig.cors_origins` to use custom deserializer
  - Added `Deserializer` import from serde

## Impact

- **Backward Compatible**: Existing YAML configurations with array syntax continue to work
- **Environment Variables**: URL fields (database, message queue, redis) now parse correctly
- **Enhanced Flexibility**: Array fields like `cors_origins` can be specified either way:
  - As YAML arrays (cleaner for config files)
  - As comma-separated strings (easier for environment variables)

## Documentation

Existing documentation already shows comma-separated format for `ATTUNE__SERVER__CORS_ORIGINS`, which now works correctly:
- `CONFIG_README.md`
- `docs/configuration.md`
- `docs/config-troubleshooting.md`
- `docs/env-to-yaml-migration.md`

No documentation updates required - behavior now matches documented expectations.

## Notes

This pattern (custom deserializer for string-or-vec fields) can be reused for any future configuration fields that need to support both array and comma-separated string formats.