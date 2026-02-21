import { readFileSync, existsSync, writeFileSync } from "fs";
import { join } from "path";

export interface PolymarketConfig {
  gamma_api_url: string;
  clob_api_url: string;
  api_key: string | null;
  api_secret: string | null;
  api_passphrase: string | null;
  private_key: string | null;
  proxy_wallet_address: string | null;
  signature_type: number | null;
}

export interface TradingConfig {
  eth_condition_id: string | null;
  btc_condition_id: string | null;
  solana_condition_id: string | null;
  xrp_condition_id: string | null;
  check_interval_ms: number;
  fixed_trade_amount: number;
  trigger_price: number;
  min_elapsed_minutes: number;
  sell_price: number;
  max_buy_price: number | null;
  stop_loss_price: number | null;
  hedge_price: number | null;
  market_closure_check_interval_seconds: number;
  min_time_remaining_seconds: number | null;
  enable_eth_trading: boolean;
  enable_solana_trading: boolean;
  enable_xrp_trading: boolean;
  dual_limit_price: number | null;
  dual_limit_shares: number | null;
}

export interface Config {
  polymarket: PolymarketConfig;
  trading: TradingConfig;
}

const DEFAULT_CONFIG: Config = {
  polymarket: {
    gamma_api_url: "https://gamma-api.polymarket.com",
    clob_api_url: "https://clob.polymarket.com",
    api_key: null,
    api_secret: null,
    api_passphrase: null,
    private_key: null,
    proxy_wallet_address: null,
    signature_type: null,
  },
  trading: {
    eth_condition_id: null,
    btc_condition_id: null,
    solana_condition_id: null,
    xrp_condition_id: null,
    check_interval_ms: 1000,
    fixed_trade_amount: 1,
    trigger_price: 0.9,
    min_elapsed_minutes: 10,
    sell_price: 0.99,
    max_buy_price: 0.95,
    stop_loss_price: 0.85,
    hedge_price: 0.5,
    market_closure_check_interval_seconds: 10,
    min_time_remaining_seconds: 30,
    enable_eth_trading: false,
    enable_solana_trading: false,
    enable_xrp_trading: false,
    dual_limit_price: 0.45,
    dual_limit_shares: null,
  },
};

export function loadConfig(configPath: string = "config.json"): Config {
  const path = join(process.cwd(), configPath);
  if (existsSync(path)) {
    const content = readFileSync(path, "utf-8");
    return JSON.parse(content) as Config;
  }
  writeFileSync(path, JSON.stringify(DEFAULT_CONFIG, null, 2));
  return DEFAULT_CONFIG;
}

export function parseArgs(): { simulation: boolean; config: string } {
  const args = process.argv.slice(2);
  let simulation = true;
  let config = "config.json";
  for (let i = 0; i < args.length; i++) {
    if (args[i] === "--no-simulation") simulation = false;
    else if (args[i] === "--simulation") simulation = true;
    else if (args[i] === "-c" || args[i] === "--config") config = args[++i] ?? config;
  }
  return { simulation, config };
}
