# OpenAPI Client Generation

This document describes the auto-generated TypeScript API client for the Attune web frontend.

## Overview

The Attune frontend uses an **auto-generated TypeScript client** created from the backend's OpenAPI specification. This ensures:

- ✅ **Type Safety** - All API calls are fully typed
- ✅ **Schema Validation** - Frontend stays in sync with backend
- ✅ **Auto-completion** - Full IDE support for all endpoints
- ✅ **Reduced Errors** - Catch API mismatches at compile time
- ✅ **Automatic Updates** - Regenerate when backend changes

## Architecture

```
Backend (Rust)                Frontend (TypeScript)
──────────────                ─────────────────────
                              
OpenAPI Spec ─────────────────> Generated Client
   (/api-spec/openapi.json)       (web/src/api/)
                                           │
                                           ├── models/      (TypeScript types)
                                           ├── services/    (API methods)
                                           └── core/        (HTTP client)
```

## Generated Files

All files in `web/src/api/` are **auto-generated** from the OpenAPI spec:

```
web/src/api/
├── core/               # HTTP client internals
│   ├── OpenAPI.ts      # Configuration
│   ├── request.ts      # Request handler
│   └── ApiError.ts     # Error types
├── models/             # TypeScript types (90+ files)
│   ├── PackResponse.ts
│   ├── CreatePackRequest.ts
│   ├── ExecutionStatus.ts (enum)
│   └── ...
├── services/           # API service classes (13 files)
│   ├── AuthService.ts
│   ├── PacksService.ts
│   ├── ActionsService.ts
│   └── ...
└── index.ts            # Barrel exports
```

**⚠️ DO NOT EDIT THESE FILES** - They will be overwritten on regeneration.

## Configuration

### 1. OpenAPI Client Config (`web/src/lib/api-config.ts`)

```typescript
import { OpenAPI } from "../api";

// Set base URL
OpenAPI.BASE = import.meta.env.VITE_API_BASE_URL || 'http://localhost:8080';

// Configure JWT token injection
OpenAPI.TOKEN = async (): Promise<string> => {
  return localStorage.getItem("access_token") || "";
};

// Optional headers
OpenAPI.HEADERS = {
  "Content-Type": "application/json",
};
```

### 2. Import in Entry Point (`web/src/main.tsx`)

```typescript
import "./lib/api-config"; // Initialize OpenAPI client
```

This ensures the client is configured before any API calls are made.

## Usage

### Basic API Calls

```typescript
import { PacksService, AuthService } from '@/api';

// List packs
const packs = await PacksService.listPacks({ page: 1, pageSize: 50 });

// Login
const response = await AuthService.login({
  requestBody: { login: 'admin', password: 'secret' }
});

// Create pack
const pack = await PacksService.createPack({
  requestBody: {
    ref: 'my-pack',
    label: 'My Pack',
    description: 'A custom pack'
  }
});
```

### With React Query

```typescript
import { useQuery, useMutation } from '@tanstack/react-query';
import { PacksService } from '@/api';
import type { CreatePackRequest } from '@/api';

// Query
const { data, isLoading } = useQuery({
  queryKey: ['packs'],
  queryFn: () => PacksService.listPacks({ page: 1, pageSize: 50 })
});

// Mutation
const { mutate } = useMutation({
  mutationFn: (data: CreatePackRequest) => 
    PacksService.createPack({ requestBody: data }),
  onSuccess: () => {
    queryClient.invalidateQueries({ queryKey: ['packs'] });
  }
});
```

### Error Handling

```typescript
import { ApiError } from '@/api';

try {
  await PacksService.getPack({ ref: 'unknown' });
} catch (error) {
  if (error instanceof ApiError) {
    console.error(`API Error ${error.status}: ${error.message}`);
    console.error('Response:', error.body);
  }
}
```

## Regenerating the Client

When the backend API changes, regenerate the client to stay in sync.

### Prerequisites

1. **API server must be running:**
   ```bash
   cd attune/crates/api
   cargo run --bin attune-api
   ```

2. **Server listening on http://localhost:8080**

### Regeneration Steps

```bash
cd attune/web

# Regenerate from OpenAPI spec
npm run generate:api
```

This command:
1. Downloads `openapi.json` from `http://localhost:8080/api-spec/openapi.json`
2. Runs `openapi-typescript-codegen` to generate TypeScript code
3. Overwrites all files in `src/api/`

### After Regeneration

1. **Check for TypeScript errors:**
   ```bash
   npm run build
   ```

2. **Fix any breaking changes** in your code that uses the API

3. **Test the application:**
   ```bash
   npm run dev
   ```

## Available Services

| Service | Endpoints | Description |
|---------|-----------|-------------|
| **AuthService** | `/auth/*` | Authentication (login, register, refresh) |
| **PacksService** | `/api/v1/packs` | Pack CRUD operations |
| **ActionsService** | `/api/v1/actions` | Action management |
| **RulesService** | `/api/v1/rules` | Rule configuration |
| **TriggersService** | `/api/v1/triggers` | Trigger definitions |
| **SensorsService** | `/api/v1/sensors` | Sensor monitoring |
| **ExecutionsService** | `/api/v1/executions` | Execution tracking |
| **EventsService** | `/api/v1/events` | Event history |
| **InquiriesService** | `/api/v1/inquiries` | Human-in-the-loop workflows |
| **WorkflowsService** | `/api/v1/workflows` | Workflow orchestration |
| **HealthService** | `/health` | Health checks |
| **SecretsService** | `/api/v1/keys` | Secret management |
| **EnforcementsService** | `/api/v1/enforcements` | Rule enforcements |

