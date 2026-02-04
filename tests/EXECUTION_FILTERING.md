# Execution Filtering Best Practices for E2E Tests

## Problem Overview

When writing E2E tests that verify execution creation, you may encounter race conditions or filtering issues where the test cannot find the executions it just created. This happens because:

1. **Imprecise filtering** - Using `action_ref` alone can match executions from other tests
2. **Data pollution** - Old executions from previous test runs aren't cleaned up
3. **Timing issues** - Executions haven't been created yet when the query runs
4. **Parallel execution** - Multiple tests creating similar resources simultaneously

## Solution: Multi-Level Filtering

The `wait_for_execution_count` helper now supports multiple filtering strategies that can be combined for maximum precision:

### 1. Rule-Based Filtering (Most Precise)

Filter executions by the rule that triggered them:

```python
from datetime import datetime, timezone

# Capture timestamp before creating rule
rule_creation_time = datetime.now(timezone.utc).isoformat()

# Create your automation
rule = create_rule(client, trigger_id=trigger['id'], action_ref=action['ref'])

# Wait for executions using rule_id
executions = wait_for_execution_count(
    client=client,
    expected_count=3,
    rule_id=rule['id'],              # Filter by rule
    created_after=rule_creation_time, # Only new executions
    timeout=30,
    verbose=True                      # Enable debug output
)
```

**How it works:**
1. Gets all enforcements for the rule: `GET /api/v1/enforcements?rule_id=<id>`
2. For each enforcement, gets executions: `GET /api/v1/executions?enforcement=<id>`
3. Filters by timestamp to exclude old data
4. Returns combined results

### 2. Enforcement-Based Filtering (Very Precise)

If you have a specific enforcement ID:

```python
executions = wait_for_execution_count(
    client=client,
    expected_count=1,
    enforcement_id=enforcement['id'],
    timeout=30
)
```

**How it works:**
- Directly queries: `GET /api/v1/executions?enforcement=<id>`
- Most direct and precise filtering

### 3. Action-Based Filtering (Less Precise)

When you only have an action reference:

```python
from datetime import datetime, timezone

action_creation_time = datetime.now(timezone.utc).isoformat()
action = create_echo_action(client, pack_ref=pack_ref)

executions = wait_for_execution_count(
    client=client,
    expected_count=5,
    action_ref=action['ref'],
    created_after=action_creation_time,  # Important!
    timeout=30
)
```

**Important:** Always use `created_after` with action_ref filtering to avoid matching old executions.

### 4. Status Filtering

Combine with any of the above:

```python
executions = wait_for_execution_count(
    client=client,
    expected_count=3,
    rule_id=rule['id'],
    status='succeeded',  # Only succeeded executions
    timeout=30
)
```

## Timestamp-Based Filtering

The `created_after` parameter filters executions created after a specific ISO timestamp:

```python
from datetime import datetime, timezone

# Capture timestamp at start of test
test_start = datetime.now(timezone.utc).isoformat()

# ... create automation ...

# Only count executions created during this test
executions = wait_for_execution_count(
    client=client,
    expected_count=3,
    created_after=test_start,
    # ... other filters ...
)
```

This prevents:
- Matching executions from previous test runs
- Counting executions from test setup/fixtures
- Race conditions with parallel tests

## Verbose Mode for Debugging

Enable verbose mode to see what's being matched:

```python
executions = wait_for_execution_count(
    client=client,
    expected_count=3,
    rule_id=rule['id'],
    verbose=True  # Print debug output
)
```

Output example:
```
  [DEBUG] Found 2 enforcements for rule 123
  [DEBUG] Enforcement 456: 3 executions
  [DEBUG] Enforcement 457: 2 executions
  [DEBUG] After timestamp filter: 3 executions (was 5)
  [DEBUG] Checking: 3 >= 3
```

## Best Practices

### ✅ DO: Use Multiple Filter Criteria

```python
# GOOD - Multiple precise filters
executions = wait_for_execution_count(
    client=client,
    expected_count=3,
    rule_id=rule['id'],           # Precise filter
    created_after=rule_created,   # Timestamp filter
    status='succeeded',           # State filter
    verbose=True                  # Debugging
)
```

### ❌ DON'T: Use Only action_ref

```python
# BAD - Too imprecise, may match old data
executions = wait_for_execution_count(
    client=client,
    expected_count=3,
    action_ref=action['ref']  # Could match previous runs
)
```

