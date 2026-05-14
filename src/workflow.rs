// TrendForge end-to-end workflow.
// Trigger → SAP discovery → SERP search → LLM analysis → image generation → report

use crate::{
    ace_client::{AceClient, AceServices},
    config::Config,
    sap,
};
use anyhow::Result;
use chrono::Utc;
use solana_rpc_client::rpc_client::RpcClient;
use solana_sdk::signature::Keypair;
use solana_sdk::signer::Signer;
use std::fs;
use tracing::{info, warn};

pub async fn run_once(config: &Config, rpc: &RpcClient, payer: &Keypair) -> Result<Report> {
    let start = Utc::now();
    info!("=== TrendForge workflow starting at {} ===", start.to_rfc3339());

    // ── Step 1: SAP tool discovery ────────────────────────────────────────────
    info!("Step 1: SAP tool discovery");
    let agents = sap::fetch_agents(rpc).unwrap_or_else(|e| {
        warn!("SAP fetch failed (continuing): {e}");
        vec![]
    });

    let active: Vec<_> = agents.iter().filter(|a| a.is_active).collect();
    info!(
        total = agents.len(),
        active = active.len(),
        "discovered SAP agents"
    );

    // Log Synapse Sentinel
    let sentinel_found = agents.iter().any(|a| {
        a.wallet.to_string() == sap::SYNAPSE_SENTINEL
            || a.name.to_lowercase().contains("sentinel")
    });
    info!(sentinel_found, "Synapse Sentinel check");

    let tool_summary: String = active
        .iter()
        .take(5)
        .map(|a| format!("• {} — {}", a.name, a.description))
        .collect::<Vec<_>>()
        .join("\n");

    // ── Step 2: Web search (Ace Data Cloud SERP) ──────────────────────────────
    info!("Step 2: Ace Data Cloud web search");
    let ace = AceClient::new(&config.ace_api_base);
    let svc = AceServices::new(&ace, rpc, payer);

    let search_results = svc.search(&config.search_query).await?;
    let snippets: String = search_results
        .iter()
        .map(|r| {
            format!(
                "Title: {}\nSnippet: {}\nURL: {}",
                r.title.as_deref().unwrap_or(""),
                r.snippet.as_deref().unwrap_or(""),
                r.link.as_deref().unwrap_or("")
            )
        })
        .collect::<Vec<_>>()
        .join("\n---\n");

    // ── Step 3: LLM analysis (Ace Data Cloud Chat) ────────────────────────────
    info!("Step 3: Ace Data Cloud LLM analysis");
    let system_prompt = "You are TrendForge, an autonomous AI research agent on Solana. \
        Summarize the provided search results into a concise trend report. \
        Highlight key developments, patterns, and actionable insights. Keep it under 300 words.";

    let user_msg = format!(
        "Query: {}\n\nSearch results:\n{}\n\nSAP network snapshot:\n{}",
        config.search_query, snippets, tool_summary
    );

    let analysis = svc.chat(system_prompt, &user_msg).await?;
    info!(chars = analysis.len(), "LLM analysis complete");

    // ── Step 4: Image generation (Ace Data Cloud Midjourney) ─────────────────
    info!("Step 4: Ace Data Cloud image generation");
    let image_prompt = format!(
        "Futuristic digital dashboard showing AI agent network on Solana blockchain, \
         trending topics visualization, neon data streams, abstract tech aesthetic, \
         topic: {}, cinematic --ar 16:9",
        &config.search_query[..config.search_query.len().min(60)]
    );

    let image_url = svc.generate_image(&image_prompt).await?;
    info!(image_url, "image generated");

    // ── Step 5: Compile and save report ──────────────────────────────────────
    let report = Report {
        timestamp: start.to_rfc3339(),
        agent_wallet: payer.pubkey().to_string(),
        query: config.search_query.clone(),
        sap_agents_discovered: agents.len(),
        sap_active_agents: active.len(),
        sentinel_found,
        analysis: analysis.clone(),
        image_url: image_url.clone(),
        search_result_count: search_results.len(),
    };

    save_report(&report)?;

    info!("=== TrendForge workflow complete ===");
    Ok(report)
}

fn save_report(report: &Report) -> Result<()> {
    let dir = "reports";
    fs::create_dir_all(dir)?;
    let filename = format!(
        "{}/{}.json",
        dir,
        report.timestamp.replace(':', "-").replace('.', "-")
    );
    let json = serde_json::to_string_pretty(report)?;
    fs::write(&filename, &json)?;
    info!(path = filename, "report saved");

    // Also write latest.md for easy reading
    let md = format!(
        "# TrendForge Report\n**Time:** {}\n**Agent:** `{}`\n\n\
         ## SAP Network\n- Agents discovered: {}\n- Active: {}\n- Sentinel found: {}\n\n\
         ## Query\n> {}\n\n## Analysis\n{}\n\n## Visual\n![cover]({})\n",
        report.timestamp,
        report.agent_wallet,
        report.sap_agents_discovered,
        report.sap_active_agents,
        report.sentinel_found,
        report.query,
        report.analysis,
        report.image_url,
    );
    fs::write("reports/latest.md", md)?;
    Ok(())
}

#[derive(Debug, serde::Serialize)]
pub struct Report {
    pub timestamp: String,
    pub agent_wallet: String,
    pub query: String,
    pub sap_agents_discovered: usize,
    pub sap_active_agents: usize,
    pub sentinel_found: bool,
    pub analysis: String,
    pub image_url: String,
    pub search_result_count: usize,
}
