# TODO: Execution-Scoped API Token Generation

**Priority:** High  
**Status:** Not Started  
**Related Work:** `work-summary/2026-02-07-env-var-standardization.md`  
**Blocked By:** None  
**Blocking:** Full API access from action executions

## Overview

Actions currently receive an empty `ATTUNE_API_TOKEN` environment variable. This TODO tracks the implementation of execution-scoped JWT token generation to enable actions to authenticate with the Attune API.

## Background

As of 2026-02-07, the environment variable standardization work updated the worker to provide standard environment variables to actions, including `ATTUNE_API_TOKEN`. However, token generation is not yet implemented - the variable is set to an empty string as a placeholder.

## Requirements

### Functional Requirements

1. **Token Generation**: Generate JWT tokens scoped to specific executions
2. **Token Claims**: Include execution-specific claims and permissions
3. **Token Lifecycle**: Tokens expire with execution or after timeout
4. **Security**: Tokens cannot access other executions or system resources
5. **Integration**: Seamlessly integrate into existing execution flow

### Non-Functional Requirements

1. **Performance**: Token generation should not significantly delay execution startup
2. **Security**: Follow JWT best practices and secure token scoping
3. **Consistency**: Match patterns from sensor token generation
4. **Testability**: Unit and integration tests for token generation and validation

## Design

### Token Claims Structure

```json
{
  "sub": "execution:12345",
  "identity_id": 42,
  "execution_id": 12345,
  "scopes": [
    "execution:read:self",
    "execution:create:child",
    "secrets:read:owned"
  ],
  "iat": 1738934400,
  "exp": 1738938000,
  "nbf": 1738934400
}
```

### Token Scopes

| Scope | Description | Use Case |
|-------|-------------|----------|
| `execution:read:self` | Read own execution data | Query execution status, retrieve parameters |
| `execution:create:child` | Create child executions | Workflow orchestration, sub-tasks |
| `secrets:read:owned` | Access secrets owned by execution identity | Retrieve API keys, credentials |

### Token Expiration

- **Default Expiration**: Execution timeout (from action metadata) or 5 minutes (300 seconds)
- **Maximum Expiration**: 1 hour (configurable)
- **Auto-Invalidation**: Token marked invalid when execution completes/fails/cancels

### Token Generation Flow

1. Executor receives execution request from queue
2. Executor loads action metadata (includes timeout)
3. Executor generates execution-scoped JWT token:
   - Subject: `execution:{id}`
   - Claims: execution ID, identity ID, scopes
   - Expiration: now + timeout or max lifetime
4. Token added to environment variables (`ATTUNE_API_TOKEN`)
5. Action script uses token for API authentication

## Implementation Tasks

### Phase 1: Token Generation Service

- [ ] Create `TokenService` or add to existing auth service
- [ ] Implement `generate_execution_token(execution_id, identity_id, timeout)` method
- [ ] Use same JWT signing key as API service
- [ ] Add token generation to `ActionExecutor::prepare_execution_context()`
- [ ] Replace empty string with generated token

**Files to Modify:**
- `crates/common/src/auth.rs` (or create new token module)
- `crates/worker/src/executor.rs` (line ~220)

**Estimated Effort:** 4-6 hours

### Phase 2: Token Validation

- [ ] Update API auth middleware to recognize execution-scoped tokens
- [ ] Validate token scopes against requested resources
- [ ] Ensure execution tokens cannot access other executions
- [ ] Add scope checking to protected endpoints

**Files to Modify:**
- `crates/api/src/auth/middleware.rs`
- `crates/api/src/auth/jwt.rs`

**Estimated Effort:** 3-4 hours

### Phase 3: Token Lifecycle Management

- [ ] Track active execution tokens in memory or cache
- [ ] Invalidate tokens when execution completes
- [ ] Handle token refresh (if needed for long-running actions)
- [ ] Add cleanup for orphaned tokens

**Files to Modify:**
- `crates/worker/src/executor.rs`
- Consider adding token registry/cache

**Estimated Effort:** 2-3 hours

### Phase 4: Testing

- [ ] Unit tests for token generation
- [ ] Unit tests for token validation and scope checking
- [ ] Integration test: action calls API with generated token
- [ ] Integration test: verify token cannot access other executions
- [ ] Integration test: verify token expires appropriately
- [ ] Test child execution creation with token

**Files to Create:**
- `crates/worker/tests/token_generation_tests.rs`
- `crates/api/tests/execution_token_auth_tests.rs`

**Estimated Effort:** 4-5 hours

### Phase 5: Documentation

