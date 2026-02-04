"""
Tier 1 E2E Tests - Core Automation Flows

This package contains Tier 1 end-to-end tests that validate the fundamental
automation lifecycle. These tests are critical for MVP and must all pass
before release.

Test Coverage:
- T1.1: Interval Timer Automation
- T1.2: Date Timer (One-Shot Execution)
- T1.3: Cron Timer Execution
- T1.4: Webhook Trigger with Payload
- T1.5: Workflow with Array Iteration (with-items)
- T1.6: Action Reads from Key-Value Store
- T1.7: Multi-Tenant Isolation
- T1.8: Action Execution Failure Handling

All tests require:
- All 5 services running (API, Executor, Worker, Sensor, Notifier)
- PostgreSQL database
- RabbitMQ message queue
- Test fixtures in tests/fixtures/

Run with:
    pytest tests/e2e/tier1/ -v
    pytest tests/e2e/tier1/test_t1_01_interval_timer.py -v
    pytest -m tier1 -v
"""

__all__ = []
