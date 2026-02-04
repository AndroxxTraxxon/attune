# Events Page: Fix Mislabeled "Pack" Column

**Date**: 2026-02-04
**Type**: Bug Fix
**Component**: Web UI - Events Page

## Problem

The Events page (`/events`) had a column labeled "Pack" that was displaying the `source_ref` field. This was misleading because:

1. The `Event` model/table does not have a `pack` field
2. The `source_ref` field refers to the sensor that generated the event, not a pack
3. The column header did not match the data being displayed

## Investigation

### Database Schema
The `event` table has the following relevant fields:
- `trigger` (BIGINT) - Foreign key to trigger table
- `trigger_ref` (TEXT) - Trigger reference (e.g., "core.webhook")
- `source` (BIGINT) - Foreign key to sensor table
- `source_ref` (TEXT) - Sensor reference (e.g., "monitoring.webhook_sensor")
- `rule` (BIGINT) - Foreign key to rule table (optional)
- `rule_ref` (TEXT) - Rule reference (optional)

**No `pack` field exists.**

### API Response
The `EventSummary` DTO includes:
```rust
pub struct EventSummary {
    pub id: Id,
    pub trigger: Option<Id>,
    pub trigger_ref: String,
    pub source: Option<Id>,
    pub source_ref: Option<String>,  // ← This is what was displayed
    pub rule: Option<Id>,
    pub rule_ref: Option<String>,
    pub has_payload: bool,
    pub created: DateTime<Utc>,
}
```

The `source_ref` is documented as "Source reference" with example "monitoring.webhook_sensor".

## Solution

**File Modified**: `attune/web/src/pages/events/EventsPage.tsx`

### Changes Made

1. **Column Header**: Changed from "Pack" to "Source"
   - Line 231: `<th>Pack</th>` → `<th>Source</th>`

2. **Cell Display**: Improved formatting to match other columns (Trigger, Rule)
   - Before: Simple text display of `source_ref` or "—"
   - After: Structured display showing both source reference and ID
   - Shows "No source" in gray italic when `source_ref` is null

### New Display Format

```tsx
{event.source_ref ? (
  <div className="text-sm">
    <div className="font-medium text-gray-900">
      {event.source_ref}
    </div>
    <div className="text-gray-500 text-xs">
      ID: {event.source || "N/A"}
    </div>
  </div>
) : (
  <span className="text-sm text-gray-400 italic">
    No source
  </span>
)}
```

## Example Data

For timer-based events (common case):
```json
{
  "id": 6123,
  "trigger_ref": "core.intervaltimer",
  "source_ref": null,  // No sensor, triggered by timer
  "rule_ref": "default.echo_every_second"
}
```

Display:
- **Trigger**: `core.intervaltimer` (ID: 123)
- **Rule**: `default.echo_every_second` (ID: 456)
- **Source**: *No source* (gray italic)

For sensor-based events:
```json
{
  "id": 789,
  "trigger_ref": "core.webhook",
  "source_ref": "monitoring.webhook_sensor",
  "rule_ref": "alert.on_webhook"
}
```

Display:
- **Trigger**: `core.webhook` (ID: 111)
- **Rule**: `alert.on_webhook` (ID: 222)
- **Source**: `monitoring.webhook_sensor` (ID: 333)

## Impact

- **User Experience**: Column now accurately describes the data being displayed
- **Consistency**: Source column now follows the same format as Trigger and Rule columns
- **Clarity**: Users can now properly understand that the field represents the sensor source, not a pack
- **No Breaking Changes**: Only UI label and formatting changed, no API or data model changes

## Testing

Verified with live data:
- Events with `source_ref = null` display "No source"
- Column header correctly shows "Source"
- Display format matches Trigger and Rule columns
- No compilation errors or runtime issues

## Future Considerations

The Source column could potentially be enhanced to:
- Make sensor references clickable (link to sensor detail page)
- Add filtering by source (similar to existing trigger filter)
- Show source type/status indicators

However, these would be new features rather than bug fixes.