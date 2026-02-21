import { PolymarketApi } from "./api.js";
import type { Market, MarketSnapshot, MarketData, TokenPrice } from "./types.js";

function parseNum(s: string | undefined): number | null {
  if (s == null) return null;
  const n = parseFloat(s);
  return Number.isFinite(n) ? n : null;
}

function marketToData(market: Market): MarketData {
  let up_token: TokenPrice | null = null;
  let down_token: TokenPrice | null = null;
  const tokens = market.tokens ?? [];
  for (const t of tokens) {
    const price = parseNum(t.price ?? "");
    const tokenId = (t as { tokenId?: string }).tokenId ?? (t as { token_id?: string }).token_id ?? "";
    const outcome = ((t as { outcome?: string }).outcome ?? "").toUpperCase();
    const tp: TokenPrice = { token_id: tokenId, bid: price, ask: price };
    if (outcome.includes("UP") || outcome === "1") up_token = tp;
    else if (outcome.includes("DOWN") || outcome === "0") down_token = tp;
  }
  return {
    condition_id: market.conditionId,
    up_token,
    down_token,
  };
}

/** Get best bid/ask from CLOB order book. Best bid = highest price, best ask = lowest (API may sort either way). */
async function fetchTokenPrice(api: PolymarketApi, tokenId: string): Promise<TokenPrice> {
  const book = await api.getOrderBook(tokenId);
  const bidPrices = (book.bids ?? []).map((b) => parseFloat(b.price)).filter((n) => Number.isFinite(n));
  const askPrices = (book.asks ?? []).map((a) => parseFloat(a.price)).filter((n) => Number.isFinite(n));
  const bestBid = bidPrices.length > 0 ? Math.max(...bidPrices) : null;
  const bestAsk = askPrices.length > 0 ? Math.min(...askPrices) : null;
  return { token_id: tokenId, bid: bestBid, ask: bestAsk };
}

/** Build snapshot with time remaining and period from market end or current time */
export function buildSnapshot(
  periodTimestamp: number,
  periodDurationSec: number,
  ethMarket: Market,
  btcMarket: Market,
  solanaMarket: Market,
  xrpMarket: Market,
  ethPrices: { up: TokenPrice | null; down: TokenPrice | null },
  btcPrices: { up: TokenPrice | null; down: TokenPrice | null },
  solanaPrices: { up: TokenPrice | null; down: TokenPrice | null },
  xrpPrices: { up: TokenPrice | null; down: TokenPrice | null }
): MarketSnapshot {
  const now = Math.floor(Date.now() / 1000);
  const endTime = periodTimestamp + periodDurationSec;
  const timeRemaining = Math.max(0, endTime - now);
  return {
    period_timestamp: periodTimestamp,
    time_remaining_seconds: timeRemaining,
    eth_market: {
      condition_id: ethMarket.conditionId,
      up_token: ethPrices.up,
      down_token: ethPrices.down,
    },
    btc_market: {
      condition_id: btcMarket.conditionId,
      up_token: btcPrices.up,
      down_token: btcPrices.down,
    },
    solana_market: {
      condition_id: solanaMarket.conditionId,
      up_token: solanaPrices.up,
      down_token: solanaPrices.down,
    },
    xrp_market: {
      condition_id: xrpMarket.conditionId,
      up_token: xrpPrices.up,
      down_token: xrpPrices.down,
    },
  };
}

const PERIOD_DURATION = 900;

/** Fetch order book prices for a market's up/down tokens */
async function fetchMarketPrices(
  api: PolymarketApi,
  market: Market
): Promise<{ up: TokenPrice | null; down: TokenPrice | null }> {
  const tokens = market.tokens ?? [];
  let upId: string | null = null;
  let downId: string | null = null;
  for (const t of tokens) {
    const id = t.tokenId ?? t.token_id ?? "";
    const outcome = (t.outcome ?? "").toUpperCase();
    if (outcome.includes("UP") || outcome === "1") upId = id;
    else if (outcome.includes("DOWN") || outcome === "0") downId = id;
  }
  const [up, down] = await Promise.all([
    upId ? fetchTokenPrice(api, upId) : Promise.resolve(null),
    downId ? fetchTokenPrice(api, downId) : Promise.resolve(null),
  ]);
  return { up, down };
}

/** Get current 15-min period timestamp */
export function currentPeriodTimestamp(): number {
  const now = Math.floor(Date.now() / 1000);
  return Math.floor(now / PERIOD_DURATION) * PERIOD_DURATION;
}

/** Fetch full snapshot for all four markets (BTC, ETH, SOL, XRP) */
export async function fetchSnapshot(
  api: PolymarketApi,
  ethMarket: Market,
  btcMarket: Market,
  solanaMarket: Market,
  xrpMarket: Market
): Promise<MarketSnapshot> {
  const period = currentPeriodTimestamp();
  const [btcPrices, ethPrices, solanaPrices, xrpPrices] = await Promise.all([
    fetchMarketPrices(api, btcMarket),
    fetchMarketPrices(api, ethMarket),
    fetchMarketPrices(api, solanaMarket),
    fetchMarketPrices(api, xrpMarket),
  ]);
  return buildSnapshot(
    period,
    PERIOD_DURATION,
    ethMarket,
    btcMarket,
    solanaMarket,
    xrpMarket,
    ethPrices,
    btcPrices,
    solanaPrices,
    xrpPrices
  );
}

/** Format one token as "bid/ask" e.g. "$0.13/$0.14" */
function fmtBidAsk(token: TokenPrice | null | undefined): string {
  if (!token) return "N/A";
  const bid = token.bid != null ? `$${token.bid.toFixed(2)}` : "N/A";
  const ask = token.ask != null ? `$${token.ask.toFixed(2)}` : "N/A";
  return `${bid}/${ask}`;
}

export function formatPrices(snap: MarketSnapshot): string {
  const t = Math.floor(snap.time_remaining_seconds / 60);
  const s = snap.time_remaining_seconds % 60;
  return (
    `BTC: U${fmtBidAsk(snap.btc_market.up_token)} D${fmtBidAsk(snap.btc_market.down_token)} | ` +
    `ETH: U${fmtBidAsk(snap.eth_market.up_token)} D${fmtBidAsk(snap.eth_market.down_token)} | ` +
    `SOL: U${fmtBidAsk(snap.solana_market.up_token)} D${fmtBidAsk(snap.solana_market.down_token)} | ` +
    `XRP: U${fmtBidAsk(snap.xrp_market.up_token)} D${fmtBidAsk(snap.xrp_market.down_token)} | ⏱️  ${t}m ${s}s`
  );
}
