//! Shared types for timer sensor
//!
//! Defines timer configurations and common data structures.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Timer configuration for different timer types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TimerConfig {
    /// Interval-based timer (fires every N seconds/minutes/hours)
    Interval {
        /// Number of units between fires
        interval: u64,
        /// Unit of time (seconds, minutes, hours, days)
        #[serde(default = "default_unit")]
        unit: TimeUnit,
    },
    /// Cron-based timer (fires based on cron expression)
    Cron {
        /// Cron expression (e.g., "0 0 * * *")
        expression: String,
    },
    /// Date/time-based timer (fires at a specific time)
    DateTime {
        /// ISO 8601 timestamp to fire at
        fire_at: DateTime<Utc>,
    },
}

fn default_unit() -> TimeUnit {
    TimeUnit::Seconds
}

/// Time unit for interval timers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TimeUnit {
    Seconds,
    Minutes,
    Hours,
    Days,
}

impl TimerConfig {
    /// Calculate total interval in seconds
    #[allow(dead_code)]
    pub fn interval_seconds(&self) -> Option<u64> {
        match self {
            TimerConfig::Interval { interval, unit } => Some(match unit {
                TimeUnit::Seconds => *interval,
                TimeUnit::Minutes => interval * 60,
                TimeUnit::Hours => interval * 3600,
                TimeUnit::Days => interval * 86400,
            }),
            _ => None,
        }
    }

    /// Get the cron expression if this is a cron timer
    #[allow(dead_code)]
    pub fn cron_expression(&self) -> Option<&str> {
        match self {
            TimerConfig::Cron { expression } => Some(expression),
            _ => None,
        }
    }

    /// Get the fire time if this is a datetime timer
    #[allow(dead_code)]
    pub fn fire_time(&self) -> Option<DateTime<Utc>> {
        match self {
            TimerConfig::DateTime { fire_at } => Some(*fire_at),
            _ => None,
        }
    }
}

/// Rule lifecycle event types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event_type", rename_all = "PascalCase")]
pub enum RuleLifecycleEvent {
    RuleCreated {
        rule_id: i64,
        rule_ref: String,
        trigger_type: String,
        trigger_params: Option<serde_json::Value>,
        enabled: bool,
        timestamp: DateTime<Utc>,
    },
    RuleEnabled {
        rule_id: i64,
        rule_ref: String,
        trigger_type: String,
        trigger_params: Option<serde_json::Value>,
        timestamp: DateTime<Utc>,
    },
    RuleDisabled {
        rule_id: i64,
        rule_ref: String,
        trigger_type: String,
        timestamp: DateTime<Utc>,
    },
    RuleDeleted {
        rule_id: i64,
        rule_ref: String,
        trigger_type: String,
        timestamp: DateTime<Utc>,
    },
}

impl RuleLifecycleEvent {
    /// Get the rule ID from any event type
    #[allow(dead_code)]
    pub fn rule_id(&self) -> i64 {
        match self {
            RuleLifecycleEvent::RuleCreated { rule_id, .. }
            | RuleLifecycleEvent::RuleEnabled { rule_id, .. }
            | RuleLifecycleEvent::RuleDisabled { rule_id, .. }
            | RuleLifecycleEvent::RuleDeleted { rule_id, .. } => *rule_id,
        }
    }

    /// Get the trigger type from any event type
    pub fn trigger_type(&self) -> &str {
        match self {
            RuleLifecycleEvent::RuleCreated { trigger_type, .. }
            | RuleLifecycleEvent::RuleEnabled { trigger_type, .. }
            | RuleLifecycleEvent::RuleDisabled { trigger_type, .. }
            | RuleLifecycleEvent::RuleDeleted { trigger_type, .. } => trigger_type,
        }
    }

    /// Get trigger params if available
    #[allow(dead_code)]
    pub fn trigger_params(&self) -> Option<&serde_json::Value> {
        match self {
            RuleLifecycleEvent::RuleCreated { trigger_params, .. }
            | RuleLifecycleEvent::RuleEnabled { trigger_params, .. } => trigger_params.as_ref(),
            _ => None,
        }
    }

