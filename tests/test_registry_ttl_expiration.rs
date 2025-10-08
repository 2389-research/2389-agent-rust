//! Agent Registry TTL Expiration Tests
//!
//! Tests the 15-second TTL mechanism for agent registry cleanup:
//! - Agents expire after 15 seconds of no updates
//! - Expired agents are excluded from selection
//! - Registry cleanup removes stale entries

use agent2389::agent::discovery::{AgentInfo, AgentRegistry};
use agent2389::routing::agent_selector::{AgentSelectionDecision, RoutingHelper};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

#[tokio::test]
async fn test_agent_expires_after_ttl() {
    // Test that agent is marked expired after 15 seconds

    let mut agent = AgentInfo::new("test-agent".to_string(), "ok".to_string(), 0.5);
    agent.capabilities = Some(vec!["test".to_string()]);

    // Immediately after creation, should not be expired
    assert!(!agent.is_expired(), "Fresh agent should not be expired");

    // Manually set last_updated to 16 seconds ago
    let past_time = chrono::Utc::now() - chrono::Duration::seconds(16);
    agent.last_updated = past_time.to_rfc3339();

    // Now should be expired
    assert!(
        agent.is_expired(),
        "Agent should be expired after 16 seconds"
    );
}

#[tokio::test]
async fn test_agent_not_expired_within_ttl() {
    // Test that agent is NOT expired within 15 seconds

    let mut agent = AgentInfo::new("test-agent".to_string(), "ok".to_string(), 0.5);

    // Set timestamp to 14 seconds ago (within TTL)
    let recent_time = chrono::Utc::now() - chrono::Duration::seconds(14);
    agent.last_updated = recent_time.to_rfc3339();

    // Should not be expired
    assert!(
        !agent.is_expired(),
        "Agent should not be expired at 14 seconds"
    );
}

#[tokio::test]
async fn test_expired_agent_excluded_from_selection() {
    // Test that expired agents are not selected for routing

    let registry = Arc::new(AgentRegistry::new());

    // Create agent and immediately expire it
    let mut expired_agent = AgentInfo::new("expired-agent".to_string(), "ok".to_string(), 0.3);
    expired_agent.capabilities = Some(vec!["email".to_string()]);
    let past_time = chrono::Utc::now() - chrono::Duration::seconds(20);
    expired_agent.last_updated = past_time.to_rfc3339();

    registry.register_agent_without_refresh(expired_agent);

    // Act: Try to find agent by capability
    let routing_helper = RoutingHelper::new();
    let decision = routing_helper.find_best_agent_for_capability("email", &registry);

    // Assert: Should not find expired agent
    assert!(
        matches!(decision, AgentSelectionDecision::NoRoute { .. }),
        "Expired agent should not be selected"
    );
}

#[tokio::test]
async fn test_expired_agent_excluded_by_id_lookup() {
    // Test that expired agents are not selected even when looking up by ID

    let registry = Arc::new(AgentRegistry::new());

    // Create expired agent
    let mut expired_agent = AgentInfo::new("specific-agent".to_string(), "ok".to_string(), 0.5);
    let past_time = chrono::Utc::now() - chrono::Duration::seconds(20);
    expired_agent.last_updated = past_time.to_rfc3339();

    registry.register_agent_without_refresh(expired_agent);

    // Act: Try to find agent by ID
    let routing_helper = RoutingHelper::new();
    let decision = routing_helper.find_agent_by_id("specific-agent", &registry);

    // Assert: Should return NoRoute because agent is expired
    match decision {
        AgentSelectionDecision::NoRoute { reason } => {
            assert!(reason.contains("expired") || reason.contains("unhealthy"));
        }
        _ => panic!("Expected NoRoute for expired agent"),
    }
}

