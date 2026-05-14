// Ace Data Cloud API client with x402 payment support.
// Each call pattern: attempt → 402 → sign payment → retry with X-Payment.

use crate::x402::{self, PaymentRequiredBody, PaymentRequirement};
use anyhow::{bail, Context, Result};
use reqwest::{Client, Response, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use solana_rpc_client::rpc_client::RpcClient;
use solana_sdk::signature::Keypair;
use tracing::{info, warn};

pub struct AceClient {
    http: Client,
    base: String,
}

impl AceClient {
    pub fn new(base: &str) -> Self {
        Self {
            http: Client::new(),
            base: base.trim_end_matches('/').to_string(),
        }
    }

    /// POST with x402 payment if required. Returns parsed JSON body.
    pub async fn post_with_payment(
        &self,
        path: &str,
        body: Value,
        rpc: &RpcClient,
        payer: &Keypair,
    ) -> Result<Value> {
        let url = format!("{}{}", self.base, path);

        // First attempt (no auth)
        let resp = self
            .http
            .post(&url)
            .json(&body)
            .send()
            .await
            .context("initial POST")?;

        if resp.status() == StatusCode::PAYMENT_REQUIRED {
            let req = self.extract_solana_req(resp).await?;
            info!(
                amount = req.max_amount_required,
                pay_to = req.pay_to,
                "402 received — signing Solana payment"
            );
            let header = x402::build_x402_header(rpc, payer, &req)
                .context("build x402 header")?;

            let retry = self
                .http
                .post(&url)
                .json(&body)
                .header("X-Payment", &header)
                .send()
                .await
                .context("retry POST with X-Payment")?;

            return self.parse_ok(retry).await;
        }

        self.parse_ok(resp).await
    }

    /// GET with x402 payment if required.
    pub async fn get_with_payment(
        &self,
        path: &str,
        query: &[(&str, &str)],
        rpc: &RpcClient,
        payer: &Keypair,
    ) -> Result<Value> {
        let url = format!("{}{}", self.base, path);

        let resp = self
            .http
            .get(&url)
            .query(query)
            .send()
            .await
            .context("initial GET")?;

        if resp.status() == StatusCode::PAYMENT_REQUIRED {
            let req = self.extract_solana_req(resp).await?;
            info!(
                amount = req.max_amount_required,
                "402 received — signing Solana payment"
            );
            let header = x402::build_x402_header(rpc, payer, &req)?;

            let retry = self
                .http
                .get(&url)
                .query(query)
                .header("X-Payment", &header)
                .send()
                .await
                .context("retry GET with X-Payment")?;

            return self.parse_ok(retry).await;
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
        let text = resp.text().await.context("read response body")?;
        if !status.is_success() {
            bail!("HTTP {status}: {text}");
        }
        serde_json::from_str(&text).context("parse JSON response")
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
        info!(query, "Ace: web search");
        let resp = self
            .client
            .get_with_payment(
                "/serp/google",
                &[("q", query), ("number", "5")],
                self.rpc,
                self.payer,
            )
            .await?;

        let results: Vec<SearchResult> = resp
            .get("organic_results")
            .or_else(|| resp.get("results"))
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();

        info!(count = results.len(), "search results");
        Ok(results)
    }

    /// Service 2 — Chat / LLM (OpenAI-compatible)
    pub async fn chat(&self, system: &str, user: &str) -> Result<String> {
        info!("Ace: chat/LLM");
        let body = serde_json::json!({
            "model": "gpt-4o-mini",
            "messages": [
                {"role": "system", "content": system},
                {"role": "user",   "content": user}
            ],
            "max_tokens": 512
        });

        let resp = self
            .client
            .post_with_payment("/openai/chat/completions", body, self.rpc, self.payer)
            .await?;

        let content = resp["choices"][0]["message"]["content"]
            .as_str()
            .context("missing content in chat response")?
            .to_string();
        Ok(content)
    }

    /// Service 3 — Image Generation (Midjourney fast)
    pub async fn generate_image(&self, prompt: &str) -> Result<String> {
        info!("Ace: image generation");
        let body = serde_json::json!({
            "prompt": prompt,
            "action": "generate",
            "model": "turbo"
        });

        let resp = self
            .client
            .post_with_payment("/midjourney/imagine", body, self.rpc, self.payer)
            .await?;

        let image_url = resp["image_url"]
            .as_str()
            .or_else(|| resp["imageUrl"].as_str())
            .unwrap_or("(image url not returned)")
            .to_string();
        Ok(image_url)
    }
}

#[derive(Debug, Deserialize, Default)]
pub struct SearchResult {
    pub title: Option<String>,
    pub link: Option<String>,
    pub snippet: Option<String>,
}