### ✅ DO: Capture Timestamps Early

```python
# GOOD - Timestamp before resource creation
test_start = datetime.now(timezone.utc).isoformat()
rule = create_rule(...)
executions = wait_for_execution_count(..., created_after=test_start)
```

### ❌ DON'T: Capture Timestamps After Waiting

```python
# BAD - Timestamp is too late
rule = create_rule(...)
time.sleep(10)  # Events already created
test_start = datetime.now(timezone.utc).isoformat()
executions = wait_for_execution_count(..., created_after=test_start)  # Will miss executions!
```

### ✅ DO: Use rule_id When Testing Automation Flows

```python
# GOOD - For trigger → rule → execution flows
executions = wait_for_execution_count(
    client=client,
    expected_count=3,
    rule_id=rule['id']  # Most natural for automation tests
)
```

### ✅ DO: Use enforcement_id When Testing Specific Enforcements

```python
# GOOD - For testing single enforcement
enforcement = enforcements[0]
executions = wait_for_execution_count(
    client=client,
    expected_count=1,
    enforcement_id=enforcement['id']
)
```

## Filter Hierarchy (Precision Order)

From most precise to least precise:

1. **enforcement_id** - Single enforcement's executions
2. **rule_id** - All executions from a rule (via enforcements)
3. **action_ref** + **created_after** - Executions of an action created recently
4. **action_ref** alone - All executions of an action (can match old data)

## API Endpoints Used

The helper uses these API endpoints internally:

```
GET /api/v1/executions?enforcement=<id>          # enforcement_id filter
GET /api/v1/enforcements?rule_id=<id>           # rule_id filter (step 1)
GET /api/v1/executions?enforcement=<id>          # rule_id filter (step 2)
GET /api/v1/executions?action_ref=<ref>         # action_ref filter
GET /api/v1/executions?status=<status>          # status filter
```

## Complete Example

```python
from datetime import datetime, timezone
from helpers import (
    AttuneClient,
    create_interval_timer,
    create_echo_action,
    create_rule,
    wait_for_event_count,
    wait_for_execution_count,
)

def test_timer_automation(client: AttuneClient, pack_ref: str):
    """Complete example with proper filtering"""
    
    # Capture timestamp at start
    test_start = datetime.now(timezone.utc).isoformat()
    
    # Create automation components
    trigger = create_interval_timer(client, interval_seconds=5, pack_ref=pack_ref)
    action = create_echo_action(client, pack_ref=pack_ref)
    rule = create_rule(
        client,
        trigger_id=trigger['id'],
        action_ref=action['ref'],
        pack_ref=pack_ref
    )
    
    # Wait for events
    events = wait_for_event_count(
        client=client,
        expected_count=3,
        trigger_id=trigger['id'],
        timeout=20
    )
    
    # Wait for executions with precise filtering
    executions = wait_for_execution_count(
        client=client,
        expected_count=3,
        rule_id=rule['id'],           # Precise: only this rule's executions
        created_after=test_start,     # Only executions from this test
        status='succeeded',           # Only successful ones
        timeout=30,
        verbose=True                  # Debug output
    )
    
    # Verify results
    assert len(executions) == 3
    for exec in executions:
        assert exec['status'] == 'succeeded'
        assert exec['action_ref'] == action['ref']
```

## Troubleshooting

### Test finds too many executions

**Cause:** Not filtering by timestamp, matching old data  
**Solution:** Add `created_after` parameter

### Test finds too few executions

**Cause:** Timestamp captured too late, after executions created  
**Solution:** Capture timestamp BEFORE creating rule/trigger

### Test times out waiting for executions

**Cause:** Executions not being created (service issue)  
**Solution:** Enable `verbose=True` to see what's being found, check service logs

### Inconsistent test results

**Cause:** Race condition with database cleanup or parallel tests  
**Solution:** Use `rule_id` filtering for isolation

## Summary

**Always prefer:**
1. `rule_id` for automation flow tests (trigger → rule → execution)
2. `enforcement_id` for specific enforcement tests
3. `created_after` to prevent matching old data
4. `verbose=True` when debugging

**This ensures:**
- ✅ Test isolation
- ✅ No race conditions
- ✅ Precise execution matching
- ✅ Easy debugging