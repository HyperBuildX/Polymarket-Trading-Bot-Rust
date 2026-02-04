use crate::models::*;
use crate::detector::TokenType;
use rust_decimal::Decimal;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;
use chrono::Utc;

/// Represents a pending limit order in simulation
#[derive(Debug, Clone)]
pub struct SimulatedLimitOrder {
    pub token_id: String,
    pub token_type: TokenType,
    pub condition_id: String,
    pub target_price: f64,
    pub size: f64,
    pub side: String, // "BUY" or "SELL"
    pub timestamp: std::time::Instant,
    pub period_timestamp: u64,
    pub filled: bool,
}

/// Represents an open position in simulation
#[derive(Debug, Clone)]
pub struct SimulatedPosition {
    pub token_id: String,
    pub token_type: TokenType,
    pub condition_id: String,
    pub purchase_price: f64,
    pub units: f64,
    pub investment_amount: f64,
    pub sell_price: Option<f64>, // Target sell price if set
    pub purchase_timestamp: std::time::Instant,
    pub period_timestamp: u64,
    pub sold: bool,
    pub sell_price_actual: Option<f64>, // Actual sell price when sold
    pub sell_timestamp: Option<std::time::Instant>,
}

/// Simulation tracker for tracking orders, positions, and PnL
pub struct SimulationTracker {
    pending_limit_orders: Arc<Mutex<HashMap<String, SimulatedLimitOrder>>>, // Key: token_id + side
    positions: Arc<Mutex<HashMap<String, SimulatedPosition>>>, // Key: token_id
    log_file: Arc<Mutex<std::fs::File>>, // Main simulation log
    market_files: Arc<Mutex<HashMap<String, Arc<Mutex<std::fs::File>>>>>, // Per-market files: condition_id -> file
    total_realized_pnl: Arc<Mutex<f64>>,
    total_invested: Arc<Mutex<f64>>,
}

impl SimulationTracker {
    pub fn new(log_file_path: &str) -> Result<Self> {
        // Create history directory if it doesn't exist
        std::fs::create_dir_all("history").context("Failed to create history directory")?;
        
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_file_path)
            .context("Failed to open simulation log file")?;
        
