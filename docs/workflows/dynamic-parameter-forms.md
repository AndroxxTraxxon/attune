# Dynamic Parameter Forms

## Overview

The web UI now supports dynamic form generation for rule creation based on parameter schemas defined in triggers and actions. This replaces the previous raw JSON textarea inputs with type-aware form fields.

## Features

- **Type-aware inputs**: Automatically renders appropriate form controls based on parameter type
- **Validation**: Real-time validation with error messages for required fields and type constraints
- **Default values**: Auto-populates form fields with default values from schema
- **Enum support**: Renders dropdown selects for enum-type parameters
- **Complex types**: JSON editing support for arrays and objects with live parsing

## Component Architecture

### ParamSchemaForm Component

Location: `web/src/components/common/ParamSchemaForm.tsx`

A reusable React component that dynamically generates form inputs based on a parameter schema.

**Props:**
- `schema`: Parameter schema object (flat key-value structure)
- `values`: Current parameter values
- `onChange`: Callback when values change
- `errors`: Validation errors to display
- `disabled`: Whether form is disabled
- `className`: Additional CSS classes

**Supported Types:**
- `string` - Text input (or select dropdown if `enum` is provided)
- `number` - Numeric input with decimal support
- `integer` - Numeric input (whole numbers only)
- `boolean` - Checkbox with label
- `array` - JSON textarea with syntax validation
- `object` - JSON textarea with syntax validation

### Parameter Schema Format

The component expects a flat schema structure:

```typescript
{
  [parameterName]: {
    type?: "string" | "number" | "integer" | "boolean" | "array" | "object";
    description?: string;
    required?: boolean;
    default?: any;
    enum?: string[];
  }
}
```

**Example:**
```json
{
  "expression": {
    "type": "string",
    "description": "Cron expression in standard format",
    "required": true
  },
  "timezone": {
    "type": "string",
    "description": "Timezone for cron schedule",
    "default": "UTC"
  },
  "interval": {
    "type": "integer",
    "description": "Number of time units between each trigger",
    "default": 60,
    "required": true
  },
  "unit": {
    "type": "string",
    "enum": ["seconds", "minutes", "hours"],
    "description": "Time unit for the interval",
    "default": "seconds",
    "required": true
  }
}
```

## Usage in Rule Creation

When creating a rule through the web UI:

1. **Select Pack**: Choose which pack to use
2. **Select Trigger**: Dropdown shows available triggers from the pack
3. **Configure Trigger Params**: Dynamic form appears based on trigger's `param_schema`
4. **Select Action**: Dropdown shows available actions from the pack
5. **Configure Action Params**: Dynamic form appears based on action's `param_schema`
6. **Set Conditions**: JSON-based conditional logic (optional)
7. **Submit**: Form validates all required fields before submission

### Data Flow

```
1. User selects trigger (from summary list)
   ↓
2. System fetches full trigger details (GET /api/v1/triggers/{ref})
   ↓
3. Extract param_schema from TriggerResponse
   ↓
4. ParamSchemaForm renders inputs based on schema
   ↓
5. User fills in parameters
   ↓
6. Validation runs on submission
   ↓
7. Parameters sent as trigger_params in CreateRuleRequest
```

Same flow applies for action parameters.

## API Design Pattern

The implementation follows the **"summary for lists, details on demand"** pattern:

- **List endpoints** (`/api/v1/packs/{pack_ref}/triggers`): Return `TriggerSummary` without `param_schema`
- **Detail endpoints** (`/api/v1/triggers/{ref}`): Return `TriggerResponse` with full `param_schema`

This keeps list responses lightweight while providing full schema information when needed.

## Pack Definition Format

### Trigger YAML Structure

```yaml
name: intervaltimer
ref: core.intervaltimer
description: "Fires at regular intervals"
enabled: true
type: interval

# Parameter schema - flat structure
parameters:
  unit:
    type: string
    enum:
      - seconds
      - minutes
      - hours
    description: "Time unit for the interval"
    default: "seconds"
    required: true
  interval:
    type: integer
    description: "Number of time units between each trigger"
    default: 60
    required: true

# Output schema (payload emitted when trigger fires)
output:
  type: object
  properties:
    type:
      type: string
    interval_seconds:
      type: integer
    fired_at:
      type: string
      format: date-time
```

### Action YAML Structure

Actions use the same flat parameter schema format:

```yaml
name: echo
ref: core.echo
description: "Echoes a message"
runtime: shell

# Parameter schema - flat structure
parameters:
  message:
    type: string
    description: "Message to echo"
    required: true
  uppercase:
    type: boolean
    description: "Convert message to uppercase"
    default: false
```

## Pack Loader Mapping

The Python pack loader (`scripts/load_core_pack.py`) maps YAML keys to database columns:

| YAML Key     | Database Column |
|--------------|-----------------|
| `parameters` | `param_schema`  |
| `output`     | `out_schema`    |

The loader serializes the YAML structure as JSON and stores it in the `param_schema` JSONB column.

## Validation

The `validateParamSchema` utility function validates parameter values against the schema:

```typescript
import { validateParamSchema } from '@/components/common/ParamSchemaForm';

const errors = validateParamSchema(schema, values);
// Returns: { [fieldName]: "error message" }
```

**Validation Rules:**
- Required fields must have non-empty values
- Numbers must be valid numeric values
- Arrays must be valid JSON arrays
- Objects must be valid JSON objects

## Future Enhancements

Potential improvements for the parameter form system:

1. **Advanced validation**: Support for min/max, pattern matching, custom validators
2. **Conditional fields**: Show/hide fields based on other field values
3. **Field hints**: Helper text, examples, tooltips
4. **Template variables**: Autocomplete for Jinja2 template syntax (e.g., `{{ trigger.payload.* }}`)
5. **Schema versioning**: Handle schema changes across pack versions
6. **Array item editing**: Better UX for editing array items individually
7. **Nested objects**: Support for deeply nested object schemas
8. **File uploads**: Support for file-type parameters
9. **Date pickers**: Native date/time inputs for datetime parameters

## Troubleshooting

### Parameters not showing in UI

**Check:**
1. Is the trigger/action selected?
2. Does the database record have `param_schema` populated?
   ```sql
   SELECT ref, param_schema FROM trigger WHERE ref = 'core.intervaltimer';
   ```
3. Is the API returning the full record (not summary)?
4. Check browser console for JavaScript errors

### Schema not loading from pack YAML

**Check:**
1. YAML uses `parameters` key (not `parameters_schema`)
2. Schema is in flat format (not nested JSON Schema with `properties`)
3. Pack was reloaded after YAML changes: `./scripts/load-core-pack.sh`
4. Database has correct schema: `SELECT param_schema FROM trigger WHERE ref = 'pack.trigger';`

### Validation errors

**Check:**
1. Required fields are marked with `required: true` in schema
2. Type matches expected format (e.g., integer vs string)
3. Enum values match exactly (case-sensitive)

## Related Files

- `web/src/components/common/ParamSchemaForm.tsx` - Core form component
- `web/src/components/forms/RuleForm.tsx` - Rule creation form using ParamSchemaForm
- `web/src/pages/actions/ActionsPage.tsx` - Execute action modal using ParamSchemaForm
- `scripts/load_core_pack.py` - Pack loader that converts YAML to database schema
- `packs/core/triggers/*.yaml` - Example trigger definitions
- `packs/core/actions/*.yaml` - Example action definitions