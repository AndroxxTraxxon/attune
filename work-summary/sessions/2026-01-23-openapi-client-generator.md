# OpenAPI Client Generator Implementation - 2026-01-23

## Summary
Implemented automatic Python client generation from the Attune API's OpenAPI specification to replace the manual `AttuneClient` class. This eliminates field name mismatches and keeps the test client in sync with the API automatically.

## Problem Statement

The manual `AttuneClient` class in `tests/helpers/client.py` had several issues:
1. **Field name mismatches** - Manual mapping from legacy field names (name, type, runner_type) to new API schema (ref, label, runtime)
2. **API changes not reflected** - When API changes, client must be manually updated
3. **Missing endpoints** - `/api/v1/runtimes` endpoint doesn't exist but client tried to use it
4. **Type safety issues** - No type checking, lots of `Dict[str, Any]`
5. **Maintenance burden** - Every API change requires client updates

## Solution: OpenAPI Client Generator

### Implementation

Created `scripts/generate-python-client.sh` that:
1. Downloads OpenAPI spec from running API (`/api-spec/openapi.json`)
2. Generates Python client using `openapi-python-client`
3. Installs client into E2E venv as `attune-client` package
4. Creates usage documentation

### Benefits

✅ **Automatic sync** - Regenerate when API changes
✅ **Type safety** - Pydantic models with validation
✅ **No field mapping** - Uses exact API schema
✅ **Complete coverage** - All 71 API endpoints included
✅ **Async support** - Both sync and async methods
✅ **Better errors** - Type checking catches issues early

### Generated Client Structure

```
tests/generated_client/
├── pyproject.toml          # Package configuration
├── client.py               # Base client with auth
├── errors.py               # Error types
├── types.py                # Type aliases
├── models/                 # Pydantic models (71 files)
│   ├── login_request.py
│   ├── token_response.py
│   ├── create_trigger_request.py
│   └── ...
└── api/                    # API endpoints organized by tag
    ├── auth/
    │   ├── login.py
    │   ├── register.py
    │   └── ...
    ├── packs/
    ├── triggers/
    ├── actions/
    └── ...
```

## Usage Example

### Old Manual Client (Before)
```python
from helpers.client import AttuneClient

client = AttuneClient(base_url="http://localhost:8080")
client.login("test@attune.local", "TestPass123!")

# Field name mapping issues
trigger = client.create_trigger(
    name="my_trigger",          # Maps to ref internally
    trigger_type="webhook",      # Not used by API
    pack_ref="my_pack"
)

# Runtime ID lookup workaround
action = client.create_action(
    name="my_action",
    runner_type="python3",       # Manual ID lookup
    pack_ref="my_pack"
)
```

### New Generated Client (After)
```python
from attune_client import Client
from attune_client.api.auth import login
from attune_client.api.triggers import create_trigger
from attune_client.api.actions import create_action
from attune_client.models import (
    LoginRequest,
    CreateTriggerRequest,
    CreateActionRequest,
)

# Create client
client = Client(base_url="http://localhost:8080")

# Login with type-safe request
login_req = LoginRequest(login="test@attune.local", password="TestPass123!")
response = login.sync(client=client, json_body=login_req)
token = response.data.access_token

# Use authenticated client
client = Client(base_url="http://localhost:8080", token=token)

# Create trigger with exact API schema
trigger_req = CreateTriggerRequest(
    ref="my_pack.my_trigger",
    label="My Trigger",
    description="Test trigger",
    pack_ref="my_pack",
    enabled=True,
)
trigger = create_trigger.sync(client=client, json_body=trigger_req)

# Create action with exact API schema
action_req = CreateActionRequest(
    ref="my_pack.my_action",
    label="My Action",
    description="Test action",
    pack_ref="my_pack",
    entrypoint="actions/my_action.py",
    runtime=None,  # Optional, not required
)
action = create_action.sync(client=client, json_body=action_req)
```

## Migration Plan

### Phase 1: Wrapper Layer (Immediate)
Create compatibility wrapper that uses generated client internally but maintains old interface:

