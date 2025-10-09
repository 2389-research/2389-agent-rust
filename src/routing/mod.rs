//! Routing Infrastructure
//!
//! This module provides two distinct routing systems:
//!
//! ## V2 Routing Architecture (router.rs)
//!
//! The Router trait and RoutingDecision enum implement the V2 routing architecture
//! where workflow routing decisions are separated from agent work. Routers decide
//! whether to complete a workflow or forward to another agent based on work output.
//!
//! ## Agent Selection Utilities (agent_selector.rs)
//!
//! Simple agent discovery and selection helpers for finding agents by capability
//! or ID. Note: This is for agent DISCOVERY, not workflow routing decisions.

pub mod agent_selector;
pub mod gatekeeper_router;
pub mod llm_router;
pub mod router;
pub mod schema;

pub use agent_selector::*;
pub use gatekeeper_router::{GatekeeperConfig, GatekeeperRouter};
pub use llm_router::LlmRouter;
pub use router::{Router, RoutingDecision};
pub use schema::RoutingDecisionOutput;
