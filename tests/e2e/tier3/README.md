# Tier 3 E2E Tests - Quick Reference Guide

**Status**: 🔄 IN PROGRESS (17/21 scenarios, 81%)  
**Focus**: Advanced features, edge cases, security validation, operational scenarios  
**Priority**: MEDIUM-LOW (after Tier 1 & 2 complete)

---

## Overview

Tier 3 tests validate advanced Attune features, edge cases, security boundaries, and operational scenarios that go beyond core automation flows. These tests ensure the platform is robust, secure, and production-ready.

---

## Implemented Tests (17 scenarios, 56 tests)

### 🔐 T3.20: Secret Injection Security (HIGH Priority)
**File**: `test_t3_20_secret_injection.py` (566 lines)  
**Tests**: 4  
**Duration**: ~20 seconds

Validates that secrets are passed securely via stdin (not environment variables) and never exposed in logs or to other tenants.

**Test Functions:**
1. `test_secret_injection_via_stdin` - Secrets via stdin validation
2. `test_secret_encryption_at_rest` - Encryption flag validation
3. `test_secret_not_in_execution_logs` - Secret redaction testing
4. `test_secret_access_tenant_isolation` - Cross-tenant isolation

**Run:**
```bash
pytest e2e/tier3/test_t3_20_secret_injection.py -v
pytest -m secrets -v
```

**Key Validations:**
- ✅ Secrets passed via stdin (secure)
- ✅ Secrets NOT in environment variables
- ✅ Secrets NOT exposed in logs
- ✅ Tenant isolation enforced

---

### 🔒 T3.10: RBAC Permission Checks (MEDIUM Priority)
**File**: `test_t3_10_rbac.py` (524 lines)  
**Tests**: 4  
**Duration**: ~20 seconds

Tests role-based access control enforcement across all API endpoints.

**Test Functions:**
1. `test_viewer_role_permissions` - Read-only access
2. `test_admin_role_permissions` - Full CRUD access
3. `test_executor_role_permissions` - Execute + read only
4. `test_role_permissions_summary` - Permission matrix documentation

**Run:**
```bash
pytest e2e/tier3/test_t3_10_rbac.py -v
pytest -m rbac -v
```

**Roles Tested:**
- **admin** - Full access
- **editor** - Create/update + execute
- **executor** - Execute + read only
- **viewer** - Read-only

---

### 🌐 T3.18: HTTP Runner Execution (MEDIUM Priority)
**File**: `test_t3_18_http_runner.py` (473 lines)  
**Tests**: 4  
**Duration**: ~10 seconds

Validates HTTP runner making REST API calls with authentication, headers, and error handling.

**Test Functions:**
1. `test_http_runner_basic_get` - GET request
2. `test_http_runner_post_with_json` - POST with JSON
3. `test_http_runner_authentication_header` - Bearer token auth
4. `test_http_runner_error_handling` - 4xx/5xx errors

**Run:**
```bash
pytest e2e/tier3/test_t3_18_http_runner.py -v
pytest -m http -v
```

**Features Validated:**
- ✅ GET and POST requests
- ✅ Custom headers
- ✅ JSON serialization
- ✅ Authentication via secrets
- ✅ Response capture
- ✅ Error handling

---

### ⚠️ T3.13: Invalid Action Parameters (MEDIUM Priority)
**File**: `test_t3_13_invalid_parameters.py` (559 lines)  
**Tests**: 4  
**Duration**: ~5 seconds

Tests parameter validation, default values, and error handling.

**Test Functions:**
1. `test_missing_required_parameter` - Required param validation
2. `test_invalid_parameter_type` - Type checking
3. `test_extra_parameters_ignored` - Extra params handling
4. `test_parameter_default_values` - Default values

**Run:**
```bash
pytest e2e/tier3/test_t3_13_invalid_parameters.py -v
pytest -m validation -v
```

**Validations:**
- ✅ Missing required parameters fail early
- ✅ Clear error messages
- ✅ Default values applied
- ✅ Extra parameters ignored gracefully

