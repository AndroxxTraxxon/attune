//! Rule management API routes

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use std::sync::Arc;
use tracing::{info, warn};
use validator::Validate;

use attune_common::mq::{
    MessageEnvelope, MessageType, RuleCreatedPayload, RuleDisabledPayload, RuleEnabledPayload,
};
use attune_common::repositories::{
    action::ActionRepository,
    pack::PackRepository,
    rule::{CreateRuleInput, RuleRepository, RuleSearchFilters, UpdateRuleInput},
    trigger::TriggerRepository,
    Create, Delete, FindByRef, Update,
};

use crate::{
    auth::middleware::RequireAuth,
    dto::{
        common::{PaginatedResponse, PaginationParams},
        rule::{CreateRuleRequest, RuleResponse, RuleSummary, UpdateRuleRequest},
        ApiResponse, SuccessResponse,
    },
    middleware::{ApiError, ApiResult},
    state::AppState,
    validation::{validate_action_params, validate_trigger_params},
};

/// List all rules with pagination
#[utoipa::path(
    get,
    path = "/api/v1/rules",
    tag = "rules",
    params(PaginationParams),
    responses(
        (status = 200, description = "List of rules", body = PaginatedResponse<RuleSummary>),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn list_rules(
    State(state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
    Query(pagination): Query<PaginationParams>,
) -> ApiResult<impl IntoResponse> {
    let filters = RuleSearchFilters {
        pack: None,
        action: None,
        trigger: None,
        enabled: None,
        limit: pagination.limit(),
        offset: pagination.offset(),
    };

    let result = RuleRepository::list_search(&state.db, &filters).await?;

    let paginated_rules: Vec<RuleSummary> =
        result.rows.into_iter().map(RuleSummary::from).collect();

    let response = PaginatedResponse::new(paginated_rules, &pagination, result.total);

    Ok((StatusCode::OK, Json(response)))
}

/// List enabled rules
#[utoipa::path(
    get,
    path = "/api/v1/rules/enabled",
    tag = "rules",
    params(PaginationParams),
    responses(
        (status = 200, description = "List of enabled rules", body = PaginatedResponse<RuleSummary>),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn list_enabled_rules(
    State(state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
    Query(pagination): Query<PaginationParams>,
) -> ApiResult<impl IntoResponse> {
    let filters = RuleSearchFilters {
        pack: None,
        action: None,
        trigger: None,
        enabled: Some(true),
        limit: pagination.limit(),
        offset: pagination.offset(),
    };

    let result = RuleRepository::list_search(&state.db, &filters).await?;

    let paginated_rules: Vec<RuleSummary> =
        result.rows.into_iter().map(RuleSummary::from).collect();

    let response = PaginatedResponse::new(paginated_rules, &pagination, result.total);

    Ok((StatusCode::OK, Json(response)))
}

/// List rules by pack reference
#[utoipa::path(
    get,
    path = "/api/v1/packs/{pack_ref}/rules",
    tag = "rules",
    params(
        ("pack_ref" = String, Path, description = "Pack reference"),
        PaginationParams
    ),
    responses(
        (status = 200, description = "List of rules in pack", body = PaginatedResponse<RuleSummary>),
        (status = 404, description = "Pack not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn list_rules_by_pack(
    State(state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
    Path(pack_ref): Path<String>,
    Query(pagination): Query<PaginationParams>,
) -> ApiResult<impl IntoResponse> {
    // Verify pack exists
    let pack = PackRepository::find_by_ref(&state.db, &pack_ref)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Pack '{}' not found", pack_ref)))?;

    let filters = RuleSearchFilters {
        pack: Some(pack.id),
        action: None,
        trigger: None,
        enabled: None,
        limit: pagination.limit(),
        offset: pagination.offset(),
    };

    let result = RuleRepository::list_search(&state.db, &filters).await?;

    let paginated_rules: Vec<RuleSummary> =
        result.rows.into_iter().map(RuleSummary::from).collect();

    let response = PaginatedResponse::new(paginated_rules, &pagination, result.total);

    Ok((StatusCode::OK, Json(response)))
}

/// List rules by action reference
#[utoipa::path(
    get,
    path = "/api/v1/actions/{action_ref}/rules",
    tag = "rules",
    params(
        ("action_ref" = String, Path, description = "Action reference"),
        PaginationParams
    ),
    responses(
        (status = 200, description = "List of rules using this action", body = PaginatedResponse<RuleSummary>),
        (status = 404, description = "Action not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn list_rules_by_action(
    State(state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
    Path(action_ref): Path<String>,
    Query(pagination): Query<PaginationParams>,
) -> ApiResult<impl IntoResponse> {
    // Verify action exists
    let action = ActionRepository::find_by_ref(&state.db, &action_ref)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Action '{}' not found", action_ref)))?;

    let filters = RuleSearchFilters {
        pack: None,
        action: Some(action.id),
        trigger: None,
        enabled: None,
        limit: pagination.limit(),
        offset: pagination.offset(),
    };

    let result = RuleRepository::list_search(&state.db, &filters).await?;

    let paginated_rules: Vec<RuleSummary> =
        result.rows.into_iter().map(RuleSummary::from).collect();

    let response = PaginatedResponse::new(paginated_rules, &pagination, result.total);

    Ok((StatusCode::OK, Json(response)))
}

/// List rules by trigger reference
#[utoipa::path(
    get,
    path = "/api/v1/triggers/{trigger_ref}/rules",
    tag = "rules",
    params(
        ("trigger_ref" = String, Path, description = "Trigger reference"),
        PaginationParams
    ),
    responses(
        (status = 200, description = "List of rules using this trigger", body = PaginatedResponse<RuleSummary>),
        (status = 404, description = "Trigger not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn list_rules_by_trigger(
    State(state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
    Path(trigger_ref): Path<String>,
    Query(pagination): Query<PaginationParams>,
) -> ApiResult<impl IntoResponse> {
    // Verify trigger exists
    let trigger = TriggerRepository::find_by_ref(&state.db, &trigger_ref)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Trigger '{}' not found", trigger_ref)))?;

    let filters = RuleSearchFilters {
        pack: None,
        action: None,
        trigger: Some(trigger.id),
        enabled: None,
        limit: pagination.limit(),
        offset: pagination.offset(),
    };

    let result = RuleRepository::list_search(&state.db, &filters).await?;

    let paginated_rules: Vec<RuleSummary> =
        result.rows.into_iter().map(RuleSummary::from).collect();

    let response = PaginatedResponse::new(paginated_rules, &pagination, result.total);

    Ok((StatusCode::OK, Json(response)))
}

/// Get a single rule by reference
#[utoipa::path(
    get,
    path = "/api/v1/rules/{ref}",
    tag = "rules",
    params(
        ("ref" = String, Path, description = "Rule reference")
    ),
    responses(
        (status = 200, description = "Rule details", body = ApiResponse<RuleResponse>),
        (status = 404, description = "Rule not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn get_rule(
    State(state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
    Path(rule_ref): Path<String>,
) -> ApiResult<impl IntoResponse> {
    let rule = RuleRepository::find_by_ref(&state.db, &rule_ref)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Rule '{}' not found", rule_ref)))?;

    let response = ApiResponse::new(RuleResponse::from(rule));

    Ok((StatusCode::OK, Json(response)))
}

/// Create a new rule
#[utoipa::path(
    post,
    path = "/api/v1/rules",
    tag = "rules",
    request_body = CreateRuleRequest,
    responses(
        (status = 201, description = "Rule created successfully", body = ApiResponse<RuleResponse>),
        (status = 400, description = "Invalid request"),
        (status = 404, description = "Pack, action, or trigger not found"),
        (status = 409, description = "Rule with same ref already exists"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn create_rule(
    State(state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
    Json(request): Json<CreateRuleRequest>,
) -> ApiResult<impl IntoResponse> {
    // Validate request
    request.validate()?;

    // Check if rule with same ref already exists
    if RuleRepository::find_by_ref(&state.db, &request.r#ref)
        .await?
        .is_some()
    {
        return Err(ApiError::Conflict(format!(
            "Rule with ref '{}' already exists",
            request.r#ref
        )));
    }

    // Verify pack exists and get its ID
    let pack = PackRepository::find_by_ref(&state.db, &request.pack_ref)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Pack '{}' not found", request.pack_ref)))?;

    // Verify action exists and get its ID
    let action = ActionRepository::find_by_ref(&state.db, &request.action_ref)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Action '{}' not found", request.action_ref)))?;

    // Verify trigger exists and get its ID
    let trigger = TriggerRepository::find_by_ref(&state.db, &request.trigger_ref)
        .await?
        .ok_or_else(|| {
            ApiError::NotFound(format!("Trigger '{}' not found", request.trigger_ref))
        })?;

    // Validate trigger parameters against schema
    validate_trigger_params(&trigger, &request.trigger_params)?;

    // Validate action parameters against schema
    validate_action_params(&action, &request.action_params)?;

    // Create rule input
    let rule_input = CreateRuleInput {
        r#ref: request.r#ref,
        pack: pack.id,
        pack_ref: pack.r#ref.clone(),
        label: request.label,
        description: request.description,
        action: action.id,
        action_ref: action.r#ref.clone(),
        trigger: trigger.id,
        trigger_ref: trigger.r#ref.clone(),
        conditions: request.conditions,
        action_params: request.action_params,
        trigger_params: request.trigger_params,
        enabled: request.enabled,
        is_adhoc: true, // Rules created via API are ad-hoc (not from pack installation)
    };

    let rule = RuleRepository::create(&state.db, rule_input).await?;

    // Publish RuleCreated message to notify sensor service
    if let Some(publisher) = state.get_publisher().await {
        let payload = RuleCreatedPayload {
            rule_id: rule.id,
            rule_ref: rule.r#ref.clone(),
            trigger_id: rule.trigger,
            trigger_ref: rule.trigger_ref.clone(),
            action_id: rule.action,
            action_ref: rule.action_ref.clone(),
            trigger_params: Some(rule.trigger_params.clone()),
            enabled: rule.enabled,
        };

        let envelope =
            MessageEnvelope::new(MessageType::RuleCreated, payload).with_source("api-service");

        if let Err(e) = publisher.publish_envelope(&envelope).await {
            warn!(
                "Failed to publish RuleCreated message for rule {}: {}",
                rule.r#ref, e
            );
        } else {
            info!("Published RuleCreated message for rule {}", rule.r#ref);
        }
    }

    let response = ApiResponse::with_message(RuleResponse::from(rule), "Rule created successfully");

    Ok((StatusCode::CREATED, Json(response)))
}

/// Update an existing rule
#[utoipa::path(
    put,
    path = "/api/v1/rules/{ref}",
    tag = "rules",
    params(
        ("ref" = String, Path, description = "Rule reference")
    ),
    request_body = UpdateRuleRequest,
    responses(
        (status = 200, description = "Rule updated successfully", body = ApiResponse<RuleResponse>),
        (status = 400, description = "Invalid request"),
        (status = 404, description = "Rule not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn update_rule(
    State(state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
    Path(rule_ref): Path<String>,
    Json(request): Json<UpdateRuleRequest>,
) -> ApiResult<impl IntoResponse> {
    // Validate request
    request.validate()?;

    // Check if rule exists
    let existing_rule = RuleRepository::find_by_ref(&state.db, &rule_ref)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Rule '{}' not found", rule_ref)))?;

    // If action parameters are being updated, validate against the action's schema
    if let Some(ref action_params) = request.action_params {
        let action = ActionRepository::find_by_ref(&state.db, &existing_rule.action_ref)
            .await?
            .ok_or_else(|| {
                ApiError::NotFound(format!("Action '{}' not found", existing_rule.action_ref))
            })?;
        validate_action_params(&action, action_params)?;
    }

    // If trigger parameters are being updated, validate against the trigger's schema
    if let Some(ref trigger_params) = request.trigger_params {
        let trigger = TriggerRepository::find_by_ref(&state.db, &existing_rule.trigger_ref)
            .await?
            .ok_or_else(|| {
                ApiError::NotFound(format!("Trigger '{}' not found", existing_rule.trigger_ref))
            })?;
        validate_trigger_params(&trigger, trigger_params)?;
    }

    // Track if trigger params changed
    let trigger_params_changed = request.trigger_params.is_some()
        && request.trigger_params != Some(existing_rule.trigger_params.clone());

    // Create update input
    let update_input = UpdateRuleInput {
        label: request.label,
        description: request.description,
        conditions: request.conditions,
        action_params: request.action_params,
        trigger_params: request.trigger_params,
        enabled: request.enabled,
    };

    let rule = RuleRepository::update(&state.db, existing_rule.id, update_input).await?;

    // If the rule is enabled and trigger params changed, publish RuleEnabled message
    // to notify sensors to restart with new parameters
    if rule.enabled && trigger_params_changed {
        if let Some(publisher) = state.get_publisher().await {
            let payload = RuleEnabledPayload {
                rule_id: rule.id,
                rule_ref: rule.r#ref.clone(),
                trigger_ref: rule.trigger_ref.clone(),
                trigger_params: Some(rule.trigger_params.clone()),
            };

            let envelope =
                MessageEnvelope::new(MessageType::RuleEnabled, payload).with_source("api-service");

            if let Err(e) = publisher.publish_envelope(&envelope).await {
                warn!(
                    "Failed to publish RuleEnabled message for updated rule {}: {}",
                    rule.r#ref, e
                );
            } else {
                info!(
                    "Published RuleEnabled message for updated rule {} (trigger params changed)",
                    rule.r#ref
                );
            }
        }
    }

    let response = ApiResponse::with_message(RuleResponse::from(rule), "Rule updated successfully");

    Ok((StatusCode::OK, Json(response)))
}

/// Delete a rule
#[utoipa::path(
    delete,
    path = "/api/v1/rules/{ref}",
    tag = "rules",
    params(
        ("ref" = String, Path, description = "Rule reference")
    ),
    responses(
        (status = 200, description = "Rule deleted successfully", body = SuccessResponse),
        (status = 404, description = "Rule not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn delete_rule(
    State(state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
    Path(rule_ref): Path<String>,
) -> ApiResult<impl IntoResponse> {
    // Check if rule exists
    let rule = RuleRepository::find_by_ref(&state.db, &rule_ref)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Rule '{}' not found", rule_ref)))?;

    // Delete the rule
    let deleted = RuleRepository::delete(&state.db, rule.id).await?;

    if !deleted {
        return Err(ApiError::NotFound(format!("Rule '{}' not found", rule_ref)));
    }

    let response = SuccessResponse::new(format!("Rule '{}' deleted successfully", rule_ref));

    Ok((StatusCode::OK, Json(response)))
}

/// Enable a rule
#[utoipa::path(
    post,
    path = "/api/v1/rules/{ref}/enable",
    tag = "rules",
    params(
        ("ref" = String, Path, description = "Rule reference")
    ),
    responses(
        (status = 200, description = "Rule enabled successfully", body = ApiResponse<RuleResponse>),
        (status = 404, description = "Rule not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn enable_rule(
    State(state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
    Path(rule_ref): Path<String>,
) -> ApiResult<impl IntoResponse> {
    // Check if rule exists
    let existing_rule = RuleRepository::find_by_ref(&state.db, &rule_ref)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Rule '{}' not found", rule_ref)))?;

    // Update rule to enabled
    let update_input = UpdateRuleInput {
        label: None,
        description: None,
        conditions: None,
        action_params: None,
        trigger_params: None,
        enabled: Some(true),
    };

    let rule = RuleRepository::update(&state.db, existing_rule.id, update_input).await?;

    // Publish RuleEnabled message to notify sensor service
    if let Some(publisher) = state.get_publisher().await {
        let payload = RuleEnabledPayload {
            rule_id: rule.id,
            rule_ref: rule.r#ref.clone(),
            trigger_ref: rule.trigger_ref.clone(),
            trigger_params: Some(rule.trigger_params.clone()),
        };

        let envelope =
            MessageEnvelope::new(MessageType::RuleEnabled, payload).with_source("api-service");

        if let Err(e) = publisher.publish_envelope(&envelope).await {
            warn!(
                "Failed to publish RuleEnabled message for rule {}: {}",
                rule.r#ref, e
            );
        } else {
            info!("Published RuleEnabled message for rule {}", rule.r#ref);
        }
    }

    let response = ApiResponse::with_message(RuleResponse::from(rule), "Rule enabled successfully");

    Ok((StatusCode::OK, Json(response)))
}

/// Disable a rule
#[utoipa::path(
    post,
    path = "/api/v1/rules/{ref}/disable",
    tag = "rules",
    params(
        ("ref" = String, Path, description = "Rule reference")
    ),
    responses(
        (status = 200, description = "Rule disabled successfully", body = ApiResponse<RuleResponse>),
        (status = 404, description = "Rule not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn disable_rule(
    State(state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
    Path(rule_ref): Path<String>,
) -> ApiResult<impl IntoResponse> {
    // Check if rule exists
    let existing_rule = RuleRepository::find_by_ref(&state.db, &rule_ref)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Rule '{}' not found", rule_ref)))?;

    // Update rule to disabled
    let update_input = UpdateRuleInput {
        label: None,
        description: None,
        conditions: None,
        action_params: None,
        trigger_params: None,
        enabled: Some(false),
    };

    let rule = RuleRepository::update(&state.db, existing_rule.id, update_input).await?;

    // Publish RuleDisabled message to notify sensor service
    if let Some(publisher) = state.get_publisher().await {
        let payload = RuleDisabledPayload {
            rule_id: rule.id,
            rule_ref: rule.r#ref.clone(),
            trigger_ref: rule.trigger_ref.clone(),
        };

        let envelope =
            MessageEnvelope::new(MessageType::RuleDisabled, payload).with_source("api-service");

        if let Err(e) = publisher.publish_envelope(&envelope).await {
            warn!(
                "Failed to publish RuleDisabled message for rule {}: {}",
                rule.r#ref, e
            );
        } else {
            info!("Published RuleDisabled message for rule {}", rule.r#ref);
        }
    }

    let response =
        ApiResponse::with_message(RuleResponse::from(rule), "Rule disabled successfully");

    Ok((StatusCode::OK, Json(response)))
}

/// Create rule routes
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/rules", get(list_rules).post(create_rule))
        .route("/rules/enabled", get(list_enabled_rules))
        .route(
            "/rules/{ref}",
            get(get_rule).put(update_rule).delete(delete_rule),
        )
        .route("/rules/{ref}/enable", post(enable_rule))
        .route("/rules/{ref}/disable", post(disable_rule))
        .route("/packs/{pack_ref}/rules", get(list_rules_by_pack))
        .route("/actions/{action_ref}/rules", get(list_rules_by_action))
        .route("/triggers/{trigger_ref}/rules", get(list_rules_by_trigger))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rule_routes_structure() {
        // Just verify the router can be constructed
        let _router = routes();
    }
}
