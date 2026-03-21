//! Runtime Auto-Detection Module
//!
//! Provides lightweight, database-free runtime detection for the Universal Worker Agent.
//! Unlike [`attune_common::runtime_detection::RuntimeDetector`] which queries the database
//! for runtime definitions and verification metadata, this module probes the local system
//! directly by checking for well-known interpreter binaries on PATH.
//!
//! This is designed for the agent entrypoint (`attune-agent`) which is injected into
//! arbitrary containers and must discover what runtimes are available without any
//! database connectivity at detection time.
//!
//! # Detection Strategy
//!
//! For each candidate runtime, the detector:
//! 1. Checks if a binary exists and is executable using `which`-style PATH lookup
//! 2. Optionally runs a version command (e.g., `python3 --version`) to capture the version
//! 3. Returns a list of [`DetectedRuntime`] structs with name, path, and version info
//!
//! # Supported Runtimes
//!
//! | Runtime  | Binaries checked (in order)   | Version command         |
//! |----------|-------------------------------|-------------------------|
//! | shell    | `bash`, `sh`                  | `bash --version`        |
//! | python   | `python3`, `python`           | `python3 --version`     |
//! | node     | `node`, `nodejs`              | `node --version`        |
//! | ruby     | `ruby`                        | `ruby --version`        |
//! | go       | `go`                          | `go version`            |
//! | java     | `java`                        | `java -version`         |
//! | r        | `Rscript`                     | `Rscript --version`     |
//! | perl     | `perl`                        | `perl --version`        |

use serde::{Deserialize, Serialize};
use std::fmt;
use std::process::Command;
use tracing::{debug, info};

/// A runtime interpreter discovered on the local system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedRuntime {
    /// Canonical runtime name (e.g., "shell", "python", "node").
    /// These names align with the normalized names from
    /// [`attune_common::runtime_detection::normalize_runtime_name`].
    pub name: String,

    /// Absolute path to the interpreter binary (as resolved by `which`).
    pub path: String,

    /// Version string if a version check command succeeded (e.g., "3.12.1").
    pub version: Option<String>,
}

impl fmt::Display for DetectedRuntime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.version {
            Some(v) => write!(f, "{} ({}, v{})", self.name, self.path, v),
            None => write!(f, "{} ({})", self.name, self.path),
        }
    }
}

/// A candidate runtime to probe for during detection.
struct RuntimeCandidate {
    /// Canonical name for this runtime (used in ATTUNE_WORKER_RUNTIMES).
    name: &'static str,

    /// Binary names to try, in priority order. The first one found wins.
    binaries: &'static [&'static str],

    /// Arguments to pass to the binary to get a version string.
    version_args: &'static [&'static str],

    /// How to extract the version from command output.
    version_parser: VersionParser,
}

/// Strategy for parsing version output from a command.
enum VersionParser {
    /// Extract a version pattern like "X.Y.Z" from the combined stdout+stderr output.
    /// This handles the common case where the version appears somewhere in the output
    /// (e.g., "Python 3.12.1", "node v20.11.0", "go1.22.0").
    SemverLike,

    /// Java uses `-version` which writes to stderr, and the format is
    /// `openjdk version "21.0.1"` or `java version "1.8.0_392"`.
    JavaStyle,
}

/// All candidate runtimes to probe, in detection order.
fn candidates() -> Vec<RuntimeCandidate> {
    vec![
        RuntimeCandidate {
            name: "shell",
            binaries: &["bash", "sh"],
            version_args: &["--version"],
            version_parser: VersionParser::SemverLike,
        },
        RuntimeCandidate {
            name: "python",
            binaries: &["python3", "python"],
            version_args: &["--version"],
            version_parser: VersionParser::SemverLike,
        },
        RuntimeCandidate {
            name: "node",
            binaries: &["node", "nodejs"],
            version_args: &["--version"],
            version_parser: VersionParser::SemverLike,
        },
        RuntimeCandidate {
            name: "ruby",
            binaries: &["ruby"],
            version_args: &["--version"],
            version_parser: VersionParser::SemverLike,
        },
        RuntimeCandidate {
            name: "go",
            binaries: &["go"],
            version_args: &["version"],
            version_parser: VersionParser::SemverLike,
        },
        RuntimeCandidate {
            name: "java",
            binaries: &["java"],
            version_args: &["-version"],
            version_parser: VersionParser::JavaStyle,
        },
        RuntimeCandidate {
            name: "r",
            binaries: &["Rscript"],
            version_args: &["--version"],
            version_parser: VersionParser::SemverLike,
        },
        RuntimeCandidate {
            name: "perl",
            binaries: &["perl"],
            version_args: &["--version"],
            version_parser: VersionParser::SemverLike,
        },
    ]
}