---

### ⏱️ T3.1: Date Timer with Past Date (LOW Priority)
**File**: `test_t3_01_past_date_timer.py` (305 lines)  
**Tests**: 3  
**Duration**: ~5 seconds

Tests edge cases for date timers with past dates.

**Test Functions:**
1. `test_past_date_timer_immediate_execution` - 1 hour past
2. `test_just_missed_date_timer` - 2 seconds past
3. `test_far_past_date_timer` - 1 year past

**Run:**
```bash
pytest e2e/tier3/test_t3_01_past_date_timer.py -v
pytest -m edge_case -v
```

**Edge Cases:**
- ✅ Past date behavior (execute or reject)
- ✅ Boundary conditions
- ✅ Clear error messages

---

### 🔗 T3.4: Webhook with Multiple Rules (LOW Priority)
**File**: `test_t3_04_webhook_multiple_rules.py` (343 lines)  
**Tests**: 2  
**Duration**: ~15 seconds

Tests single webhook triggering multiple rules simultaneously.

**Test Functions:**
1. `test_webhook_fires_multiple_rules` - 1 webhook → 3 rules
2. `test_webhook_multiple_posts_multiple_rules` - 3 posts × 2 rules

**Run:**
```bash
pytest e2e/tier3/test_t3_04_webhook_multiple_rules.py -v
pytest -m webhook e2e/tier3/ -v
```

**Validations:**
- ✅ Single event triggers multiple rules
- ✅ Independent rule execution
- ✅ Correct execution count (posts × rules)

---

### ⏱️ T3.2: Timer Cancellation (LOW Priority)
**File**: `test_t3_02_timer_cancellation.py` (335 lines)  
**Tests**: 3  
**Duration**: ~15 seconds

Tests that disabling/deleting rules stops timer executions.

**Test Functions:**
1. `test_timer_cancellation_via_rule_disable` - Disable stops executions
2. `test_timer_resume_after_re_enable` - Re-enable resumes timer
3. `test_timer_delete_stops_executions` - Delete permanently stops

**Run:**
```bash
pytest e2e/tier3/test_t3_02_timer_cancellation.py -v
pytest -m timer e2e/tier3/ -v
```

**Validations:**
- ✅ Disabling rule stops future executions
- ✅ Re-enabling rule resumes timer
- ✅ Deleting rule permanently stops timer
- ✅ In-flight executions complete normally

---

### ⏱️ T3.3: Multiple Concurrent Timers (LOW Priority)
**File**: `test_t3_03_concurrent_timers.py` (438 lines)  
**Tests**: 3  
**Duration**: ~30 seconds

Tests that multiple timers run independently without interference.

**Test Functions:**
1. `test_multiple_concurrent_timers` - 3 timers with different intervals
2. `test_many_concurrent_timers` - 5 concurrent timers (stress test)
3. `test_timer_precision_under_load` - Precision validation

**Run:**
```bash
pytest e2e/tier3/test_t3_03_concurrent_timers.py -v
pytest -m performance e2e/tier3/ -v
```

**Validations:**
- ✅ Multiple timers fire independently
- ✅ Correct execution counts per timer
- ✅ No timer interference
- ✅ System handles concurrent load
- ✅ Timing precision maintained

---

### 🎯 T3.5: Webhook with Rule Criteria Filtering (MEDIUM Priority)
**File**: `test_t3_05_rule_criteria.py` (507 lines)  
**Tests**: 4  
**Duration**: ~20 seconds

Tests conditional rule firing based on event payload criteria.

**Test Functions:**
1. `test_rule_criteria_basic_filtering` - Equality checks
2. `test_rule_criteria_numeric_comparison` - Numeric operators
3. `test_rule_criteria_complex_expressions` - AND/OR logic
4. `test_rule_criteria_list_membership` - List membership

**Run:**
```bash
pytest e2e/tier3/test_t3_05_rule_criteria.py -v
pytest -m criteria -v
```