- [ ] Document token generation in worker architecture docs
- [ ] Update QUICKREF-execution-environment.md with token details
- [ ] Add security considerations to documentation
- [ ] Provide examples of actions using API with token
- [ ] Document troubleshooting for token-related issues

**Files to Update:**
- `docs/QUICKREF-execution-environment.md`
- `docs/architecture/worker-service.md`
- `docs/authentication/authentication.md`
- `packs/core/actions/README.md` (add API usage examples)

**Estimated Effort:** 2-3 hours

## Technical Details

### JWT Signing

Use the same JWT secret as the API service:

```rust
use jsonwebtoken::{encode, EncodingKey, Header};

let token = encode(
    &Header::default(),
    &claims,
    &EncodingKey::from_secret(jwt_secret.as_bytes()),
)?;
```

### Token Structure Reference

Look at sensor token generation in `crates/sensor/src/api_client.rs` for patterns:
- Similar claims structure
- Similar expiration handling
- Can reuse token generation utilities

### Middleware Integration

Update `RequireAuth` extractor to handle execution-scoped tokens:

```rust
// Pseudo-code
match token_subject_type {
    "user" => validate_user_token(token),
    "service_account" => validate_service_token(token),
    "execution" => validate_execution_token(token, execution_id_from_route),
}
```

### Scope Validation

Add scope checking helper:

```rust
fn require_scope(token: &Token, required_scope: &str) -> Result<()> {
    if token.scopes.contains(&required_scope.to_string()) {
        Ok(())
    } else {
        Err(Error::Forbidden("Insufficient scope"))
    }
}
```

## Security Considerations

### Token Scoping

1. **Execution Isolation**: Token must only access its own execution
2. **No System Access**: Cannot modify system configuration
3. **Limited Secrets**: Only secrets owned by execution identity
4. **Time-Bounded**: Expires with execution or timeout

### Attack Vectors to Prevent

1. **Token Reuse**: Expired tokens must be rejected
2. **Cross-Execution Access**: Token for execution A cannot access execution B
3. **Privilege Escalation**: Cannot use token to gain admin access
4. **Token Leakage**: Never log full token value

### Validation Checklist

- [ ] Token signature verified
- [ ] Token not expired
- [ ] Execution ID matches token claims
- [ ] Required scopes present in token
- [ ] Identity owns requested resources

## Testing Strategy

### Unit Tests

```rust
#[test]
fn test_generate_execution_token() {
    let token = generate_execution_token(12345, 42, 300).unwrap();
    let claims = decode_token(&token).unwrap();
    
    assert_eq!(claims.execution_id, 12345);
    assert_eq!(claims.identity_id, 42);
    assert!(claims.scopes.contains(&"execution:read:self".to_string()));
}

#[test]
fn test_token_cannot_access_other_execution() {
    let token = generate_execution_token(12345, 42, 300).unwrap();
    
    // Try to access execution 99999 with token for execution 12345
    let result = api_client.get_execution(99999, &token).await;
    assert!(result.is_err());
}
```

### Integration Tests

1. **Happy Path**: Action successfully calls API with token
2. **Scope Enforcement**: Action cannot perform unauthorized operations
3. **Token Expiration**: Expired token is rejected
4. **Child Execution**: Action can create child execution with token

## Dependencies

### Required Access

- JWT secret (same as API service)
- Access to execution data (for claims)
- Access to identity data (for ownership checks)

### Configuration

Add to worker config (or use existing values):

```yaml
security:
  jwt_secret: "..." # Shared with API
  execution_token_max_lifetime: 3600 # 1 hour
```

## Success Criteria

1. ✅ Actions receive valid JWT token in `ATTUNE_API_TOKEN`
2. ✅ Actions can authenticate with API using token
3. ✅ Token scopes are enforced correctly
4. ✅ Tokens cannot access other executions
5. ✅ Tokens expire appropriately
6. ✅ All tests pass
7. ✅ Documentation is complete and accurate

## References

- [Environment Variable Standardization](../work-summary/2026-02-07-env-var-standardization.md) - Background and context
- [QUICKREF: Execution Environment](./QUICKREF-execution-environment.md) - Token usage documentation
- [Worker Service Architecture](./architecture/worker-service.md) - Executor implementation details
- [Authentication Documentation](./authentication/authentication.md) - JWT patterns and security
- Sensor Token Generation: `crates/sensor/src/api_client.rs` - Reference implementation

## Estimated Total Effort

**Total:** 15-21 hours (approximately 2-3 days of focused work)

## Notes

- Consider reusing token generation utilities from API service
- Ensure consistency with sensor token generation patterns
- Document security model clearly for pack developers
- Add examples to core pack showing API usage from actions