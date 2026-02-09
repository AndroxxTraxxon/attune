# Examples Pack

**Demonstration actions and workflows for learning Attune**

## Overview

The Examples pack provides reference implementations that demonstrate various Attune features and best practices. These examples are designed for learning and can be used as templates for building your own actions.

## Contents

### Actions

#### `list_example` - JSON Lines Output Demo

Demonstrates the JSON Lines (JSONL) output format for streaming results.

**Features:**
- Streams multiple JSON objects as output
- Each line is a separate JSON object
- Results are collected into an array
- Useful for processing lists or progress updates

**Usage:**
```bash
attune action execute examples.list_example --param count=10
```

**Parameters:**
- `count` (integer): Number of items to generate (default: 5, range: 1-100)

**Output Format:** JSONL - Each line is parsed as JSON and collected into an array

**Example Output:**
```json
[
  {"id": 0, "value": "item-0", "timestamp": "2024-01-20T10:30:00Z"},
  {"id": 1, "value": "item-1", "timestamp": "2024-01-20T10:30:01Z"},
  {"id": 2, "value": "item-2", "timestamp": "2024-01-20T10:30:02Z"}
]
```

## Use Cases

### Learning Attune
- Study action structure and metadata
- Understand parameter schemas
- Learn about different output formats
- See working implementations

### Templates
- Copy and modify examples for your own actions
- Reference implementations for common patterns
- Starting point for new packs

## Installation

The examples pack is not installed by default but can be easily added:

```bash
# Via pack registry (if published)
attune pack install examples

# Via local directory
attune pack install --local ./packs/examples
```

## Development

### Adding New Examples

When adding new example actions:

1. Create action metadata in `actions/<name>.yaml`
2. Implement the action script in `actions/<name>.sh` (or .py, .js)
3. Use ref format: `examples.<action_name>`
4. Add documentation to this README
5. Include clear comments in the code
6. Demonstrate a specific feature or pattern

### Guidelines

- **Keep it simple** - Examples should be easy to understand
- **One concept per example** - Focus on demonstrating one feature clearly
- **Well-commented** - Explain what the code does and why
- **Self-contained** - Minimize external dependencies
- **Documented** - Update this README with usage examples

## Related Documentation

- [Action Development Guide](../../docs/action-development-guide.md)
- [Pack Structure](../../docs/packs/pack-structure.md)
- [Parameter Configuration](../../docs/action-development-guide.md#parameter-configuration)
- [Output Formats](../../docs/action-development-guide.md#output-configuration)

## Contributing

Have an idea for a useful example? Contributions are welcome! Please ensure:

- Examples are educational and demonstrate best practices
- Code is well-commented and easy to follow
- Documentation is updated
- Examples are tested and working

## License

This pack is part of the Attune project and follows the same license terms.