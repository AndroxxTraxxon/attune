//! RBAC authorization service for API handlers.
//!
//! This module evaluates grants assigned to user identities via
//! `permission_set` and `permission_assignment`.

use crate::{
    auth::{jwt::TokenType, middleware::AuthenticatedUser},
    middleware::ApiError,
};
use attune_common::{
    rbac::{Action, AuthorizationContext, Grant, Resource},
    repositories::{
        identity::{IdentityRepository, IdentityRoleAssignmentRepository, PermissionSetRepository},
        FindById,
    },
};
use sqlx::PgPool;

#[derive(Debug, Clone)]
pub struct AuthorizationCheck {
    pub resource: Resource,
    pub action: Action,
    pub context: AuthorizationContext,
}

#[derive(Clone)]
pub struct AuthorizationService {
    db: PgPool,
}

impl AuthorizationService {
    pub fn new(db: PgPool) -> Self {
        Self { db }
    }

    pub async fn authorize(
        &self,
        user: &AuthenticatedUser,
        mut check: AuthorizationCheck,
    ) -> Result<(), ApiError> {
        // Sensor and Refresh tokens have dedicated scope checks elsewhere and
        // are not subject to identity-based RBAC.
        //
        // Access and Execution tokens both carry an identity in `sub` and are
        // evaluated through identity RBAC: Access = "user logged in via the
        // UI/CLI"; Execution = "callback from a running action, scoped to the
        // identity that triggered it". Execution-scoped tokens are additionally
        // restricted by the execution scope itself (e.g., the worker's writes
        // to `/api/v1/executions/{id}/...` validate the token's execution_id
        // matches the path), but at the resource/action level they get the
        // permissions of the triggering identity — never more.
        match user.claims.token_type {
            TokenType::Access | TokenType::Execution => {}
            _ => return Ok(()),
        }

        let identity_id = user.identity_id().map_err(|_| {
            ApiError::Unauthorized("Invalid authentication subject in token".to_string())
        })?;

        // Ensure identity exists and load identity attributes used by attribute constraints.
        let identity = IdentityRepository::find_by_id(&self.db, identity_id)
            .await?
            .ok_or_else(|| ApiError::Unauthorized("Identity not found".to_string()))?;

        check.context.identity_id = identity_id;
        check.context.identity_attributes = match identity.attributes {
            serde_json::Value::Object(map) => map.into_iter().collect(),
            _ => Default::default(),
        };

        let grants = self.load_effective_grants(identity_id).await?;

        let allowed = Self::is_allowed(&grants, check.resource, check.action, &check.context);

        if !allowed {
            return Err(ApiError::Forbidden(format!(
                "Insufficient permissions: {}:{}",
                resource_name(check.resource),
                action_name(check.action)
            )));
        }

        Ok(())
    }

    pub async fn effective_grants(&self, user: &AuthenticatedUser) -> Result<Vec<Grant>, ApiError> {
        match user.claims.token_type {
            TokenType::Access | TokenType::Execution => {}
            _ => return Ok(Vec::new()),
        }

        let identity_id = user.identity_id().map_err(|_| {
            ApiError::Unauthorized("Invalid authentication subject in token".to_string())
        })?;
        self.load_effective_grants(identity_id).await
    }

    pub fn is_allowed(
        grants: &[Grant],
        resource: Resource,
        action: Action,
        context: &AuthorizationContext,
    ) -> bool {
        grants.iter().any(|g| g.allows(resource, action, context))
    }

    async fn load_effective_grants(&self, identity_id: i64) -> Result<Vec<Grant>, ApiError> {
        let mut permission_sets =
            PermissionSetRepository::find_by_identity(&self.db, identity_id).await?;
        let roles =
            IdentityRoleAssignmentRepository::find_role_names_by_identity(&self.db, identity_id)
                .await?;
        let role_permission_sets = PermissionSetRepository::find_by_roles(&self.db, &roles).await?;
        permission_sets.extend(role_permission_sets);

        let mut seen_permission_sets = std::collections::HashSet::new();
        permission_sets.retain(|permission_set| seen_permission_sets.insert(permission_set.id));

        let mut grants = Vec::new();
        for permission_set in permission_sets {
            let set_grants: Vec<Grant> =
                serde_json::from_value(permission_set.grants).map_err(|e| {
                    ApiError::InternalServerError(format!(
                        "Invalid grant schema in permission set '{}': {}",
                        permission_set.r#ref, e
                    ))
                })?;
            grants.extend(set_grants);
        }

        Ok(grants)
    }
}

fn resource_name(resource: Resource) -> &'static str {
    match resource {
        Resource::Packs => "packs",
        Resource::Actions => "actions",
        Resource::Queues => "queues",
        Resource::Rules => "rules",
        Resource::Triggers => "triggers",
        Resource::Executions => "executions",
        Resource::Events => "events",
        Resource::Enforcements => "enforcements",
        Resource::Inquiries => "inquiries",
        Resource::Keys => "keys",
        Resource::Artifacts => "artifacts",
        Resource::Identities => "identities",
        Resource::Permissions => "permissions",
        Resource::AuditLog => "audit_log",
    }
}

fn action_name(action: Action) -> &'static str {
    match action {
        Action::Read => "read",
        Action::Create => "create",
        Action::Update => "update",
        Action::Delete => "delete",
        Action::Execute => "execute",
        Action::Cancel => "cancel",
        Action::Respond => "respond",
        Action::Manage => "manage",
        Action::Decrypt => "decrypt",
    }
}
