//! Event and Enforcement query API routes

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::sync::Arc;
use utoipa::ToSchema;
use validator::Validate;

use attune_common::{
    mq::{EventCreatedPayload, MessageEnvelope, MessageType},
    repositories::{
        event::{CreateEventInput, EnforcementRepository, EventRepository},
        trigger::TriggerRepository,
        Create, FindById, FindByRef, List,
    },
};

use crate::auth::RequireAuth;
use crate::{
    dto::{
        common::{PaginatedResponse, PaginationParams},
        event::{
            EnforcementQueryParams, EnforcementResponse, EnforcementSummary, EventQueryParams,
            EventResponse, EventSummary,
        },
        ApiResponse,
    },
    middleware::{ApiError, ApiResult},
    state::AppState,
};

/// Request body for creating an event
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct CreateEventRequest {
    /// Trigger reference (e.g., "core.timer", "core.webhook")
    /// Also accepts "trigger_type" for compatibility with the sensor interface spec.
    #[validate(length(min = 1))]
    #[serde(alias = "trigger_type")]
    #[schema(example = "core.timer")]
    pub trigger_ref: String,

    /// Event payload data
    #[schema(value_type = Object, example = json!({"timestamp": "2024-01-13T10:30:00Z"}))]
    pub payload: Option<JsonValue>,

    /// Event configuration
    #[schema(value_type = Object)]
    pub config: Option<JsonValue>,

    /// Trigger instance ID (for correlation, often rule_id)
    #[schema(example = "rule_123")]
    pub trigger_instance_id: Option<String>,
}