/// Detect available runtimes by probing the local system for known interpreter binaries.
///
/// This function performs synchronous subprocess calls (`std::process::Command`) since
/// it is a one-time startup operation. It checks each candidate runtime's binaries
/// in priority order using `which`-style PATH lookup, and optionally captures the
/// interpreter version.
///
/// # Returns
///
/// A vector of [`DetectedRuntime`] for each runtime that was found on the system.
/// The order matches the detection order (shell first, then python, node, etc.).
///
/// # Example
///
/// ```no_run
/// use attune_worker::runtime_detect::detect_runtimes;
///
/// let runtimes = detect_runtimes();
/// for rt in &runtimes {
///     println!("Found: {}", rt);
/// }
/// // Convert to ATTUNE_WORKER_RUNTIMES format
/// let names: Vec<&str> = runtimes.iter().map(|r| r.name.as_str()).collect();
/// println!("ATTUNE_WORKER_RUNTIMES={}", names.join(","));
/// ```
pub fn detect_runtimes() -> Vec<DetectedRuntime> {
    info!("Starting runtime auto-detection...");

    let mut detected = Vec::new();

    for candidate in candidates() {
        match detect_single_runtime(&candidate) {
            Some(runtime) => {
                info!("  ✓ Detected: {}", runtime);
                detected.push(runtime);
            }
            None => {
                debug!("  ✗ Not found: {}", candidate.name);
            }
        }
    }

    info!(
        "Runtime auto-detection complete: found {} runtime(s): [{}]",
        detected.len(),
        detected
            .iter()
            .map(|r| r.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );

    detected
}

/// Attempt to detect a single runtime by checking its candidate binaries.
fn detect_single_runtime(candidate: &RuntimeCandidate) -> Option<DetectedRuntime> {
    for binary in candidate.binaries {
        if let Some(path) = which_binary(binary) {
            debug!(
                "Found {} at {} (for runtime '{}')",
                binary, path, candidate.name
            );

            // Attempt to get version info (non-fatal if it fails)
            let version = get_version(&path, candidate.version_args, &candidate.version_parser);

            return Some(DetectedRuntime {
                name: candidate.name.to_string(),
                path,
                version,
            });
        }
    }

    None
}

/// Look up a binary on PATH, similar to the `which` command.
///
/// Uses `which <binary>` on the system to resolve the full path.
/// Returns `None` if the binary is not found or `which` fails.
fn which_binary(binary: &str) -> Option<String> {
    // First check well-known absolute paths for shell interpreters
    // (these may not be on PATH in minimal containers)
    if binary == "bash" || binary == "sh" {
        let absolute_path = format!("/bin/{}", binary);
        if std::path::Path::new(&absolute_path).exists() {
            return Some(absolute_path);
        }
    }

    // Fall back to PATH lookup via `which`
    match Command::new("which").arg(binary).output() {
        Ok(output) => {
            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !path.is_empty() {
                    Some(path)
                } else {
                    None
                }
            } else {
                None
            }
        }
        Err(e) => {
            // `which` itself not found — try `command -v` as fallback
            debug!("'which' command failed ({}), trying 'command -v'", e);
            match Command::new("sh")
                .args(["-c", &format!("command -v {}", binary)])
                .output()
            {
                Ok(output) if output.status.success() => {
                    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    if !path.is_empty() {
                        Some(path)
                    } else {
                        None
                    }
                }
                _ => None,
            }
        }
    }
}

