"""
Polling Utilities for E2E Tests

Provides helper functions for waiting on asynchronous conditions
during end-to-end testing.
"""

import time
from typing import Any, Callable, List, Optional

from requests.models import HTTPError

from .client import AttuneClient


def wait_for_condition(
    condition_fn: Callable[[], bool],
    timeout: float = 30.0,
    poll_interval: float = 0.5,
    error_message: str = "Condition not met within timeout",
) -> bool:
    """
    Wait for a condition function to return True

    Args:
        condition_fn: Function that returns True when condition is met
        timeout: Maximum time to wait in seconds
        poll_interval: Time between checks in seconds
        error_message: Error message if timeout occurs

    Returns:
        True if condition met

    Raises:
        TimeoutError: If condition not met within timeout
    """
    start_time = time.time()
    elapsed = 0.0

    while elapsed < timeout:
        try:
            if condition_fn():
                return True
        except Exception:
            # Ignore exceptions during polling (e.g., 404 errors)
            pass

        time.sleep(poll_interval)
        elapsed = time.time() - start_time

    raise TimeoutError(f"{error_message} (waited {elapsed:.1f}s)")


def wait_for_execution_status(
    client: AttuneClient,
    execution_id: int,
    expected_status: str,
    timeout: float = 30.0,
    poll_interval: float = 0.5,
) -> dict:
    """
    Wait for execution to reach expected status

    Args:
        client: AttuneClient instance
        execution_id: Execution ID to monitor
        expected_status: Expected status (succeeded, failed, canceled, etc)
        timeout: Maximum time to wait in seconds
        poll_interval: Time between status checks

    Returns:
        Final execution object

    Raises:
        TimeoutError: If status not reached within timeout
    """
    execution = client.get_execution(execution_id)

    def check_status():
        nonlocal execution
        execution = client.get_execution(execution_id)
        return execution["status"] == expected_status

    wait_for_condition(
        check_status,
        timeout=timeout,
        poll_interval=poll_interval,
        error_message=f"Execution {execution_id} did not reach status '{expected_status}'",
    )

    return execution


def wait_for_execution_completion(
    client: AttuneClient,
    execution_id: int,
    timeout: float = 30.0,
    poll_interval: float = 0.5,
) -> dict:
    """
    Wait for execution to complete (reach terminal status)

    Terminal statuses are: succeeded, failed, canceled, timeout

    Args:
        client: AttuneClient instance
        execution_id: Execution ID to monitor
        timeout: Maximum time to wait in seconds
        poll_interval: Time between status checks

    Returns:
        Final execution object

    Raises:
        TimeoutError: If execution doesn't complete within timeout
    """
    execution = client.get_execution(execution_id)

    def check_completion():
        nonlocal execution
        execution = client.get_execution(execution_id)
        terminal_statuses = ["succeeded", "failed", "canceled", "timeout"]
        return execution["status"] in terminal_statuses

    wait_for_condition(
        check_completion,
        timeout=timeout,
        poll_interval=poll_interval,
        error_message=f"Execution {execution_id} did not complete",
    )

    return execution


