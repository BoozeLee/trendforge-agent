// Ace Data Cloud client with automatic free-tier fallbacks.
//
// Priority chain:
//   search  — Ace Data Cloud → Serper.dev (SERPER_API_KEY) → simulated
//   chat    — Ace Data Cloud → Groq       (GROQ_API_KEY)
//   images  — Ace Data Cloud → Pollinations.ai (no key needed)
//
// Set ACE_API_KEY for free-credit mode; leave unset to use x402 payments.

use crate::x402::{self, PaymentRequiredBody, PaymentRequirement};
use anyhow::{bail, Context, Result};
use reqwest::{Client, Response, StatusCode};
use serde::Deserialize;
use serde_json::Value;
use solana_rpc_client::rpc_client::RpcClient;
use solana_sdk::signature::Keypair;
use tracing::{info, warn};

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

// ─── Service wrappers with fallback chain ─────────────────────────────────────

pub struct AceServices<'a> {
    pub client: &'a AceClient,
    pub rpc: &'a RpcClient,
    pub payer: &'a Keypair,
    http: Client,
}

impl<'a> AceServices<'a> {
    pub fn new(client: &'a AceClient, rpc: &'a RpcClient, payer: &'a Keypair) -> Self {
        Self { client, rpc, payer, http: Client::new() }
    }

    /// Web search — Ace Data Cloud → Serper.dev → simulated results
    pub async fn search(&self, query: &str) -> Result<Vec<SearchResult>> {
        info!(query, mode = self.client.mode(), "search: trying Ace Data Cloud");
        let body = serde_json::json!({ "query": query, "number": 5, "type": "search" });
        match self.client.post("/serp/google", body, self.rpc, self.payer).await {
            Ok(resp) => {
                let results: Vec<SearchResult> = resp
                    .get("positions")
                    .or_else(|| resp.get("organic_results"))
                    .or_else(|| resp.get("results"))
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
                    .unwrap_or_default();
                info!(count = results.len(), "Ace Data Cloud search OK");
                return Ok(results);
            }
            Err(e) => warn!("Ace Data Cloud search failed ({e}), trying Serper.dev"),
        }

        if let Ok(key) = std::env::var("SERPER_API_KEY") {
            info!(query, "search: trying Serper.dev");
            match self.serper_search(query, &key).await {
                Ok(r) => return Ok(r),
                Err(e) => warn!("Serper.dev failed ({e}), using simulated results"),
            }
        } else {
            warn!("SERPER_API_KEY not set, using simulated results (set for real search)");
        }

        Ok(simulate_search(query))
    }

    async fn serper_search(&self, query: &str, api_key: &str) -> Result<Vec<SearchResult>> {
        let resp = self.http
            .post("https://google.serper.dev/search")
            .header("X-API-KEY", api_key)
            .json(&serde_json::json!({ "q": query, "num": 5 }))
            .send().await.context("Serper.dev request")?;

        let status = resp.status();
        let text = resp.text().await?;
        if !status.is_success() {
            bail!("Serper.dev HTTP {status}: {text}");
        }
        let val: Value = serde_json::from_str(&text)?;
        let results: Vec<SearchResult> = val
            .get("organic")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();
        info!(count = results.len(), "Serper.dev search OK");
        Ok(results)
    }

    /// LLM chat — Ace Data Cloud → Groq (llama-3.1-8b-instant)
    pub async fn chat(&self, system: &str, user: &str) -> Result<String> {
        info!(mode = self.client.mode(), "chat: trying Ace Data Cloud");
        let body = serde_json::json!({
            "model": "gpt-4o-mini",
            "messages": [
                {"role": "system", "content": system},
                {"role": "user",   "content": user}
            ],
            "max_tokens": 512
        });

        match self.client.post("/openai/chat/completions", body, self.rpc, self.payer).await {
            Ok(resp) => {
                if let Some(content) = resp["choices"][0]["message"]["content"].as_str() {
                    info!("Ace Data Cloud LLM OK");
                    return Ok(content.to_string());
                }
                warn!("Ace Data Cloud chat: unexpected response shape, falling back");
            }
            Err(e) => warn!("Ace Data Cloud chat failed ({e}), trying Groq"),
        }

        if let Ok(key) = std::env::var("GROQ_API_KEY") {
            info!("chat: trying Groq (llama-3.1-8b-instant)");
            match self.openai_compat_chat("https://api.groq.com/openai/v1", "llama-3.1-8b-instant", system, user, &key).await {
                Ok(r) => return Ok(r),
                Err(e) => warn!("Groq failed ({e}), trying NVIDIA NIM"),
            }
        }

        if let Ok(key) = std::env::var("NVIDIA_API_KEY") {
            info!("chat: trying NVIDIA NIM (meta/llama-3.1-8b-instruct)");
            match self.openai_compat_chat(
                "https://integrate.api.nvidia.com/v1",
                "meta/llama-3.1-8b-instruct",
                system, user, &key,
            ).await {
                Ok(r) => return Ok(r),
                Err(e) => warn!("NVIDIA NIM failed ({e}), trying HuggingFace"),
            }
        }

        // HuggingFace Inference API — completely free, no key required
        info!("chat: trying HuggingFace (Mistral-7B-Instruct, no key)");
        self.hf_chat(system, user, std::env::var("HF_API_TOKEN").ok().as_deref()).await
            .map_err(|e| anyhow::anyhow!(
                "all LLM providers failed (last: {e}). Set GROQ_API_KEY or NVIDIA_API_KEY for reliability."
            ))
    }

