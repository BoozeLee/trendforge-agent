// x402 Solana USDC payment signing.
// Protocol: make API call → get 402 → build + sign SPL TransferChecked tx
// → encode X-Payment header → retry.

use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use serde::{Deserialize, Serialize};
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

// ─── 402 response types ───────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct PaymentRequiredBody {
    pub accepts: Vec<PaymentRequirement>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PaymentRequirement {
    pub scheme: Option<String>,
    pub network: Option<String>,
    #[serde(rename = "maxAmountRequired")]
    pub max_amount_required: String,
    #[serde(rename = "payTo")]
    pub pay_to: String,
    pub asset: String,
    pub extra: Option<PaymentExtra>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PaymentExtra {
    pub decimals: Option<u8>,
}

// ─── X-Payment envelope ───────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct X402Envelope {
    #[serde(rename = "x402Version")]
    pub x402_version: u32,
    pub scheme: String,
    pub network: String,
    pub payload: SolanaPayload,
}

#[derive(Debug, Serialize)]
pub struct SolanaPayload {
    pub signature: String,
}

// ─── SPL token constants ──────────────────────────────────────────────────────

const TOKEN_PROGRAM: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
const ATA_PROGRAM: &str = "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL";

fn find_ata(owner: &Pubkey, mint: &Pubkey) -> Pubkey {
    let token_prog = Pubkey::from_str(TOKEN_PROGRAM).unwrap();
    let ata_prog = Pubkey::from_str(ATA_PROGRAM).unwrap();
    Pubkey::find_program_address(
        &[owner.as_ref(), token_prog.as_ref(), mint.as_ref()],
        &ata_prog,
    )
    .0
}

// SPL TransferChecked instruction data: [12u8, amount_le_u64, decimals_u8]
fn transfer_checked_data(amount: u64, decimals: u8) -> [u8; 10] {
    let mut data = [0u8; 10];
    data[0] = 12; // TransferChecked discriminator
    data[1..9].copy_from_slice(&amount.to_le_bytes());
    data[9] = decimals;
    data
}

/// Build, sign, and submit a Solana x402 payment, returning the X-Payment header value.
pub fn build_x402_header(
    rpc: &RpcClient,
    payer: &Keypair,
    req: &PaymentRequirement,
) -> Result<String> {
    let mint = Pubkey::from_str(&req.asset).context("parse mint")?;
    let pay_to = Pubkey::from_str(&req.pay_to).context("parse payTo")?;
    let amount: u64 = req
        .max_amount_required
        .parse()
        .context("parse maxAmountRequired")?;
    let decimals = req.extra.as_ref().and_then(|e| e.decimals).unwrap_or(6);

    let payer_pk = payer.pubkey();
    let src_ata = find_ata(&payer_pk, &mint);
    let dst_ata = find_ata(&pay_to, &mint);
    let token_prog = Pubkey::from_str(TOKEN_PROGRAM).unwrap();

    let ix = Instruction {
        program_id: token_prog,
        accounts: vec![
            AccountMeta::new(src_ata, false),
            AccountMeta::new_readonly(mint, false),
            AccountMeta::new(dst_ata, false),
            AccountMeta::new_readonly(payer_pk, true),
        ],
        data: transfer_checked_data(amount, decimals).to_vec(),
    };

    let blockhash = rpc
        .get_latest_blockhash_with_commitment(CommitmentConfig::confirmed())
        .context("get_latest_blockhash")?
        .0;

    let msg = Message::new_with_blockhash(&[ix], Some(&payer_pk), &blockhash);
    let tx = Transaction::new(&[payer], msg, blockhash);

    let signature = rpc
        .send_and_confirm_transaction_with_spinner(&tx)
        .context("send_and_confirm_transaction")?;

    let envelope = X402Envelope {
        x402_version: 2,
        scheme: req.scheme.clone().unwrap_or_else(|| "exact".into()),
        network: req.network.clone().unwrap_or_else(|| "solana".into()),
        payload: SolanaPayload {
            signature: signature.to_string(),
        },
    };

    let json = serde_json::to_string(&envelope)?;
    Ok(B64.encode(json.as_bytes()))
}

/// Pick the Solana/USDC requirement from a 402 accepts list.
pub fn pick_solana_req(accepts: &[PaymentRequirement]) -> Option<&PaymentRequirement> {
    accepts.iter().find(|r| {
        r.network
            .as_deref()
            .map(|n| n.contains("solana"))
            .unwrap_or(false)
    })
}
