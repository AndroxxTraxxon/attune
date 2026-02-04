# Timer Sensor Crate Rename

**Date**: 2026-02-04
**Task**: Rename `sensor-timer` crate to `core-timer-sensor`
**Status**: ✅ Complete

## Summary

Renamed the timer sensor crate from `crates/sensor-timer` to `crates/core-timer-sensor` to better reflect its role as a core pack component. The binary name was kept as `attune-core-timer-sensor` for backward compatibility.

## Approach: Minimal Disruption

**Decision**: Keep binary name as `attune-core-timer-sensor` to minimize changes
- ✅ No changes needed to pack YAML files
- ✅ No changes needed to Dockerfiles (except build commands)
- ✅ No changes needed to systemd service files
- ✅ Backward compatible with existing deployments

## Files Modified

### 1. Workspace Configuration
**File**: `Cargo.toml`
- Updated workspace members from `crates/sensor-timer` → `crates/core-timer-sensor`

### 2. Crate Configuration  
**File**: `crates/core-timer-sensor/Cargo.toml`
- Changed package name to `core-timer-sensor`
- Added `[[bin]]` section to keep binary name as `attune-core-timer-sensor`:
```toml
[[bin]]
name = "attune-core-timer-sensor"
path = "src/main.rs"
```

### 3. Documentation Updates
Updated crate path references in all documentation:

- `crates/core-timer-sensor/README.md`
  - Build commands: `cargo build -p core-timer-sensor`
  - Test commands: `cargo test -p core-timer-sensor`
  - Install path: `cargo install --path crates/core-timer-sensor`
  - Code structure path

- `docs/guides/timer-sensor-quickstart.md`
  - Build/test/run commands
  - Code path references

- `docs/sensors/timer-sensor-implementation.md`
  - Test command reference

- `docs/authentication/token-rotation.md`
  - README path reference

- `docs/sensors/sensor-lifecycle-management.md`
  - Implementation path reference

- `docs/sensors/native-runtime.md`
  - Implementation path reference

## Files NOT Modified (No Changes Needed)

- ✅ `packs/core/sensors/interval_timer_sensor.yaml` - entry_point still `attune-core-timer-sensor`
- ✅ `docker/Dockerfile` - binary name unchanged
- ✅ `docker/Dockerfile.pack-builder` - binary name unchanged
- ✅ `docker-compose.yaml` - no direct references
- ✅ Systemd service files (in docs) - binary name unchanged
- ✅ All deployment configurations - binary name unchanged

## Verification

### Build Test
```bash
$ cargo build -p core-timer-sensor
Compiling core-timer-sensor v0.1.0 (.../crates/core-timer-sensor)
Finished `dev` profile [unoptimized + debuginfo]
```

### Binary Verification
```bash
$ ls -lh target/debug/attune-core-timer-sensor
-rwxrwxr-x 2 david david 150M Feb  4 14:10 target/debug/attune-core-timer-sensor
```
Binary name preserved: ✓

### Test Verification
```bash
$ cargo test -p core-timer-sensor
test result: ok. 34 passed; 0 failed; 0 ignored
```
All tests passing: ✓

### Package Name Verification
```bash
$ cargo metadata --format-version 1 | jq '.packages[] | select(.name=="core-timer-sensor") | .name'
"core-timer-sensor"
```
Package renamed: ✓

## Benefits

1. **Clearer Naming**: `core-timer-sensor` better indicates this is part of the core pack
2. **Minimal Disruption**: Binary name unchanged means:
   - No deployment changes needed
   - No Docker changes needed
   - No pack YAML changes needed
3. **Backward Compatible**: Existing references to `attune-core-timer-sensor` binary still work
4. **Consistent Pattern**: Aligns with naming convention for core pack components

## Migration Guide

For developers:

### Old Commands
```bash
cargo build -p attune-core-timer-sensor
cargo test -p attune-core-timer-sensor
cargo run -p attune-core-timer-sensor
```

### New Commands
```bash
cargo build -p core-timer-sensor
cargo test -p core-timer-sensor
cargo run -p core-timer-sensor
```

### Binary Name (Unchanged)
```bash
./target/release/attune-core-timer-sensor  # Still works!
```

## Technical Details

### Cargo Binary Name Override

The key to maintaining backward compatibility is the `[[bin]]` section in Cargo.toml:

```toml
[package]
name = "core-timer-sensor"  # Package name (used in -p flag)

[[bin]]
name = "attune-core-timer-sensor"  # Binary name (unchanged)
path = "src/main.rs"
```

This allows:
- Package name to be `core-timer-sensor` (for organization)
- Binary name to remain `attune-core-timer-sensor` (for compatibility)

### Path References

All path references updated from:
- `crates/sensor-timer/` → `crates/core-timer-sensor/`

Binary name references unchanged:
- `attune-core-timer-sensor` (no changes needed)

## Related Work

This rename is part of organizing the core pack components:
- Timer sensor (this rename)
- Future: Other core pack sensors may follow similar pattern

## Conclusion

The crate rename was completed successfully with minimal disruption. By keeping the binary name unchanged, we avoided breaking changes to deployments, Docker configurations, and pack definitions while improving code organization and clarity.

**Status**: Ready for use with new package name `core-timer-sensor`
