//! Read-side queries for audit events.

use chrono::{DateTime, Utc};
use sqlx::{Executor, Postgres, QueryBuilder};
use uuid::Uuid;

use crate::models::Id;
use crate::Result;

use super::{AuditCategory, AuditEvent, AuditOutcome};

/// Filters for [`AuditRepository::search`].
#[derive(Debug, Clone, Default)]
pub struct AuditEventFilters {
    pub category: Option<AuditCategory>,
    pub event_type: Option<String>,
    pub outcome: Option<AuditOutcome>,
    pub actor_identity: Option<Id>,
    pub resource_type: Option<String>,
    pub resource_id: Option<Id>,
    pub resource_ref: Option<String>,
    pub request_id: Option<Uuid>,
    pub created_after: Option<DateTime<Utc>>,
    pub created_before: Option<DateTime<Utc>>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

pub struct AuditRepository;

const SELECT_COLUMNS: &str = "id, created, category, event_type, outcome, \
    actor_identity, actor_login, actor_token_type, host(actor_ip) as actor_ip, actor_user_agent, \
    request_id, resource_type, resource_id, resource_ref, \
    http_method, http_path, http_status, duration_ms, \
    details, correlation_chain";

impl AuditRepository {
    /// Look up a single audit event by its ID. Composite-PK hypertables can
    /// be queried by `id` alone; the planner will scan all chunks (rare path).
    pub async fn find_by_id<'e, E>(executor: E, id: Id) -> Result<Option<AuditEvent>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let sql = format!("SELECT {SELECT_COLUMNS} FROM audit_event WHERE id = $1 LIMIT 1");
        sqlx::query_as::<_, AuditEvent>(&sql)
            .bind(id)
            .fetch_optional(executor)
            .await
            .map_err(Into::into)
    }

    /// Return all audit events for a single request_id ordered by created.
    pub async fn find_by_request_id<'e, E>(executor: E, request_id: Uuid) -> Result<Vec<AuditEvent>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let sql = format!(
            "SELECT {SELECT_COLUMNS} FROM audit_event WHERE request_id = $1 ORDER BY created ASC, id ASC"
        );
        sqlx::query_as::<_, AuditEvent>(&sql)
            .bind(request_id)
            .fetch_all(executor)
            .await
            .map_err(Into::into)
    }

    /// Search audit events with optional filters. Results are ordered by
    /// `created DESC, id DESC` and are paginated by `limit`/`offset`.
    pub async fn search<'e, E>(executor: E, filters: &AuditEventFilters) -> Result<Vec<AuditEvent>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let mut qb: QueryBuilder<Postgres> = QueryBuilder::new("SELECT ");
        qb.push(SELECT_COLUMNS);
        qb.push(" FROM audit_event WHERE 1=1");

        if let Some(category) = filters.category {
            qb.push(" AND category = ").push_bind(category);
        }
        if let Some(event_type) = &filters.event_type {
            qb.push(" AND event_type = ").push_bind(event_type.clone());
        }
        if let Some(outcome) = filters.outcome {
            qb.push(" AND outcome = ").push_bind(outcome);
        }
        if let Some(actor) = filters.actor_identity {
            qb.push(" AND actor_identity = ").push_bind(actor);
        }
        if let Some(rt) = &filters.resource_type {
            qb.push(" AND resource_type = ").push_bind(rt.clone());
        }
        if let Some(rid) = filters.resource_id {
            qb.push(" AND resource_id = ").push_bind(rid);
        }
        if let Some(rref) = &filters.resource_ref {
            qb.push(" AND resource_ref = ").push_bind(rref.clone());
        }
        if let Some(rid) = filters.request_id {
            qb.push(" AND request_id = ").push_bind(rid);
        }
        if let Some(after) = filters.created_after {
            qb.push(" AND created >= ").push_bind(after);
        }
        if let Some(before) = filters.created_before {
            qb.push(" AND created < ").push_bind(before);
        }

        qb.push(" ORDER BY created DESC, id DESC");
        let limit = filters.limit.unwrap_or(100).clamp(1, 1000);
        let offset = filters.offset.unwrap_or(0).max(0);
        qb.push(" LIMIT ").push_bind(limit);
        qb.push(" OFFSET ").push_bind(offset);

        qb.build_query_as::<AuditEvent>()
            .fetch_all(executor)
            .await
            .map_err(Into::into)
    }
}
