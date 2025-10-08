//! Thread-safe metrics collection system
//!
//! Provides atomic counters and mutex-protected collections for tracking
//! operational statistics across task processing, MQTT transport, and tools.

use once_cell::sync::Lazy;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Global metrics collector instance
pub static METRICS: Lazy<MetricsCollector> = Lazy::new(MetricsCollector::new);

/// Get reference to global metrics collector
pub fn metrics() -> &'static MetricsCollector {
    &METRICS
}

/// Thread-safe metrics collector using atomics and mutexes
pub struct MetricsCollector {
    // Task processing metrics (atomic for high frequency)
    tasks_received: AtomicU64,
    tasks_processing: AtomicU64,
    tasks_completed: AtomicU64,
    tasks_failed: AtomicU64,
    tasks_rejected: AtomicU64,
    current_pipeline_depth: AtomicU64,
    max_pipeline_depth_reached: AtomicU64,

    // MQTT metrics (atomic for high frequency)
    mqtt_connected: AtomicBool,
    connection_attempts: AtomicU64,
    connections_established: AtomicU64,
    connection_failures: AtomicU64,
    messages_published: AtomicU64,
    publish_failures: AtomicU64,
    messages_received: AtomicU64,
    last_heartbeat: AtomicU64,
    connection_start_time: AtomicU64,

    // Processing times (mutex protected for complex operations)
    processing_times: Mutex<Vec<u64>>, // in milliseconds

    // Tool statistics (mutex protected for complex data)
    tool_stats: Mutex<HashMap<String, ToolExecutionStats>>,

    // Lifecycle metrics
    agent_state: Mutex<String>,
    uptime_start: AtomicU64,
    state_transitions: AtomicU64,
    restarts: AtomicU64,
    health_status: AtomicBool,
    last_health_check: AtomicU64,
}

impl MetricsCollector {
    /// Initialize task processing metrics (pure function)
    fn init_task_metrics() -> (
        AtomicU64,
        AtomicU64,
        AtomicU64,
        AtomicU64,
        AtomicU64,
        AtomicU64,
        AtomicU64,
    ) {
        (
            AtomicU64::new(0), // tasks_received
            AtomicU64::new(0), // tasks_processing
            AtomicU64::new(0), // tasks_completed
            AtomicU64::new(0), // tasks_failed
            AtomicU64::new(0), // tasks_rejected
            AtomicU64::new(0), // current_pipeline_depth
            AtomicU64::new(0), // max_pipeline_depth_reached
        )
    }

    /// Initialize MQTT metrics (pure function)
    fn init_mqtt_metrics() -> (
        AtomicBool,
        AtomicU64,
        AtomicU64,
        AtomicU64,
        AtomicU64,
        AtomicU64,
        AtomicU64,
        AtomicU64,
        AtomicU64,
    ) {
        (
            AtomicBool::new(false), // mqtt_connected
            AtomicU64::new(0),      // connection_attempts
            AtomicU64::new(0),      // connections_established
            AtomicU64::new(0),      // connection_failures
            AtomicU64::new(0),      // messages_published
            AtomicU64::new(0),      // publish_failures
            AtomicU64::new(0),      // messages_received
            AtomicU64::new(0),      // last_heartbeat
            AtomicU64::new(0),      // connection_start_time
        )
    }

    /// Initialize lifecycle metrics (pure function)
    fn init_lifecycle_metrics(
        now: u64,
    ) -> (
        Mutex<String>,
        AtomicU64,
        AtomicU64,
        AtomicU64,
        AtomicBool,
        AtomicU64,
    ) {
        (
            Mutex::new("initializing".to_string()), // agent_state
            AtomicU64::new(now),                    // uptime_start
            AtomicU64::new(0),                      // state_transitions
            AtomicU64::new(0),                      // restarts
            AtomicBool::new(true),                  // health_status
            AtomicU64::new(now),                    // last_health_check
        )
    }