**Validations:**
- ✅ Jinja2 expression evaluation
- ✅ Event filtering by criteria
- ✅ Numeric comparisons (>, <, >=, <=)
- ✅ Complex boolean logic (AND/OR)
- ✅ List membership (in operator)
- ✅ Only matching rules fire

---

### 🔒 T3.11: System vs User Packs (MEDIUM Priority)
**File**: `test_t3_11_system_packs.py` (401 lines)  
**Tests**: 4  
**Duration**: ~15 seconds

Tests multi-tenant pack isolation and system pack availability.

**Test Functions:**
1. `test_system_pack_visible_to_all_tenants` - System packs visible to all
2. `test_user_pack_isolation` - User packs isolated per tenant
3. `test_system_pack_actions_available_to_all` - System actions executable
4. `test_system_pack_identification` - Documentation reference

**Run:**
```bash
pytest e2e/tier3/test_t3_11_system_packs.py -v
pytest -m multi_tenant -v
```

**Validations:**
- ✅ System packs visible to all tenants
- ✅ User packs isolated per tenant
- ✅ Cross-tenant access blocked
- ✅ System actions executable by all
- ✅ Pack isolation enforced

---

### 🔔 T3.14: Execution Completion Notifications (MEDIUM Priority)
**File**: `test_t3_14_execution_notifications.py` (374 lines)  
**Tests**: 4  
**Duration**: ~20 seconds

Tests real-time notification system for execution lifecycle events.

**Test Functions:**
1. `test_execution_success_notification` - Success completion notifications
2. `test_execution_failure_notification` - Failure event notifications
3. `test_execution_timeout_notification` - Timeout event notifications
4. `test_websocket_notification_delivery` - Real-time WebSocket delivery (skipped)

**Run:**
```bash
pytest e2e/tier3/test_t3_14_execution_notifications.py -v
pytest -m notifications -v
```

**Key Validations:**
- ✅ Notification metadata for execution events
- ✅ Success, failure, and timeout notifications
- ✅ Execution tracking for real-time updates
- ⏭️ WebSocket delivery (infrastructure pending)

---

### 🔔 T3.15: Inquiry Creation Notifications (MEDIUM Priority)
**File**: `test_t3_15_inquiry_notifications.py` (405 lines)  
**Tests**: 4  
**Duration**: ~20 seconds

Tests notification system for human-in-the-loop inquiry workflows.

**Test Functions:**
1. `test_inquiry_creation_notification` - Inquiry creation event
2. `test_inquiry_response_notification` - Response submission event
3. `test_inquiry_timeout_notification` - Inquiry timeout handling
4. `test_websocket_inquiry_notification_delivery` - Real-time delivery (skipped)

**Run:**
```bash
pytest e2e/tier3/test_t3_15_inquiry_notifications.py -v
pytest -m "notifications and inquiry" -v
```

**Key Validations:**
- ✅ Inquiry lifecycle events (created, responded, timeout)
- ✅ Notification metadata for approval workflows
- ✅ Human-in-the-loop notification flow
- ⏭️ Real-time WebSocket delivery (pending)

---

### 🐳 T3.17: Container Runner Execution (MEDIUM Priority)
**File**: `test_t3_17_container_runner.py` (472 lines)  
**Tests**: 4  
**Duration**: ~30 seconds

Tests Docker-based container runner for isolated action execution.

**Test Functions:**
1. `test_container_runner_basic_execution` - Basic Python container execution
2. `test_container_runner_with_parameters` - Parameter injection via stdin
3. `test_container_runner_isolation` - Container isolation validation
4. `test_container_runner_failure_handling` - Failure capture and cleanup

**Run:**
```bash
pytest e2e/tier3/test_t3_17_container_runner.py -v
pytest -m container -v
```

**Key Validations:**
- ✅ Container-based execution (python:3.11-slim)
- ✅ Parameter passing via JSON stdin
- ✅ Container isolation (no state leakage)
- ✅ Failure handling and cleanup
- ✅ Docker image specification

**Prerequisites**: Docker daemon running

---

