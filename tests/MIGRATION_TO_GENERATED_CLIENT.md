# Migration to Generated API Client

## Overview

The E2E tests are being migrated from a manually maintained `AttuneClient` to an auto-generated OpenAPI client. This migration improves:

- **Type Safety**: Full Pydantic models with compile-time type checking
- **API Schema Accuracy**: Client generated from OpenAPI spec matches API exactly
- **Maintainability**: No manual field mapping to keep in sync
- **Future-Proof**: Client regenerates automatically when API changes

## Current Status

✅ **Completed**:
- Generated Python client from OpenAPI spec (`tests/generated_client/`)
- Created backward-compatible wrapper (`tests/helpers/client_wrapper.py`)
- Updated dependencies (added `attrs`, `httpx`, `python-dateutil`)
- Updated `helpers/__init__.py` to use wrapper

🔄 **In Progress**:
- Testing wrapper compatibility with existing tests
- Fixing any edge cases in wrapper implementation

📋 **TODO**:
- Install updated dependencies in test venv
- Run Tier 1 E2E tests with new client
- Fix any compatibility issues discovered
- Gradually remove wrapper as tests adopt generated client directly
- Update documentation and examples

## Architecture

### Generated Client Structure

```
tests/generated_client/
├── api/                    # API endpoint modules
│   ├── actions/           # Action endpoints
│   ├── auth/              # Authentication endpoints
│   ├── enforcements/      # Enforcement endpoints
│   ├── events/            # Event endpoints
│   ├── executions/        # Execution endpoints
│   ├── health/            # Health check endpoints
│   ├── inquiries/         # Inquiry endpoints
│   ├── packs/             # Pack management endpoints
│   ├── rules/             # Rule endpoints
│   ├── secrets/           # Secret/key management endpoints
│   ├── sensors/           # Sensor endpoints
│   ├── triggers/          # Trigger endpoints
│   ├── webhooks/          # Webhook endpoints
│   └── workflows/         # Workflow endpoints
├── models/                # Pydantic models (200+ files)
├── client.py              # Client and AuthenticatedClient classes
├── errors.py              # Error types
├── types.py               # Helper types (UNSET, etc.)
└── pyproject.toml         # Package metadata

```

### Wrapper Architecture

The wrapper (`tests/helpers/client_wrapper.py`) provides backward compatibility:

1. **Same Interface**: Maintains exact same method signatures as old client
2. **Generated Backend**: Uses generated API functions internally
3. **Dict Conversion**: Converts Pydantic models to dicts for compatibility
4. **Auth Management**: Handles login/logout and token management
5. **ID to Ref Mapping**: API uses `ref` in paths, wrapper handles ID lookups

## Key Differences: Old vs New Client

### API Uses `ref` in Paths, Not `id`

**Old Behavior**:
```python
client.get_pack(pack_id=123)  # GET /api/v1/packs/123
```

**New Behavior**:
```python
# API expects: GET /api/v1/packs/{ref}
client.get_pack("core")  # GET /api/v1/packs/core
```

**Wrapper Solution**: Lists all items, finds by ID, then fetches by ref.

### Client Initialization

**Old**:
```python
client = AttuneClient(
    base_url="http://localhost:8080",
    timeout=30,
    auto_login=True
)
```

**New (Generated)**:
```python
from generated_client import Client, AuthenticatedClient

# Unauthenticated client
client = Client(base_url="http://localhost:8080/api/v1")

# Authenticated client
auth_client = AuthenticatedClient(
    base_url="http://localhost:8080/api/v1",
    token="access_token_here"
)
```

**Wrapper**: Maintains old interface, manages both clients internally.

### API Function Signatures

**Generated API Pattern**:
```python
# Positional args for path params, keyword-only for client and query params
from generated_client.api.packs import get_pack

response = get_pack.sync(
    ref="core",              # Positional: path parameter
    client=auth_client       # Keyword-only: client instance
)
```

### Response Handling

**Generated API Returns**:
- Pydantic models (e.g., `GetPackResponse200`)
- Models have `to_dict()` method
- Responses wrap data in `{"data": {...}}` structure

