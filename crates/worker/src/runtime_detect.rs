//! Compatibility wrapper around the shared agent runtime detection module.

pub use attune_common::agent_runtime_detection::{
    detect_runtimes, format_as_env_value, DetectedRuntime,
};

pub fn print_detection_report(runtimes: &[DetectedRuntime]) {
    attune_common::agent_runtime_detection::print_detection_report_for_env(
        "ATTUNE_WORKER_RUNTIMES",
        runtimes,
    );
}
