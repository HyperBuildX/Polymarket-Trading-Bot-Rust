# Polymarket Trading Bot (TypeScript)

TypeScript port of [Polymarket-Trading-Bot-Rust](https://github.com/your-org/Polymarket-Trading-Bot-Rust). At each 15-minute market start, places limit buys for BTC (and optionally ETH, Solana, XRP) Up/Down at a fixed price (default $0.45).

## Requirements

- Node.js >= 18
- `config.json` with Polymarket `private_key` (and optional API creds)

## Setup

```bash
npm install
cp config.json.example config.json   # or copy from Rust project
# Edit config.json: set polymarket.private_key (hex, with or without 0x)
```

## Usage

- **Simulation (default)** – no real orders, logs what would be placed:
  ```bash
  npm run dev
  # or
  npx tsx src/main-dual-limit-045.ts
  ```

- **Production** – place real limit orders:
  ```bash
  npx tsx src/main-dual-limit-045.ts --no-simulation
  ```

- **Config path**:
  ```bash
  npx tsx src/main-dual-limit-045.ts -c /path/to/config.json
  ```

## Config

Same shape as the Rust bot:

- `polymarket.gamma_api_url`, `polymarket.clob_api_url` – API base URLs
- `polymarket.private_key` – EOA private key (hex)
- `polymarket.proxy_wallet_address` – optional proxy/Magic wallet
- `trading.dual_limit_price` – limit price (default 0.45)
- `trading.dual_limit_shares` – optional fixed shares per order
- `trading.enable_eth_trading`, `enable_solana_trading`, `enable_xrp_trading` – enable extra markets

## Project layout

- `src/config.ts` – load config, parse CLI args
- `src/types.ts` – Market, Token, BuyOpportunity, MarketSnapshot
- `src/api.ts` – Gamma API (market by slug), CLOB order book
- `src/clob.ts` – CLOB client (ethers + @polymarket/clob-client), place limit order
- `src/monitor.ts` – fetch snapshot (prices, time remaining)
- `src/trader.ts` – hasActivePosition, executeLimitBuy
- `src/main-dual-limit-045.ts` – discover markets, monitor loop, place limit orders at period start

## Build

```bash
npm run build
node dist/main-dual-limit-045.js
```
