use anyhow::Result;
use clap::ValueEnum;
use colored::Colorize;
use comfy_table::{modifiers::UTF8_ROUND_CORNERS, presets::UTF8_FULL, Cell, Color, Table};
use serde::Serialize;
use std::fmt::Display;

/// Output format for CLI commands
#[derive(Debug, Clone, Copy, ValueEnum, PartialEq)]
pub enum OutputFormat {
    /// Human-readable table format
    Table,
    /// JSON format for scripting
    Json,
    /// YAML format
    Yaml,
}

impl Display for OutputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutputFormat::Table => write!(f, "table"),
            OutputFormat::Json => write!(f, "json"),
            OutputFormat::Yaml => write!(f, "yaml"),
        }
    }
}

/// Print output in the specified format
pub fn print_output<T: Serialize>(data: &T, format: OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(data)?;
            println!("{}", json);
        }
        OutputFormat::Yaml => {
            let yaml = serde_yaml_ng::to_string(data)?;
            println!("{}", yaml);
        }
        OutputFormat::Table => {
            // For table format, the caller should use specific table functions
            let json = serde_json::to_string_pretty(data)?;
            println!("{}", json);
        }
    }
    Ok(())
}

/// Print a success message
pub fn print_success(message: &str) {
    println!("{} {}", "✓".green().bold(), message);
}

/// Print an info message
pub fn print_info(message: &str) {
    println!("{} {}", "ℹ".blue().bold(), message);
}

/// Print a warning message
pub fn print_warning(message: &str) {
    eprintln!("{} {}", "⚠".yellow().bold(), message);
}

/// Print an error message
pub fn print_error(message: &str) {
    eprintln!("{} {}", "✗".red().bold(), message);
}

/// Create a new table with default styling
pub fn create_table() -> Table {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS);
    table
}

/// Add a header row to a table with styling
pub fn add_header(table: &mut Table, headers: Vec<&str>) {
    let cells: Vec<Cell> = headers
        .into_iter()
        .map(|h| Cell::new(h).fg(Color::Cyan))
        .collect();
    table.set_header(cells);
}

/// Print a table of key-value pairs
pub fn print_key_value_table(pairs: Vec<(&str, String)>) {
    let mut table = create_table();
    add_header(&mut table, vec!["Key", "Value"]);

    for (key, value) in pairs {
        table.add_row(vec![Cell::new(key).fg(Color::Yellow), Cell::new(value)]);
    }

    println!("{}", table);
}

/// Print a simple list
pub fn print_list(items: Vec<String>) {
    for item in items {
        println!("  • {}", item);
    }
}

/// Print a titled section
pub fn print_section(title: &str) {
    println!("\n{}", title.bold().underline());
}

/// Format a boolean as a colored checkmark or cross
pub fn format_bool(value: bool) -> String {
    if value {
        "✓".green().to_string()
    } else {
        "✗".red().to_string()
    }
}

/// Format a status with color
pub fn format_status(status: &str) -> String {
    match status.to_lowercase().as_str() {
        "succeeded" | "success" | "enabled" | "active" | "running" => status.green().to_string(),
        "failed" | "error" | "disabled" | "inactive" => status.red().to_string(),
        "pending" | "scheduled" | "queued" => status.yellow().to_string(),
        "canceled" | "cancelled" => status.bright_black().to_string(),
        _ => status.to_string(),
    }
}

/// Truncate a string to a maximum length with ellipsis
pub fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

/// Format a timestamp in a human-readable way
pub fn format_timestamp(timestamp: &str) -> String {
    // Try to parse and format nicely, otherwise return as-is
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(timestamp) {
        dt.format("%Y-%m-%d %H:%M:%S").to_string()
    } else {
        timestamp.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("short", 10), "short");
        assert_eq!(truncate("this is a long string", 10), "this is...");
        assert_eq!(truncate("exactly10!", 10), "exactly10!");
    }

    #[test]
    fn test_output_format_display() {
        assert_eq!(OutputFormat::Table.to_string(), "table");
        assert_eq!(OutputFormat::Json.to_string(), "json");
        assert_eq!(OutputFormat::Yaml.to_string(), "yaml");
    }
}
