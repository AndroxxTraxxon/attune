# Quick Reference: Timer Types

## Overview
The Attune timer sensor supports three timer types for flexible scheduling.

## Interval Timers (core.intervaltimer)
**Purpose**: Fire at regular intervals

```yaml
# Every 30 seconds
trigger_ref: core.intervaltimer
parameters:
  unit: seconds
  interval: 30

# Every 5 minutes
trigger_ref: core.intervaltimer
parameters:
  unit: minutes
  interval: 5

# Every 2 hours
trigger_ref: core.intervaltimer
parameters:
  unit: hours
  interval: 2

# Daily
trigger_ref: core.intervaltimer
parameters:
  unit: days
  interval: 1
```

## Cron Timers (core.crontimer)
**Purpose**: Fire based on cron expressions

```yaml
# Every hour at :00
trigger_ref: core.crontimer
parameters:
  expression: "0 0 * * * *"

# Every 15 minutes
trigger_ref: core.crontimer
parameters:
  expression: "0 */15 * * * *"

# Daily at midnight
trigger_ref: core.crontimer
parameters:
  expression: "0 0 0 * * *"

# Weekdays at 9 AM
trigger_ref: core.crontimer
parameters:
  expression: "0 0 9 * * 1-5"

# Every Monday at 8:30 AM
trigger_ref: core.crontimer
parameters:
  expression: "0 30 8 * * 1"
```

### Cron Format
```
second minute hour day_of_month month day_of_week
  |      |     |        |         |        |
  0-59  0-59  0-23    1-31      1-12    0-6 (0=Sunday)
```

## DateTime Timers (core.datetimetimer)
**Purpose**: Fire once at a specific time (one-shot)

```yaml
# New Year's Eve countdown
trigger_ref: core.datetimetimer
parameters:
  fire_at: "2024-12-31T23:59:59Z"
  timezone: "UTC"

# Specific deployment time
trigger_ref: core.datetimetimer
parameters:
  fire_at: "2024-06-15T14:00:00-05:00"
  timezone: "America/New_York"
  description: "Production deployment"
```

**Note**: DateTime timers automatically remove themselves after firing.

## Choosing a Timer Type

| Use Case | Recommended Type |
|----------|------------------|
| Regular health checks | Interval |
| Periodic sync/backup | Interval |
| Business hours only | Cron |
| Complex schedules | Cron |
| One-time events | DateTime |
| Reminders/deadlines | DateTime |

## Implementation Details

- All timers managed by `tokio-cron-scheduler`
- Efficient async scheduling
- Low memory overhead
- Automatic cleanup on rule deletion
- Support for concurrent timers

## Event Payloads

Each timer type creates events with specific metadata:

**Interval**: Includes `interval_seconds`, `execution_count`
**Cron**: Includes `expression`, `next_fire_at`, `execution_count`
**DateTime**: Includes `fire_at`, `fired_at`, `delay_ms`

All events include `sensor_ref: "core.interval_timer_sensor"`
