//! RFC-compliant builtin tools for 2389 Agent Protocol
//!
//! This module provides focused, decomposed builtin tool implementations.
//! Each tool type has its own module with pure functions separated from I/O.

pub mod file_operations;
pub mod http_request;
pub mod web_search;

// Re-export public types for backwards compatibility
pub use file_operations::{FileReadTool, FileWriteTool};
pub use http_request::HttpRequestTool;
pub use web_search::WebSearchTool;
