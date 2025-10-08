//! RFC-compliant task processing implementation
//!
//! This module implements ONLY the exact 9-step processing algorithm
//! specified in the 2389 Agent Protocol RFC Section 5.

pub mod nine_step;

#[cfg(test)]
mod dynamic_routing_tests;

pub use nine_step::{NineStepProcessor, ProcessingResult, ProcessorConfig};