### 📝 T3.21: Action Log Size Limits (MEDIUM Priority)
**File**: `test_t3_21_log_size_limits.py` (481 lines)  
**Tests**: 4  
**Duration**: ~20 seconds

Tests log capture, size limits, and handling of large outputs.

**Test Functions:**
1. `test_large_log_output_truncation` - Large log truncation (~5MB output)
2. `test_stderr_log_capture` - Separate stdout/stderr capture
3. `test_log_line_count_limits` - High line count handling (10k lines)
4. `test_binary_output_handling` - Binary/non-UTF8 output sanitization

**Run:**
```bash
pytest e2e/tier3/test_t3_21_log_size_limits.py -v
pytest -m logs -v
```

**Key Validations:**
- ✅ Log size limits enforced (max 10MB)
- ✅ Stdout and stderr captured separately
- ✅ High line count (10,000+) handled gracefully
- ✅ Binary data properly sanitized
- ✅ No crashes from large output

---

### 🔄 T3.7: Complex Workflow Orchestration (MEDIUM Priority)
**File**: `test_t3_07_complex_workflows.py` (718 lines)  
**Tests**: 4  
**Duration**: ~45 seconds

Tests advanced workflow features including parallel execution, branching, and data transformation.

**Test Functions:**
1. `test_parallel_workflow_execution` - Parallel task execution
2. `test_conditional_workflow_branching` - If/else conditional logic
3. `test_nested_workflow_with_error_handling` - Nested workflows with error recovery
4. `test_workflow_with_data_transformation` - Data pipeline with transformations

**Run:**
```bash
pytest e2e/tier3/test_t3_07_complex_workflows.py -v
pytest -m orchestration -v
```

**Key Validations:**
- ✅ Parallel task execution (3 tasks concurrently)
- ✅ Conditional branching (if/else based on parameters)
- ✅ Nested workflow execution with error handling
- ✅ Data transformation and passing between tasks
- ✅ Workflow orchestration patterns

---

### 🔗 T3.8: Chained Webhook Triggers (MEDIUM Priority)
**File**: `test_t3_08_chained_webhooks.py` (686 lines)  
**Tests**: 4  
**Duration**: ~30 seconds

Tests webhook chains where webhooks trigger workflows that trigger other webhooks.

**Test Functions:**
1. `test_webhook_triggers_workflow_triggers_webhook` - A→Workflow→B chain
2. `test_webhook_cascade_multiple_levels` - Multi-level cascade (A→B→C)
3. `test_webhook_chain_with_data_passing` - Data transformation in chains
4. `test_webhook_chain_error_propagation` - Error handling in chains

**Run:**
```bash
pytest e2e/tier3/test_t3_08_chained_webhooks.py -v
pytest -m "webhook and orchestration" -v
```

**Key Validations:**
- ✅ Webhook chaining through workflows
- ✅ Multi-level webhook cascades
- ✅ Data passing and transformation through chains
- ✅ Error propagation and isolation
- ✅ HTTP runner triggering webhooks

---

### 🔐 T3.9: Multi-Step Approval Workflow (MEDIUM Priority)
**File**: `test_t3_09_multistep_approvals.py` (788 lines)  
**Tests**: 4  
**Duration**: ~40 seconds

Tests complex approval workflows with multiple sequential and conditional inquiries.

**Test Functions:**
1. `test_sequential_multi_step_approvals` - 3 sequential approvals (Manager→Director→VP)
2. `test_conditional_approval_workflow` - Conditional approval based on response
3. `test_approval_with_timeout_and_escalation` - Timeout triggers escalation
4. `test_approval_denial_stops_workflow` - Denial stops subsequent steps

**Run:**
```bash
pytest e2e/tier3/test_t3_09_multistep_approvals.py -v
pytest -m "inquiry and workflow" -v
```

**Key Validations:**
- ✅ Sequential multi-step approvals
- ✅ Conditional approval logic
- ✅ Timeout and escalation handling
- ✅ Denial stops workflow execution
- ✅ Human-in-the-loop orchestration

---