## Type Definitions

All backend models have corresponding TypeScript types:

### Request Types
- `CreatePackRequest`
- `UpdatePackRequest`
- `CreateActionRequest`
- `LoginRequest`
- `RegisterRequest`
- etc.

### Response Types
- `PackResponse`
- `ActionResponse`
- `ExecutionResponse`
- `ApiResponse_PackResponse` (wrapped responses)
- `PaginatedResponse_PackSummary` (paginated lists)
- etc.

### Enums
- `ExecutionStatus` (`Requested`, `Running`, `Completed`, etc.)
- `EnforcementStatus`
- `InquiryStatus`
- `OwnerType`
- etc.

## Benefits Over Manual API Calls

| Manual Axios | Generated Client |
|--------------|------------------|
| ❌ No type safety | ✅ Full TypeScript types |
| ❌ Manual type definitions | ✅ Auto-generated from spec |
| ❌ Runtime errors | ✅ Compile-time validation |
| ❌ Out-of-sync schemas | ✅ Always matches backend |
| ❌ No auto-completion | ✅ Full IDE support |
| ❌ More code to write | ✅ Less boilerplate |

**Example - Schema Mismatch Caught at Compile Time:**

```typescript
// Manual call - runtime error!
await apiClient.post('/api/v1/packs', {
  name: 'my-pack',      // ❌ Wrong field (should be 'ref')
  system: false         // ❌ Wrong field (should be 'is_standard')
});

// Generated client - compile error!
await PacksService.createPack({
  requestBody: {
    name: 'my-pack',    // ❌ TypeScript error: Property 'name' does not exist
    ref: 'my-pack',     // ✅ Correct field
    is_standard: false  // ✅ Correct field
  }
});
```

## Troubleshooting

### Build Errors After Regeneration

**Symptom:** TypeScript errors after running `npm run generate:api`

**Cause:** Backend schema changed, breaking existing code

**Solution:**
1. Read error messages carefully
2. Update code to match new schema
3. Check backend OpenAPI spec at http://localhost:8080/docs

### "command not found: openapi-typescript-codegen"
### "openapi-typescript-codegen: command not found"

**Solution:**
```bash
# Ensure dependencies are installed
npm install

# The npm script already uses npx, but you can run manually:
npx openapi-typescript-codegen --input ./openapi.json --output ./src/api --client axios --useOptions
```

### Token Not Sent with Requests

**Symptom:** 401 Unauthorized errors despite being logged in

**Cause:** `api-config.ts` not imported

**Solution:** Ensure `import './lib/api-config'` is in `main.tsx`

### Cannot Fetch OpenAPI Spec

**Symptom:** Error downloading from `http://localhost:8080/api-spec/openapi.json`

**Solution:**
1. Start the API server: `cargo run --bin attune-api`
2. Verify server is running: `curl http://localhost:8080/health`
3. Check OpenAPI endpoint: `curl http://localhost:8080/api-spec/openapi.json`

## Development Workflow

1. **Make backend API changes** in Rust code
2. **Update OpenAPI annotations** (`#[utoipa::path(...)]`)
3. **Test backend** with Swagger UI at http://localhost:8080/docs
4. **Regenerate frontend client:** `npm run generate:api`
5. **Fix TypeScript errors** in frontend code
6. **Test integration** end-to-end

## Best Practices

1. ✅ **Always use generated services** - Don't make manual API calls
2. ✅ **Regenerate frequently** - Stay in sync with backend
3. ✅ **Use generated types** - Import from `@/api`, not manual definitions
4. ✅ **Create custom hooks** - Wrap services in React Query hooks
5. ✅ **Handle errors properly** - Use `ApiError` for typed error handling
6. ❌ **Never edit generated files** - Changes will be overwritten
7. ❌ **Don't duplicate types** - Reuse generated types

## Related Documentation

- **Migration Guide:** `web/MIGRATION-TO-GENERATED-CLIENT.md`
- **Generated Client README:** `web/src/api/README.md`
- **Backend OpenAPI Module:** `crates/api/src/openapi.rs`
- **Interactive API Docs:** http://localhost:8080/docs (Swagger UI)
- **OpenAPI Spec:** http://localhost:8080/api-spec/openapi.json

## Tools Used

- **openapi-typescript-codegen** v0.30.0 - Generates TypeScript client
- **Axios** - HTTP client (configured by generator)
- **utoipa** (Rust) - Generates OpenAPI spec from code annotations

## Summary

The auto-generated OpenAPI client provides **type-safe, schema-validated API access** for the Attune frontend. By regenerating the client whenever the backend changes, we ensure the frontend always matches the backend API contract, catching integration issues at compile time rather than runtime.