**Wrapper Converts**:
```python
response = gen_get_pack.sync(ref=ref, client=client)
if response:
    result = to_dict(response)  # Convert Pydantic to dict
    if isinstance(result, dict) and "data" in result:
        return result["data"]  # Unwrap data field
```

## Migration Path

### Phase 1: Wrapper Compatibility (Current)

Tests use existing `AttuneClient` interface, wrapper uses generated client:

```python
# Test code (unchanged)
from helpers import AttuneClient

client = AttuneClient()
pack = client.get_pack_by_ref("core")
```

### Phase 2: Direct Generated Client Usage (Future)

Tests migrate to use generated client directly:

```python
from generated_client import AuthenticatedClient
from generated_client.api.packs import get_pack

auth_client = AuthenticatedClient(
    base_url="http://localhost:8080/api/v1",
    token=access_token
)

response = get_pack.sync(ref="core", client=auth_client)
pack_data = response.data if response else None
```

### Phase 3: Wrapper Removal

Once all tests use generated client, remove wrapper and old client.

## Regenerating the Client

When API schema changes:

```bash
cd tests
./scripts/generate-python-client.sh
```

This script:
1. Fetches OpenAPI spec from running API
2. Generates client with `openapi-python-client`
3. Installs into test venv

## Common Issues & Solutions

### Issue: Import Errors

**Problem**: `ModuleNotFoundError: No module named 'attrs'`

**Solution**: Install updated dependencies:
```bash
cd tests
source venvs/e2e/bin/activate
pip install -r requirements.txt
```

### Issue: Field Name Mismatches

**Problem**: Test expects `name` but API returns `label`

**Solution**: API schema uses standardized fields:
- `ref`: Unique identifier (e.g., `core.echo`)
- `label`: Human-readable name
- `runtime`: Execution runtime (was `runner_type`)

Update test code to use correct field names.

### Issue: Path Parameter Confusion

**Problem**: API endpoint returns 404

**Solution**: Check if endpoint uses `ref` or `id` in path:
- Most endpoints: `/api/v1/{resource}/{ref}`
- Some endpoints: `/api/v1/{resource}/id/{id}`

Use wrapper methods that handle this automatically.

## Testing Strategy

1. **Run existing tests**: Verify wrapper maintains compatibility
2. **Check field names**: Ensure tests use correct schema fields
3. **Validate responses**: Confirm data structure matches expectations
4. **Test edge cases**: Error handling, pagination, filtering
5. **Performance check**: Ensure no significant slowdown

## Benefits of Migration

### Before (Manual Client)

**Pros**:
- Simple dict-based interface
- Easy to use in tests

**Cons**:
- Manual field mapping (out of sync with API)
- No type safety
- Frequent breakage on API changes
- Missing endpoints
- High maintenance burden

### After (Generated Client)

**Pros**:
- Always matches API schema
- Full type safety with Pydantic models
- All 71 endpoints included
- Auto-updates when API changes
- IDE autocomplete and type checking

**Cons**:
- Slightly more verbose
- Requires understanding Pydantic models
- Initial learning curve

## Next Steps

1. **Install Dependencies**:
   ```bash
   cd tests
   source venvs/e2e/bin/activate
   pip install -r requirements.txt
   ```

2. **Test Wrapper**:
   ```bash
   pytest tests/e2e/tier1/test_t1_01_interval_timer.py -v
   ```

3. **Fix Issues**: Address any compatibility problems found

4. **Expand Coverage**: Test all wrapper methods

5. **Document Patterns**: Create examples for common operations

6. **CI Integration**: Add client generation to CI pipeline

## Resources

- Generated Client: `tests/generated_client/`
- Wrapper Implementation: `tests/helpers/client_wrapper.py`
- API OpenAPI Spec: `http://localhost:8080/api-spec/openapi.json`
- Swagger UI: `http://localhost:8080/docs`
- Generator Tool: `openapi-python-client` (https://github.com/openapi-generators/openapi-python-client)

## Contact

For questions or issues with the migration:
- Review `work-summary/2026-01-23-openapi-client-generator.md`
- Check `PROBLEM.md` for known issues
- Test changes incrementally