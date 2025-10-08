//! Structured logging system using tracing crate
//!
//! Provides contextual, machine-readable logging with specialized span macros
//! for different agent operations.
//!
//! ## Log Format Options
//!
//! The logging system supports three output formats controlled by the `LOG_FORMAT` environment variable:
//!
//! - `json` - Structured JSON format for production and log aggregation systems
//! - `pretty` - Human-readable format with colors and indentation for development
//! - `compact` - Terminal-friendly format with colors but minimal spacing
//!
//! ## Environment Variables
//!
//! - `LOG_LEVEL`: Log level (ERROR, WARN, INFO, DEBUG, TRACE) - defaults to INFO
//! - `LOG_FORMAT`: Output format (json, pretty, compact) - defaults to json
//! - `LOG_SPANS`: Include span events (true/false) - defaults to false
//! - `RUST_LOG`: Override log filtering (follows env_logger format)
//!
//! ## Examples
//!
//! ```bash
//! # Production JSON logging
//! LOG_FORMAT=json LOG_LEVEL=INFO ./agent2389
//!
//! # Development with colors
//! LOG_FORMAT=pretty LOG_LEVEL=DEBUG ./agent2389
//!
//! # Compact terminal output
//! LOG_FORMAT=compact LOG_LEVEL=INFO ./agent2389
//! ```

use std::env;
use tracing::Level;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

/// Log output format options
#[derive(Debug, Clone, Copy)]
pub enum LogFormat {
    /// JSON format for structured logging (machine-readable)
    Json,
    /// Pretty format with colors and indentation (human-readable)
    Pretty,
    /// Compact format with colors but minimal spacing (terminal-friendly)
    Compact,
}

impl LogFormat {
    /// Parse log format from string
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "json" => LogFormat::Json,
            "pretty" => LogFormat::Pretty,
            "compact" => LogFormat::Compact,
            _ => LogFormat::Json, // Default to JSON for production
        }
    }
}

/// Initialize logging with manual configuration
pub fn init_logging(level: Level, format: LogFormat, include_spans: bool) {
    let mut filter = EnvFilter::new(level.to_string())
        // Reduce noise from dependencies
        .add_directive("rumqttc=warn".parse().unwrap())
        .add_directive("hyper=warn".parse().unwrap())
        .add_directive("tokio=warn".parse().unwrap())
        .add_directive("article_scraper=warn".parse().unwrap());

    // Allow RUST_LOG to override
    if let Ok(rust_log) = env::var("RUST_LOG") {
        filter = EnvFilter::new(rust_log);
    }

    let subscriber = tracing_subscriber::registry().with(filter);

    match format {
        LogFormat::Json => {
            let fmt_layer = fmt::layer().json().with_span_events(if include_spans {
                fmt::format::FmtSpan::NEW | fmt::format::FmtSpan::CLOSE
            } else {
                fmt::format::FmtSpan::NONE
            });
            subscriber.with(fmt_layer).init();
        }
        LogFormat::Pretty => {
            let fmt_layer =
                fmt::layer()
                    .pretty()
                    .with_ansi(true)
                    .with_span_events(if include_spans {
                        fmt::format::FmtSpan::NEW | fmt::format::FmtSpan::CLOSE
                    } else {
                        fmt::format::FmtSpan::NONE
                    });
            subscriber.with(fmt_layer).init();
        }
        LogFormat::Compact => {
            let fmt_layer = fmt::layer()
                .compact()
                .with_ansi(true)
                .with_target(false)
                .with_span_events(if include_spans {
                    fmt::format::FmtSpan::NEW | fmt::format::FmtSpan::CLOSE
                } else {
                    fmt::format::FmtSpan::NONE
                });
            subscriber.with(fmt_layer).init();
        }
    }
}

/// Initialize logging from environment variables
pub fn init_default_logging() {
    let log_level = env::var("LOG_LEVEL").unwrap_or_else(|_| "INFO".to_string());

    let level = match log_level.to_uppercase().as_str() {
        "ERROR" => Level::ERROR,
        "WARN" => Level::WARN,
        "INFO" => Level::INFO,
        "DEBUG" => Level::DEBUG,
        "TRACE" => Level::TRACE,
        _ => Level::INFO,
    };

    let format = env::var("LOG_FORMAT").unwrap_or_else(|_| "json".to_string());
    let log_format = LogFormat::parse(&format);

    let include_spans = env::var("LOG_SPANS")
        .unwrap_or_else(|_| "false".to_string())
        .to_lowercase()
        == "true";

    init_logging(level, log_format, include_spans);
}

