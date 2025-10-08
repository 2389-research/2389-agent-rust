# Ignored Tests Audit

This document tracks all ignored tests in the codebase and their rationale.

## Summary

- **Total ignored tests:** 4
- **All have documented rationale:** ✅
- **Action required:** None for v0.1.0

## Test Details

### 1. `test_status_actually_expires_after_interval`
- **Location:** `tests/test_mqtt5_expiry_integration.rs:124`
- **Reason:** Requires waiting 3600 seconds (1 hour) to verify MQTT message expiry
- **Legitimacy:** ✅ Valid - impractical for CI
- **Status:** Should remain ignored for automated testing
- **Run manually:** When testing MQTT v5 expiry features

### 2. `test_process_task_timeout_handling`
- **Location:** `tests/test_agent_processor.rs:325`
- **Reason:** Causes 30s timeout wait during test execution
- **Legitimacy:** ✅ Valid - too slow for CI
- **Status:** Should remain ignored for automated testing
- **Run manually:** When testing timeout behavior
- **Note:** TEST_COVERAGE_SUMMARY.md recommends refactoring to use mock time

### 3. `test_real_time_ttl_expiration`
- **Location:** `tests/test_registry_ttl_expiration.rs:202`
- **Reason:** Requires actual 16 seconds of real time passage
- **Legitimacy:** ✅ Valid - too slow for CI
- **Status:** Should remain ignored for automated testing
- **Run manually:** For thorough TTL validation

### 4. `test_routing_disabled`
- **Location:** `src/processing/dynamic_routing_tests.rs:16`
- **Reason:** Placeholder test during v2.0 routing refactor (experimental)
- **Legitimacy:** ✅ Valid - v2.0 dynamic routing marked experimental in v0.1.0
- **Status:** Will be rewritten in v0.2 milestone
- **TODO:** Linked to v0.2 milestone for agent decision-based routing

## Recommendations

### For v0.1.0
No action required. All ignored tests have valid, documented rationale.

### For Future Releases

1. **Mock time refactoring (v0.2+)**
   - Consider refactoring timeout tests to use mock time instead of real delays
   - Would allow testing timeout behavior in milliseconds instead of 30+ seconds
   - See: `test_process_task_timeout_handling` and `test_real_time_ttl_expiration`

2. **v2.0 routing tests (v0.2 milestone)**
   - Implement tests for agent decision-based routing
   - Replace placeholder `test_routing_disabled` with real tests
   - See: `DYNAMIC_ROUTING_ANALYSIS.md` for implementation status

3. **MQTT expiry validation**
   - `test_status_actually_expires_after_interval` can remain ignored
   - Could add a faster variant that tests expiry with shorter intervals (e.g., 5 seconds)
   - Would require broker configuration to support shorter expiry for testing

## Quality Gate

✅ **All ignored tests reviewed and documented**
- No ignored tests without clear rationale
- All reasons are legitimate (too slow for CI or experimental features)
- Documentation updated for future reference