### 🔔 T3.16: Rule Trigger Notifications (MEDIUM Priority)
**File**: `test_t3_16_rule_notifications.py` (464 lines)  
**Tests**: 4  
**Duration**: ~20 seconds

Tests real-time notifications for rule lifecycle events.

**Test Functions:**
1. `test_rule_trigger_notification` - Rule trigger notification metadata
2. `test_rule_enable_disable_notification` - State change notifications
3. `test_multiple_rule_triggers_notification` - Multiple rules from one event
4. `test_rule_criteria_evaluation_notification` - Criteria match/no-match

**Run:**
```bash
pytest e2e/tier3/test_t3_16_rule_notifications.py -v
pytest -m "notifications and rules" -v
```

**Key Validations:**
- ✅ Rule trigger notification metadata
- ✅ Rule state change notifications (enable/disable)
- ✅ Multiple rule trigger notifications from single event
- ✅ Rule criteria evaluation tracking
- ✅ Enforcement creation notification

---

## Remaining Scenarios (4 scenarios, ~4 tests)

### LOW Priority (4 remaining)
- [ ] **T3.6**: Sensor-generated custom events
- [ ] **T3.12**: Worker crash recovery
- [ ] **T3.19**: Dependency conflict isolation (virtualenv)
- [ ] **T3.22**: Additional edge cases (TBD)

---

## Quick Commands

### Run All Tier 3 Tests
```bash
cd tests
pytest e2e/tier3/ -v
```

### Run by Category
```bash
# Security tests (secrets + RBAC)
pytest -m security e2e/tier3/ -v

# HTTP runner tests
pytest -m http -v

# Parameter validation tests
pytest -m validation -v

# Edge cases
pytest -m edge_case -v

# All webhook tests
pytest -m webhook e2e/tier3/ -v
```

### Run Specific Test
```bash
# Secret injection (most important security test)
pytest e2e/tier3/test_t3_20_secret_injection.py::test_secret_injection_via_stdin -v

# RBAC viewer permissions
pytest e2e/tier3/test_t3_10_rbac.py::test_viewer_role_permissions -v

# HTTP GET request
pytest e2e/tier3/test_t3_18_http_runner.py::test_http_runner_basic_get -v
```

### Run with Output
```bash
# Show print statements
pytest e2e/tier3/ -v -s

# Stop on first failure
pytest e2e/tier3/ -v -x

# Run specific marker with output
pytest -m secrets -v -s
```

---

## Test Markers

Use pytest markers to run specific test categories:

- `@pytest.mark.tier3` - All Tier 3 tests
- `@pytest.mark.security` - Security and RBAC tests
- `@pytest.mark.secrets` - Secret management tests
- `@pytest.mark.rbac` - Role-based access control
- `@pytest.mark.http` - HTTP runner tests
- `@pytest.mark.runner` - Action runner tests
- `@pytest.mark.validation` - Parameter validation
- `@pytest.mark.parameters` - Parameter handling
- `@pytest.mark.edge_case` - Edge cases
- `@pytest.mark.webhook` - Webhook tests
- `@pytest.mark.rules` - Rule evaluation tests
- `@pytest.mark.timer` - Timer tests
- `@pytest.mark.criteria` - Rule criteria tests
- `@pytest.mark.multi_tenant` - Multi-tenancy tests
- `@pytest.mark.packs` - Pack management tests
- `@pytest.mark.notifications` - Notification system tests
- `@pytest.mark.websocket` - WebSocket tests (skipped - pending infrastructure)
- `@pytest.mark.container` - Container runner tests
- `@pytest.mark.logs` - Log capture and size tests
- `@pytest.mark.limits` - Resource and size limit tests
- `@pytest.mark.orchestration` - Advanced workflow orchestration tests

---

## Prerequisites

### Services Required
1. PostgreSQL (port 5432)
2. RabbitMQ (port 5672)
3. attune-api (port 8080)
4. attune-executor
5. attune-worker
6. attune-sensor
7. attune-notifier (for notification tests)