#[tokio::test]
async fn test_registry_cleanup_removes_expired_agents() {
    // Test that cleanup actually removes expired agents from registry

    let registry = Arc::new(AgentRegistry::new());

    // Register fresh agent
    let fresh_agent = AgentInfo::new("fresh-agent".to_string(), "ok".to_string(), 0.3)
        .with_capabilities(vec!["fresh".to_string()]);

    // Register expired agent
    let mut expired_agent = AgentInfo::new("expired-agent".to_string(), "ok".to_string(), 0.5);
    expired_agent.capabilities = Some(vec!["expired".to_string()]);
    let past_time = chrono::Utc::now() - chrono::Duration::seconds(20);
    expired_agent.last_updated = past_time.to_rfc3339();

    registry.register_agent(fresh_agent);
    registry.register_agent_without_refresh(expired_agent);

    // Act: Run cleanup
    registry.force_cleanup_for_test();

    // Assert: Fresh agent should still be in registry
    let fresh_lookup = registry.get_agent("fresh-agent");
    assert!(
        fresh_lookup.is_some(),
        "Fresh agent should remain in registry"
    );

    // Expired agent should be removed
    let expired_lookup = registry.get_agent("expired-agent");
    assert!(
        expired_lookup.is_none(),
        "Expired agent should be removed from registry"
    );
}

#[tokio::test]
async fn test_agent_refresh_timestamp_prevents_expiry() {
    // Test that refreshing timestamp prevents agent from expiring

    let mut agent = AgentInfo::new("test-agent".to_string(), "ok".to_string(), 0.5);

    // Set to almost expired (14 seconds ago)
    let almost_expired = chrono::Utc::now() - chrono::Duration::seconds(14);
    agent.last_updated = almost_expired.to_rfc3339();

    // Refresh timestamp
    agent.refresh_timestamp();

    // Should no longer be expired
    assert!(
        !agent.is_expired(),
        "Agent should not be expired after refresh"
    );
}

#[tokio::test]
async fn test_multiple_agents_selective_cleanup() {
    // Test that cleanup only removes expired agents, keeps fresh ones

    let registry = Arc::new(AgentRegistry::new());

    // Register 3 agents: fresh, almost expired, definitely expired
    let fresh = AgentInfo::new("fresh".to_string(), "ok".to_string(), 0.3);

    let mut almost_expired = AgentInfo::new("almost".to_string(), "ok".to_string(), 0.4);
    let almost_time = chrono::Utc::now() - chrono::Duration::seconds(14);
    almost_expired.last_updated = almost_time.to_rfc3339();

    let mut definitely_expired = AgentInfo::new("expired".to_string(), "ok".to_string(), 0.5);
    let expired_time = chrono::Utc::now() - chrono::Duration::seconds(20);
    definitely_expired.last_updated = expired_time.to_rfc3339();

    registry.register_agent(fresh);
    registry.register_agent(almost_expired);
    registry.register_agent_without_refresh(definitely_expired);

    // Act: Cleanup
    registry.force_cleanup_for_test();

    // Assert: Fresh and almost-expired should remain
    assert!(
        registry.get_agent("fresh").is_some(),
        "Fresh agent should remain"
    );
    assert!(
        registry.get_agent("almost").is_some(),
        "Almost expired agent should remain"
    );

    // Definitely expired should be removed
    assert!(
        registry.get_agent("expired").is_none(),
        "Expired agent should be removed"
    );
}

#[tokio::test]
#[ignore] // Requires actual time passage - run manually for thorough validation
async fn test_real_time_ttl_expiration() {
    // This test actually waits 16 seconds to verify TTL works with real time

    let registry = Arc::new(AgentRegistry::new());

    let agent = AgentInfo::new("time-test-agent".to_string(), "ok".to_string(), 0.5)
        .with_capabilities(vec!["time-test".to_string()]);

    registry.register_agent(agent);

    // Should be available immediately
    let routing_helper = RoutingHelper::new();
    let decision = routing_helper.find_best_agent_for_capability("time-test", &registry);
    assert!(
        matches!(decision, AgentSelectionDecision::RouteToAgent { .. }),
        "Agent should be available initially"
    );

    // Wait for TTL to expire (16 seconds)
    sleep(Duration::from_secs(16)).await;

    // Run cleanup
    registry.force_cleanup_for_test();

    // Should no longer be available
    let decision = routing_helper.find_best_agent_for_capability("time-test", &registry);
    assert!(
        matches!(decision, AgentSelectionDecision::NoRoute { .. }),
        "Agent should be expired after 16 seconds"
    );
}

#[tokio::test]
async fn test_malformed_timestamp_treated_as_expired() {
    // Test that agents with unparseable timestamps are considered expired

    let mut agent = AgentInfo::new("bad-timestamp-agent".to_string(), "ok".to_string(), 0.5);
    agent.last_updated = "not-a-valid-timestamp".to_string();

    // Should be considered expired
    assert!(
        agent.is_expired(),
        "Agent with bad timestamp should be treated as expired"
    );
}