/// Run a version command and parse the version string from the output.
fn get_version(binary_path: &str, version_args: &[&str], parser: &VersionParser) -> Option<String> {
    let output = match Command::new(binary_path).args(version_args).output() {
        Ok(output) => output,
        Err(e) => {
            debug!("Failed to run version command for {}: {}", binary_path, e);
            return None;
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);

    match parser {
        VersionParser::SemverLike => parse_semver_like(&combined),
        VersionParser::JavaStyle => parse_java_version(&combined),
    }
}

/// Extract a semver-like version (X.Y.Z or X.Y) from output text.
///
/// Handles common patterns:
/// - "Python 3.12.1"
/// - "node v20.11.0"
/// - "go version go1.22.0 linux/amd64"
/// - "GNU bash, version 5.2.15(1)-release"
/// - "ruby 3.2.2 (2023-03-30 revision e51014f9c0)"
/// - "perl 5, version 36, subversion 0 (v5.36.0)"
fn parse_semver_like(output: &str) -> Option<String> {
    // Try to find a pattern like X.Y.Z or X.Y (with optional leading 'v')
    // Also handle go's "go1.22.0" format
    let re = regex::Regex::new(r"(?:v|go)?(\d+\.\d+(?:\.\d+)?)").ok()?;

    if let Some(captures) = re.captures(output) {
        captures.get(1).map(|m| m.as_str().to_string())
    } else {
        None
    }
}

/// Parse Java's peculiar version output format.
///
/// Java writes to stderr and uses formats like:
/// - `openjdk version "21.0.1" 2023-10-17`
/// - `java version "1.8.0_392"`
fn parse_java_version(output: &str) -> Option<String> {
    // Look for version inside quotes first
    let quoted_re = regex::Regex::new(r#"version\s+"([^"]+)""#).ok()?;
    if let Some(captures) = quoted_re.captures(output) {
        return captures.get(1).map(|m| m.as_str().to_string());
    }

    // Fall back to semver-like parsing
    parse_semver_like(output)
}

/// Format detected runtimes as a comma-separated string suitable for
/// the `ATTUNE_WORKER_RUNTIMES` environment variable.
///
/// # Example
///
/// ```no_run
/// use attune_worker::runtime_detect::{detect_runtimes, format_as_env_value};
///
/// let runtimes = detect_runtimes();
/// let env_val = format_as_env_value(&runtimes);
/// // e.g., "shell,python,node"
/// ```
pub fn format_as_env_value(runtimes: &[DetectedRuntime]) -> String {
    runtimes
        .iter()
        .map(|r| r.name.as_str())
        .collect::<Vec<_>>()
        .join(",")
}

/// Print a human-readable detection report to stdout.
///
/// Used by the `--detect-only` flag to show detection results and exit.
pub fn print_detection_report(runtimes: &[DetectedRuntime]) {
    println!("=== Attune Agent Runtime Detection Report ===");
    println!();

    if runtimes.is_empty() {
        println!("No runtimes detected!");
        println!();
        println!("The agent could not find any supported interpreter binaries.");
        println!("Ensure at least one of the following is installed and on PATH:");
        println!("  - bash / sh       (shell scripts)");
        println!("  - python3 / python (Python scripts)");
        println!("  - node / nodejs    (Node.js scripts)");
        println!("  - ruby             (Ruby scripts)");
        println!("  - go               (Go programs)");
        println!("  - java             (Java programs)");
        println!("  - Rscript          (R scripts)");
        println!("  - perl             (Perl scripts)");
    } else {
        println!("Detected {} runtime(s):", runtimes.len());
        println!();
        for rt in runtimes {
            let version_str = rt.version.as_deref().unwrap_or("unknown version");
            println!("  ✓ {:<10} {} ({})", rt.name, rt.path, version_str);
        }
    }

    println!();
    println!("ATTUNE_WORKER_RUNTIMES={}", format_as_env_value(runtimes));
    println!();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_semver_like_python() {
        assert_eq!(
            parse_semver_like("Python 3.12.1"),
            Some("3.12.1".to_string())
        );
    }

    #[test]
    fn test_parse_semver_like_node() {
        assert_eq!(parse_semver_like("v20.11.0"), Some("20.11.0".to_string()));
    }

    #[test]
    fn test_parse_semver_like_go() {
        assert_eq!(
            parse_semver_like("go version go1.22.0 linux/amd64"),
            Some("1.22.0".to_string())
        );
    }

    #[test]
    fn test_parse_semver_like_bash() {
        assert_eq!(
            parse_semver_like("GNU bash, version 5.2.15(1)-release (x86_64-pc-linux-gnu)"),
            Some("5.2.15".to_string())
        );
    }

    #[test]
    fn test_parse_semver_like_ruby() {
        assert_eq!(
            parse_semver_like("ruby 3.2.2 (2023-03-30 revision e51014f9c0) [x86_64-linux]"),
            Some("3.2.2".to_string())
        );
    }

    #[test]
    fn test_parse_semver_like_two_part() {
        assert_eq!(
            parse_semver_like("SomeRuntime 1.5"),
            Some("1.5".to_string())
        );
    }

    #[test]
    fn test_parse_semver_like_no_match() {
        assert_eq!(parse_semver_like("no version here"), None);
    }

    #[test]
    fn test_parse_java_version_openjdk() {
        assert_eq!(
            parse_java_version(r#"openjdk version "21.0.1" 2023-10-17"#),
            Some("21.0.1".to_string())
        );
    }

    #[test]
    fn test_parse_java_version_legacy() {
        assert_eq!(
            parse_java_version(r#"java version "1.8.0_392""#),
            Some("1.8.0_392".to_string())
        );
    }

    #[test]
    fn test_format_as_env_value_empty() {
        let runtimes: Vec<DetectedRuntime> = vec![];
        assert_eq!(format_as_env_value(&runtimes), "");
    }

    #[test]
    fn test_format_as_env_value_multiple() {
        let runtimes = vec![
            DetectedRuntime {
                name: "shell".to_string(),
                path: "/bin/bash".to_string(),
                version: Some("5.2.15".to_string()),
            },
            DetectedRuntime {
                name: "python".to_string(),
                path: "/usr/bin/python3".to_string(),
                version: Some("3.12.1".to_string()),
            },
            DetectedRuntime {
                name: "node".to_string(),
                path: "/usr/bin/node".to_string(),
                version: None,
            },
        ];
        assert_eq!(format_as_env_value(&runtimes), "shell,python,node");
    }

    #[test]
    fn test_detected_runtime_display_with_version() {
        let rt = DetectedRuntime {
            name: "python".to_string(),
            path: "/usr/bin/python3".to_string(),
            version: Some("3.12.1".to_string()),
        };
        assert_eq!(format!("{}", rt), "python (/usr/bin/python3, v3.12.1)");
    }

    #[test]
    fn test_detected_runtime_display_without_version() {
        let rt = DetectedRuntime {
            name: "shell".to_string(),
            path: "/bin/bash".to_string(),
            version: None,
        };
        assert_eq!(format!("{}", rt), "shell (/bin/bash)");
    }

    #[test]
    fn test_detect_runtimes_runs_without_panic() {
        // This test verifies the detection logic doesn't panic,
        // regardless of what's actually installed on the system.
        let runtimes = detect_runtimes();
        // We should at least find a shell on any Unix system
        // but we don't assert that since test environments vary.
        let _ = runtimes;
    }

    #[test]
    fn test_which_binary_sh() {
        // /bin/sh should exist on virtually all Unix systems
        let result = which_binary("sh");
        assert!(result.is_some(), "Expected to find 'sh' on this system");
    }

    #[test]
    fn test_which_binary_nonexistent() {
        let result = which_binary("definitely_not_a_real_binary_xyz123");
        assert!(result.is_none());
    }

    #[test]
    fn test_candidates_order() {
        let c = candidates();
        assert_eq!(c[0].name, "shell");
        assert_eq!(c[1].name, "python");
        assert_eq!(c[2].name, "node");
        assert_eq!(c[3].name, "ruby");
        assert_eq!(c[4].name, "go");
        assert_eq!(c[5].name, "java");
        assert_eq!(c[6].name, "r");
        assert_eq!(c[7].name, "perl");
    }

    #[test]
    fn test_candidates_binaries_priority() {
        let c = candidates();
        // shell prefers bash over sh
        assert_eq!(c[0].binaries, &["bash", "sh"]);
        // python prefers python3 over python
        assert_eq!(c[1].binaries, &["python3", "python"]);
        // node prefers node over nodejs
        assert_eq!(c[2].binaries, &["node", "nodejs"]);
    }
}
