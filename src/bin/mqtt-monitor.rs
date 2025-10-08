//! MQTT Monitor - 2389 Agent Protocol
//!
//! A clean Rust implementation for monitoring MQTT topics for agent communication.
//! Features 3-mode monitoring with syntax highlighted JSON payloads.

use clap::Parser;
use rumqttc::{AsyncClient, Event, EventLoop, MqttOptions, Packet, QoS};
use std::sync::Arc;
use tokio::signal;
use tracing::{error, info, warn};

/// MQTT Monitor for 2389 Agent Protocol
#[derive(Parser)]
#[command(name = "mqtt-monitor")]
#[command(about = "Monitor MQTT topics for agent communication")]
#[command(version)]
struct Args {
    /// Monitoring mode (availability, conversations, inputs, progress, or all)
    #[arg(short, long, default_value = "all")]
    mode: MonitorMode,

    /// Output format (pretty, compact, or json)
    #[arg(short, long, default_value = "pretty")]
    format: OutputFormat,

    /// Agent ID to monitor (for legacy compatibility and filtering)
    #[arg(long, default_value = "dev-agent")]
    agent_id: String,

    /// Filter by specific conversation ID (conversations mode only)
    #[arg(long)]
    conversation_id: Option<String>,

    /// MQTT broker host
    #[arg(long, default_value = "localhost")]
    broker_host: String,

    /// MQTT broker port
    #[arg(long, default_value_t = 1883)]
    broker_port: u16,

    /// MQTT username (optional)
    #[arg(long)]
    username: Option<String>,

    /// MQTT password (optional)
    #[arg(long)]
    password: Option<String>,

    /// Show only specific message types (legacy - use mode instead)
    #[arg(long)]
    filter: Option<String>,
}

/// Monitoring modes for different types of MQTT traffic
#[derive(Clone, Debug, clap::ValueEnum)]
enum MonitorMode {
    /// Monitor all traffic (legacy mode)
    All,
    /// Monitor agent availability and status messages
    Availability,
    /// Monitor conversation topics (agent-to-agent communication)
    Conversations,
    /// Monitor agent input topics (incoming task requests)
    Inputs,
    /// Monitor agent progress reporting
    Progress,
}

/// Output formatting options
#[derive(Clone, Debug, clap::ValueEnum)]
enum OutputFormat {
    /// Color-coded, human-readable with timestamps (default)
    Pretty,
    /// Single line per message, minimal formatting
    Compact,
    /// Raw JSON output for programmatic processing
    Json,
}

/// Message types with associated colors and mode relevance
#[derive(Debug, Clone, PartialEq)]
enum MessageType {
    /// Agent status messages (/control/agents/+/status)
    AgentStatus,
    /// General status and broadcast messages
    Status,
    /// Error messages
    Error,
    /// Agent input messages (/control/agents/+/input)
    Input,
    /// Conversation messages (/conversations/+/+)
    Output,
    /// Broadcast messages
    Broadcast,
    /// Progress messages (/control/agents/+/progress/*)
    Progress,
    /// Unknown message type
    Unknown,
}

impl MessageType {
    fn from_topic(topic: &str) -> Self {
        if topic.starts_with("/control/agents/") && topic.ends_with("/status") {
            Self::AgentStatus
        } else if topic.starts_with("/control/agents/") && topic.contains("/progress") {
            Self::Progress
        } else if topic.contains("/status") {
            Self::Status
        } else if topic.contains("/errors") {
            Self::Error
        } else if topic.contains("/input") {
            Self::Input
        } else if topic.starts_with("/conversations/") {
            Self::Output
        } else if topic.contains("/broadcast") {
            Self::Broadcast
        } else {
            Self::Unknown
        }
    }