def wait_for_execution_count(
    client: AttuneClient,
    expected_count: int,
    action_ref: Optional[str] = None,
    status: Optional[str] = None,
    enforcement_id: Optional[int] = None,
    rule_id: Optional[int] = None,
    created_after: Optional[str] = None,
    timeout: float = 30.0,
    poll_interval: float = 0.5,
    operator: str = ">=",
    verbose: bool = False,
) -> List[dict]:
    """
    Wait for execution count to reach threshold

    Args:
        client: AttuneClient instance
        expected_count: Expected number of executions
        action_ref: Optional filter by action reference
        status: Optional filter by status
        enforcement_id: Optional filter by enforcement ID (most precise)
        rule_id: Optional filter by rule ID (via enforcement)
        created_after: Optional ISO timestamp to filter executions created after this time
        timeout: Maximum time to wait
        poll_interval: Time between checks
        operator: Comparison operator (>=, ==, <=, >, <)
        verbose: Print debug information during polling

    Returns:
        List of executions

    Raises:
        TimeoutError: If count not reached within timeout
    """
    executions = []

    def check_count():
        nonlocal executions

        # If rule_id is provided, get executions via enforcements
        if rule_id is not None:
            # Get all enforcements for this rule
            enforcements = client.list_enforcements(rule_id=rule_id, limit=1000)
            if verbose:
                print(
                    f"  [DEBUG] Found {len(enforcements)} enforcements for rule {rule_id}"
                )
            # Get executions for each enforcement
            all_executions = []
            for enf in enforcements:
                enf_executions = client.list_executions(
                    enforcement_id=enf["id"], status=status, limit=1000
                )
                if verbose:
                    print(
                        f"  [DEBUG] Enforcement {enf['id']}: {len(enf_executions)} executions"
                    )
                all_executions.extend(enf_executions)
            executions = all_executions
        elif enforcement_id is not None:
            # Filter by specific enforcement
            executions = client.list_executions(
                enforcement_id=enforcement_id, status=status, limit=1000
            )
            if verbose:
                print(
                    f"  [DEBUG] Found {len(executions)} executions for enforcement {enforcement_id}"
                )
        else:
            # Use action_ref and status filters
            executions = client.list_executions(
                action_ref=action_ref, status=status, limit=1000
            )
            if verbose:
                filter_str = f"action_ref={action_ref}" if action_ref else "all"
                if status:
                    filter_str += f", status={status}"
                print(f"  [DEBUG] Found {len(executions)} executions ({filter_str})")

        # Apply timestamp filter if provided
        if created_after:
            from datetime import datetime

            cutoff = datetime.fromisoformat(created_after.replace("Z", "+00:00"))
            filtered = []
            for exec in executions:
                exec_time = datetime.fromisoformat(
                    exec["created"].replace("Z", "+00:00")
                )
                if exec_time > cutoff:
                    filtered.append(exec)
            if verbose:
                print(
                    f"  [DEBUG] After timestamp filter: {len(filtered)} executions (was {len(executions)})"
                )
            executions = filtered

        actual_count = len(executions)

        if verbose:
            print(f"  [DEBUG] Checking: {actual_count} {operator} {expected_count}")

        if operator == ">=":
            return actual_count >= expected_count
        elif operator == "==":
            return actual_count == expected_count
        elif operator == "<=":
            return actual_count <= expected_count
        elif operator == ">":
            return actual_count > expected_count
        elif operator == "<":
            return actual_count < expected_count
        else:
            raise ValueError(f"Invalid operator: {operator}")

    filter_desc = ""
    if rule_id:
        filter_desc += f" for rule {rule_id}"
    elif enforcement_id:
        filter_desc += f" for enforcement {enforcement_id}"
    elif action_ref:
        filter_desc += f" for action {action_ref}"
    if status:
        filter_desc += f" with status {status}"
    if created_after:
        filter_desc += f" created after {created_after}"

    wait_for_condition(
        check_count,
        timeout=timeout,
        poll_interval=poll_interval,
        error_message=f"Execution count did not reach {operator} {expected_count}{filter_desc}",
    )

    return executions


def wait_for_event_count(
    client: AttuneClient,
    expected_count: int,
    trigger_id: Optional[int] = None,
    timeout: float = 30.0,
    poll_interval: float = 0.5,
    operator: str = ">=",
) -> List[dict]:
    """
    Wait for event count to reach threshold

    Args:
        client: AttuneClient instance
        expected_count: Expected number of events
        trigger_id: Optional filter by trigger ID
        timeout: Maximum time to wait
        poll_interval: Time between checks
        operator: Comparison operator (>=, ==, <=, >, <)

    Returns:
        List of events

    Raises:
        TimeoutError: If count not reached within timeout
    """
    events = []

    def check_count():
        nonlocal events
        events = client.list_events(trigger_id=trigger_id, limit=1000)
        actual_count = len(events)

        if operator == ">=":
            return actual_count >= expected_count
        elif operator == "==":
            return actual_count == expected_count
        elif operator == "<=":
            return actual_count <= expected_count
        elif operator == ">":
            return actual_count > expected_count
        elif operator == "<":
            return actual_count < expected_count
        else:
            raise ValueError(f"Invalid operator: {operator}")

    filter_desc = f" for trigger {trigger_id}" if trigger_id else ""

    wait_for_condition(
        check_count,
        timeout=timeout,
        poll_interval=poll_interval,
        error_message=f"Event count did not reach {operator} {expected_count}{filter_desc}",
    )

    return events


