//! API route modules

pub mod actions;
pub mod agent;
pub mod analytics;
pub mod artifacts;
pub mod auth;
pub mod events;
pub mod executions;
pub mod health;
pub mod history;
pub mod inquiries;
pub mod keys;
pub mod packs;
pub mod permissions;
pub mod rules;
pub mod runtimes;
pub mod triggers;
pub mod webhooks;
pub mod workflows;

pub use actions::routes as action_routes;
pub use agent::routes as agent_routes;
pub use analytics::routes as analytics_routes;
pub use artifacts::routes as artifact_routes;
pub use auth::routes as auth_routes;
pub use events::routes as event_routes;
pub use executions::routes as execution_routes;
pub use health::routes as health_routes;
pub use history::routes as history_routes;
pub use inquiries::routes as inquiry_routes;
pub use keys::routes as key_routes;
pub use packs::routes as pack_routes;
pub use permissions::routes as permission_routes;
pub use rules::routes as rule_routes;
pub use runtimes::routes as runtime_routes;
pub use triggers::routes as trigger_routes;
pub use webhooks::routes as webhook_routes;
pub use workflows::routes as workflow_routes;