    fn label(&self) -> &'static str {
        match self {
            Self::AgentStatus => "AGENT_STATUS",
            Self::Status => "STATUS",
            Self::Error => "ERROR",
            Self::Input => "INPUT",
            Self::Output => "CONVERSATION",
            Self::Broadcast => "BROADCAST",
            Self::Progress => "PROGRESS",
            Self::Unknown => "UNKNOWN",
        }
    }

    fn color_code(&self) -> &'static str {
        match self {
            Self::AgentStatus => "\x1b[1;36m", // Cyan
            Self::Status => "\x1b[1;33m",      // Yellow
            Self::Error => "\x1b[1;31m",       // Red
            Self::Input => "\x1b[1;34m",       // Blue
            Self::Output => "\x1b[1;32m",      // Green
            Self::Broadcast => "\x1b[1;35m",   // Magenta
            Self::Progress => "\x1b[1;93m",    // Bright Yellow
            Self::Unknown => "\x1b[0;37m",     // White
        }
    }

    /// Check if this message type should be shown in the given monitor mode
    fn is_relevant_for_mode(&self, mode: &MonitorMode) -> bool {
        match mode {
            MonitorMode::All => true,
            MonitorMode::Availability => {
                matches!(self, Self::AgentStatus | Self::Broadcast | Self::Status)
            }
            MonitorMode::Conversations => matches!(self, Self::Output),
            MonitorMode::Inputs => matches!(self, Self::Input),
            MonitorMode::Progress => matches!(self, Self::Progress),
        }
    }
}

const RESET: &str = "\x1b[0m";

/// ANSI color codes for JSON syntax highlighting
const JSON_KEY_COLOR: &str = "\x1b[94m"; // Bright blue
const JSON_STRING_COLOR: &str = "\x1b[92m"; // Bright green
const JSON_NUMBER_COLOR: &str = "\x1b[93m"; // Bright yellow
const JSON_BOOL_COLOR: &str = "\x1b[95m"; // Bright magenta
const JSON_NULL_COLOR: &str = "\x1b[90m"; // Dark gray
const JSON_PUNCT_COLOR: &str = "\x1b[37m"; // White

/// Apply syntax highlighting to JSON string
fn highlight_json(json_str: &str) -> String {
    let mut result = String::with_capacity(json_str.len() * 2);
    let chars: Vec<char> = json_str.chars().collect();
    let mut i = 0;
    let mut in_string = false;

    while i < chars.len() {
        let ch = chars[i];

        match ch {
            '"' => {
                if !in_string {
                    // Starting a string
                    in_string = true;
                    // Determine if this is a key by looking backwards for : after this string
                    let mut is_key = false;
                    let mut j = i + 1;
                    let mut quote_count = 1;

                    // Find the end of this string
                    while j < chars.len() && quote_count == 1 {
                        if chars[j] == '"' && (j == 0 || chars[j - 1] != '\\') {
                            quote_count += 1;
                        }
                        j += 1;
                    }

                    // Look for colon after string
                    while j < chars.len() {
                        match chars[j] {
                            ' ' | '\n' | '\t' => j += 1,
                            ':' => {
                                is_key = true;
                                break;
                            }
                            _ => break,
                        }
                    }

                    if is_key {
                        result.push_str(JSON_KEY_COLOR);
                    } else {
                        result.push_str(JSON_STRING_COLOR);
                    }
                } else {
                    // Ending a string (unless escaped)
                    let mut backslash_count = 0;
                    let mut j = i;
                    while j > 0 && chars[j - 1] == '\\' {
                        backslash_count += 1;
                        j -= 1;
                    }

                    if backslash_count % 2 == 0 {
                        // Not escaped, actually ending string
                        result.push(ch);
                        result.push_str(RESET);
                        in_string = false;
                        i += 1;
                        continue;
                    }
                }
                result.push(ch);
            }
            '{' | '[' | '}' | ']' | ':' | ',' => {
                if !in_string {
                    result.push_str(JSON_PUNCT_COLOR);
                    result.push(ch);
                    result.push_str(RESET);
                } else {
                    result.push(ch);
                }
            }
            _ if !in_string => {
                // Handle numbers, booleans, null
                if ch.is_ascii_digit() || ch == '-' {
                    // Start of a number
                    result.push_str(JSON_NUMBER_COLOR);
                    result.push(ch);
                    i += 1;

                    // Continue collecting the number
                    while i < chars.len() {
                        let num_ch = chars[i];
                        if num_ch.is_ascii_digit()
                            || num_ch == '.'
                            || num_ch == 'e'
                            || num_ch == 'E'
                            || num_ch == '+'
                            || num_ch == '-'
                        {
                            result.push(num_ch);
                            i += 1;
                        } else {
                            break;
                        }
                    }
                    result.push_str(RESET);
                    continue;
                } else if i + 4 <= chars.len()
                    && chars[i..i + 4].iter().collect::<String>() == "true"
                {
                    // Boolean true
                    result.push_str(JSON_BOOL_COLOR);
                    result.push_str("true");
                    result.push_str(RESET);
                    i += 3; // Skip the next 3 chars (will be incremented by 1 at end of loop)
                } else if i + 5 <= chars.len()
                    && chars[i..i + 5].iter().collect::<String>() == "false"
                {
                    // Boolean false
                    result.push_str(JSON_BOOL_COLOR);
                    result.push_str("false");
                    result.push_str(RESET);
                    i += 4; // Skip the next 4 chars
                } else if i + 4 <= chars.len()
                    && chars[i..i + 4].iter().collect::<String>() == "null"
                {
                    // Null value
                    result.push_str(JSON_NULL_COLOR);
                    result.push_str("null");
                    result.push_str(RESET);
                    i += 3; // Skip the next 3 chars
                } else {
                    result.push(ch);
                }
            }
            _ => {
                result.push(ch);
            }
        }

        i += 1;
    }

    // Make sure we end with reset if we were in a color
    if in_string {
        result.push_str(RESET);
    }

    result
}