def wait_for_enforcement_count(
    client: AttuneClient,
    expected_count: int,
    rule_id: Optional[int] = None,
    timeout: float = 30.0,
    poll_interval: float = 0.5,
    operator: str = ">=",
) -> List[dict]:
    """
    Wait for enforcement count to reach threshold

    Args:
        client: AttuneClient instance
        expected_count: Expected number of enforcements
        rule_id: Optional filter by rule ID
        timeout: Maximum time to wait
        poll_interval: Time between checks
        operator: Comparison operator (>=, ==, <=, >, <)

    Returns:
        List of enforcements

    Raises:
        TimeoutError: If count not reached within timeout
    """
    enforcements = []

    def check_count():
        nonlocal enforcements
        enforcements = client.list_enforcements(rule_id=rule_id, limit=1000)
        actual_count = len(enforcements)

        if operator == ">=":
            return actual_count >= expected_count
        elif operator == "==":
            return actual_count == expected_count
        elif operator == "<=":
            return actual_count <= expected_count
        elif operator == ">":
            return actual_count > expected_count
        elif operator == "<":
            return actual_count < expected_count
        else:
            raise ValueError(f"Invalid operator: {operator}")

    filter_desc = f" for rule {rule_id}" if rule_id else ""

    wait_for_condition(
        check_count,
        timeout=timeout,
        poll_interval=poll_interval,
        error_message=f"Enforcement count did not reach {operator} {expected_count}{filter_desc}",
    )

    return enforcements


def wait_for_inquiry_status(
    client: AttuneClient,
    inquiry_id: int,
    expected_status: str,
    timeout: float = 30.0,
    poll_interval: float = 0.5,
) -> dict:
    """
    Wait for inquiry to reach expected status

    Args:
        client: AttuneClient instance
        inquiry_id: Inquiry ID to monitor
        expected_status: Expected status (pending, responded, expired)
        timeout: Maximum time to wait
        poll_interval: Time between checks

    Returns:
        Final inquiry object

    Raises:
        TimeoutError: If status not reached within timeout
    """
    inquiry = client.get_inquiry(inquiry_id)

    def check_status():
        nonlocal inquiry
        inquiry = client.get_inquiry(inquiry_id)
        return inquiry["status"] == expected_status

    wait_for_condition(
        check_status,
        timeout=timeout,
        poll_interval=poll_interval,
        error_message=f"Inquiry {inquiry_id} did not reach status '{expected_status}'",
    )

    return inquiry


def wait_for_inquiry_count(
    client: AttuneClient,
    expected_count: int,
    status: Optional[str] = None,
    timeout: float = 30.0,
    poll_interval: float = 0.5,
    operator: str = ">=",
) -> List[dict]:
    """
    Wait for inquiry count to reach expected value

    Args:
        client: AttuneClient instance
        expected_count: Expected number of inquiries
        status: Optional status filter (pending, responded, expired, etc)
        timeout: Maximum time to wait
        poll_interval: Time between checks
        operator: Comparison operator (>=, ==, <=, >, <)

    Returns:
        List of inquiries matching criteria

    Raises:
        TimeoutError: If count not reached within timeout
    """
    inquiries = []

    def check_count():
        nonlocal inquiries
        try:
            response = client.get("/inquiries")
        except HTTPError:
            return False

        inquiries = response.get("data", [])

        # Filter by status if specified
        if status:
            inquiries = [i for i in inquiries if i.get("status") == status]

        actual_count = len(inquiries)

        # Check count based on operator
        if operator == "==":
            return actual_count == expected_count
        elif operator == ">=":
            return actual_count >= expected_count
        elif operator == "<=":
            return actual_count <= expected_count
        elif operator == ">":
            return actual_count > expected_count
        elif operator == "<":
            return actual_count < expected_count
        else:
            raise ValueError(f"Invalid operator: {operator}")

    filter_desc = f" with status {status}" if status else ""

    wait_for_condition(
        check_count,
        timeout=timeout,
        poll_interval=poll_interval,
        error_message=f"Inquiry count did not reach {operator} {expected_count}{filter_desc}",
    )

    return inquiries
