# Work Summary: CLI Output Enhancements
**Date**: 2026-01-18  
**Session**: 7 (continued)  
**Status**: ✅ Complete

## Overview

Enhanced the CLI with shorthand output flags for better interoperability with Unix tools, and added a new command to extract raw execution results for piping to other command-line utilities.

## Objectives

- Add shorthand flags for JSON (`-j`) and YAML (`-y`) output
- Enable easy piping to tools like `jq`, `yq`, etc.
- Add command to get raw execution result data
- Maintain backward compatibility
- Follow Unix command-line conventions

## Implementation Details

### 1. Shorthand Output Flags

#### Added Global Flags (`main.rs`)

```rust
/// Output as JSON (shorthand for --output json)
#[arg(short = 'j', long, global = true, conflicts_with_all = ["output", "yaml"])]
json: bool,

/// Output as YAML (shorthand for --output yaml)
#[arg(short = 'y', long, global = true, conflicts_with_all = ["output", "json"])]
yaml: bool,
```

#### Flag Resolution Logic

Implemented priority-based resolution:
1. `-j/--json` → JSON output
2. `-y/--yaml` → YAML output
3. `--output <format>` → Specified format
4. Default → Table format

```rust
let output_format = if cli.json {
    output::OutputFormat::Json
} else if cli.yaml {
    output::OutputFormat::Yaml
} else {
    cli.output
};
```

#### Conflict Handling

Flags are mutually exclusive:
- `-j` conflicts with `--output` and `-y`
- `-y` conflicts with `--output` and `-j`
- `--output` conflicts with `-j` and `-y`

### 2. Raw Execution Result Command

#### New Subcommand (`commands/execution.rs`)

Added `attune execution result <id>` command:

```rust
Result {
    /// Execution ID
    execution_id: i64,

    /// Output format (json or yaml, default: json)
    #[arg(short = 'f', long, value_enum, default_value = "json")]
    format: ResultFormat,
}
```

#### Result Format Enum

```rust
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum ResultFormat {
    Json,
    Yaml,
}
```

#### Implementation

```rust
async fn handle_result(
    execution_id: i64,
    format: ResultFormat,
    api_url: &Option<String>,
) -> Result<()> {
    let config = CliConfig::load()?;
    let client = ApiClient::from_config(&config, api_url);

    let path = format!("/executions/{}", execution_id);
    let execution: ExecutionDetail = client.get(&path).await?;

    if let Some(result) = execution.result {
        match format {
            ResultFormat::Json => {
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
            ResultFormat::Yaml => {
                println!("{}", serde_yaml::to_string(&result)?);
            }
        }
    } else {
        anyhow::bail!("Execution {} has no result yet", execution_id);
    }

    Ok(())
}
```

## Usage Examples

### Shorthand Output Flags

```bash
# JSON output (long form)
attune pack list --output json

# JSON output (shorthand)
attune pack list -j

# YAML output (shorthand)
attune pack list -y

# Works with all commands
attune execution list -j
attune action list -y
attune rule show core.my_rule -j
```

### Raw Execution Result

```bash
# Get result as JSON (default)
attune execution result 123

# Get result as YAML
attune execution result 123 --format yaml

# Pipe to jq for field extraction
attune execution result 123 | jq '.data.status'

# Extract specific array elements
attune execution result 123 | jq -r '.errors[]'

# Complex jq filtering
attune execution result 123 | jq '.results[] | select(.status == "success")'

# Pipe to yq
attune execution result 123 --format yaml | yq '.data.field'

# Use in scripts
STATUS=$(attune execution result 123 | jq -r '.status')
if [ "$STATUS" = "success" ]; then
  echo "Execution succeeded"
fi
```

### Piping to Other Tools

```bash
# Count items in result
attune execution result 123 | jq '.items | length'

# Format and colorize JSON
attune execution result 123 | jq -C '.'

# Convert between formats
attune execution result 123 | yq -P

# Extract to file
attune execution result 123 | jq '.data' > result.json

# Grep in JSON
attune execution result 123 | jq -r '.logs[]' | grep ERROR

# Chain multiple commands
attune execution list -j | \
  jq -r '.[] | select(.status == "failed") | .id' | \
  xargs -I {} attune execution result {} | \
  jq -r '.error'
```

## Integration with Unix Tools

### With jq (JSON processor)

```bash
# Filter executions and extract results
attune execution list -j | \
  jq '.[] | select(.pack_name == "monitoring")' | \
  jq -r '.id' | \
  while read id; do
    attune execution result $id | jq '.metrics'
  done
```

### With yq (YAML processor)

```bash
# Get pack config as YAML
attune pack show core -y | yq '.metadata'
```

### With grep/awk/sed

```bash
# Search in execution results
attune execution list --pack core -j | \
  jq -r '.[] | "\(.id) \(.status)"' | \
  grep failed | \
  awk '{print $1}' | \
  xargs -I {} attune execution result {}
```

### With xargs for batch operations

```bash
# Process all failed executions
attune execution list --status failed -j | \
  jq -r '.[].id' | \
  xargs -I {} attune execution result {} | \
  jq -s '.'  # Combine all results into single array
```

## Benefits

### 1. Interoperability
- Follows Unix convention for shorthand flags
- Easy piping to standard tools (`jq`, `yq`, `grep`, etc.)
- Compatible with shell scripting best practices