fn format_message(
    msg_type: &MessageType,
    topic: &str,
    payload: &str,
    format: &OutputFormat,
    conversation_filter: Option<&String>,
) -> Option<String> {
    // Filter by conversation ID if specified
    if let Some(conv_id) = conversation_filter {
        if topic.starts_with("/conversations/") {
            let parts: Vec<&str> = topic.split('/').collect();
            if parts.len() >= 3 && parts[2] != conv_id {
                return None; // Skip messages not from the specified conversation
            }
        }
    }

    let timestamp = chrono::Utc::now().format("%H:%M:%S");

    match format {
        OutputFormat::Json => {
            let json_output = serde_json::json!({
                "timestamp": timestamp.to_string(),
                "message_type": msg_type.label(),
                "topic": topic,
                "payload": if let Ok(json) = serde_json::from_str::<serde_json::Value>(payload) {
                    json
                } else {
                    serde_json::Value::String(payload.to_string())
                }
            });
            Some(serde_json::to_string(&json_output).unwrap_or_else(|_| "{}".to_string()))
        }
        OutputFormat::Compact => Some(format!(
            "{} [{}] {} {}",
            timestamp,
            msg_type.label(),
            topic,
            payload.replace('\n', " ").trim()
        )),
        OutputFormat::Pretty => {
            let color = msg_type.color_code();
            let label = msg_type.label();

            // Pretty print JSON with syntax highlighting if possible
            let formatted_payload =
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(payload) {
                    let pretty_json =
                        serde_json::to_string_pretty(&json).unwrap_or_else(|_| payload.to_string());
                    highlight_json(&pretty_json)
                } else {
                    payload.to_string()
                };

            Some(format!(
                "{color}[{label}]{RESET} {timestamp} {topic}\n{formatted_payload}\n{RESET}"
            ))
        }
    }
}

async fn setup_mqtt_client(
    args: &Args,
) -> Result<(AsyncClient, EventLoop), Box<dyn std::error::Error>> {
    // Generate unique client ID to avoid conflicts
    let client_id = format!("mqtt-monitor-{}", std::process::id());
    let mut mqtt_options = MqttOptions::new(client_id, &args.broker_host, args.broker_port);

    if let (Some(username), Some(password)) = (&args.username, &args.password) {
        mqtt_options.set_credentials(username, password);
    }

    // Improved connection stability settings
    mqtt_options.set_keep_alive(std::time::Duration::from_secs(60));
    mqtt_options.set_max_packet_size(1024 * 1024, 1024 * 1024); // 1MB
    mqtt_options.set_clean_session(true); // Start fresh each time

    // Enable automatic reconnection with backoff
    mqtt_options.set_manual_acks(false);

    // Increase channel capacity for better throughput
    let (client, eventloop) = AsyncClient::new(mqtt_options, 100);
    Ok((client, eventloop))
}