### External Dependencies
- **HTTP tests**: Internet access (uses httpbin.org)
- **Container tests**: Docker daemon running
- **Notification tests**: Notifier service running
- **Secret tests**: Encryption key configured

---

## Test Patterns

### Common Test Structure
```python
def test_feature(client: AttuneClient, test_pack):
    """Test description"""
    print("\n" + "=" * 80)
    print("TEST: Feature Name")
    print("=" * 80)
    
    # Step 1: Setup
    print("\n[STEP 1] Setting up...")
    # Create resources
    
    # Step 2: Execute
    print("\n[STEP 2] Executing...")
    # Trigger action
    
    # Step 3: Verify
    print("\n[STEP 3] Verifying...")
    # Check results
    
    # Summary
    print("\n" + "=" * 80)
    print("SUMMARY")
    print("=" * 80)
    # Print results
    
    # Assertions
    assert condition, "Error message"
```

### Polling Pattern
```python
from helpers.polling import wait_for_execution_status

final_exec = wait_for_execution_status(
    client=client,
    execution_id=execution_id,
    expected_status="succeeded",
    timeout=20,
)
```

### Secret Testing Pattern
```python
# Create secret
secret_response = client.create_secret(
    key="api_key",
    value="secret_value",
    encrypted=True
)

# Use secret in action
execution_data = {
    "action": action_ref,
    "parameters": {},
    "secrets": ["api_key"]
}
```

---

## Troubleshooting

### Test Failures

**Secret injection test fails:**
- Check if worker is passing secrets via stdin
- Verify encryption key is configured
- Check worker logs for secret handling

**RBAC test fails:**
- RBAC may not be fully implemented yet
- Tests use `pytest.skip()` for unavailable features
- Check if role-based registration is available

**HTTP runner test fails:**
- Verify internet access (uses httpbin.org)
- Check if HTTP runner is implemented
- Verify proxy settings if behind firewall

**Parameter validation test fails:**
- Check if parameter validation is implemented
- Verify error messages are clear
- Check executor parameter handling

### Common Issues

**Timeouts:**
- Increase timeout values in polling functions
- Check if services are running and responsive
- Verify network connectivity

**Import Errors:**
- Run `pip install -r requirements-test.txt`
- Check Python path includes test helpers

**Authentication Errors:**
- Check if test user credentials are correct
- Verify JWT_SECRET is configured
- Check API service logs

---

## Contributing

### Adding New Tests

1. Create test file: `test_t3_XX_feature_name.py`
2. Add docstring with scenario number and description
3. Use consistent test structure (steps, summary, assertions)
4. Add appropriate pytest markers
5. Update this README with test information
6. Update `E2E_TESTS_COMPLETE.md` with completion status

### Test Writing Guidelines

- ✅ Clear step-by-step output for debugging
- ✅ Comprehensive assertions with descriptive messages
- ✅ Summary section at end of each test
- ✅ Handle unimplemented features gracefully (pytest.skip)
- ✅ Use unique references to avoid conflicts
- ✅ Clean up resources when possible
- ✅ Document expected behavior in docstrings

---

## Statistics

**Completed**: 17/21 scenarios (81%)  
**Test Functions**: 56  
**Lines of Code**: ~8,700  
**Average Duration**: ~240 seconds total

**Priority Status:**
- HIGH: 5/5 complete (100%) ✅
- MEDIUM: 11/11 complete (100%) ✅
- LOW: 1/5 complete (20%) 🔄

---

## References

- **Test Plan**: `docs/e2e-test-plan.md`
- **Complete Report**: `tests/E2E_TESTS_COMPLETE.md`
- **Helpers**: `tests/helpers/`
- **Tier 1 Tests**: `tests/e2e/tier1/`
- **Tier 2 Tests**: `tests/e2e/tier2/`

---

**Last Updated**: 2026-01-21  
**Status**: 🔄 IN PROGRESS (17/21 scenarios, 81%)  
**Next**: T3.6 (Custom events), T3.12 (Crash recovery), T3.19 (Dependency isolation)