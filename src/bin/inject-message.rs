//! 2389 Agent Message Injection Utility
//!
//! A clean, simple tool for injecting test messages into running agents.
//! Perfect for experimentation and testing different agent behaviors.
//!
//! ## Usage
//!
//! ```bash
//! # Simple message
//! inject-message --agent-id dev-agent --message "Hello, please introduce yourself"
//!
//! # Tool execution request
//! inject-message --agent-id dev-agent --message "Get current weather" \
//!   --tool curl --tool-params '{"url": "http://api.weather.com", "method": "GET"}'
//!
//! # Chain to another agent
//! inject-message --agent-id dev-agent --message "Process and forward" \
//!   --next-agent processing-agent
//!
//! # Multi-step pipeline (researcher → writer → editor)
//! inject-message --agent-id researcher-agent \
//!   --message "Research and write an article about Rust async programming" \
//!   --next-agent "writer-agent,editor-agent"
//!
//! # Custom conversation
//! inject-message --agent-id dev-agent --conversation-id "my-experiment-1" \
//!   --message "Start experiment"
//! ```

use std::time::{SystemTime, UNIX_EPOCH};

use clap::Parser;
use rumqttc::{AsyncClient, MqttOptions, QoS};
use serde_json::{Value, json};
use tokio::time::{Duration, sleep};
use uuid::Uuid;

// Standalone implementation - no library dependencies due to compilation issues

#[derive(Parser)]
#[command(
    name = "inject-message",
    about = "Inject test messages into 2389 agents for experimentation",
    long_about = "A clean, simple tool for injecting test messages into running agents.\nPerfect for experimentation and testing different agent behaviors."
)]
struct Args {
    /// Target agent ID
    #[arg(long, required = true)]
    agent_id: String,

    /// Message to send to the agent
    #[arg(long, required = true)]
    message: String,

    /// Conversation ID (auto-generated if not provided)
    #[arg(long)]
    conversation_id: Option<String>,

    /// Tool name to request execution
    #[arg(long)]
    tool: Option<String>,

    /// Tool parameters as JSON string
    #[arg(long)]
    tool_params: Option<String>,

    /// Next agent(s) in pipeline (comma-separated for multi-step pipeline, e.g., "writer-agent,editor-agent")
    #[arg(long)]
    next_agent: Option<String>,

    /// MQTT broker URL
    #[arg(long, default_value = "localhost")]
    broker_url: String,

    /// MQTT broker port
    #[arg(long, default_value = "1883")]
    broker_port: u16,
}

struct MessageInjector {
    client: AsyncClient,
    connected: bool,
}

impl MessageInjector {
    /// Canonicalize MQTT topic according to 2389 protocol rules
    fn canonicalize_topic(&self, topic: &str) -> String {
        if topic.is_empty() {
            return "/".to_string();
        }

        // Rule 1: Ensure single leading slash
        let mut result = if topic.starts_with('/') {
            topic.to_string()
        } else {
            format!("/{topic}")
        };

        // Rule 3: Collapse multiple consecutive slashes
        while result.contains("//") {
            result = result.replace("//", "/");
        }

        // Rule 2: Remove trailing slashes (except for root "/")
        if result.len() > 1 && result.ends_with('/') {
            result.pop();
        }

        result
    }

    async fn new(broker_url: &str, broker_port: u16) -> Result<Self, Box<dyn std::error::Error>> {
        let client_id = format!(
            "inject-message-{}",
            SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs()
        );

        let mut mqttoptions = MqttOptions::new(client_id, broker_url, broker_port);
        mqttoptions.set_keep_alive(Duration::from_secs(60));

        let (client, mut eventloop) = AsyncClient::new(mqttoptions, 10);

        // Start event loop in background
        tokio::spawn(async move {
            loop {
                match eventloop.poll().await {
                    Ok(_) => {}
                    Err(e) => {
                        eprintln!("MQTT eventloop error: {e}");
                        break;
                    }
                }
            }
        });

        // Wait for connection
        println!("Connecting to MQTT broker {broker_url}:{broker_port}...");
        sleep(Duration::from_millis(1000)).await;

        Ok(MessageInjector {
            client,
            connected: true,
        })
    }

