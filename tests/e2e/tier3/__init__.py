"""
Tier 3: Advanced Features & Edge Cases E2E Tests

This package contains end-to-end tests for advanced Attune features,
edge cases, security validation, and operational scenarios.

Test Coverage (9/21 scenarios implemented):
- T3.1: Date timer with past date (edge case)
- T3.2: Timer cancellation (disable/enable)
- T3.3: Multiple concurrent timers
- T3.4: Webhook with multiple rules
- T3.5: Webhook with rule criteria filtering
- T3.10: RBAC permission checks
- T3.11: System vs user packs (multi-tenancy)
- T3.13: Invalid action parameters
- T3.18: HTTP runner execution
- T3.20: Secret injection security

Status: 🔄 IN PROGRESS (43% complete)
Priority: LOW-MEDIUM
Duration: ~2 minutes total for all implemented tests
Dependencies: All services (API, Executor, Worker, Sensor)

Usage:
    # Run all Tier 3 tests
    pytest e2e/tier3/ -v

    # Run specific test file
    pytest e2e/tier3/test_t3_20_secret_injection.py -v

    # Run by category
    pytest -m security e2e/tier3/ -v
    pytest -m rbac e2e/tier3/ -v
    pytest -m http e2e/tier3/ -v
    pytest -m timer e2e/tier3/ -v
    pytest -m criteria e2e/tier3/ -v
"""

__all__ = [
    "test_t3_01_past_date_timer",
    "test_t3_02_timer_cancellation",
    "test_t3_03_concurrent_timers",
    "test_t3_04_webhook_multiple_rules",
    "test_t3_05_rule_criteria",
    "test_t3_10_rbac",
    "test_t3_11_system_packs",
    "test_t3_13_invalid_parameters",
    "test_t3_18_http_runner",
    "test_t3_20_secret_injection",
]
