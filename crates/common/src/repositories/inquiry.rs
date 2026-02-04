//! Inquiry repository for database operations

use crate::models::{enums::InquiryStatus, inquiry::*, Id, JsonDict, JsonSchema};
use crate::Result;
use chrono::{DateTime, Utc};
use sqlx::{Executor, Postgres, QueryBuilder};

use super::{Create, Delete, FindById, List, Repository, Update};

pub struct InquiryRepository;

impl Repository for InquiryRepository {
    type Entity = Inquiry;
    fn table_name() -> &'static str {
        "inquiry"
    }
}

#[derive(Debug, Clone)]
pub struct CreateInquiryInput {
    pub execution: Id,
    pub prompt: String,
    pub response_schema: Option<JsonSchema>,
    pub assigned_to: Option<Id>,
    pub status: InquiryStatus,
    pub response: Option<JsonDict>,
    pub timeout_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Default)]
pub struct UpdateInquiryInput {
    pub status: Option<InquiryStatus>,
    pub response: Option<JsonDict>,
    pub responded_at: Option<DateTime<Utc>>,
    pub assigned_to: Option<Id>,
}

#[async_trait::async_trait]
impl FindById for InquiryRepository {
    async fn find_by_id<'e, E>(executor: E, id: i64) -> Result<Option<Self::Entity>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        sqlx::query_as::<_, Inquiry>(
            "SELECT id, execution, prompt, response_schema, assigned_to, status, response, timeout_at, responded_at, created, updated FROM inquiry WHERE id = $1"
        ).bind(id).fetch_optional(executor).await.map_err(Into::into)
    }
}

#[async_trait::async_trait]
impl List for InquiryRepository {
    async fn list<'e, E>(executor: E) -> Result<Vec<Self::Entity>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        sqlx::query_as::<_, Inquiry>(
            "SELECT id, execution, prompt, response_schema, assigned_to, status, response, timeout_at, responded_at, created, updated FROM inquiry ORDER BY created DESC LIMIT 1000"
        ).fetch_all(executor).await.map_err(Into::into)
    }
}

#[async_trait::async_trait]
impl Create for InquiryRepository {
    type CreateInput = CreateInquiryInput;
    async fn create<'e, E>(executor: E, input: Self::CreateInput) -> Result<Self::Entity>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        sqlx::query_as::<_, Inquiry>(
            "INSERT INTO inquiry (execution, prompt, response_schema, assigned_to, status, response, timeout_at) VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id, execution, prompt, response_schema, assigned_to, status, response, timeout_at, responded_at, created, updated"
        ).bind(input.execution).bind(&input.prompt).bind(&input.response_schema).bind(input.assigned_to).bind(input.status).bind(&input.response).bind(input.timeout_at).fetch_one(executor).await.map_err(Into::into)
    }
}

#[async_trait::async_trait]
impl Update for InquiryRepository {
    type UpdateInput = UpdateInquiryInput;
    async fn update<'e, E>(executor: E, id: i64, input: Self::UpdateInput) -> Result<Self::Entity>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        // Build update query
        let mut query = QueryBuilder::new("UPDATE inquiry SET ");
        let mut has_updates = false;

        if let Some(status) = input.status {
            query.push("status = ").push_bind(status);
            has_updates = true;
        }
        if let Some(response) = &input.response {
            if has_updates {
                query.push(", ");
            }
            query.push("response = ").push_bind(response);
            has_updates = true;
        }
        if let Some(responded_at) = input.responded_at {
            if has_updates {
                query.push(", ");
            }
            query.push("responded_at = ").push_bind(responded_at);
            has_updates = true;
        }
        if let Some(assigned_to) = input.assigned_to {
            if has_updates {
                query.push(", ");
            }
            query.push("assigned_to = ").push_bind(assigned_to);
            has_updates = true;
        }

        if !has_updates {
            // No updates requested, fetch and return existing entity
            return Self::get_by_id(executor, id).await;
        }

        query.push(", updated = NOW() WHERE id = ").push_bind(id);
        query.push(" RETURNING id, execution, prompt, response_schema, assigned_to, status, response, timeout_at, responded_at, created, updated");

        query
            .build_query_as::<Inquiry>()
            .fetch_one(executor)
            .await
            .map_err(Into::into)
    }
}

#[async_trait::async_trait]
impl Delete for InquiryRepository {
    async fn delete<'e, E>(executor: E, id: i64) -> Result<bool>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let result = sqlx::query("DELETE FROM inquiry WHERE id = $1")
            .bind(id)
            .execute(executor)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}

impl InquiryRepository {
    pub async fn find_by_status<'e, E>(executor: E, status: InquiryStatus) -> Result<Vec<Inquiry>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        sqlx::query_as::<_, Inquiry>(
            "SELECT id, execution, prompt, response_schema, assigned_to, status, response, timeout_at, responded_at, created, updated FROM inquiry WHERE status = $1 ORDER BY created DESC"
        ).bind(status).fetch_all(executor).await.map_err(Into::into)
    }

    pub async fn find_by_execution<'e, E>(executor: E, execution_id: Id) -> Result<Vec<Inquiry>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        sqlx::query_as::<_, Inquiry>(
            "SELECT id, execution, prompt, response_schema, assigned_to, status, response, timeout_at, responded_at, created, updated FROM inquiry WHERE execution = $1 ORDER BY created DESC"
        ).bind(execution_id).fetch_all(executor).await.map_err(Into::into)
    }
}
