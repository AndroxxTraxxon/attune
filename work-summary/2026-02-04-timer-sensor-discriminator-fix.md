# Timer Sensor Discriminator Fix

**Date**: 2026-02-04  
**Status**: ✅ Fixed  
**Type**: Bug Fix

## Problem

The timer sensor was failing to start timers with the error:

```
Failed to start timer for rule 1: Failed to parse trigger_params as TimerConfig
```

## Root Cause

The timer sensor's `TimerConfig` enum is defined as a **tagged union** using serde's `#[serde(tag = "type")]`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TimerConfig {
    Interval { interval: u64, unit: TimeUnit },
    Cron { expression: String },
    DateTime { fire_at: DateTime<Utc> },
}
```

This requires JSON with a `type` discriminator field:

```json
{
  "type": "interval",
  "interval": 1,
  "unit": "seconds"
}
```

However, the `trigger_params` stored in the rule table was missing the `type` field:

```json
{
  "interval": 1,
  "unit": "seconds"
}
```

Without the discriminator, serde couldn't deserialize the enum variant, causing the parse error.

## Investigation Steps

1. Observed error in sensor logs
2. Checked database for `trigger_params` content
3. Found parameters were correct but missing `type` field
4. Examined `TimerConfig` enum definition in `crates/sensor-timer/src/types.rs`
5. Identified the tagged union requirement

## Solution

**Immediate Fix**: Manually added the `type` field to existing rule:

```sql
UPDATE rule 
SET trigger_params = jsonb_set(trigger_params, '{type}', '"interval"') 
WHERE trigger_ref = 'core.intervaltimer';
```

**Result**: Timer sensor now working correctly, firing events every second.

## Not Related to JSON Schema Migration

This issue was **NOT caused by the JSON Schema format migration**. This is a separate data format issue:

- **Schema format**: How parameter definitions are structured (inline vs standard JSON Schema)
- **Data format**: How parameter values are stored (with or without type discriminator)

The JSON Schema migration only affected the **parameter schema definitions** in YAML files and the database. It did not affect the **parameter values** stored in rules.

## Proper Long-Term Fix

The issue should be fixed at the source - when rules are created. The system should automatically add the appropriate `type` discriminator to `trigger_params` based on the `trigger_ref`:

| Trigger Ref | Type Value |
|-------------|------------|
| `core.intervaltimer` | `"interval"` |
| `core.crontimer` | `"cron"` |
| `core.datetimetimer` | `"datetime"` |

### Implementation Options

1. **Database Trigger**: Add a PostgreSQL trigger to automatically inject the `type` field
2. **API Layer**: Add the `type` field in the API when creating/updating rules
3. **Repository Layer**: Add the `type` field in the rule repository's create/update methods
4. **Timer Sensor**: Make the timer sensor more flexible to infer type from trigger_ref

**Recommended**: Option 3 (Repository Layer) - keep the logic in Rust code where it's testable and maintainable.

## Files Involved

- `crates/sensor-timer/src/types.rs` - Defines `TimerConfig` enum with tagged union
- `crates/sensor-timer/src/timer_manager.rs` - Deserializes `TimerConfig` from rule params
- Database: `rule` table, `trigger_params` column

## Verification

After applying the fix:

```bash
# Check rule parameters
docker compose exec -T postgres psql -U attune -d attune \
  -c "SELECT id, ref, jsonb_pretty(trigger_params) FROM rule;"

# Verify timer is working
docker compose logs sensor | grep "Timer fired"
```

Expected output:
- Rule parameters include `"type": "interval"`
- Sensor logs show "Timer fired for rule 1, created event X" every second
- Events are being created in the database

## Action Items

- [ ] Implement automatic `type` field injection in rule repository
- [ ] Add validation to ensure timer trigger params include required `type` field
- [ ] Update rule creation tests to verify `type` field is present
- [ ] Consider making timer sensor more lenient (infer type from trigger_ref)
- [ ] Document the required format for timer trigger parameters

## Conclusion

The timer sensor is now operational. The issue was a missing discriminator field in the stored parameter data, not a problem with the JSON Schema migration. The system needs a proper fix to automatically add the `type` field when creating rules with timer triggers.