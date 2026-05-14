// TrendForge x402 server — other agents pay USDC to get research reports.
//
// POST /research
//   - No payment: returns HTTP 402 with USDC amount + facilitator address
//   - With X-Payment header: verifies the on-chain tx, runs workflow, returns report
//
// Register this server's URL as x402_endpoint when running `cargo run --bin register`.

use crate::{config::Config, workflow};
use anyhow::Result;
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::Json,
    routing::post,
    Router,
};
use serde_json::{json, Value};
use solana_rpc_client::rpc_client::RpcClient;
use solana_sdk::{commitment_config::CommitmentConfig, pubkey::Pubkey, signature::Keypair, signer::Signer};
use std::{str::FromStr, sync::Arc};
use tracing::{info, warn};

// Price per research report in USDC micro-units (1 USDC = 1_000_000)
// Set to 0.10 USDC — cheap enough to attract agent calls, profitable vs cost
const RESEARCH_PRICE_USDC: u64 = 100_000; // 0.10 USDC

pub struct AppState {
    pub config: Config,
    pub rpc: Arc<RpcClient>,
    pub payer: Keypair,
    pub usdc_mint: Pubkey,
}

pub async fn run_server(port: u16, config: Config, payer: Keypair) -> Result<()> {
    let rpc = Arc::new(RpcClient::new_with_commitment(
        config.synapse_rpc_url.clone(),
        CommitmentConfig::confirmed(),
    ));
    let usdc_mint = Pubkey::from_str(&config.usdc_mint)?;

    let state = Arc::new(AppState { config, rpc, payer, usdc_mint });

    let app = Router::new()
        .route("/research", post(handle_research))
        .route("/health", axum::routing::get(health))
        .with_state(state);

    let addr = format!("0.0.0.0:{port}");
    info!("TrendForge x402 server listening on {addr}");
    info!("Price per research report: {} USDC", RESEARCH_PRICE_USDC as f64 / 1_000_000.0);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn health() -> Json<Value> {
    Json(json!({ "status": "ok", "service": "trendforge-x402" }))
}

async fn handle_research(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Option<Json<Value>>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let query = body
        .as_ref()
        .and_then(|b| b.get("query"))
        .and_then(|q| q.as_str())
        .unwrap_or(&state.config.search_query)
        .to_string();

    // Check for X-Payment header
    let payment_header = headers.get("x-payment").and_then(|v| v.to_str().ok()).map(String::from);

    if payment_header.is_none() {
        // No payment — return 402 with payment requirements
        warn!("research request without payment — returning 402");
        return Err((
            StatusCode::PAYMENT_REQUIRED,
            Json(json!({
                "x402Version": 1,
                "error": "Payment required for research service",
                "accepts": [{
                    "scheme": "exact",
                    "network": "solana-mainnet",
                    "maxAmountRequired": RESEARCH_PRICE_USDC.to_string(),
                    "resource": "/research",
                    "description": "TrendForge research report",
                    "mimeType": "application/json",
                    "payTo": spl_associated_token_account::get_associated_token_address(
                        &state.payer.pubkey(),
                        &state.usdc_mint
                    ).to_string(),
                    "asset": state.usdc_mint.to_string(),
                    "extra": { "name": "USDC", "version": "1.0" }
                }]
            })),
        ));
    }

    let payment = payment_header.unwrap();
    info!(query, payment_len = payment.len(), "research request with payment");

    // Verify the payment on-chain
    if let Err(e) = verify_payment(&state, &payment) {
        warn!("payment verification failed: {e}");
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": format!("Invalid payment: {e}") })),
        ));
    }

    info!(query, "payment verified — running research workflow");

    // Run the research workflow
    let mut config = state.config.clone();
    config.search_query = query.clone();

    match workflow::run_once(&config, &state.rpc, &state.payer).await {
        Ok(report) => {
            info!(query, "research complete — returning report");
            Ok(Json(serde_json::to_value(&report).unwrap_or_default()))
        }
        Err(e) => {
            warn!("workflow error: {e}");
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Research failed: {e}") })),
            ))
        }
    }
}

fn verify_payment(state: &AppState, x_payment: &str) -> Result<()> {
    use base64::{engine::general_purpose::STANDARD, Engine};
    use solana_sdk::transaction::Transaction;

    let bytes = STANDARD.decode(x_payment)?;
    let tx: Transaction = bincode::deserialize(&bytes)
        .map_err(|e| anyhow::anyhow!("deserialize tx: {e}"))?;

    // Confirm the tx landed on-chain
    let sig = tx.signatures.first().ok_or_else(|| anyhow::anyhow!("no signature"))?;
    let confirmed = state.rpc.get_signature_status(sig)?;

    match confirmed {
        Some(Ok(())) => {
            info!(sig = %sig, "payment confirmed on-chain");
            Ok(())
        }
        Some(Err(e)) => anyhow::bail!("tx failed on-chain: {e}"),
        None => anyhow::bail!("tx not yet confirmed — retry after commitment"),
    }
}
