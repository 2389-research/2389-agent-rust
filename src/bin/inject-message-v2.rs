//! 2389 Agent V2 Message Injection Utility
//!
//! A clean tool for injecting v2.0 TaskEnvelope messages with workflow context.
//! Minimal required inputs with smart defaults.
//!
//! ## Usage
//!
//! ```bash
//! # Minimal - just query and first agent
//! inject-message-v2 --query "Research Rust async programming" --agent researcher-agent
//!
//! # With custom conversation ID
//! inject-message-v2 --query "Write a blog post about AI" --agent writer-agent --conversation-id my-test-1
//!
//! # With additional input data
//! inject-message-v2 --query "Analyze data" --agent data-agent --input '{"dataset": "sales.csv"}'
//! ```

use clap::Parser;
use rumqttc::{AsyncClient, MqttOptions, QoS};
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::time::{Duration, sleep};
use uuid::Uuid;

#[derive(Parser)]
#[command(
    name = "inject-message-v2",
    about = "Inject v2.0 workflow messages into 2389 agents",
    long_about = "A clean tool for injecting v2.0 TaskEnvelope messages with workflow context.\nMinimal required inputs with smart defaults."
)]
struct Args {
    /// The user's original query/request
    #[arg(long, required = true)]
    query: String,

    /// First agent to send the message to
    #[arg(long, required = true)]
    agent: String,

    /// Conversation ID (auto-generated if not provided)
    #[arg(long)]
    conversation_id: Option<String>,

    /// Additional input data as JSON string
    #[arg(long)]
    input: Option<String>,

    /// Custom instruction (defaults to query)
    #[arg(long)]
    instruction: Option<String>,

    /// MQTT broker host
    #[arg(long, default_value = "localhost")]
    broker_host: String,

    /// MQTT broker port
    #[arg(long, default_value = "1883")]
    broker_port: u16,
}

struct V2Injector {
    client: AsyncClient,
}

impl V2Injector {
    async fn new(broker_host: &str, broker_port: u16) -> Result<Self, Box<dyn std::error::Error>> {
        let client_id = format!(
            "inject-v2-{}",
            SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs()
        );

        let mut mqttoptions = MqttOptions::new(client_id, broker_host, broker_port);
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
        println!("Connecting to MQTT broker {broker_host}:{broker_port}...");
        sleep(Duration::from_millis(1000)).await;

        Ok(V2Injector { client })
    }

    async fn inject_v2_message(
        &self,
        query: &str,
        agent_id: &str,
        conversation_id: Option<&str>,
        input_data: Option<&str>,
        instruction: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Generate conversation ID if not provided
        let conversation_id = conversation_id.map(|s| s.to_string()).unwrap_or_else(|| {
            format!(
                "v2-{}",
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
            )
        });

        // Parse input data or use empty object
        let input = if let Some(input_str) = input_data {
            serde_json::from_str(input_str)?
        } else {
            json!({})
        };

        // Use custom instruction or default to query
        let instruction_text = instruction.unwrap_or(query);

        // Build v2.0 TaskEnvelope
        let task_envelope = json!({
            "task_id": Uuid::new_v4(),
            "conversation_id": conversation_id,
            "topic": format!("/control/agents/{}/input", agent_id),
            "instruction": instruction_text,
            "input": input,
            "version": "2.0",
            "context": {
                "original_query": query,
                "steps_completed": [],
                "iteration_count": 0
            }
        });

        // Publish to agent's input topic
        let topic = format!("/control/agents/{agent_id}/input");
        let payload = serde_json::to_string_pretty(&task_envelope)?;

        println!("\n📤 Injecting v2.0 message to {topic}");
        println!("   Conversation: {conversation_id}");
        println!("   Task ID: {}", task_envelope["task_id"]);
        println!("   Query: {query}");
        println!("   First Agent: {agent_id}");

        self.client
            .publish(topic, QoS::AtLeastOnce, false, payload)
            .await?;

        println!("✓ V2 message injected successfully");

        // Brief pause to allow message delivery
        sleep(Duration::from_millis(500)).await;

        println!("\n💡 Monitor conversation at:");
        println!(
            "   cargo run --bin mqtt-monitor -- --mode conversations --conversation-id {conversation_id}"
        );

        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Validate input JSON if provided
    if let Some(ref input_str) = args.input {
        if let Err(e) = serde_json::from_str::<serde_json::Value>(input_str) {
            eprintln!("✗ Invalid input JSON: {e}");
            std::process::exit(1);
        }
    }

    // Create injector and connect
    let injector = V2Injector::new(&args.broker_host, args.broker_port).await?;

    // Inject the v2 message
    if let Err(e) = injector
        .inject_v2_message(
            &args.query,
            &args.agent,
            args.conversation_id.as_deref(),
            args.input.as_deref(),
            args.instruction.as_deref(),
        )
        .await
    {
        eprintln!("✗ Failed to inject v2 message: {e}");
        std::process::exit(1);
    }

    Ok(())
}
