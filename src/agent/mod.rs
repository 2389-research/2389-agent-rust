//! Agent processing pipeline for 2389 Agent Protocol
//!
//! This module implements the core agent processing pipeline that orchestrates
//! task execution using the 9-step algorithm defined in the protocol.

pub mod discovery;
pub mod discovery_integration;
pub mod lifecycle;
pub mod pipeline;
pub mod processor;
pub mod response;
pub mod route_decision;

pub use discovery::*;
pub use discovery_integration::*;
pub use lifecycle::*;
pub use pipeline::*;
pub use processor::*;
pub use response::*;
pub use route_decision::*;
