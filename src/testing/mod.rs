//! Testing utilities and mock implementations
//!
//! This module provides mock implementations for testing the 2389 Agent Protocol
//! without requiring external dependencies like MQTT brokers or LLM providers.

pub mod mocks;

pub use mocks::*;
