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
        identity::{IdentityRepository, PermissionSetRepository},
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
        // Non-access tokens are governed by dedicated scope checks in route logic.
        // They are not evaluated through identity RBAC grants.
        if user.claims.token_type != TokenType::Access {
            return Ok(());
        }

        let identity_id = user.identity_id().map_err(|_| {
            ApiError::Unauthorized("Invalid authentication subject in access token".to_string())
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
        if user.claims.token_type != TokenType::Access {
            return Ok(Vec::new());
        }

        let identity_id = user.identity_id().map_err(|_| {
            ApiError::Unauthorized("Invalid authentication subject in access token".to_string())
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
        let permission_sets =
            PermissionSetRepository::find_by_identity(&self.db, identity_id).await?;

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
        Resource::Rules => "rules",
        Resource::Triggers => "triggers",
        Resource::Executions => "executions",
        Resource::Events => "events",
        Resource::Enforcements => "enforcements",
        Resource::Inquiries => "inquiries",
        Resource::Keys => "keys",
        Resource::Artifacts => "artifacts",
        Resource::Workflows => "workflows",
        Resource::Webhooks => "webhooks",
        Resource::Analytics => "analytics",
        Resource::History => "history",
        Resource::Identities => "identities",
        Resource::Permissions => "permissions",
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
    }
}