/// Create a new event
#[utoipa::path(
    post,
    path = "/api/v1/events",
    tag = "events",
    request_body = CreateEventRequest,
    security(("bearer_auth" = [])),
    responses(
        (status = 201, description = "Event created successfully", body = ApiResponse<EventResponse>),
        (status = 400, description = "Validation error"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Trigger not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn create_event(
    user: RequireAuth,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateEventRequest>,
) -> ApiResult<impl IntoResponse> {
    // Validate request
    payload
        .validate()
        .map_err(|e| ApiError::ValidationError(format!("Invalid event request: {}", e)))?;

    // Lookup trigger by reference to get trigger ID
    let trigger = TriggerRepository::find_by_ref(&state.db, &payload.trigger_ref)
        .await?
        .ok_or_else(|| {
            ApiError::NotFound(format!("Trigger '{}' not found", payload.trigger_ref))
        })?;

    // Parse trigger_instance_id to extract rule ID (format: "rule_{id}")
    let (rule_id, rule_ref) = if let Some(instance_id) = &payload.trigger_instance_id {
        if let Some(id_str) = instance_id.strip_prefix("rule_") {
            if let Ok(rid) = id_str.parse::<i64>() {
                // Fetch rule reference from database
                let fetched_rule_ref: Option<String> =
                    sqlx::query_scalar("SELECT ref FROM rule WHERE id = $1")
                        .bind(rid)
                        .fetch_optional(&state.db)
                        .await?;

                if let Some(rref) = fetched_rule_ref {
                    tracing::debug!("Event associated with rule {} (id: {})", rref, rid);
                    (Some(rid), Some(rref))
                } else {
                    tracing::warn!("trigger_instance_id {} provided but rule not found", rid);
                    (None, None)
                }
            } else {
                tracing::warn!("Invalid rule ID in trigger_instance_id: {}", instance_id);
                (None, None)
            }
        } else {
            tracing::debug!(
                "trigger_instance_id doesn't match rule format: {}",
                instance_id
            );
            (None, None)
        }
    } else {
        (None, None)
    };

    // Determine source (sensor) from authenticated user if it's a sensor token
    use crate::auth::jwt::TokenType;
    let (source_id, source_ref) = match user.0.claims.token_type {
        TokenType::Sensor => {
            // Extract sensor reference from login
            let sensor_ref = user.0.claims.login.clone();

            // Look up sensor by reference
            let sensor_id: Option<i64> = sqlx::query_scalar("SELECT id FROM sensor WHERE ref = $1")
                .bind(&sensor_ref)
                .fetch_optional(&state.db)
                .await?;

            match sensor_id {
                Some(id) => {
                    tracing::debug!("Event created by sensor {} (id: {})", sensor_ref, id);
                    (Some(id), Some(sensor_ref))
                }
                None => {
                    tracing::warn!("Sensor token for ref '{}' but sensor not found", sensor_ref);
                    (None, Some(sensor_ref))
                }
            }
        }
        _ => (None, None),
    };

    // Create event input
    let input = CreateEventInput {
        trigger: Some(trigger.id),
        trigger_ref: payload.trigger_ref.clone(),
        config: payload.config,
        payload: payload.payload,
        source: source_id,
        source_ref,
        rule: rule_id,
        rule_ref,
    };

    // Create the event
    let event = EventRepository::create(&state.db, input).await?;

    // Publish EventCreated message to message queue if publisher is available
    if let Some(ref publisher) = state.publisher {
        let message_payload = EventCreatedPayload {
            event_id: event.id,
            trigger_id: event.trigger,
            trigger_ref: event.trigger_ref.clone(),
            sensor_id: event.source,
            sensor_ref: event.source_ref.clone(),
            payload: event.payload.clone().unwrap_or(serde_json::json!({})),
            config: event.config.clone(),
        };

        let envelope = MessageEnvelope::new(MessageType::EventCreated, message_payload)
            .with_source("api-service");

        if let Err(e) = publisher.publish_envelope(&envelope).await {
            tracing::warn!(
                "Failed to publish EventCreated message for event {}: {}",
                event.id,
                e
            );
            // Continue even if message publishing fails - event is already recorded
        } else {
            tracing::debug!(
                "Published EventCreated message for event {} (trigger: {})",
                event.id,
                event.trigger_ref
            );
        }
    }

    let response = ApiResponse::new(EventResponse::from(event));

    Ok((StatusCode::CREATED, Json(response)))
}

/// List all events with pagination and optional filters
#[utoipa::path(
    get,
    path = "/api/v1/events",
    tag = "events",
    params(EventQueryParams),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "List of events", body = PaginatedResponse<EventSummary>),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn list_events(
    _user: RequireAuth,
    State(state): State<Arc<AppState>>,
    Query(query): Query<EventQueryParams>,
) -> ApiResult<impl IntoResponse> {
    // Get events based on filters
    let events = if let Some(trigger_id) = query.trigger {
        // Filter by trigger ID
        EventRepository::find_by_trigger(&state.db, trigger_id).await?
    } else if let Some(trigger_ref) = &query.trigger_ref {
        // Filter by trigger reference
        EventRepository::find_by_trigger_ref(&state.db, trigger_ref).await?
    } else {
        // Get all events
        EventRepository::list(&state.db).await?
    };

    // Apply additional filters in memory
    let mut filtered_events = events;

    if let Some(source_id) = query.source {
        filtered_events.retain(|e| e.source == Some(source_id));
    }

    if let Some(rule_ref) = &query.rule_ref {
        let rule_ref_lower = rule_ref.to_lowercase();
        filtered_events.retain(|e| {
            e.rule_ref
                .as_ref()
                .map(|r| r.to_lowercase().contains(&rule_ref_lower))
                .unwrap_or(false)
        });
    }

    // Calculate pagination
    let total = filtered_events.len() as u64;
    let start = query.offset() as usize;
    let end = (start + query.limit() as usize).min(filtered_events.len());

    // Get paginated slice
    let paginated_events: Vec<EventSummary> = filtered_events[start..end]
        .iter()
        .map(|event| EventSummary::from(event.clone()))
        .collect();

    // Convert query params to pagination params for response
    let pagination_params = PaginationParams {
        page: query.page,
        page_size: query.per_page,
    };

    let response = PaginatedResponse::new(paginated_events, &pagination_params, total);

    Ok((StatusCode::OK, Json(response)))
}

/// Get a single event by ID
#[utoipa::path(
    get,
    path = "/api/v1/events/{id}",
    tag = "events",
    params(
        ("id" = i64, Path, description = "Event ID")
    ),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Event details", body = ApiResponse<EventResponse>),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Event not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn get_event(
    _user: RequireAuth,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> ApiResult<impl IntoResponse> {
    let event = EventRepository::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Event with ID {} not found", id)))?;

    let response = ApiResponse::new(EventResponse::from(event));

    Ok((StatusCode::OK, Json(response)))
}

/// List all enforcements with pagination and optional filters
#[utoipa::path(
    get,
    path = "/api/v1/enforcements",
    tag = "enforcements",
    params(EnforcementQueryParams),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "List of enforcements", body = PaginatedResponse<EnforcementSummary>),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn list_enforcements(
    _user: RequireAuth,
    State(state): State<Arc<AppState>>,
    Query(query): Query<EnforcementQueryParams>,
) -> ApiResult<impl IntoResponse> {
    // Get enforcements based on filters
    let enforcements = if let Some(status) = query.status {
        // Filter by status
        EnforcementRepository::find_by_status(&state.db, status).await?
    } else if let Some(rule_id) = query.rule {
        // Filter by rule ID
        EnforcementRepository::find_by_rule(&state.db, rule_id).await?
    } else if let Some(event_id) = query.event {
        // Filter by event ID
        EnforcementRepository::find_by_event(&state.db, event_id).await?
    } else {
        // Get all enforcements
        EnforcementRepository::list(&state.db).await?
    };

    // Apply additional filters in memory
    let mut filtered_enforcements = enforcements;

    if let Some(trigger_ref) = &query.trigger_ref {
        filtered_enforcements.retain(|e| e.trigger_ref == *trigger_ref);
    }

    // Calculate pagination
    let total = filtered_enforcements.len() as u64;
    let start = query.offset() as usize;
    let end = (start + query.limit() as usize).min(filtered_enforcements.len());

    // Get paginated slice
    let paginated_enforcements: Vec<EnforcementSummary> = filtered_enforcements[start..end]
        .iter()
        .map(|enforcement| EnforcementSummary::from(enforcement.clone()))
        .collect();

    // Convert query params to pagination params for response
    let pagination_params = PaginationParams {
        page: query.page,
        page_size: query.per_page,
    };

    let response = PaginatedResponse::new(paginated_enforcements, &pagination_params, total);

    Ok((StatusCode::OK, Json(response)))
}

/// Get a single enforcement by ID
#[utoipa::path(
    get,
    path = "/api/v1/enforcements/{id}",
    tag = "enforcements",
    params(
        ("id" = i64, Path, description = "Enforcement ID")
    ),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Enforcement details", body = ApiResponse<EnforcementResponse>),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Enforcement not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn get_enforcement(
    _user: RequireAuth,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> ApiResult<impl IntoResponse> {
    let enforcement = EnforcementRepository::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Enforcement with ID {} not found", id)))?;

    let response = ApiResponse::new(EnforcementResponse::from(enforcement));

    Ok((StatusCode::OK, Json(response)))
}

/// Register event and enforcement routes
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/events", get(list_events).post(create_event))
        .route("/events/{id}", get(get_event))
        .route("/enforcements", get(list_enforcements))
        .route("/enforcements/{id}", get(get_enforcement))
}
