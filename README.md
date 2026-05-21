# TrendForge — Autonomous Research Agent on Solana

> Discovers AI tools via SAP → searches the web → analyzes with LLM → generates images → settles every payment on-chain via x402 USDC — zero human input required.

Built for the **OOBE Protocol × Ace Data Cloud Bounty** (Ace Data Cloud Usage category).

---

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    TrendForge Agent Loop                    │
│                                                             │
│  ┌──────────┐   ┌──────────┐   ┌──────────┐   ┌────────┐  │
│  │  SAP     │   │  Ace     │   │  Ace     │   │  Ace   │  │
│  │ Discovery│──>│  SERP    │──>│  Chat    │──>│ Image  │  │
│  │          │   │ /search  │   │  /llm    │   │  /mj   │  │
│  └──────────┘   └──────────┘   └──────────┘   └────────┘  │
│       │               │               │               │    │
│       │          x402 USDC       x402 USDC       x402 USDC │
│       │          payment         payment          payment   │
│       v               v               v               v    │
│  Solana mainnet ─────────────────────────────────────────  │
└─────────────────────────────────────────────────────────────┘
```

**5-step workflow (autonomous, runs every hour):**

1. **SAP Discovery** — `getProgramAccounts` on `SAPpUhsWLJG1FfkGRcXagEDMrMsWGjbky7AyhGpFETZ` to find all registered agents (31 live on mainnet)
2. **Web Search** — `POST /serp/google` via Ace Data Cloud, paid with x402 USDC
3. **LLM Analysis** — `POST /openai/chat/completions` via Ace Data Cloud, paid with x402 USDC
4. **Image Generation** — `POST /midjourney/imagine` via Ace Data Cloud, paid with x402 USDC
5. **Report** — saved to `reports/latest.md` with timestamp, analysis, and cover image

---

## x402 Payment Flow

Every Ace Data Cloud API call is paid autonomously:

```
Agent  ->  POST /serp/google
Server <-  HTTP 402 Payment Required
           { "accepts": [{ "network": "solana", "maxAmount": 18000, "asset": "USDC" }] }

Agent  ->  sign SPL TransferChecked tx (USDC -> facilitator ATA)
       ->  POST /serp/google  +  X-Payment: <base64 tx>
Server <-  HTTP 200 OK  +  results
```

No API subscriptions. No monthly billing. Pure pay-per-call machine-to-machine payments on Solana.

---

## SAP Integration

TrendForge registers itself on the Synapse Agent Protocol, making it discoverable by other agents:

```bash
cargo run --bin register
# Registers on SAPpUhsWLJG1FfkGRcXagEDMrMsWGjbky7AyhGpFETZ
# Explorer: https://explorer.oobeprotocol.ai/agents/<wallet>
```

Agent wallet: `3L5ZJQDzBUDwautD734carHphTtgojAktSvNywnQsuQF`
GitHub Repository: [BoozeLee/trendforge-agent](https://github.com/BoozeLee/trendforge-agent)

---

## Run

```bash
# Clone and configure
git clone https://github.com/BoozeLee/trendforge-agent
cd trendforge-agent
cp .env.example .env
# Add your ACE_API_KEY from acedata.cloud dashboard -> API Keys

# Demo (no funds required -- shows full workflow visually)
cargo run --bin trendforge -- --demo

# Live run (requires SOL for registration + USDC for x402 payments)
cargo run --bin trendforge -- --once     # single run
cargo run --bin trendforge              # autonomous loop (every hour)
```

---

## Environment

| Variable | Description |
|---|---|
| `ACE_API_KEY` | Ace Data Cloud bearer token (acedata.cloud dashboard -> API Keys) |
| `SYNAPSE_RPC_URL` | Solana RPC endpoint |
| `SOLANA_KEYPAIR_PATH` | Path to agent keypair JSON |
| `SEARCH_QUERY` | Research topic for autonomous runs |
| `WORKFLOW_INTERVAL_SECS` | Loop interval (default: 3600) |
| `NVIDIA_API_KEY` | Fallback LLM if Ace unavailable |

---

## Stack

- **Rust** — `tokio` async runtime, `reqwest`, `serde`
- **Solana** — `solana-sdk 2.1`, `solana-rpc-client`, SPL token
- **SAP** — Anchor program `SAPpUhsWLJG1FfkGRcXagEDMrMsWGjbky7AyhGpFETZ`
- **x402** — HTTP 402 -> SPL TransferChecked -> X-Payment header
- **Ace Data Cloud** — SERP + GPT-4o-mini + Midjourney via unified x402 API

---

## Deployment & Verification

**Agent wallet:** `3L5ZJQDzBUDwautD734carHphTtgojAktSvNywnQsuQF`

Fund with `≥ 0.01 SOL` (registration) + `~5 USDC` (x402 calls), then:

```bash
# 1. Register on SAP Mainnet (run once)
cargo run --bin register

# 2. Verify registration on explorer
# https://explorer.oobeprotocol.ai/agents/3L5ZJQDzBUDwautD734carHphTtgojAktSvNywnQsuQF

# 3. Run one autonomous cycle
cargo run --bin trendforge -- --once
```

**IDL verification:** The SAP v2 IDL was cloned from Mainnet (`--clone SAPpUhsWLJG1FfkGRcXagEDMrMsWGjbky7AyhGpFETZ`) and all discriminators verified against the Rust Anchor types before submission.

---

## Bounty

Built for [OOBE Protocol x Ace Data Cloud Bounty](https://oobeprotocol.ai) — Ace Data Cloud Usage category.

- Demonstrates all 3 Ace Data Cloud services: web search, LLM, image generation
- Every service call autonomously settled via x402 on Solana
- SAP-registered agent discoverable on-chain by other agents
- Fully autonomous loop -- no human input after deployment
