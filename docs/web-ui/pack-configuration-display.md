# Pack Configuration Display

## Overview

The Pack detail page now displays pack configuration in a unified view that combines the configuration schema (`conf_schema`) with actual configuration values (`config`).

## Features

### Configuration Section

When viewing a pack that has configuration properties defined in its schema, a "Configuration" section is automatically displayed on the pack detail page.

### Display Components

The configuration display shows:

1. **Property Name**: The configuration key (displayed in monospace font)
2. **Type Badge**: The data type (string, boolean, integer, number, array, object)
3. **Default Badge**: A yellow badge indicating when the default value is being used
4. **Description**: Human-readable description from the schema
5. **Current Value**: The actual configuration value, formatted based on type
6. **Range Information**: For numeric types with min/max constraints

### Value Formatting

Values are formatted intelligently based on their type:

- **Boolean**: Green checkmark badge for `true`, gray badge for `false`
- **Numbers**: Displayed in monospace font
- **Strings**: Plain text (truncated if over 50 characters)
- **Arrays**: Shows item count (e.g., "[3 items]")
- **Objects**: Shows key count (e.g., "{5 keys}")
- **Not Set**: Displays as italic gray "not set" text

### Default Value Handling

When a configuration property has a default value defined in the schema but no actual value is set in `config`:
- The default value is displayed
- A yellow "default" badge indicates it's using the schema default
- No "default" badge appears when an explicit value is set

## Example

For a pack with the following schema and config:

```yaml
conf_schema:
  type: object
  properties:
    max_action_timeout:
      type: integer
      description: "Maximum timeout for action execution in seconds"
      default: 300
      minimum: 1
      maximum: 3600
    enable_debug_logging:
      type: boolean
      description: "Enable debug logging for core pack actions"
      default: false
  required: []

config:
  max_action_timeout: 300
  enable_debug_logging: false
```

The UI will display:

```
Configuration
─────────────────────────────────────────────────

max_action_timeout          [integer]
Maximum timeout for action execution in seconds
                                              300
Range: 1 - 3600

enable_debug_logging        [boolean]
Enable debug logging for core pack actions
                                      ✗ false
```

## No Configuration

If a pack has no `conf_schema` properties defined, the Configuration section is not displayed.

## Implementation

- **Component**: `PackConfiguration` in `web/src/pages/packs/PacksPage.tsx`
- **Value Renderer**: `ConfigValue` helper component for type-specific formatting
- **Location**: Displayed in the pack detail view, after "Pack Information" card

## API Data

The configuration display uses data from the pack detail endpoint:

```
GET /api/v1/packs/{ref}
```

Response includes:
- `conf_schema`: JSON Schema defining configuration structure
- `config`: JSON object with actual configuration values

Both fields are already included in the `PackResponse` DTO.

## Usage

1. Navigate to any pack detail page: `/packs/{ref}`
2. If the pack has configuration properties, scroll to the "Configuration" section
3. View current values, types, and descriptions
4. See which values are using defaults (yellow badge)
5. For numeric values, view valid range constraints

## Future Enhancements

Potential improvements:
- Inline editing of configuration values
- Validation against schema constraints
- Configuration history/audit trail
- Environment-specific configuration overrides
- Secret/sensitive value masking
- Configuration export/import