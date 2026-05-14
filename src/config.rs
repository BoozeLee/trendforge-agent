use anyhow::{Context, Result};
use solana_sdk::signature::Keypair;

#[derive(Debug, Clone)]
pub struct Config {
    pub synapse_rpc_url: String,
    pub ace_api_base: String,
    pub solana_keypair_path: String,
    pub usdc_mint: String,
    pub workflow_interval_secs: u64,
    pub search_query: String,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        dotenv::dotenv().ok();

        Ok(Self {
            synapse_rpc_url: std::env::var("SYNAPSE_RPC_URL")
                .unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".into()),
            ace_api_base: std::env::var("ACE_API_BASE")
                .unwrap_or_else(|_| "https://api.acedata.cloud".into()),
            solana_keypair_path: std::env::var("SOLANA_KEYPAIR_PATH")
                .unwrap_or_else(|_| "keys/agent.json".into()),
            usdc_mint: std::env::var("USDC_MINT")
                .unwrap_or_else(|_| "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".into()),
            workflow_interval_secs: std::env::var("WORKFLOW_INTERVAL_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(3600),
            search_query: std::env::var("SEARCH_QUERY")
                .unwrap_or_else(|_| "AI agent autonomous Solana 2026".into()),
        })
    }

    pub fn load_keypair(&self) -> Result<Keypair> {
        let path = &self.solana_keypair_path;
        if path.starts_with('[') {
            let bytes: Vec<u8> = serde_json::from_str(path)
                .context("parse inline keypair JSON")?;
            return Keypair::try_from(bytes.as_slice()).context("build keypair from bytes");
        }
        let contents = std::fs::read_to_string(path)
            .with_context(|| format!("read keypair file: {path}"))?;
        let bytes: Vec<u8> = serde_json::from_str(contents.trim())
            .context("parse keypair JSON")?;
        Keypair::try_from(bytes.as_slice()).context("build keypair from bytes")
    }
}
