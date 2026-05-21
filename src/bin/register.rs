// One-time SAP agent registration on mainnet.
// Run: cargo run --bin register

use anyhow::{Context, Result};
use borsh::BorshSerialize;
use solana_rpc_client::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    instruction::{AccountMeta, Instruction},
    message::Message,
    pubkey::Pubkey,
    signature::Keypair,
    signer::Signer,
    transaction::Transaction,
};
use std::str::FromStr;

const SAP_PROGRAM_ID: &str = "SAPpUhsWLJG1FfkGRcXagEDMrMsWGjbky7AyhGpFETZ";
const GLOBAL_REGISTRY: &str = "9odFrYBBZq6UQC6aGyzMPNXWJQn55kMtfigzhLg6S6L5";

#[derive(BorshSerialize)]
struct Capability {
    id: String,
    description: Option<String>,
    protocol_id: Option<String>,
    version: Option<String>,
}

#[derive(BorshSerialize)]
struct RegisterAgentArgs {
    name: String,
    description: String,
    capabilities: Vec<Capability>,
    pricing: Vec<String>,
    protocols: Vec<String>,
    agent_id: Option<String>,
    agent_uri: Option<String>,
    x402_endpoint: Option<String>,
}

fn anchor_discriminator(namespace: &str) -> [u8; 8] {
    use sha2::{Digest, Sha256};
    let h = Sha256::digest(namespace.as_bytes());
    h[..8].try_into().unwrap()
}

fn pda(seeds: &[&[u8]], program: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(seeds, program).0
}

fn load_keypair(path: &str) -> Result<Keypair> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("read keypair {path}"))?;
    let bytes: Vec<u8> = serde_json::from_str(raw.trim())?;
    Keypair::try_from(bytes.as_slice()).context("build keypair")
}

fn main() -> Result<()> {
    dotenv::dotenv().ok();

    let keypair_path = std::env::var("SOLANA_KEYPAIR_PATH").unwrap_or_else(|_| "keys/agent.json".into());
    let rpc_url = std::env::var("SYNAPSE_RPC_URL")
        .unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".into());

    let payer = load_keypair(&keypair_path)?;
    let rpc = RpcClient::new_with_commitment(rpc_url, CommitmentConfig::confirmed());

    let program = Pubkey::from_str(SAP_PROGRAM_ID).unwrap();
    let global_registry = Pubkey::from_str(GLOBAL_REGISTRY).unwrap();

    let agent_pda = pda(&[b"sap_agent", payer.pubkey().as_ref()], &program);
    let stats_pda = pda(&[b"sap_stats", payer.pubkey().as_ref()], &program);

    if rpc.get_account(&agent_pda).is_ok() {
        println!("Agent already registered: {}", payer.pubkey());
        return Ok(());
    }

    println!("Registering {} on SAP mainnet…", payer.pubkey());

    let args = RegisterAgentArgs {
        name: "TrendForge".into(),
        description: "Autonomous research agent: SAP discovery → SERP search → LLM analysis \
                      → image generation, settled via x402 on Solana."
            .into(),
        capabilities: vec![Capability {
            id: "trend:research".into(),
            description: Some("Web search + LLM + image gen via Ace Data Cloud x402".into()),
            protocol_id: Some("acedata".to_string()),
            version: Some("1.0.0".to_string()),
        }],
        pricing: vec![],
        protocols: vec!["acedata".into()],
        agent_id: Some("trendforge-v1".into()),
        agent_uri: Some("https://github.com/BoozeLee/trendforge-agent".into()),
        x402_endpoint: Some(
            std::env::var("X402_ENDPOINT")
                .unwrap_or_else(|_| "https://trendforge-agent.onrender.com".into())
        ),
    };

    let disc = anchor_discriminator("global:register_agent");
    let mut data = disc.to_vec();
    borsh::to_writer(&mut data, &args)?;

    let ix = Instruction {
        program_id: program,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(agent_pda, false),
            AccountMeta::new(stats_pda, false),
            AccountMeta::new(global_registry, false),
            AccountMeta::new_readonly(Pubkey::default(), false),
        ],
        data,
    };

    let blockhash = rpc.get_latest_blockhash()?;
    let msg = Message::new_with_blockhash(&[ix], Some(&payer.pubkey()), &blockhash);
    let tx = Transaction::new(&[&payer], msg, blockhash);

    let sig = rpc
        .send_and_confirm_transaction_with_spinner(&tx)
        .context("register_agent tx failed")?;

    println!("✓ Registered! Tx: {sig}");
    println!("  Explorer: https://explorer.oobeprotocol.ai/agents/{}", payer.pubkey());
    Ok(())
}
