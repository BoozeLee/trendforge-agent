# TrendForge — Autonomous SAP Research Agent

An autonomous on-chain agent built in Rust for the OOBE Protocol × Ace Data Cloud bounty.

**Category:** Ace Data Cloud Usage (x402 Facilitator)

## What it does

```
Trigger → SAP Discovery → SERP Search → LLM Analysis → Image Generation → Report
```

Every cycle the agent:
1. **Discovers** active agents on the Synapse Agent Protocol (SAP) mainnet
2. **Searches** trending topics via Ace Data Cloud Google SERP API
3. **Analyzes** results with Ace Data Cloud LLM (gpt-4o-mini via `/openai/chat/completions`)
4. **Generates** a visual cover via Ace Data Cloud Midjourney (`/midjourney/imagine`)
5. Saves a JSON + Markdown report to `reports/`

All three Ace Data Cloud API calls are paid automatically via **x402 on Solana USDC** — no manual input, no API key required.

## Ace Data Cloud services used (≥ 3 required)

| # | Service | Endpoint |
|---|---------|----------|
| 1 | Web Search (SERP) | `GET /serp/google` |
| 2 | Chat / LLM | `POST /openai/chat/completions` |
| 3 | Image Generation | `POST /midjourney/imagine` |

## Setup

### 1. Prerequisites

- Rust (stable)
- Solana CLI
- A funded Solana wallet with mainnet USDC

### 2. Create a keypair

```bash
mkdir keys
solana-keygen new --outfile keys/agent.json
```

### 3. Configure

```bash
cp .env.example .env
# Edit .env:
# - Set SYNAPSE_RPC_URL (get free tier at https://synapse.oobeprotocol.ai/)
# - Set SOLANA_KEYPAIR_PATH=keys/agent.json
```

### 4. Register on SAP mainnet

```bash
cargo run --bin register
# Output: ✓ Registered! Explorer: https://explorer.oobeprotocol.ai/agents/<YOUR_WALLET>
```

### 5. Fund your wallet with USDC

Each workflow cycle costs roughly ~$0.05–0.10 USDC (3 API calls).
Bridge USDC to Solana mainnet or buy via any DEX.

### 6. Run the agent

```bash
# Single run (demo / test)
cargo run --bin trendforge -- --once

# Autonomous loop (runs every WORKFLOW_INTERVAL_SECS)
cargo run --bin trendforge

# Custom query
cargo run --bin trendforge -- --once --query "DeFi yield strategies Solana 2026"
```

## x402 Payment Flow

```
Agent calls Ace Data Cloud API (no auth)
       │
       ▼  HTTP 402 Payment Required
       │  { accepts: [{ network: "solana", asset: "USDC_MINT", maxAmountRequired: "...", payTo: "..." }] }
       │
       ▼  Build SPL TransferChecked tx → sign → submit via Synapse RPC
       │
       ▼  Retry with X-Payment: base64({"x402Version":2,"scheme":"exact","network":"solana","payload":{"signature":"..."}})
       │
       ▼  HTTP 200 OK — API response + x402_tx in headers
```

## Output

Each run writes to `reports/`:
- `<timestamp>.json` — full structured report
- `latest.md` — human-readable Markdown report

## Project structure

```
src/
├── main.rs          autonomous loop + CLI
├── config.rs        env/keypair loading
├── sap.rs           SAP on-chain agent discovery
├── ace_client.rs    Ace Data Cloud HTTP client (x402-aware)
├── x402.rs          Solana USDC x402 payment signing
├── workflow.rs      end-to-end orchestration
└── bin/
    └── register.rs  one-time SAP agent registration
```