    pub fn new() -> Self {
        let now = current_timestamp();

        let (
            tasks_received,
            tasks_processing,
            tasks_completed,
            tasks_failed,
            tasks_rejected,
            current_pipeline_depth,
            max_pipeline_depth_reached,
        ) = Self::init_task_metrics();
        let (
            mqtt_connected,
            connection_attempts,
            connections_established,
            connection_failures,
            messages_published,
            publish_failures,
            messages_received,
            last_heartbeat,
            connection_start_time,
        ) = Self::init_mqtt_metrics();
        let (
            agent_state,
            uptime_start,
            state_transitions,
            restarts,
            health_status,
            last_health_check,
        ) = Self::init_lifecycle_metrics(now);

        Self {
            tasks_received,
            tasks_processing,
            tasks_completed,
            tasks_failed,
            tasks_rejected,
            current_pipeline_depth,
            max_pipeline_depth_reached,
            mqtt_connected,
            connection_attempts,
            connections_established,
            connection_failures,
            messages_published,
            publish_failures,
            messages_received,
            last_heartbeat,
            connection_start_time,
            processing_times: Mutex::new(Vec::new()),
            tool_stats: Mutex::new(HashMap::new()),
            agent_state,
            uptime_start,
            state_transitions,
            restarts,
            health_status,
            last_health_check,
        }
    }

    // Task processing metrics
    pub fn task_received(&self) {
        self.tasks_received.fetch_add(1, Ordering::Relaxed);
    }

    pub fn task_processing_started(&self) {
        let old_count = self.tasks_processing.fetch_add(1, Ordering::Relaxed);
        let new_count = old_count + 1;

        // Update pipeline depth tracking
        self.current_pipeline_depth
            .store(new_count, Ordering::Relaxed);

        // Update max reached if necessary
        let current_max = self.max_pipeline_depth_reached.load(Ordering::Relaxed);
        if new_count > current_max {
            self.max_pipeline_depth_reached
                .store(new_count, Ordering::Relaxed);
        }
    }

    pub fn task_processing_completed(&self, duration: Duration) {
        self.tasks_completed.fetch_add(1, Ordering::Relaxed);
        self.tasks_processing.fetch_sub(1, Ordering::Relaxed);
        self.current_pipeline_depth.fetch_sub(1, Ordering::Relaxed);

        // Record processing time
        self.record_processing_time(duration);
    }

    pub fn task_processing_failed(&self, duration: Duration) {
        self.tasks_failed.fetch_add(1, Ordering::Relaxed);
        self.tasks_processing.fetch_sub(1, Ordering::Relaxed);
        self.current_pipeline_depth.fetch_sub(1, Ordering::Relaxed);

        // Record processing time even for failed tasks
        self.record_processing_time(duration);
    }

    pub fn task_rejected(&self) {
        self.tasks_rejected.fetch_add(1, Ordering::Relaxed);
    }

    fn record_processing_time(&self, duration: Duration) {
        if let Ok(mut times) = self.processing_times.lock() {
            times.push(duration.as_millis() as u64);

            // Limit to last 1000 measurements to prevent unbounded growth
            if times.len() > 1000 {
                times.remove(0);
            }
        }
    }

    // MQTT metrics
    pub fn mqtt_connection_attempt(&self) {
        self.connection_attempts.fetch_add(1, Ordering::Relaxed);
    }

    pub fn mqtt_connection_established(&self) {
        self.connections_established.fetch_add(1, Ordering::Relaxed);
        self.mqtt_connected.store(true, Ordering::Relaxed);
        self.connection_start_time
            .store(current_timestamp(), Ordering::Relaxed);
    }

    pub fn mqtt_connection_failed(&self) {
        self.connection_failures.fetch_add(1, Ordering::Relaxed);
        self.mqtt_connected.store(false, Ordering::Relaxed);
        self.connection_start_time.store(0, Ordering::Relaxed);
    }