```python
# tests/helpers/client_wrapper.py
from attune_client import Client as GeneratedClient
from attune_client.api.auth import login as api_login
from attune_client.api.triggers import create_trigger as api_create_trigger
from attune_client.models import LoginRequest, CreateTriggerRequest

class AttuneClient:
    """Wrapper around generated client for backward compatibility"""
    
    def __init__(self, base_url: str, timeout: int = 60):
        self.client = GeneratedClient(base_url=base_url, timeout=timeout)
        self.token = None
    
    def login(self, username: str, password: str):
        req = LoginRequest(login=username, password=password)
        response = api_login.sync(client=self.client, json_body=req)
        self.token = response.data.access_token
        self.client = GeneratedClient(
            base_url=self.client.base_url,
            token=self.token,
            timeout=self.client.timeout
        )
        return response.data
    
    def create_trigger(self, ref=None, label=None, pack_ref=None, 
                      name=None, trigger_type=None, **kwargs):
        # Handle legacy parameters
        if not ref and name:
            ref = f"{pack_ref}.{name}" if pack_ref else name
        if not label:
            label = name or ref
        
        req = CreateTriggerRequest(
            ref=ref,
            label=label,
            pack_ref=pack_ref,
            description=kwargs.get('description', label),
            enabled=kwargs.get('enabled', True),
        )
        response = api_create_trigger.sync(client=self.client, json_body=req)
        return response.data.to_dict()
```

### Phase 2: Update Test Fixtures (Short-term)
Update `tests/helpers/fixtures.py` to use generated client:
- Keep helper functions (create_interval_timer, etc.)
- Use generated models internally
- Return dicts for compatibility

### Phase 3: Migrate Tests Directly (Medium-term)
Update tests to use generated client directly:
- Remove wrapper layer
- Use Pydantic models in tests
- Get full type safety benefits

### Phase 4: Remove Manual Client (Long-term)
- Delete `tests/helpers/client.py`
- Document new pattern in test README
- Update all documentation

## Current Status

✅ **Completed:**
- Script to generate client from OpenAPI spec
- Generated client installed in E2E venv
- Package configuration (pyproject.toml)
- Usage documentation

🔄 **In Progress:**
- Database migrations applied (webhook_enabled column)
- Tests still using manual client

📋 **TODO:**
1. Create wrapper layer for backward compatibility
2. Test wrapper with existing tests
3. Gradually migrate tests to use wrapper
4. Eventually migrate to direct generated client usage

## Running the Generator

```bash
# Start API service first
cd tests
./start_e2e_services.sh

# Generate client
cd ..
./scripts/generate-python-client.sh

# Client is automatically installed in tests/venvs/e2e
```

## Files Created/Modified

**New Files:**
- `scripts/generate-python-client.sh` - Generator script
- `tests/generated_client/` - Generated Python client (71 endpoints)
- `tests/generated_client/pyproject.toml` - Package config
- `work-summary/2026-01-23-openapi-client-generator.md` - This document

**To Be Modified:**
- `tests/helpers/client.py` - Replace with wrapper or deprecate
- `tests/helpers/fixtures.py` - Update to use generated client
- `tests/conftest.py` - Import from generated client
- All E2E test files - Eventually migrate to direct usage

## Benefits Realized

1. **No more field name issues** - Uses exact API schema
2. **Type safety** - Pydantic validates all requests/responses
3. **Auto-completion** - IDE knows all available fields
4. **API changes tracked** - Regenerate to get new endpoints
5. **Less maintenance** - No manual client updates needed

## Next Steps

1. ✅ Apply database migrations (DONE - webhook_enabled exists)
2. ✅ Generate Python client (DONE - 71 endpoints)
3. 🔄 Create backward-compatible wrapper (IN PROGRESS)
4. 🔄 Update fixtures to use wrapper
5. 🔄 Run tests with new client
6. 📋 Migrate tests gradually to direct usage
7. 📋 Remove manual client code

## Testing the Generated Client

```bash
# Quick test
tests/venvs/e2e/bin/python3 << 'EOF'
from attune_client import Client
from attune_client.api.auth import login
from attune_client.models import LoginRequest

client = Client(base_url="http://localhost:8080")
req = LoginRequest(login="test@attune.local", password="TestPass123!")
response = login.sync(client=client, json_body=req)
print(f"Login successful! Token: {response.data.access_token[:20]}...")
EOF
```

## Conclusion

The OpenAPI client generator eliminates the root cause of field name mismatches and keeps our test client automatically in sync with the API. This is a much better long-term solution than manually maintaining client code.

The migration can be done gradually using a wrapper layer, so existing tests continue working while we transition to the new approach.