async fn subscribe_to_topics(
    client: &AsyncClient,
    args: &Args,
) -> Result<(), rumqttc::ClientError> {
    match args.mode {
        MonitorMode::All => {
            info!("Subscribing to all topics for agent: {}", args.agent_id);

            // Agent-specific topics
            client
                .subscribe(
                    format!("/control/agents/{}/status", args.agent_id),
                    QoS::AtLeastOnce,
                )
                .await?;
            client
                .subscribe(
                    format!("/control/agents/{}/errors", args.agent_id),
                    QoS::AtLeastOnce,
                )
                .await?;
            client
                .subscribe(
                    format!("/control/agents/{}/input", args.agent_id),
                    QoS::AtLeastOnce,
                )
                .await?;
            client
                .subscribe(
                    format!("/conversations/+/{}", args.agent_id),
                    QoS::AtLeastOnce,
                )
                .await?;

            // General monitoring topics
            client
                .subscribe("/control/agents/+/status", QoS::AtLeastOnce)
                .await?;
            client
                .subscribe("/control/broadcast", QoS::AtLeastOnce)
                .await?;
        }
        MonitorMode::Availability => {
            info!("Subscribing to availability topics");

            // Only availability-related topics
            client
                .subscribe("/control/agents/+/status", QoS::AtLeastOnce)
                .await?;
            client
                .subscribe("/control/broadcast", QoS::AtLeastOnce)
                .await?;
        }
        MonitorMode::Conversations => {
            info!("Subscribing to conversation topics");

            if let Some(conv_id) = &args.conversation_id {
                // Specific conversation
                client
                    .subscribe(format!("/conversations/{conv_id}/+"), QoS::AtLeastOnce)
                    .await?;
            } else {
                // All conversations
                client
                    .subscribe("/conversations/+/+", QoS::AtLeastOnce)
                    .await?;
            }
        }
        MonitorMode::Inputs => {
            info!("Subscribing to agent input topics");

            // All agent input topics
            client
                .subscribe("/control/agents/+/input", QoS::AtLeastOnce)
                .await?;
        }
        MonitorMode::Progress => {
            info!("Subscribing to progress reporting topics");

            // All progress topics - general, tools, and LLM
            client
                .subscribe("/control/agents/+/progress", QoS::AtLeastOnce)
                .await?;
            client
                .subscribe("/control/agents/+/progress/tools", QoS::AtLeastOnce)
                .await?;
            client
                .subscribe("/control/agents/+/progress/llm", QoS::AtLeastOnce)
                .await?;
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter("mqtt_monitor=info,rumqttc=warn")
        .init();

    let args = Args::parse();

    println!("2389 Agent Protocol - MQTT Monitor");
    println!("==================================");
    println!("Mode: {:?}", args.mode);
    println!("Format: {:?}", args.format);
    println!("Agent ID: {}", args.agent_id);
    println!("MQTT Broker: {}:{}", args.broker_host, args.broker_port);

    if let Some(ref conv_id) = args.conversation_id {
        println!("Conversation Filter: {conv_id}");
    }
    if let Some(ref filter) = args.filter {
        println!("Legacy Filter: {filter}");
    }

    println!("Press Ctrl+C to stop monitoring");
    println!();

    // Show what we're monitoring
    match args.mode {
        MonitorMode::All => {
            println!("Monitoring ALL traffic for agent: {}", args.agent_id);
            println!("  - Agent status, errors, inputs, and conversations");
            println!("  - General broadcast messages");
        }
        MonitorMode::Availability => {
            println!("Monitoring AVAILABILITY traffic:");
            println!("  - /control/agents/+/status (agent status updates)");
            println!("  - /control/broadcast (broadcast messages)");
        }
        MonitorMode::Conversations => {
            if let Some(ref conv_id) = args.conversation_id {
                println!("Monitoring CONVERSATION traffic for: {conv_id}");
                println!("  - /conversations/{conv_id}/+ (specific conversation)");
            } else {
                println!("Monitoring ALL CONVERSATION traffic:");
                println!("  - /conversations/+/+ (all conversations)");
            }
        }
        MonitorMode::Inputs => {
            println!("Monitoring INPUT traffic:");
            println!("  - /control/agents/+/input (all agent inputs)");
        }
        MonitorMode::Progress => {
            println!("Monitoring PROGRESS reporting:");
            println!("  - /control/agents/+/progress (general progress)");
            println!("  - /control/agents/+/progress/tools (tool execution progress)");
            println!("  - /control/agents/+/progress/llm (LLM interaction progress)");
        }
    }
    println!();

    // Handle Ctrl+C gracefully
    let shutdown = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let shutdown_clone = shutdown.clone();

    tokio::spawn(async move {
        if let Err(e) = signal::ctrl_c().await {
            error!("Failed to listen for shutdown signal: {}", e);
        }
        info!("Shutdown signal received...");
        shutdown_clone.store(true, std::sync::atomic::Ordering::Relaxed);

        // If we don't exit within 2 seconds, force exit
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        warn!("Graceful shutdown timed out, forcing exit");
        std::process::exit(0);
    });

    // Main connection loop with automatic reconnection
    let mut reconnect_delay = 1;
    const MAX_RECONNECT_DELAY: u64 = 30;

    loop {
        if shutdown.load(std::sync::atomic::Ordering::Relaxed) {
            info!("Shutting down monitor...");
            break;
        }

        info!("Connecting to MQTT broker...");

        // Setup MQTT client
        let (client, mut eventloop) = match setup_mqtt_client(&args).await {
            Ok(client_and_loop) => client_and_loop,
            Err(e) => {
                error!("Failed to setup MQTT client: {}", e);
                tokio::time::sleep(std::time::Duration::from_secs(reconnect_delay)).await;
                reconnect_delay = std::cmp::min(reconnect_delay * 2, MAX_RECONNECT_DELAY);
                continue;
            }
        };

        // Subscribe to topics based on mode
        if let Err(e) = subscribe_to_topics(&client, &args).await {
            error!("Failed to subscribe to topics: {}", e);
            tokio::time::sleep(std::time::Duration::from_secs(reconnect_delay)).await;
            reconnect_delay = std::cmp::min(reconnect_delay * 2, MAX_RECONNECT_DELAY);
            continue;
        }

        // Reset reconnect delay on successful connection
        reconnect_delay = 1;
        let mut connection_stable = false;

        // Process MQTT events until disconnection
        loop {
            // Check for shutdown more frequently
            if shutdown.load(std::sync::atomic::Ordering::Relaxed) {
                info!("Disconnecting from MQTT broker...");
                // Give disconnect a short timeout, then force exit
                let disconnect_timeout = tokio::time::timeout(
                    std::time::Duration::from_millis(500),
                    client.disconnect(),
                )
                .await;

                if disconnect_timeout.is_err() {
                    warn!("Disconnect timed out, forcing exit");
                }
                return Ok(());
            }

            // Poll with timeout to allow regular shutdown checks
            let poll_result =
                tokio::time::timeout(std::time::Duration::from_millis(100), eventloop.poll()).await;

            match poll_result {
                Ok(Ok(Event::Incoming(Packet::Publish(publish)))) => {
                    let topic = &publish.topic;
                    let payload = String::from_utf8_lossy(&publish.payload);

                    let msg_type = MessageType::from_topic(topic);

                    // Skip messages not relevant to current mode
                    if !msg_type.is_relevant_for_mode(&args.mode) {
                        continue;
                    }

                    // Apply legacy filter if specified
                    if let Some(ref filter) = args.filter {
                        let filter_lower = filter.to_lowercase();
                        let label_lower = msg_type.label().to_lowercase();
                        if !label_lower.contains(&filter_lower) {
                            continue;
                        }
                    }

                    // Format and display message with syntax highlighting
                    if let Some(formatted) = format_message(
                        &msg_type,
                        topic,
                        &payload,
                        &args.format,
                        args.conversation_id.as_ref(),
                    ) {
                        match args.format {
                            OutputFormat::Json => println!("{formatted}"),
                            OutputFormat::Compact => println!("{formatted}"),
                            OutputFormat::Pretty => print!("{formatted}"),
                        }
                    }
                }
                Ok(Ok(Event::Incoming(Packet::ConnAck(_)))) => {
                    info!("✅ Connected to MQTT broker");
                    connection_stable = true;
                }
                Ok(Ok(Event::Incoming(Packet::SubAck(_)))) => {
                    info!("✅ Successfully subscribed to topics");
                }
                Ok(Ok(_)) => {} // Ignore other events
                Ok(Err(e)) => {
                    if connection_stable {
                        warn!("MQTT connection lost: {}", e);
                    } else {
                        error!("MQTT connection error during setup: {}", e);
                    }
                    break; // Exit inner loop to reconnect
                }
                Err(_) => {
                    // Timeout occurred, continue to check for shutdown
                    continue;
                }
            }
        }

        // Connection lost, wait before reconnecting
        if !shutdown.load(std::sync::atomic::Ordering::Relaxed) {
            warn!("Reconnecting in {} seconds...", reconnect_delay);
            tokio::time::sleep(std::time::Duration::from_secs(reconnect_delay)).await;
            reconnect_delay = std::cmp::min(reconnect_delay * 2, MAX_RECONNECT_DELAY);
        }
    }

    Ok(())
}