    async fn openai_compat_chat(&self, base: &str, model: &str, system: &str, user: &str, api_key: &str) -> Result<String> {
        let body = serde_json::json!({
            "model": model,
            "messages": [
                {"role": "system", "content": system},
                {"role": "user",   "content": user}
            ],
            "max_tokens": 512
        });

        let url = format!("{base}/chat/completions");
        let resp = self.http
            .post(&url)
            .header("Authorization", format!("Bearer {api_key}"))
            .json(&body)
            .send().await.with_context(|| format!("{base} request"))?;

        let status = resp.status();
        let text = resp.text().await?;
        if !status.is_success() {
            bail!("{base} HTTP {status}: {text}");
        }
        let val: Value = serde_json::from_str(&text)?;
        val["choices"][0]["message"]["content"]
            .as_str()
            .with_context(|| format!("{base}: missing content field"))
            .map(|s| {
                info!(provider = base, "LLM OK");
                s.to_string()
            })
    }

    // HuggingFace Inference API — free tier, no key required (slower without token)
    async fn hf_chat(&self, system: &str, user: &str, token: Option<&str>) -> Result<String> {
        let prompt = format!("[INST] {system}\n\n{user} [/INST]");
        let body = serde_json::json!({
            "inputs": prompt,
            "parameters": { "max_new_tokens": 512, "return_full_text": false }
        });
        let mut req = self.http
            .post("https://api-inference.huggingface.co/models/mistralai/Mistral-7B-Instruct-v0.2")
            .json(&body);
        if let Some(t) = token {
            req = req.header("Authorization", format!("Bearer {t}"));
        }
        let resp = req.send().await.context("HuggingFace request")?;
        let status = resp.status();
        let text = resp.text().await?;
        if !status.is_success() {
            bail!("HuggingFace HTTP {status}: {text}");
        }
        let val: Value = serde_json::from_str(&text)?;
        // HF returns [{generated_text: "..."}]
        val[0]["generated_text"]
            .as_str()
            .context("HuggingFace: missing generated_text")
            .map(|s| {
                info!("HuggingFace LLM OK");
                s.to_string()
            })
    }

    /// Image generation — Ace Data Cloud → Pollinations.ai (free, no key)
    pub async fn generate_image(&self, prompt: &str) -> Result<String> {
        info!(mode = self.client.mode(), "image: trying Ace Data Cloud");
        let body = serde_json::json!({
            "prompt": prompt,
            "action": "generate",
            "model": "turbo"
        });

        match self.client.post("/midjourney/imagine", body, self.rpc, self.payer).await {
            Ok(resp) => {
                let url = resp["image_url"]
                    .as_str()
                    .or_else(|| resp["imageUrl"].as_str())
                    .unwrap_or("")
                    .to_string();
                if !url.is_empty() {
                    info!("Ace Data Cloud image OK");
                    return Ok(url);
                }
                warn!("Ace Data Cloud image: empty URL, falling back to Pollinations.ai");
            }
            Err(e) => warn!("Ace Data Cloud image failed ({e}), using Pollinations.ai"),
        }

        // Pollinations.ai — completely free, no API key, just encode prompt in URL
        let encoded = urlencoding::encode(prompt);
        let url = format!("https://image.pollinations.ai/prompt/{encoded}?width=1280&height=720&nologo=true");
        info!(url, "Pollinations.ai image URL built");
        Ok(url)
    }
}

fn simulate_search(query: &str) -> Vec<SearchResult> {
    vec![
        SearchResult {
            title: Some("AI Agents on Solana: 2026 State of the Ecosystem".to_string()),
            link: Some("https://solana.com/news/ai-agents-2026".into()),
            snippet: Some(format!("Autonomous agents leveraging SAP protocol and x402 payments are reshaping how AI interacts with blockchain. Query context: {query}")),
        },
        SearchResult {
            title: Some("Synapse Agent Protocol: Building Autonomous On-Chain Agents".into()),
            link: Some("https://oobeprotocol.ai/blog/sap-overview".into()),
            snippet: Some("SAP enables agents to discover each other on-chain, negotiate services, and settle micro-payments via x402 USDC transfers on Solana.".into()),
        },
        SearchResult {
            title: Some("x402: The HTTP Payment Protocol for AI Agents".into()),
            link: Some("https://x402.org".into()),
            snippet: Some("HTTP 402 Payment Required — agents send X-Payment headers with signed Solana transactions, enabling machine-to-machine micropayments at scale.".into()),
        },
    ]
}

#[derive(Debug, Deserialize, Default)]
pub struct SearchResult {
    pub title: Option<String>,
    pub link: Option<String>,
    pub snippet: Option<String>,
}