### 2. Convenience
- Shorter commands: `-j` vs `--output json`
- Faster typing for interactive use
- Less verbose scripts

### 3. Data Extraction
- Get just the result data without wrapper
- No need to parse full execution object
- Direct access to execution output

### 4. Scripting
- Easy automation with standard tools
- Clean data flow in pipelines
- Reduced need for custom parsing

## Use Cases

### 1. Monitoring Scripts

```bash
#!/bin/bash
# Check execution results for errors

for id in $(attune execution list --status succeeded -j | jq -r '.[].id'); do
  ERROR_COUNT=$(attune execution result $id | jq '.errors | length')
  if [ "$ERROR_COUNT" -gt 0 ]; then
    echo "Execution $id has $ERROR_COUNT errors"
  fi
done
```

### 2. Data Aggregation

```bash
# Aggregate metrics from multiple executions
attune execution list --pack monitoring -j | \
  jq -r '.[].id' | \
  xargs -I {} attune execution result {} | \
  jq -s 'map(.metrics) | add'
```

### 3. CI/CD Integration

```bash
# Execute action and check result
EXEC_ID=$(attune action execute ci.deploy -j | jq -r '.id')
attune execution result $EXEC_ID | jq -e '.success' || exit 1
```

### 4. Log Analysis

```bash
# Extract and analyze logs from results
attune execution result 123 | \
  jq -r '.logs[]' | \
  grep ERROR | \
  sort | uniq -c
```

## Technical Details

### Flag Conflicts

Clap's `conflicts_with_all` ensures mutual exclusivity:
- Prevents: `attune pack list -j -y` (error)
- Prevents: `attune pack list -j --output table` (error)
- Allows: `attune pack list -j` (valid)

### Error Handling

Result command provides clear error when execution has no result:
```
Error: Execution 123 has no result yet
```

This helps users understand when to use the command.

### Format Defaults

- Shorthand flags: Infer format from flag
- Result command: Defaults to JSON (most common for piping)
- Can override with `--format` for result command

## Documentation Updates

### Updated Files

1. **CLI README** (`crates/cli/README.md`)
   - Added shorthand flag examples
   - Added result command documentation
   - Added scripting examples with jq
   - Updated all examples to use shorthand flags

2. **CLI Docs** (`docs/cli.md`)
   - Added global flags section with shorthands
   - Added result extraction examples
   - Updated scripting examples
   - Added integration examples

3. **Main README** (`README.md`)
   - Added shorthand flag quick examples
   - Added result extraction to CLI features
   - Updated feature highlights

## Backward Compatibility

✅ **Fully Backward Compatible**
- `--output` flag still works
- No changes to existing behavior
- New flags are optional additions
- All existing scripts continue to work

## Testing

### Manual Tests

```bash
# Build
cargo build -p attune-cli

# Test shorthand flags
./target/debug/attune config list -j    # Should output JSON
./target/debug/attune config list -y    # Should output YAML

# Test conflicts
./target/debug/attune config list -j -y  # Should error
./target/debug/attune config list -j --output table  # Should error

# Test result command
./target/debug/attune execution result --help  # Should show help
```

### Build Status
- ✅ Compiles successfully
- ✅ No new warnings
- ✅ Help text displays correctly
- ✅ Conflict handling works

## Metrics

- **Files Modified**: 5
- **Lines Added**: ~150
- **New Command**: 1 (`execution result`)
- **New Flags**: 2 (`-j`, `-y`)
- **Documentation Updates**: 3 files
- **Development Time**: ~45 minutes

## Files Changed

### Modified Files
- `crates/cli/src/main.rs`: Added shorthand flags and resolution logic
- `crates/cli/src/commands/execution.rs`: Added result command
- `crates/cli/README.md`: Updated documentation
- `docs/cli.md`: Updated documentation
- `README.md`: Updated examples

## Future Enhancements

Potential improvements:
1. **Result Templating**: `--template '{{.status}}: {{.message}}'`
2. **JSONPath Support**: `--path '$.data.metrics'`
3. **Output Streaming**: For large results
4. **Format Conversion**: `--from json --to yaml`
5. **Pretty Printing Options**: `--compact`, `--indent 2`
6. **Result Diffing**: `attune execution diff 123 124`

## Comparison with Other Tools

Following conventions from popular CLI tools:

| Tool     | JSON Flag | YAML Flag | Notes                    |
|----------|-----------|-----------|--------------------------|
| kubectl  | `-o json` | `-o yaml` | Uses `-o` for output     |
| docker   | `--format`| N/A       | Uses Go templates        |
| jq       | (default) | N/A       | JSON only                |
| yq       | N/A       | (default) | YAML only                |
| **attune** | `-j`    | `-y`      | Convenient shorthands    |

Our approach is more concise and follows the Unix tradition of single-letter flags for common options.

## Conclusion

Successfully enhanced the CLI with:
- Unix-friendly shorthand output flags
- Raw result extraction for piping
- Better interoperability with standard tools
- Improved scripting capabilities

The changes make the CLI more powerful for automation and integration with existing Unix toolchains, while maintaining full backward compatibility.

## Related Documents
- [CLI README](../crates/cli/README.md)
- [CLI Documentation](../docs/cli.md)
- [CLI Implementation Work Summary](2026-01-18-cli-implementation.md)
- [Execution Search Enhancement](2026-01-18-execution-search-enhancement.md)