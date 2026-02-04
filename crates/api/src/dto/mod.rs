//! Data Transfer Objects (DTOs) for API requests and responses

pub mod action;
pub mod auth;
pub mod common;
pub mod event;
pub mod execution;
pub mod inquiry;
pub mod key;
pub mod pack;
pub mod rule;
pub mod trigger;
pub mod webhook;
pub mod workflow;

pub use action::{ActionResponse, ActionSummary, CreateActionRequest, UpdateActionRequest};
pub use auth::{
    ChangePasswordRequest, CurrentUserResponse, LoginRequest, RefreshTokenRequest, RegisterRequest,
    TokenResponse,
};
pub use common::{
    ApiResponse, PaginatedResponse, PaginationMeta, PaginationParams, SuccessResponse,
};
pub use event::{
    EnforcementQueryParams, EnforcementResponse, EnforcementSummary, EventQueryParams,
    EventResponse, EventSummary,
};
pub use execution::{CreateExecutionRequest, ExecutionQueryParams, ExecutionResponse, ExecutionSummary};
pub use inquiry::{
    CreateInquiryRequest, InquiryQueryParams, InquiryRespondRequest, InquiryResponse,
    InquirySummary, UpdateInquiryRequest,
};
pub use key::{CreateKeyRequest, KeyQueryParams, KeyResponse, KeySummary, UpdateKeyRequest};
pub use pack::{CreatePackRequest, PackResponse, PackSummary, UpdatePackRequest};
pub use rule::{CreateRuleRequest, RuleResponse, RuleSummary, UpdateRuleRequest};
pub use trigger::{
    CreateSensorRequest, CreateTriggerRequest, SensorResponse, SensorSummary, TriggerResponse,
    TriggerSummary, UpdateSensorRequest, UpdateTriggerRequest,
};
pub use webhook::{WebhookReceiverRequest, WebhookReceiverResponse};
pub use workflow::{
    CreateWorkflowRequest, UpdateWorkflowRequest, WorkflowResponse, WorkflowSearchParams,
    WorkflowSummary,
};