    fn create_task_envelope(
        &self,
        agent_id: &str,
        conversation_id: &str,
        instruction: &str,
        input_data: Value,
        next_agents: Option<&str>,
    ) -> Value {
        let topic = self.canonicalize_topic(&format!("/control/agents/{agent_id}/input"));

        // Build nested pipeline chain from comma-separated agent IDs
        let next = if let Some(agents_str) = next_agents {
            let agent_ids: Vec<&str> = agents_str.split(',').map(|s| s.trim()).collect();
            self.build_pipeline_chain(&agent_ids)
        } else {
            None
        };

        let mut task_envelope = json!({
            "task_id": Uuid::new_v4(),
            "conversation_id": conversation_id,
            "topic": topic,
            "instruction": instruction,
            "input": input_data
        });

        if let Some(next_val) = next {
            task_envelope["next"] = next_val;
        }

        task_envelope
    }

    /// Build nested pipeline chain from agent IDs
    fn build_pipeline_chain(&self, agent_ids: &[&str]) -> Option<Value> {
        if agent_ids.is_empty() {
            return None;
        }

        // Build from the end backwards to create proper nesting
        let mut current_next: Option<Value> = None;

        for agent_id in agent_ids.iter().rev() {
            current_next = Some(json!({
                "topic": self.canonicalize_topic(&format!("/control/agents/{agent_id}/input")),
                "instruction": format!("Continue processing for {}", agent_id),
                "input": null,
                "next": current_next
            }));
        }

        current_next
    }

    async fn inject_message(
        &self,
        agent_id: &str,
        message: &str,
        conversation_id: Option<&str>,
        tool_name: Option<&str>,
        tool_params: Option<&str>,
        next_agent: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if !self.connected {
            return Err("Not connected to MQTT broker".into());
        }

        // Generate conversation ID if not provided
        let conversation_id = conversation_id.map(|s| s.to_string()).unwrap_or_else(|| {
            format!(
                "experiment-{}",
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
            )
        });

        // Prepare input data
        let mut input_data = json!({"message": message});

        if let Some(tool) = tool_name {
            let params = if let Some(params_str) = tool_params {
                serde_json::from_str::<Value>(params_str)?
            } else {
                json!({})
            };

            input_data["tool_request"] = json!({
                "name": tool,
                "parameters": params
            });
        }

        // Create task envelope
        let task_envelope = self.create_task_envelope(
            agent_id,
            &conversation_id,
            &format!("Process this message: {message}"),
            input_data,
            next_agent,
        );

        // Publish to agent's input topic
        let topic = self.canonicalize_topic(&format!("/control/agents/{agent_id}/input"));
        let payload = serde_json::to_string_pretty(&task_envelope)?;

        println!("\n📤 Injecting message to {topic}");
        println!("   Conversation: {conversation_id}");
        println!("   Task ID: {}", task_envelope["task_id"]);
        if let Some(tool) = tool_name {
            println!("   Tool: {tool}");
        }
        if let Some(next) = next_agent {
            println!("   Pipeline: {} → {}", agent_id, next.replace(',', " → "));
        }
        println!("   Message: {message}");

        self.client
            .publish(topic, QoS::AtLeastOnce, false, payload)
            .await?;

        println!("✓ Message injected successfully");

        // Brief pause to allow message delivery
        sleep(Duration::from_millis(500)).await;

        println!("\n💡 Monitor agent responses at:");
        println!("   /conversations/{conversation_id}/{agent_id}");
        println!(
            "\n   Use: cargo run --bin monitor-mqtt -- --conversation {conversation_id} (when implemented)"
        );

        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Parse tool parameters if provided
    if let Some(ref params_str) = args.tool_params {
        if args.tool.is_none() {
            eprintln!("✗ --tool-params provided without --tool");
            std::process::exit(1);
        }

        // Validate JSON
        if let Err(e) = serde_json::from_str::<Value>(params_str) {
            eprintln!("✗ Invalid tool parameters JSON: {e}");
            std::process::exit(1);
        }
    }

    // Create injector and connect
    let injector = MessageInjector::new(&args.broker_url, args.broker_port).await?;

    // Inject the message
    if let Err(e) = injector
        .inject_message(
            &args.agent_id,
            &args.message,
            args.conversation_id.as_deref(),
            args.tool.as_deref(),
            args.tool_params.as_deref(),
            args.next_agent.as_deref(),
        )
        .await
    {
        eprintln!("✗ Failed to inject message: {e}");
        std::process::exit(1);
    }

    Ok(())
}