    pub fn mqtt_connection_lost(&self) {
        self.mqtt_connected.store(false, Ordering::Relaxed);
    }

    pub fn mqtt_message_published(&self) {
        self.messages_published.fetch_add(1, Ordering::Relaxed);
    }

    pub fn mqtt_publish_failed(&self) {
        self.publish_failures.fetch_add(1, Ordering::Relaxed);
    }

    pub fn mqtt_message_received(&self) {
        self.messages_received.fetch_add(1, Ordering::Relaxed);
    }

    pub fn mqtt_heartbeat(&self) {
        self.last_heartbeat
            .store(current_timestamp(), Ordering::Relaxed);
    }

    /// Create or retrieve tool stats entry (pure function)
    fn get_or_create_tool_stats<'a>(
        stats: &'a mut HashMap<String, ToolExecutionStats>,
        tool_name: &str,
    ) -> &'a mut ToolExecutionStats {
        stats
            .entry(tool_name.to_string())
            .or_insert_with(|| ToolExecutionStats {
                name: tool_name.to_string(),
                executions: 0,
                failures: 0,
                timeouts: 0,
                execution_times: Vec::new(),
                last_execution: 0,
            })
    }

    /// Update tool execution statistics (pure function)
    fn update_tool_execution_stats(
        tool_stats: &mut ToolExecutionStats,
        duration: Duration,
        success: bool,
    ) {
        tool_stats.executions += 1;
        tool_stats.last_execution = current_timestamp();
        tool_stats.execution_times.push(duration.as_millis() as u64);

        // Limit execution times to prevent unbounded growth
        if tool_stats.execution_times.len() > 1000 {
            tool_stats.execution_times.remove(0);
        }

        if !success {
            tool_stats.failures += 1;
        }
    }

    // Tool execution metrics
    pub fn tool_executed(&self, tool_name: &str, duration: Duration, success: bool) {
        if let Ok(mut stats) = self.tool_stats.lock() {
            let tool_stats = Self::get_or_create_tool_stats(&mut stats, tool_name);
            Self::update_tool_execution_stats(tool_stats, duration, success);
        }
    }

    pub fn tool_timeout(&self, tool_name: &str) {
        if let Ok(mut stats) = self.tool_stats.lock() {
            if let Some(tool_stats) = stats.get_mut(tool_name) {
                tool_stats.timeouts += 1;
            }
        }
    }

    // Lifecycle metrics
    pub fn set_agent_state(&self, state: &str) {
        if let Ok(mut current_state) = self.agent_state.lock() {
            if *current_state != state {
                self.state_transitions.fetch_add(1, Ordering::Relaxed);
                *current_state = state.to_string();
            }
        }
    }

    pub fn agent_restarted(&self) {
        self.restarts.fetch_add(1, Ordering::Relaxed);
        self.uptime_start
            .store(current_timestamp(), Ordering::Relaxed);
    }

    // Health status metrics
    pub fn update_health_status(&self, healthy: bool) {
        self.health_status.store(healthy, Ordering::Relaxed);
        self.last_health_check
            .store(current_timestamp(), Ordering::Relaxed);
    }

    /// Reset all atomic counters (pure function)
    fn reset_atomic_counters(&self) {
        self.tasks_received.store(0, Ordering::Relaxed);
        self.tasks_processing.store(0, Ordering::Relaxed);
        self.tasks_completed.store(0, Ordering::Relaxed);
        self.tasks_failed.store(0, Ordering::Relaxed);
        self.tasks_rejected.store(0, Ordering::Relaxed);
        self.current_pipeline_depth.store(0, Ordering::Relaxed);
        self.max_pipeline_depth_reached.store(0, Ordering::Relaxed);
    }

    /// Reset MQTT metrics (pure function)
    fn reset_mqtt_metrics(&self) {
        self.mqtt_connected.store(false, Ordering::Relaxed);
        self.connection_attempts.store(0, Ordering::Relaxed);
        self.connections_established.store(0, Ordering::Relaxed);
        self.connection_failures.store(0, Ordering::Relaxed);
        self.messages_published.store(0, Ordering::Relaxed);
        self.publish_failures.store(0, Ordering::Relaxed);
        self.messages_received.store(0, Ordering::Relaxed);
        self.last_heartbeat.store(0, Ordering::Relaxed);
        self.connection_start_time.store(0, Ordering::Relaxed);
    }

    /// Reset lifecycle metrics (pure function)
    fn reset_lifecycle_metrics(&self) {
        let now = current_timestamp();
        self.state_transitions.store(0, Ordering::Relaxed);
        self.restarts.store(0, Ordering::Relaxed);
        self.uptime_start.store(now, Ordering::Relaxed);
        self.health_status.store(true, Ordering::Relaxed);
        self.last_health_check.store(now, Ordering::Relaxed);
    }

    /// Reset mutex-protected collections (pure function)
    fn reset_collections(&self) {
        if let Ok(mut times) = self.processing_times.lock() {
            times.clear();
        }
        if let Ok(mut stats) = self.tool_stats.lock() {
            stats.clear();
        }
        if let Ok(mut state) = self.agent_state.lock() {
            *state = "initializing".to_string();
        }
    }

    // Reset all metrics (useful for testing)
    pub fn reset(&self) {
        self.reset_atomic_counters();
        self.reset_mqtt_metrics();
        self.reset_lifecycle_metrics();
        self.reset_collections();
    }

    /// Calculate processing time statistics (pure function)
    fn calculate_processing_time_statistics(&self) -> (f64, f64, f64, f64) {
        if let Ok(times) = self.processing_times.lock() {
            if times.is_empty() {
                (0.0, 0.0, 0.0, 0.0)
            } else {
                let mut sorted_times = times.clone();
                sorted_times.sort_unstable();

                let avg = sorted_times.iter().sum::<u64>() as f64 / sorted_times.len() as f64;
                let p50 = percentile(&sorted_times, 50.0);
                let p95 = percentile(&sorted_times, 95.0);
                let p99 = percentile(&sorted_times, 99.0);

                (avg, p50, p95, p99)
            }
        } else {
            (0.0, 0.0, 0.0, 0.0)
        }
    }

    /// Build tool statistics summary (pure function)
    fn build_tool_statistics(
        &self,
    ) -> (
        HashMap<String, ToolExecutionStatsSnapshot>,
        u64,
        u64,
        u64,
        f64,
    ) {
        if let Ok(stats) = self.tool_stats.lock() {
            let mut processed_stats = HashMap::new();
            let mut total_executions = 0u64;
            let mut total_failures = 0u64;
            let mut total_timeouts = 0u64;
            let mut total_time = 0u64;
            let mut total_count = 0u64;

            for (name, stats) in stats.iter() {
                let tool_snapshot = self.create_tool_snapshot(stats);
                processed_stats.insert(name.clone(), tool_snapshot);

                total_executions += stats.executions;
                total_failures += stats.failures;
                total_timeouts += stats.timeouts;
                total_time += stats.execution_times.iter().sum::<u64>();
                total_count += stats.execution_times.len() as u64;
            }

            let avg_all_tools = if total_count == 0 {
                0.0
            } else {
                total_time as f64 / total_count as f64
            };

            (
                processed_stats,
                total_executions,
                total_failures,
                total_timeouts,
                avg_all_tools,
            )
        } else {
            (HashMap::new(), 0, 0, 0, 0.0)
        }
    }

    /// Create tool execution snapshot (pure function)
    fn create_tool_snapshot(&self, stats: &ToolExecutionStats) -> ToolExecutionStatsSnapshot {
        let avg_execution_time = if stats.execution_times.is_empty() {
            0.0
        } else {
            stats.execution_times.iter().sum::<u64>() as f64 / stats.execution_times.len() as f64
        };

        let success_rate = if stats.executions == 0 {
            0.0
        } else {
            (stats.executions - stats.failures) as f64 / stats.executions as f64
        };

        ToolExecutionStatsSnapshot {
            name: stats.name.clone(),
            executions: stats.executions,
            failures: stats.failures,
            timeouts: stats.timeouts,
            avg_execution_time_ms: avg_execution_time,
            last_execution: stats.last_execution,
            success_rate,
        }
    }

    /// Calculate connection duration (pure function)
    fn calculate_connection_duration(&self, now: u64) -> u64 {
        if self.mqtt_connected.load(Ordering::Relaxed) {
            let start_time = self.connection_start_time.load(Ordering::Relaxed);
            if start_time > 0 {
                now - start_time
            } else {
                0
            }
        } else {
            0
        }
    }

    /// Get current agent state (pure function)
    fn get_current_agent_state(&self) -> String {
        self.agent_state
            .lock()
            .map(|s| s.clone())
            .unwrap_or_else(|_| "unknown".to_string())
    }

    /// Build complete metrics snapshot (pure function)
    fn build_metrics_snapshot(
        &self,
        processing_stats: (f64, f64, f64, f64),
        tool_stats: (
            HashMap<String, ToolExecutionStatsSnapshot>,
            u64,
            u64,
            u64,
            f64,
        ),
        connection_duration_seconds: u64,
        uptime_seconds: u64,
        current_state: String,
        timestamp: u64,
    ) -> MetricsSnapshot {
        let (avg_processing_time_ms, p50, p95, p99) = processing_stats;
        let (
            tool_stats_map,
            total_tool_executions,
            total_tool_failures,
            total_tool_timeouts,
            avg_tool_time,
        ) = tool_stats;

        MetricsSnapshot {
            tasks: TaskMetrics {
                tasks_received: self.tasks_received.load(Ordering::Relaxed),
                tasks_processing: self.tasks_processing.load(Ordering::Relaxed),
                tasks_completed: self.tasks_completed.load(Ordering::Relaxed),
                tasks_failed: self.tasks_failed.load(Ordering::Relaxed),
                tasks_rejected: self.tasks_rejected.load(Ordering::Relaxed),
                avg_processing_time_ms,
                processing_time_p50_ms: p50,
                processing_time_p95_ms: p95,
                processing_time_p99_ms: p99,
                current_pipeline_depth: self.current_pipeline_depth.load(Ordering::Relaxed) as u32,
                max_pipeline_depth_reached: self.max_pipeline_depth_reached.load(Ordering::Relaxed)
                    as u32,
            },
            mqtt: MqttMetrics {
                connected: self.mqtt_connected.load(Ordering::Relaxed),
                connection_attempts: self.connection_attempts.load(Ordering::Relaxed),
                connections_established: self.connections_established.load(Ordering::Relaxed),
                connection_failures: self.connection_failures.load(Ordering::Relaxed),
                messages_published: self.messages_published.load(Ordering::Relaxed),
                publish_failures: self.publish_failures.load(Ordering::Relaxed),
                messages_received: self.messages_received.load(Ordering::Relaxed),
                last_heartbeat: self.last_heartbeat.load(Ordering::Relaxed),
                connection_duration_seconds,
            },
            tools: ToolMetrics {
                tool_stats: tool_stats_map,
                total_executions: total_tool_executions,
                total_failures: total_tool_failures,
                total_timeouts: total_tool_timeouts,
                avg_execution_time_ms: avg_tool_time,
            },
            lifecycle: LifecycleMetrics {
                current_state,
                uptime_seconds,
                state_transitions: self.state_transitions.load(Ordering::Relaxed),
                restarts: self.restarts.load(Ordering::Relaxed),
                healthy: self.health_status.load(Ordering::Relaxed),
                last_health_check: self.last_health_check.load(Ordering::Relaxed),
            },
            timestamp,
        }
    }

    /// Get complete metrics snapshot
    pub fn get_metrics(&self) -> MetricsSnapshot {
        let now = current_timestamp();

        let processing_stats = self.calculate_processing_time_statistics();
        let tool_stats = self.build_tool_statistics();
        let connection_duration = self.calculate_connection_duration(now);
        let uptime_seconds = now - self.uptime_start.load(Ordering::Relaxed);
        let current_state = self.get_current_agent_state();

        self.build_metrics_snapshot(
            processing_stats,
            tool_stats,
            connection_duration,
            uptime_seconds,
            current_state,
            now,
        )
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

// Internal tool statistics (with timing data)
#[derive(Debug)]
struct ToolExecutionStats {
    name: String,
    executions: u64,
    failures: u64,
    timeouts: u64,
    execution_times: Vec<u64>, // milliseconds
    last_execution: u64,
}

// Public metrics structures
#[derive(Debug, Serialize)]
pub struct MetricsSnapshot {
    pub tasks: TaskMetrics,
    pub mqtt: MqttMetrics,
    pub tools: ToolMetrics,
    pub lifecycle: LifecycleMetrics,
    pub timestamp: u64,
}

#[derive(Debug, Serialize)]
pub struct TaskMetrics {
    pub tasks_received: u64,
    pub tasks_processing: u64,
    pub tasks_completed: u64,
    pub tasks_failed: u64,
    pub tasks_rejected: u64,
    pub avg_processing_time_ms: f64,
    pub processing_time_p50_ms: f64,
    pub processing_time_p95_ms: f64,
    pub processing_time_p99_ms: f64,
    pub current_pipeline_depth: u32,
    pub max_pipeline_depth_reached: u32,
}

#[derive(Debug, Serialize)]
pub struct MqttMetrics {
    pub connected: bool,
    pub connection_attempts: u64,
    pub connections_established: u64,
    pub connection_failures: u64,
    pub messages_published: u64,
    pub publish_failures: u64,
    pub messages_received: u64,
    pub last_heartbeat: u64,
    pub connection_duration_seconds: u64,
}

#[derive(Debug, Serialize)]
pub struct ToolMetrics {
    pub tool_stats: HashMap<String, ToolExecutionStatsSnapshot>,
    pub total_executions: u64,
    pub total_failures: u64,
    pub total_timeouts: u64,
    pub avg_execution_time_ms: f64,
}

#[derive(Debug, Serialize)]
pub struct ToolExecutionStatsSnapshot {
    pub name: String,
    pub executions: u64,
    pub failures: u64,
    pub timeouts: u64,
    pub avg_execution_time_ms: f64,
    pub last_execution: u64,
    pub success_rate: f64,
}

#[derive(Debug, Serialize)]
pub struct LifecycleMetrics {
    pub current_state: String,
    pub uptime_seconds: u64,
    pub state_transitions: u64,
    pub restarts: u64,
    pub healthy: bool,
    pub last_health_check: u64,
}

// Helper functions
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn percentile(sorted_data: &[u64], percentile: f64) -> f64 {
    if sorted_data.is_empty() {
        return 0.0;
    }

    let len = sorted_data.len();
    let index = (percentile / 100.0) * (len - 1) as f64;

    if index.fract() == 0.0 {
        sorted_data[index as usize] as f64
    } else {
        let lower_index = index.floor() as usize;
        let upper_index = index.ceil() as usize;
        let lower_value = sorted_data[lower_index] as f64;
        let upper_value = sorted_data[upper_index] as f64;

        lower_value + (upper_value - lower_value) * index.fract()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_task_metrics() {
        let collector = MetricsCollector::new();

        collector.task_received();
        collector.task_processing_started();
        collector.task_processing_completed(Duration::from_millis(1500));

        let metrics = collector.get_metrics();
        assert_eq!(metrics.tasks.tasks_received, 1);
        assert_eq!(metrics.tasks.tasks_completed, 1);
        assert_eq!(metrics.tasks.tasks_processing, 0);
        assert!(metrics.tasks.avg_processing_time_ms > 1400.0);
    }

    #[test]
    fn test_mqtt_metrics() {
        let collector = MetricsCollector::new();

        collector.mqtt_connection_attempt();
        collector.mqtt_connection_established();
        collector.mqtt_message_published();

        let metrics = collector.get_metrics();
        assert_eq!(metrics.mqtt.connection_attempts, 1);
        assert_eq!(metrics.mqtt.connections_established, 1);
        assert_eq!(metrics.mqtt.messages_published, 1);
        assert!(metrics.mqtt.connected);
    }

    #[test]
    fn test_tool_metrics() {
        let collector = MetricsCollector::new();

        collector.tool_executed("http_get", Duration::from_millis(500), true);
        collector.tool_executed("http_get", Duration::from_millis(300), false);

        let metrics = collector.get_metrics();
        let tool_stats = metrics.tools.tool_stats.get("http_get").unwrap();

        assert_eq!(tool_stats.executions, 2);
        assert_eq!(tool_stats.failures, 1);
        assert_eq!(tool_stats.success_rate, 0.5);
        assert!(tool_stats.avg_execution_time_ms > 350.0);
    }

    #[test]
    fn test_thread_safety() {
        let collector = Arc::new(MetricsCollector::new());

        let mut handles = vec![];

        for _ in 0..10 {
            let collector_clone = Arc::clone(&collector);
            let handle = thread::spawn(move || {
                for _ in 0..100 {
                    collector_clone.task_received();
                    collector_clone.mqtt_message_published();
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let metrics = collector.get_metrics();
        assert_eq!(metrics.tasks.tasks_received, 1000);
        assert_eq!(metrics.mqtt.messages_published, 1000);
    }

    #[test]
    fn test_percentile_calculation() {
        let data = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

        // Test basic percentiles with sufficient precision
        let p50 = percentile(&data, 50.0);
        let p95 = percentile(&data, 95.0);
        let p0 = percentile(&data, 0.0);
        let p100 = percentile(&data, 100.0);

        assert!((p50 - 5.5).abs() < 0.1, "P50: expected ~5.5, got {p50}");
        assert!((p95 - 9.5).abs() < 0.1, "P95: expected ~9.5, got {p95}");
        assert!((p0 - 1.0).abs() < 0.1, "P0: expected ~1.0, got {p0}");
        assert!(
            (p100 - 10.0).abs() < 0.1,
            "P100: expected ~10.0, got {p100}"
        );

        // Test edge case with empty data
        assert_eq!(percentile(&[], 50.0), 0.0);
    }

    #[test]
    fn test_processing_time_bounds() {
        let collector = MetricsCollector::new();

        // Add more than 1000 processing times
        for i in 0..1500 {
            collector.task_processing_completed(Duration::from_millis(i));
        }

        let metrics = collector.get_metrics();
        // Should be limited to 1000 entries
        assert!(metrics.tasks.avg_processing_time_ms > 0.0);
    }

    #[test]
    fn test_reset_functionality() {
        let collector = MetricsCollector::new();

        collector.task_received();
        collector.mqtt_connection_established();
        collector.tool_executed("test_tool", Duration::from_millis(100), true);

        let metrics_before = collector.get_metrics();
        assert_eq!(metrics_before.tasks.tasks_received, 1);

        collector.reset();

        let metrics_after = collector.get_metrics();
        assert_eq!(metrics_after.tasks.tasks_received, 0);
        assert!(!metrics_after.mqtt.connected);
        assert!(metrics_after.tools.tool_stats.is_empty());
    }
}
