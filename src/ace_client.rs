// Ace Data Cloud client — supports both API key (free credits) and x402 payment modes.
// Set ACE_API_KEY in .env for free-credit mode. Leave unset to use x402.

use crate::x402::{self, PaymentRequiredBody, PaymentRequirement};
use anyhow::{bail, Context, Result};
use reqwest::{Client, Response, StatusCode};
use serde::Deserialize;
use serde_json::Value;
use solana_rpc_client::rpc_client::RpcClient;
use solana_sdk::signature::Keypair;
use tracing::info;

pub struct AceClient {
    http: Client,
    base: String,
    api_key: Option<String>,
}

impl AceClient {
    pub fn new(base: &str) -> Self {
        Self {
            http: Client::new(),
            base: base.trim_end_matches('/').to_string(),
            api_key: std::env::var("ACE_API_KEY").ok(),
        }
    }

    pub fn mode(&self) -> &str {
        if self.api_key.is_some() { "api-key" } else { "x402" }
    }

    /// POST — uses API key if available, falls back to x402 payment.
    pub async fn post(&self, path: &str, body: Value, rpc: &RpcClient, payer: &Keypair) -> Result<Value> {
        let url = format!("{}{}", self.base, path);
        let req = self.http.post(&url).json(&body);
        let req = if let Some(k) = &self.api_key {
            req.header("Authorization", format!("Bearer {k}"))
        } else {
            req
        };

        let resp = req.send().await.context("POST request")?;

        if resp.status() == StatusCode::PAYMENT_REQUIRED && self.api_key.is_none() {
            let req_info = self.extract_solana_req(resp).await?;
            info!(amount = req_info.max_amount_required, "x402: signing Solana payment");
            let header = x402::build_x402_header(rpc, payer, &req_info)?;
            return self.parse_ok(
                self.http.post(&url).json(&body)
                    .header("X-Payment", &header)
                    .send().await.context("retry with X-Payment")?
            ).await;
        }

        self.parse_ok(resp).await
    }

    /// GET — uses API key if available, falls back to x402.
    pub async fn get(&self, path: &str, query: &[(&str, &str)], rpc: &RpcClient, payer: &Keypair) -> Result<Value> {
        let url = format!("{}{}", self.base, path);
        let req = self.http.get(&url).query(query);
        let req = if let Some(k) = &self.api_key {
            req.header("Authorization", format!("Bearer {k}"))
        } else {
            req
        };

        let resp = req.send().await.context("GET request")?;

        if resp.status() == StatusCode::PAYMENT_REQUIRED && self.api_key.is_none() {
            let req_info = self.extract_solana_req(resp).await?;
            info!(amount = req_info.max_amount_required, "x402: signing Solana payment");
            let header = x402::build_x402_header(rpc, payer, &req_info)?;
            return self.parse_ok(
                self.http.get(&url).query(query)
                    .header("X-Payment", &header)
                    .send().await.context("retry with X-Payment")?
            ).await;
        }

        self.parse_ok(resp).await
    }

    async fn extract_solana_req(&self, resp: Response) -> Result<PaymentRequirement> {
        let body: PaymentRequiredBody = resp.json().await.context("parse 402 body")?;
        x402::pick_solana_req(&body.accepts)
            .cloned()
            .context("no Solana payment option in 402 accepts")
    }

    async fn parse_ok(&self, resp: Response) -> Result<Value> {
        let status = resp.status();
        let text = resp.text().await.context("read body")?;
        if !status.is_success() {
            bail!("HTTP {status}: {text}");
        }
        serde_json::from_str(&text).context("parse JSON")
    }
}

// ─── Service wrappers ─────────────────────────────────────────────────────────

pub struct AceServices<'a> {
    pub client: &'a AceClient,
    pub rpc: &'a RpcClient,
    pub payer: &'a Keypair,
}

impl<'a> AceServices<'a> {
    pub fn new(client: &'a AceClient, rpc: &'a RpcClient, payer: &'a Keypair) -> Self {
        Self { client, rpc, payer }
    }

    /// Service 1 — Web Search (Google SERP)
    pub async fn search(&self, query: &str) -> Result<Vec<SearchResult>> {
        info!(query, mode = self.client.mode(), "Ace: web search");
        let body = serde_json::json!({ "query": query, "number": 5, "type": "search" });
        let resp = self.client.post("/serp/google", body, self.rpc, self.payer).await?;

        let results: Vec<SearchResult> = resp
            .get("positions")
            .or_else(|| resp.get("organic_results"))
            .or_else(|| resp.get("results"))
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();
        info!(count = results.len(), "search results received");
        Ok(results)
    }

    /// Service 2 — Chat / LLM
    pub async fn chat(&self, system: &str, user: &str) -> Result<String> {
        info!(mode = self.client.mode(), "Ace: chat/LLM");
        let body = serde_json::json!({
            "model": "gpt-4o-mini",
            "messages": [
                {"role": "system", "content": system},
                {"role": "user",   "content": user}
            ],
            "max_tokens": 512
        });
        let resp = self.client.post("/openai/chat/completions", body, self.rpc, self.payer).await?;
        resp["choices"][0]["message"]["content"]
            .as_str()
            .context("missing content")
            .map(|s| s.to_string())
    }

    /// Service 3 — Image Generation (Midjourney)
    pub async fn generate_image(&self, prompt: &str) -> Result<String> {
        info!(mode = self.client.mode(), "Ace: image generation");
        let body = serde_json::json!({
            "prompt": prompt,
            "action": "generate",
            "model": "turbo"
        });
        let resp = self.client.post("/midjourney/imagine", body, self.rpc, self.payer).await?;
        Ok(resp["image_url"]
            .as_str()
            .or_else(|| resp["imageUrl"].as_str())
            .unwrap_or("(no image url)")
            .to_string())
    }
}

#[derive(Debug, Deserialize, Default)]
pub struct SearchResult {
    pub title: Option<String>,
    pub link: Option<String>,
    pub snippet: Option<String>,
}