    /// Check if rule should be active (created and enabled, or explicitly enabled)
    #[allow(dead_code)]
    pub fn is_active(&self) -> bool {
        match self {
            RuleLifecycleEvent::RuleCreated { enabled, .. } => *enabled,
            RuleLifecycleEvent::RuleEnabled { .. } => true,
            RuleLifecycleEvent::RuleDisabled { .. } | RuleLifecycleEvent::RuleDeleted { .. } => {
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timer_config_interval_seconds() {
        let config = TimerConfig::Interval {
            interval: 5,
            unit: TimeUnit::Seconds,
        };
        assert_eq!(config.interval_seconds(), Some(5));

        let config = TimerConfig::Interval {
            interval: 2,
            unit: TimeUnit::Minutes,
        };
        assert_eq!(config.interval_seconds(), Some(120));

        let config = TimerConfig::Interval {
            interval: 1,
            unit: TimeUnit::Hours,
        };
        assert_eq!(config.interval_seconds(), Some(3600));

        let config = TimerConfig::Interval {
            interval: 1,
            unit: TimeUnit::Days,
        };
        assert_eq!(config.interval_seconds(), Some(86400));
    }

    #[test]
    fn test_timer_config_cron() {
        let config = TimerConfig::Cron {
            expression: "0 0 * * *".to_string(),
        };
        assert_eq!(config.cron_expression(), Some("0 0 * * *"));
        assert_eq!(config.interval_seconds(), None);
    }

    #[test]
    fn test_timer_config_datetime() {
        let fire_at = Utc::now();
        let config = TimerConfig::DateTime { fire_at };
        assert_eq!(config.fire_time(), Some(fire_at));
        assert_eq!(config.interval_seconds(), None);
    }

    #[test]
    fn test_timer_config_deserialization_interval() {
        let json = r#"{
            "type": "interval",
            "interval": 30,
            "unit": "seconds"
        }"#;

        let config: TimerConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.interval_seconds(), Some(30));
    }

    #[test]
    fn test_timer_config_deserialization_interval_default_unit() {
        let json = r#"{
            "type": "interval",
            "interval": 60
        }"#;

        let config: TimerConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.interval_seconds(), Some(60));
    }

    #[test]
    fn test_timer_config_deserialization_cron() {
        let json = r#"{
            "type": "cron",
            "expression": "0 0 * * *"
        }"#;

        let config: TimerConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.cron_expression(), Some("0 0 * * *"));
    }

    #[test]
    fn test_rule_lifecycle_event_rule_id() {
        let event = RuleLifecycleEvent::RuleCreated {
            rule_id: 123,
            rule_ref: "test".to_string(),
            trigger_type: "core.timer".to_string(),
            trigger_params: None,
            enabled: true,
            timestamp: Utc::now(),
        };
        assert_eq!(event.rule_id(), 123);
    }

    #[test]
    fn test_rule_lifecycle_event_trigger_type() {
        let event = RuleLifecycleEvent::RuleEnabled {
            rule_id: 123,
            rule_ref: "test".to_string(),
            trigger_type: "core.timer".to_string(),
            trigger_params: None,
            timestamp: Utc::now(),
        };
        assert_eq!(event.trigger_type(), "core.timer");
    }

    #[test]
    fn test_rule_lifecycle_event_is_active() {
        let event = RuleLifecycleEvent::RuleCreated {
            rule_id: 123,
            rule_ref: "test".to_string(),
            trigger_type: "core.timer".to_string(),
            trigger_params: None,
            enabled: true,
            timestamp: Utc::now(),
        };
        assert!(event.is_active());

        let event = RuleLifecycleEvent::RuleDisabled {
            rule_id: 123,
            rule_ref: "test".to_string(),
            trigger_type: "core.timer".to_string(),
            timestamp: Utc::now(),
        };
        assert!(!event.is_active());
    }
}
