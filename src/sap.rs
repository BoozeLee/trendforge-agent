// SAP on-chain tool discovery via Solana RPC.
// Reads AgentAccount accounts from the SAP program to find available agents/tools.

use anyhow::{Context, Result};
use solana_client::rpc_filter::{Memcmp, RpcFilterType};
use std::str::FromStr;
use solana_rpc_client::rpc_client::RpcClient;
use solana_rpc_client_api::config::{RpcAccountInfoConfig, RpcProgramAccountsConfig};
use solana_sdk::pubkey::Pubkey;
use tracing::info;

pub const SAP_PROGRAM_ID: &str = "SAPpUhsWLJG1FfkGRcXagEDMrMsWGjbky7AyhGpFETZ";
#[allow(dead_code)]
pub const GLOBAL_REGISTRY: &str = "9odFrYBBZq6UQC6aGyzMPNXWJQn55kMtfigzhLg6S6L5";

// Synapse Sentinel agent (registered on SAP mainnet)
pub const SYNAPSE_SENTINEL: &str = "Ccr2yK3hLALU4p8oNRqrh4dGuvPJTth5KCLMio8cE1ph";

/// Minimal decoded form of an on-chain AgentAccount.
#[derive(Debug, Clone)]
pub struct AgentInfo {
    pub wallet: Pubkey,
    pub name: String,
    pub description: String,
    pub is_active: bool,
    pub x402_endpoint: Option<String>,
}

/// Fetch active agents from the SAP program (first 20).
pub fn fetch_agents(rpc: &RpcClient) -> Result<Vec<AgentInfo>> {
    let program = Pubkey::from_str(SAP_PROGRAM_ID).context("parse SAP program ID")?;

    // Anchor discriminator for AgentAccount = sha256("account:AgentAccount")[0..8]
    let discriminator = anchor_discriminator("account:AgentAccount");

    let config = RpcProgramAccountsConfig {
        filters: Some(vec![RpcFilterType::Memcmp(Memcmp::new_raw_bytes(
            0,
            discriminator.to_vec(),
        ))]),
        account_config: RpcAccountInfoConfig {
            encoding: Some(solana_account_decoder_client_types::UiAccountEncoding::Base64),
            ..Default::default()
        },
        with_context: Some(false),
        sort_results: None,
    };

    let accounts = rpc
        .get_program_accounts_with_config(&program, config)
        .context("getProgramAccounts for AgentAccount")?;

    info!(count = accounts.len(), "fetched SAP agent accounts");

    let mut agents = Vec::new();
    for (_pubkey, account) in accounts.iter().take(20) {
        if let Some(agent) = parse_agent_account(&account.data) {
            agents.push(agent);
        }
    }

    Ok(agents)
}

/// Parse minimal AgentAccount fields from raw Borsh bytes.
/// Layout: [8 disc][32 wallet][1 bump][1 version][1 is_active][4+N name][4+N description]...
fn parse_agent_account(data: &[u8]) -> Option<AgentInfo> {
    if data.len() < 48 {
        return None;
    }
    let mut cursor = 8usize; // skip discriminator

    let wallet = Pubkey::try_from(&data[cursor..cursor + 32]).ok()?;
    cursor += 32;

    if data.len() < cursor + 3 {
        return None;
    }
    let _bump = data[cursor];
    cursor += 1;
    let _version = data[cursor];
    cursor += 1;
    let is_active = data[cursor] != 0;
    cursor += 1;

    let name = read_string(data, &mut cursor)?;
    let description = read_string(data, &mut cursor)?;

    Some(AgentInfo {
        wallet,
        name,
        description,
        is_active,
        x402_endpoint: None,
    })
}

fn read_string(data: &[u8], cursor: &mut usize) -> Option<String> {
    if data.len() < *cursor + 4 {
        return None;
    }
    let len = u32::from_le_bytes(data[*cursor..*cursor + 4].try_into().ok()?) as usize;
    *cursor += 4;
    if data.len() < *cursor + len {
        return None;
    }
    let s = String::from_utf8(data[*cursor..*cursor + len].to_vec()).ok()?;
    *cursor += len;
    Some(s)
}

fn anchor_discriminator(namespace: &str) -> [u8; 8] {
    use sha2::{Digest, Sha256};
    let hash = Sha256::digest(namespace.as_bytes());
    hash[..8].try_into().unwrap()
}

/// Derive the agent PDA for a given wallet.
pub fn agent_pda(wallet: &Pubkey) -> Pubkey {
    let program = Pubkey::from_str(SAP_PROGRAM_ID).unwrap();
    Pubkey::find_program_address(&[b"sap_agent", wallet.as_ref()], &program).0
}



/// Check if our agent is registered on-chain.
pub fn is_registered(rpc: &RpcClient, wallet: &Pubkey) -> bool {
    let pda = agent_pda(wallet);
    rpc.get_account(&pda).is_ok()
}
