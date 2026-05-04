//! Background monitor for expiring pending inquiries.

use sqlx::PgPool;
use std::time::Duration;
use tokio::time::{interval, MissedTickBehavior};
use tracing::{error, info};

use attune_common::repositories::inquiry::InquiryRepository;

/// Poll for pending inquiries whose `timeout_at` has passed and mark them
/// timeout. PostgreSQL triggers publish the corresponding notifications.
pub async fn start_inquiry_timeout_monitor(pool: PgPool) {
    let mut ticker = interval(Duration::from_secs(1));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

    loop {
        ticker.tick().await;
        match InquiryRepository::timeout_expired_pending(&pool).await {
            Ok(expired) if !expired.is_empty() => {
                info!(count = expired.len(), "Timed out expired inquiries");
            }
            Ok(_) => {}
            Err(err) => {
                error!("Failed to time out expired inquiries: {}", err);
            }
        }
    }
}