        Ok(Self {
            pending_limit_orders: Arc::new(Mutex::new(HashMap::new())),
            positions: Arc::new(Mutex::new(HashMap::new())),
            log_file: Arc::new(Mutex::new(file)),
            market_files: Arc::new(Mutex::new(HashMap::new())),
            total_realized_pnl: Arc::new(Mutex::new(0.0)),
            total_invested: Arc::new(Mutex::new(0.0)),
        })
    }

    /// Get or create a market-specific log file
    /// Skips dummy markets - they should only log to simulation.toml
    async fn get_market_file(&self, condition_id: &str, period_timestamp: u64) -> Result<Arc<Mutex<std::fs::File>>> {
        // Skip dummy markets - they don't need separate files
        if condition_id == "dummy_eth_fallba" || 
           condition_id == "dummy_solana_fal" || 
           condition_id == "dummy_xrp_fallba" ||
           condition_id.starts_with("dummy_") {
            return Err(anyhow::anyhow!("Skipping dummy market file creation"));
        }
        
        let mut files = self.market_files.lock().await;
        
        if let Some(file) = files.get(condition_id) {
            return Ok(file.clone());
        }
        
        // Create new file for this market
        let file_name = format!("history/market_{}_{}.toml", &condition_id[..16], period_timestamp);
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file_name)
            .context(format!("Failed to create market log file: {}", file_name))?;
        
        let file_arc = Arc::new(Mutex::new(file));
        files.insert(condition_id.to_string(), file_arc.clone());
        
        Ok(file_arc)
    }

    /// Log to simulation file (and optionally to market-specific file)
    pub async fn log_to_file(&self, message: &str) {
        let timestamp = Utc::now().format("%Y-%m-%dT%H:%M:%SZ");
        let log_message = format!("[{}] {}\n", timestamp, message);
        
        let mut file = self.log_file.lock().await;
        let _ = write!(*file, "{}", log_message);
        let _ = file.flush();
    }

    /// Log to both main file and market-specific file
    /// For dummy markets, only logs to simulation.toml (no separate market file)
    pub async fn log_to_market(&self, condition_id: &str, period_timestamp: u64, message: &str) {
        // Write to main simulation log (always)
        self.log_to_file(message).await;
        
        // Write to market-specific file (skip dummy markets)
        if let Ok(market_file) = self.get_market_file(condition_id, period_timestamp).await {
            let timestamp = Utc::now().format("%Y-%m-%dT%H:%M:%SZ");
            let log_message = format!("[{}] {}\n", timestamp, message);
            
            let mut file = market_file.lock().await;
            let _ = write!(*file, "{}", log_message);
            let _ = file.flush();
        }
        // If get_market_file returns an error (e.g., for dummy markets), silently skip
    }

    /// Add a limit order to simulation tracking
    pub async fn add_limit_order(
        &self,
        token_id: String,
        token_type: TokenType,
        condition_id: String,
        target_price: f64,
        size: f64,
        side: String,
        period_timestamp: u64,
    ) {
        let side_display = side.clone();
        let token_type_str = match &token_type {
            TokenType::BtcUp | TokenType::EthUp | TokenType::SolanaUp | TokenType::XrpUp => "Up",
            TokenType::BtcDown | TokenType::EthDown | TokenType::SolanaDown | TokenType::XrpDown => "Down",
        };
        let order_key = format!("{}_{}", token_id, side);
        let order = SimulatedLimitOrder {
            token_id: token_id.clone(),
            token_type,
            condition_id,
            target_price,
            size,
            side,
            timestamp: std::time::Instant::now(),
            period_timestamp,
            filled: false,
        };
        
        let mut orders = self.pending_limit_orders.lock().await;
        orders.insert(order_key.clone(), order);
        
        let total_pending = orders.values().filter(|o| !o.filled).count();
        let order_count = orders.len();
        drop(orders);
        
        self.log_to_file(&format!(
            "📋 SIMULATION: Limit {} order added - Token: {} ({}), Price: ${:.6}, Size: {:.6} | Total orders: {}, Unfilled: {}",
            if side_display == "BUY" { "BUY" } else { "SELL" },
            token_id,
            token_type_str,
            target_price,
            size,
            order_count,
            total_pending
        )).await;
        
        // Verify order was stored
        {
            let orders_check = self.pending_limit_orders.lock().await;
            if orders_check.contains_key(&order_key) {
                self.log_to_file(&format!(
                    "✅ SIMULATION: Order verified stored - Key: {}",
                    &order_key[..32]
                )).await;
            } else {
                self.log_to_file(&format!(
                    "❌ SIMULATION: Order NOT found after storage - Key: {}",
                    &order_key[..32]
                )).await;
            }
        }
    }

    /// Check if any limit orders should be filled based on current prices
    pub async fn check_limit_orders(&self, current_prices: &HashMap<String, TokenPrice>) {
        let mut orders_to_fill = Vec::new();
        
        {
            let orders = self.pending_limit_orders.lock().await;
            let unfilled_count = orders.values().filter(|o| !o.filled).count();
            
            if unfilled_count > 0 && current_prices.is_empty() {
                self.log_to_file(&format!(
                    "⚠️  SIMULATION: Checking {} pending order(s) but no price data available",
                    unfilled_count
                )).await;
                return; // Can't check fills without price data
            }
            
            for (key, order) in orders.iter() {
                if order.filled {
                    continue;
                }
                
                if let Some(price_data) = current_prices.get(&order.token_id) {
                    let should_fill = match order.side.as_str() {
                        "BUY" => {
                            // Buy order fills if ask price <= target price
                            if let Some(ask) = price_data.ask {
                                let ask_f64: f64 = ask.to_string().parse().unwrap_or(0.0);
                                let fill_condition = ask_f64 > 0.0 && ask_f64 <= order.target_price;
                                
                                let token_type_str = match &order.token_type {
                                    TokenType::BtcUp | TokenType::EthUp | TokenType::SolanaUp | TokenType::XrpUp => "Up",
                                    TokenType::BtcDown | TokenType::EthDown | TokenType::SolanaDown | TokenType::XrpDown => "Down",
                                };
                                
                                // Always log price check for BUY orders
                                let bid_str = price_data.bid.map(|b| format!("${:.6}", b.to_string().parse::<f64>().unwrap_or(0.0))).unwrap_or_else(|| "N/A".to_string());
                                let diff_pct = if order.target_price > 0.0 {
                                    ((order.target_price - ask_f64) / order.target_price * 100.0)
                                } else {
                                    0.0
                                };
                                
                                if fill_condition {
                                    self.log_to_file(&format!(
                                        "🎯 SIMULATION: ✅ FILL DETECTED! BUY {} - Token: {} ({}), Ask: ${:.6} <= Target: ${:.6}",
                                        token_type_str,
                                        &order.token_id[..16],
                                        token_type_str,
                                        ask_f64,
                                        order.target_price
                                    )).await;
                                } else {
                                    // Log price check details (always log when checking)
                                    self.log_to_file(&format!(
                                        "🔍 SIMULATION: BUY {} check - Token: {} ({}), Bid: {}, Ask: ${:.6}, Target: ${:.6}, Diff: {:.2}% {}",
                                        token_type_str,
                                        &order.token_id[..16],
                                        token_type_str,
                                        bid_str,
                                        ask_f64,
                                        order.target_price,
                                        diff_pct,
                                        if ask_f64 > order.target_price { "(Ask > Target - waiting)" } else { "(Ask <= Target - should fill!)" }
                                    )).await;
                                }
                                
                                fill_condition
                            } else {
                                let token_type_str = match &order.token_type {
                                    TokenType::BtcUp | TokenType::EthUp | TokenType::SolanaUp | TokenType::XrpUp => "Up",
                                    TokenType::BtcDown | TokenType::EthDown | TokenType::SolanaDown | TokenType::XrpDown => "Down",
                                };
                                self.log_to_file(&format!(
                                    "⚠️  SIMULATION: BUY {} - Token: {} ({}), No ask price available",
                                    token_type_str,
                                    &order.token_id[..16],
                                    token_type_str
                                )).await;
                                false
                            }
                        }
                        "SELL" => {
                            // Sell order fills if bid price >= target price
                            if let Some(bid) = price_data.bid {
                                let bid_f64: f64 = bid.to_string().parse().unwrap_or(0.0);
                                let fill_condition = bid_f64 > 0.0 && bid_f64 >= order.target_price;
                                
                                // Log when we find a fill opportunity
                                if fill_condition {
                                    let token_type_str = match &order.token_type {
                                        TokenType::BtcUp | TokenType::EthUp | TokenType::SolanaUp | TokenType::XrpUp => "Up",
                                        TokenType::BtcDown | TokenType::EthDown | TokenType::SolanaDown | TokenType::XrpDown => "Down",
                                    };
                                    self.log_to_file(&format!(
                                        "🎯 SIMULATION: Fill detected! SELL {} - Token: {} ({}), Bid: ${:.6} >= Target: ${:.6}",
                                        token_type_str,
                                        &order.token_id[..16],
                                        token_type_str,
                                        bid_f64,
                                        order.target_price
                                    )).await;
                                }
                                
                                fill_condition
                            } else {
                                false
                            }
                        }
                        _ => false,
                    };
                    
                    if should_fill {
                        orders_to_fill.push(key.clone());
                    }
                }
            }
        }
        
        // Fill the orders
        let fills_count = orders_to_fill.len();
        if fills_count > 0 {
            self.log_to_file(&format!(
                "🔄 SIMULATION: Processing {} fill(s)...",
                fills_count
            )).await;
        }
        
        for key in orders_to_fill {
            self.fill_limit_order(&key, current_prices).await;
        }
    }

    /// Fill a limit order and create a position (for BUY) or close a position (for SELL)
    async fn fill_limit_order(&self, order_key: &str, current_prices: &HashMap<String, TokenPrice>) {
        let mut orders = self.pending_limit_orders.lock().await;
        let order = match orders.get_mut(order_key) {
            Some(o) if !o.filled => o,
            _ => return,
        };
        
        let fill_price = match order.side.as_str() {
            "BUY" => {
                current_prices.get(&order.token_id)
                    .and_then(|p| p.ask)
                    .map(|ask| ask.to_string().parse::<f64>().unwrap_or(order.target_price))
                    .unwrap_or(order.target_price)
            }
            "SELL" => {
                current_prices.get(&order.token_id)
                    .and_then(|p| p.bid)
                    .map(|bid| bid.to_string().parse::<f64>().unwrap_or(order.target_price))
                    .unwrap_or(order.target_price)
            }
            _ => order.target_price,
        };
        
        order.filled = true;
        
        match order.side.as_str() {
            "BUY" => {
                // Create a new position
                let investment_amount = order.size * fill_price;
                let position_key = order.token_id.clone();
                
                let position = SimulatedPosition {
                    token_id: order.token_id.clone(),
                    token_type: order.token_type.clone(),
                    condition_id: order.condition_id.clone(),
                    purchase_price: fill_price,
                    units: order.size,
                    investment_amount,
                    sell_price: None, // Will be set when sell order is placed
                    purchase_timestamp: std::time::Instant::now(),
                    period_timestamp: order.period_timestamp,
                    sold: false,
                    sell_price_actual: None,
                    sell_timestamp: None,
                };
                
                {
                    let mut positions = self.positions.lock().await;
                    positions.insert(position_key, position);
                }
                
                {
                    let mut total_invested = self.total_invested.lock().await;
                    *total_invested += investment_amount;
                }
                
                let token_type_str = match &order.token_type {
                    TokenType::BtcUp | TokenType::EthUp | TokenType::SolanaUp | TokenType::XrpUp => "Up",
                    TokenType::BtcDown | TokenType::EthDown | TokenType::SolanaDown | TokenType::XrpDown => "Down",
                };
                
                let fill_msg = format!(
                    "✅ SIMULATION: Limit BUY order FILLED - Token: {} ({}), Fill Price: ${:.6}, Size: {:.6}, Investment: ${:.2}",
                    order.token_id,
                    token_type_str,
                    fill_price,
                    order.size,
                    investment_amount
                );
                self.log_to_file(&fill_msg).await;
                self.log_to_market(&order.condition_id, order.period_timestamp, &fill_msg).await;
                
                // Log position creation summary
                let (total_spent, total_earned, total_realized_pnl) = self.get_total_spending_and_earnings().await;
                let open_positions = self.positions.lock().await.values().filter(|p| !p.sold).count();
                self.log_to_file(&format!(
                    "📊 SIMULATION: Position created! Open positions: {}, Total invested: ${:.2}, Total realized PnL: ${:.2}",
                    open_positions,
                    total_spent,
                    total_realized_pnl
                )).await;
            }
            "SELL" => {
                // Close an existing position
                let mut positions = self.positions.lock().await;
                if let Some(position) = positions.get_mut(&order.token_id) {
                    if !position.sold {
                        position.sold = true;
                        position.sell_price_actual = Some(fill_price);
                        position.sell_timestamp = Some(std::time::Instant::now());
                        
                        let realized_pnl = (fill_price - position.purchase_price) * position.units;
                        
                        {
                            let mut total_pnl = self.total_realized_pnl.lock().await;
                            *total_pnl += realized_pnl;
                        }
                        
                        let token_type_str = match &position.token_type {
                            TokenType::BtcUp | TokenType::EthUp | TokenType::SolanaUp | TokenType::XrpUp => "Up",
                            TokenType::BtcDown | TokenType::EthDown | TokenType::SolanaDown | TokenType::XrpDown => "Down",
                        };
                        
                        let sell_msg = format!(
                            "✅ SIMULATION: Limit SELL order FILLED - Token: {} ({}), Fill Price: ${:.6}, Size: {:.6}, Realized PnL: ${:.2}",
                            order.token_id,
                            token_type_str,
                            fill_price,
                            order.size,
                            realized_pnl
                        );
                        self.log_to_file(&sell_msg).await;
                        self.log_to_market(&order.condition_id, order.period_timestamp, &sell_msg).await;
                    }
                }
            }
            _ => {}
        }
    }

    /// Update sell price for a position (when limit sell order is placed)
    pub async fn set_position_sell_price(&self, token_id: &str, sell_price: f64) {
        let mut positions = self.positions.lock().await;
        if let Some(position) = positions.get_mut(token_id) {
            position.sell_price = Some(sell_price);
        }
    }

    /// Calculate unrealized PnL for all open positions
    pub async fn calculate_unrealized_pnl(&self, current_prices: &HashMap<String, TokenPrice>) -> f64 {
        let positions = self.positions.lock().await;
        let mut total_unrealized = 0.0;
        
        for position in positions.values() {
            if position.sold {
                continue;
            }
            
            if let Some(price_data) = current_prices.get(&position.token_id) {
                let current_price = price_data.mid_price()
                    .map(|p| p.to_string().parse::<f64>().unwrap_or(position.purchase_price))
                    .unwrap_or(position.purchase_price);
                
                let unrealized = (current_price - position.purchase_price) * position.units;
                total_unrealized += unrealized;
            }
        }
        
        total_unrealized
    }

    /// Get position summary
    pub async fn get_position_summary(&self, current_prices: &HashMap<String, TokenPrice>) -> String {
        let positions = self.positions.lock().await;
        let total_realized = *self.total_realized_pnl.lock().await;
        let total_invested = *self.total_invested.lock().await;
        let unrealized = self.calculate_unrealized_pnl(current_prices).await;
        let total_pnl = total_realized + unrealized;
        
        let open_positions: Vec<_> = positions.values()
            .filter(|p| !p.sold)
            .collect();
        
        let mut summary = format!(
            "═══════════════════════════════════════════════════════════\n\
             📊 SIMULATION POSITION SUMMARY\n\
             ═══════════════════════════════════════════════════════════\n\
             Total Invested: ${:.2}\n\
             Realized PnL: ${:.2}\n\
             Unrealized PnL: ${:.2}\n\
             Total PnL: ${:.2}\n\
             Open Positions: {}\n",
            total_invested,
            total_realized,
            unrealized,
            total_pnl,
            open_positions.len()
        );
        
        if !open_positions.is_empty() {
            summary.push_str("\nOpen Positions:\n");
            for (idx, pos) in open_positions.iter().enumerate() {
                let current_price = current_prices.get(&pos.token_id)
                    .and_then(|p| p.mid_price())
                    .map(|p| p.to_string().parse::<f64>().unwrap_or(pos.purchase_price))
                    .unwrap_or(pos.purchase_price);
                
                let unrealized = (current_price - pos.purchase_price) * pos.units;
                summary.push_str(&format!(
                    "  {}. {} - Purchase: ${:.6}, Current: ${:.6}, Units: {:.6}, Unrealized PnL: ${:.2}\n",
                    idx + 1,
                    pos.token_type.display_name(),
                    pos.purchase_price,
                    current_price,
                    pos.units,
                    unrealized
                ));
            }
        }
        
        summary.push_str("═══════════════════════════════════════════════════════════\n");
        summary
    }

    /// Write position summary to log file
    pub async fn log_position_summary(&self, current_prices: &HashMap<String, TokenPrice>) {
        let summary = self.get_position_summary(current_prices).await;
        self.log_to_file(&summary).await;
    }

    /// Check if a position exists for a given token_id
    pub async fn has_position(&self, token_id: &str) -> bool {
        let positions = self.positions.lock().await;
        positions.contains_key(token_id)
    }

    /// Get all token IDs from open positions
    pub async fn get_position_token_ids(&self) -> Vec<String> {
        let positions = self.positions.lock().await;
        positions.values()
            .filter(|p| !p.sold)
            .map(|p| p.token_id.clone())
            .collect()
    }

    /// Get all positions (for market closure checking)
    pub async fn get_all_positions(&self) -> Vec<SimulatedPosition> {
        let positions = self.positions.lock().await;
        positions.values()
            .filter(|p| !p.sold)
            .cloned()
            .collect()
    }

    /// Get all token IDs from pending limit orders
    pub async fn get_pending_order_token_ids(&self) -> Vec<String> {
        let orders = self.pending_limit_orders.lock().await;
        orders.values()
            .filter(|o| !o.filled)
            .map(|o| o.token_id.clone())
            .collect()
    }

    /// Get count of pending (unfilled) limit orders
    pub async fn get_pending_order_count(&self) -> usize {
        let orders = self.pending_limit_orders.lock().await;
        orders.values().filter(|o| !o.filled).count()
    }

    /// Calculate final PnL when a market resolves
    /// Resolves all positions for a given condition_id based on market outcome
    /// Returns: (total_spent, total_earned, net_pnl)
    pub async fn resolve_market_positions(
        &self,
        condition_id: &str,
        market_resolved_up: bool,
    ) -> (f64, f64, f64) {
        let mut positions_to_resolve = Vec::new();
        
        {
            let positions = self.positions.lock().await;
            for (token_id, position) in positions.iter() {
                if position.condition_id == condition_id && !position.sold {
                    positions_to_resolve.push((token_id.clone(), position.clone()));
                }
            }
        }
        
        let mut total_spent_for_market = 0.0;
        let mut total_earned_for_market = 0.0;
        
        for (token_id, position) in positions_to_resolve {
            // Determine if this position won based on token type and market outcome
            let position_won = match position.token_type {
                TokenType::BtcUp | TokenType::EthUp | TokenType::SolanaUp | TokenType::XrpUp => {
                    market_resolved_up
                }
                TokenType::BtcDown | TokenType::EthDown | TokenType::SolanaDown | TokenType::XrpDown => {
                    !market_resolved_up
                }
            };
            
            let final_value = if position_won { 1.0 } else { 0.0 };
            let position_value = position.units * final_value;
            let position_cost = position.investment_amount;
            
            total_spent_for_market += position_cost;
            total_earned_for_market += position_value;
            
            // Update position as sold
            {
                let mut positions = self.positions.lock().await;
                if let Some(pos) = positions.get_mut(&token_id) {
                    pos.sold = true;
                    pos.sell_price_actual = Some(final_value);
                    pos.sell_timestamp = Some(std::time::Instant::now());
                }
            }
            
            // Update realized PnL
            let position_pnl = position_value - position_cost;
            {
                let mut total_pnl = self.total_realized_pnl.lock().await;
                *total_pnl += position_pnl;
            }
            
            // Log the resolution
            let resolve_msg = format!(
                "🏁 MARKET RESOLVED: {} - {} | Purchase: ${:.6} | Final Value: ${:.6} | Units: {:.6} | Value: ${:.2} | Cost: ${:.2} | PnL: ${:.2}",
                position.token_type.display_name(),
                if position_won { "WON ($1.00)" } else { "LOST ($0.00)" },
                position.purchase_price,
                final_value,
                position.units,
                position_value,
                position_cost,
                position_pnl
            );
            self.log_to_file(&resolve_msg).await;
            self.log_to_market(&position.condition_id, position.period_timestamp, &resolve_msg).await;
        }
        
        let net_pnl = total_earned_for_market - total_spent_for_market;
        (total_spent_for_market, total_earned_for_market, net_pnl)
    }

    /// Get total spending and earnings across all positions
    pub async fn get_total_spending_and_earnings(&self) -> (f64, f64, f64) {
        let total_invested = *self.total_invested.lock().await;
        let total_realized = *self.total_realized_pnl.lock().await;
        let total_earned = total_invested + total_realized;
        (total_invested, total_earned, total_realized)
    }

    /// Log market start event
    /// Logs once to simulation.toml and writes to market-specific files (without duplicating main log)
    pub async fn log_market_start(&self, period_timestamp: u64, eth_condition_id: &str, btc_condition_id: &str, sol_condition_id: &str, xrp_condition_id: &str) {
        let msg = format!(
            "🆕 NEW MARKET STARTED | Period: {} | ETH: {} | BTC: {} | SOL: {} | XRP: {}",
            period_timestamp,
            &eth_condition_id[..16],
            &btc_condition_id[..16],
            &sol_condition_id[..16],
            &xrp_condition_id[..16]
        );
        // Log once to main simulation file
        self.log_to_file(&msg).await;
        
        // Write to market-specific files only (skip dummy markets and don't log to main file again)
        let timestamp = Utc::now().format("%Y-%m-%dT%H:%M:%SZ");
        let log_message = format!("[{}] {}\n", timestamp, &msg);
        
        if eth_condition_id != "dummy_eth_fallba" {
            if let Ok(market_file) = self.get_market_file(eth_condition_id, period_timestamp).await {
                let mut file = market_file.lock().await;
                let _ = write!(*file, "{}", log_message);
                let _ = file.flush();
            }
        }
        if btc_condition_id.len() > 16 {
            if let Ok(market_file) = self.get_market_file(btc_condition_id, period_timestamp).await {
                let mut file = market_file.lock().await;
                let _ = write!(*file, "{}", log_message);
                let _ = file.flush();
            }
        }
        if sol_condition_id != "dummy_solana_fal" {
            if let Ok(market_file) = self.get_market_file(sol_condition_id, period_timestamp).await {
                let mut file = market_file.lock().await;
                let _ = write!(*file, "{}", log_message);
                let _ = file.flush();
            }
        }
        if xrp_condition_id != "dummy_xrp_fallba" {
            if let Ok(market_file) = self.get_market_file(xrp_condition_id, period_timestamp).await {
                let mut file = market_file.lock().await;
                let _ = write!(*file, "{}", log_message);
                let _ = file.flush();
            }
        }
    }

    /// Log market end event
    pub async fn log_market_end(&self, market_name: &str, period_timestamp: u64, condition_id: &str) {
        let msg = format!(
            "🏁 MARKET ENDED | Market: {} | Period: {} | Condition: {}",
            market_name,
            period_timestamp,
            &condition_id[..16]
        );
        self.log_to_file(&msg).await;
        self.log_to_market(condition_id, period_timestamp, &msg).await;
    }

    /// Log summary of pending orders
    pub async fn log_pending_orders_summary(&self, current_prices: &HashMap<String, TokenPrice>) {
        let orders = self.pending_limit_orders.lock().await;
        let unfilled_orders: Vec<_> = orders.values()
            .filter(|o| !o.filled)
            .collect();
        
        if unfilled_orders.is_empty() {
            // Log that there are no pending orders (helps debug)
            self.log_to_file("📊 SIMULATION: No pending orders (all filled or none exist)").await;
            return;
        }
        
        let mut summary = format!(
            "📊 SIMULATION: {} pending order(s) waiting for fills:\n",
            unfilled_orders.len()
        );
        
        for (idx, order) in unfilled_orders.iter().enumerate() {
            let token_type_str = match &order.token_type {
                TokenType::BtcUp | TokenType::EthUp | TokenType::SolanaUp | TokenType::XrpUp => "Up",
                TokenType::BtcDown | TokenType::EthDown | TokenType::SolanaDown | TokenType::XrpDown => "Down",
            };
            
            if let Some(price_data) = current_prices.get(&order.token_id) {
                let (current_price, status) = match order.side.as_str() {
                    "BUY" => {
                        if let Some(ask) = price_data.ask {
                            let ask_f64: f64 = ask.to_string().parse().unwrap_or(0.0);
                            if ask_f64 > 0.0 {
                                (ask_f64, if ask_f64 <= order.target_price + 0.0001 { "✅ READY" } else { "⏳ waiting" })
                            } else {
                                (0.0, "⚠️  zero price")
                            }
                        } else {
                            (0.0, "⚠️  no ask")
                        }
                    }
                    "SELL" => {
                        if let Some(bid) = price_data.bid {
                            let bid_f64: f64 = bid.to_string().parse().unwrap_or(0.0);
                            if bid_f64 > 0.0 {
                                (bid_f64, if bid_f64 >= order.target_price - 0.0001 { "✅ READY" } else { "⏳ waiting" })
                            } else {
                                (0.0, "⚠️  zero price")
                            }
                        } else {
                            (0.0, "⚠️  no bid")
                        }
                    }
                    _ => (0.0, "unknown"),
                };
                
                summary.push_str(&format!(
                    "  {}. {} {} ({}): Target ${:.6}, Current ${:.6}, Status: {}\n",
                    idx + 1,
                    order.side,
                    token_type_str,
                    &order.token_id[..16],
                    order.target_price,
                    if current_price > 0.0 { current_price } else { 0.0 },
                    status
                ));
            } else {
                summary.push_str(&format!(
                    "  {}. {} {} ({}): Target ${:.6}, Current: N/A, Status: ⚠️  no price data\n",
                    idx + 1,
                    order.side,
                    token_type_str,
                    &order.token_id[..16],
                    order.target_price
                ));
            }
        }
        
        self.log_to_file(&summary).await;
    }
}

use anyhow::{Result, Context};