/// Create a task processing span with contextual information
#[macro_export]
macro_rules! task_span {
    ($($field:tt)*) => {
        tracing::info_span!("task_processing", $($field)*)
    };
}

/// Create a tool execution span
#[macro_export]
macro_rules! tool_span {
    ($($field:tt)*) => {
        tracing::info_span!("tool_execution", $($field)*)
    };
}

/// Create an MQTT operation span
#[macro_export]
macro_rules! mqtt_span {
    ($($field:tt)*) => {
        tracing::info_span!("mqtt_operation", $($field)*)
    };
}

/// Create a lifecycle event span
#[macro_export]
macro_rules! lifecycle_span {
    ($($field:tt)*) => {
        tracing::info_span!("lifecycle_event", $($field)*)
    };
}

// Re-export macros for convenience
pub use {lifecycle_span, mqtt_span, task_span, tool_span};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_format_parse_json() {
        assert!(matches!(LogFormat::parse("json"), LogFormat::Json));
        assert!(matches!(LogFormat::parse("JSON"), LogFormat::Json));
        assert!(matches!(LogFormat::parse("Json"), LogFormat::Json));
    }

    #[test]
    fn test_log_format_parse_pretty() {
        assert!(matches!(LogFormat::parse("pretty"), LogFormat::Pretty));
        assert!(matches!(LogFormat::parse("PRETTY"), LogFormat::Pretty));
        assert!(matches!(LogFormat::parse("Pretty"), LogFormat::Pretty));
    }

    #[test]
    fn test_log_format_parse_compact() {
        assert!(matches!(LogFormat::parse("compact"), LogFormat::Compact));
        assert!(matches!(LogFormat::parse("COMPACT"), LogFormat::Compact));
        assert!(matches!(LogFormat::parse("Compact"), LogFormat::Compact));
    }

    #[test]
    fn test_log_format_parse_invalid_defaults_to_json() {
        // Invalid formats should default to JSON for production safety
        assert!(matches!(LogFormat::parse("invalid"), LogFormat::Json));
        assert!(matches!(LogFormat::parse(""), LogFormat::Json));
        assert!(matches!(LogFormat::parse("xml"), LogFormat::Json));
        assert!(matches!(LogFormat::parse("yaml"), LogFormat::Json));
    }

    #[test]
    fn test_log_format_parse_case_insensitive() {
        // Verify case insensitivity
        assert!(matches!(LogFormat::parse("jSoN"), LogFormat::Json));
        assert!(matches!(LogFormat::parse("PrEtTy"), LogFormat::Pretty));
        assert!(matches!(LogFormat::parse("CoMpAcT"), LogFormat::Compact));
    }

    #[test]
    fn test_log_format_clone_and_copy() {
        // Verify LogFormat is Clone and Copy
        let format = LogFormat::Json;
        let _cloned = format;
        let _copied = format;

        // Original should still be usable (proving Copy)
        assert!(matches!(format, LogFormat::Json));
    }

    #[test]
    fn test_log_level_string_matching() {
        // Test the level matching logic from init_default_logging
        let test_cases = vec![
            ("ERROR", Level::ERROR),
            ("WARN", Level::WARN),
            ("INFO", Level::INFO),
            ("DEBUG", Level::DEBUG),
            ("TRACE", Level::TRACE),
            ("invalid", Level::INFO), // Invalid should default to INFO
        ];

        for (input, expected) in test_cases {
            let level = match input.to_uppercase().as_str() {
                "ERROR" => Level::ERROR,
                "WARN" => Level::WARN,
                "INFO" => Level::INFO,
                "DEBUG" => Level::DEBUG,
                "TRACE" => Level::TRACE,
                _ => Level::INFO,
            };
            assert_eq!(level, expected, "Failed for input: {input}");
        }
    }

    #[test]
    fn test_log_spans_boolean_parsing() {
        // Test the boolean parsing logic for LOG_SPANS environment variable
        let test_cases = vec![
            ("true", true),
            ("TRUE", true),
            ("True", true),
            ("false", false),
            ("FALSE", false),
            ("", false),    // Empty defaults to false
            ("yes", false), // Non-"true" values default to false
            ("1", false),
        ];

        for (input, expected) in test_cases {
            let result = input.to_lowercase() == "true";
            assert_eq!(result, expected, "Failed for input: '{input}'");
        }
    }
}
