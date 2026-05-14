mod ace_client;
mod config;
mod demo;
mod sap;
mod workflow;
mod x402;

use anyhow::Result;
use clap::Parser;
use solana_rpc_client::rpc_client::RpcClient;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::signer::Signer;
use std::time::Duration;
use tracing::{error, info};

#[derive(Parser, Debug)]
#[command(name = "trendforge", about = "Autonomous SAP agent — TrendForge")]
struct Cli {
    /// Run once and exit
    #[arg(long)]
    once: bool,

    /// Override the search query
    #[arg(long)]
    query: Option<String>,

    /// Run demo mode (no wallet/funds needed)
    #[arg(long)]
    demo: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("trendforge=info".parse()?),
        )
        .init();

    let cli = Cli::parse();

    if cli.demo {
        return demo::run().await;
    }

    let mut config = config::Config::from_env()?;
    if let Some(q) = cli.query {
        config.search_query = q;
    }

    let payer = config.load_keypair()?;
    info!(wallet = %payer.pubkey(), "TrendForge agent starting");

    let rpc = RpcClient::new_with_commitment(
        config.synapse_rpc_url.clone(),
        CommitmentConfig::confirmed(),
    );

    if sap::is_registered(&rpc, &payer.pubkey()) {
        info!("Agent confirmed registered on SAP mainnet");
    } else {
        info!("Agent not yet registered — run `cargo run --bin register` first");
    }

    if cli.once {
        let report = workflow::run_once(&config, &rpc, &payer).await?;
        println!("\n{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }

    let interval = Duration::from_secs(config.workflow_interval_secs);
    loop {
        match workflow::run_once(&config, &rpc, &payer).await {
            Ok(report) => {
                info!(
                    query = report.query,
                    agents = report.sap_agents_discovered,
                    "workflow complete — sleeping {}s",
                    config.workflow_interval_secs
                );
            }
            Err(e) => {
                error!("workflow error: {e:#}");
            }
        }
        tokio::time::sleep(interval).await;
    }
}
