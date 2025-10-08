//! RFC-compliant 2389 Agent Protocol - Main Entry Point
//!
//! This implements ONLY the functionality specified in the RFC.
//! No additional features beyond the RFC specification are allowed.

use agent2389::config::AgentConfig;
use agent2389::observability::{health::HealthServer, init_default_logging, metrics::metrics};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process;
use std::sync::Arc;
use tokio::{
    signal,
    time::{sleep, Duration},
};
use tracing::{error, info};

/// RFC-compliant 2389 Agent Protocol Implementation
#[derive(Parser)]
#[command(name = "agent2389")]
#[command(about = "RFC-compliant 2389 Agent Protocol implementation")]
#[command(version)]
struct Cli {
    /// Configuration file path
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,

    /// Verbose logging
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run the agent per RFC Section 7
    Run,
    /// Validate configuration per RFC Section 9
    Config {
        /// Show current configuration
        #[arg(long)]
        show: bool,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // Initialize observability system
    init_default_logging();

    info!(
        "Starting RFC-compliant 2389 Agent Protocol v{}",
        env!("CARGO_PKG_VERSION")
    );

    // Load configuration
    let config = match load_configuration(&cli.config).await {
        Ok(config) => config,
        Err(e) => {
            error!("Failed to load configuration: {}", e);
            process::exit(1);
        }
    };

    // Execute command
    let result = match cli.command {
        Commands::Run => run_agent(config).await,
        Commands::Config { show } => handle_config_command(config, show).await,
    };

    if let Err(e) = result {
        error!("Command failed: {}", e);
        process::exit(1);
    }

    info!("Application shutdown complete");
}

async fn load_configuration(
    config_path: &Option<PathBuf>,
) -> Result<AgentConfig, Box<dyn std::error::Error>> {
    match config_path {
        Some(path) => {
            info!("Loading configuration from: {}", path.display());
            Ok(AgentConfig::load_from_file(path)?)
        }
        None => {
            // Try default locations
            let default_paths = vec!["agent.toml", "config/agent.toml", "agent-rfc.toml"];

            for path_str in default_paths {
                let path = PathBuf::from(path_str);
                if path.exists() {
                    info!("Loading configuration from: {}", path.display());
                    return Ok(AgentConfig::load_from_file(&path)?);
                }
            }

            error!(
                "No configuration file found. Please provide one with -c/--config or create agent.toml"
            );
            process::exit(1);
        }
    }
}

async fn run_agent(config: AgentConfig) -> Result<(), Box<dyn std::error::Error>> {
    info!("Application starting with agent ID: {}", config.agent.id);

    // Initialize metrics
    let collector = metrics();
    collector.set_agent_state("initializing");

    // Bootstrap: Build agent with injected dependencies (Zen pattern)
    let mut agent = build_agent(config.clone()).await?;

    // Start health server
    let health_port = std::env::var("HEALTH_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);

    let health_server = Arc::new(HealthServer::new(config.agent.id.clone(), health_port));
    let health_server_clone = health_server.clone();

    tokio::spawn(async move {
        if let Err(e) = health_server_clone.start().await {
            error!("Health server error: {}", e);
        }
    });

    // Set health server on agent for task completion tracking
    agent.set_health_server(health_server.clone());

    // RFC Section 7.1: Initialize the agent
    agent.initialize().await?;
    collector.set_agent_state("initialized");

    // RFC Section 7.1: Start the agent
    agent.start().await?;
    collector.set_agent_state("running");

    // Update health server with MQTT status
    health_server.set_mqtt_connected(true).await;

    // Set up signal handling for graceful shutdown per RFC Section 7.2
    let mut sigint = signal::unix::signal(signal::unix::SignalKind::interrupt())?;
    let mut sigterm = signal::unix::signal(signal::unix::SignalKind::terminate())?;

    info!("Agent is running and waiting for tasks on MQTT...");

    // Wait for shutdown signals or permanent disconnection
    tokio::select! {
        _ = sigint.recv() => {
            info!("Received SIGINT, shutting down gracefully...");
        }
        _ = sigterm.recv() => {
            info!("Received SIGTERM, shutting down gracefully...");
        }
        _ = monitor_connection_health(&agent) => {
            error!("MQTT connection permanently lost, shutting down agent...");
            health_server.set_mqtt_connected(false).await;
        }
    }

    // RFC Section 7.2: Graceful shutdown
    info!("Application shutdown initiated");
    collector.set_agent_state("stopping");
    if let Err(e) = agent.shutdown().await {
        error!("Error during shutdown: {}", e);
        collector.set_agent_state("error");
        return Err(e.into());
    }

    collector.set_agent_state("stopped");
    Ok(())
}

/// Provider factory for creating LLM providers from configuration
struct LlmProviderFactory;

impl LlmProviderFactory {
    fn create_provider(
        config: &AgentConfig,
    ) -> Result<Box<dyn agent2389::llm::provider::LlmProvider>, Box<dyn std::error::Error>> {
        use agent2389::llm::providers::{
            AnthropicConfig, AnthropicProvider, OpenAiConfig, OpenAiProvider,
        };

        match config.llm.provider.as_str() {
            "openai" => {
                let api_key = config.get_llm_api_key()?;
                let openai_config = OpenAiConfig {
                    api_key,
                    ..Default::default()
                };
                let provider = OpenAiProvider::new(openai_config)?;
                Ok(Box::new(provider))
            }
            "anthropic" => {
                let api_key = config.get_llm_api_key()?;
                let anthropic_config = AnthropicConfig {
                    api_key,
                    ..Default::default()
                };
                let provider = AnthropicProvider::new(anthropic_config)?;
                Ok(Box::new(provider))
            }
            provider => Err(format!("Unsupported LLM provider: {provider}").into()),
        }
    }
}

/// Transport factory for creating transport instances
struct TransportFactory;

impl TransportFactory {
    async fn create_mqtt_transport(
        agent_id: &str,
        mqtt_config: agent2389::config::MqttSection,
    ) -> Result<agent2389::transport::mqtt::MqttClient, Box<dyn std::error::Error>> {
        Ok(agent2389::transport::mqtt::MqttClient::new(agent_id, mqtt_config).await?)
    }
}

/// Bootstrap factory - creates agent with injected dependencies
/// This is where all the coupling/factory logic lives, separated from business logic
async fn build_agent(
    config: AgentConfig,
) -> Result<
    agent2389::agent::AgentLifecycle<agent2389::transport::mqtt::MqttClient>,
    Box<dyn std::error::Error>,
> {
    // Create transport (injected dependency) - now using factory
    let transport =
        TransportFactory::create_mqtt_transport(&config.agent.id, config.mqtt.clone()).await?;

    // Create LLM provider (injected dependency) - now using factory
    let llm_provider = LlmProviderFactory::create_provider(&config)?;

    // Inject dependencies into AgentLifecycle (no factory logic in business logic)
    Ok(agent2389::agent::AgentLifecycle::new(
        config,
        transport,
        llm_provider,
    ))
}

async fn handle_config_command(
    config: AgentConfig,
    show: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if show {
        println!("Current RFC-compliant configuration:");
        println!("{}", toml::to_string_pretty(&config)?);
    }

    info!("Configuration validation complete");
    Ok(())
}

/// Monitor MQTT connection health and signal when permanently disconnected
async fn monitor_connection_health<T>(agent: &agent2389::agent::AgentLifecycle<T>)
where
    T: agent2389::transport::Transport,
{
    loop {
        if agent.is_permanently_disconnected() {
            break;
        }
        sleep(Duration::from_millis(100)).await;
    }
}
