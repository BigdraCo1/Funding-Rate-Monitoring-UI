use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub struct ApiFundingRatesResponse {
    pub code: u16,
    pub funding_rates: Vec<FundingRate>,
}

#[derive(Debug, Deserialize)]
pub struct FundingRate {
    pub market_id: u8,
    pub exchange: String,
    pub symbol: String,
    pub rate: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MarketStatsMessage {
    pub channel: String,
    pub market_stats: HashMap<String, MarketStatEntry>,
    #[serde(rename = "type")]
    pub message_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MarketStatEntry {
    pub market_id: u64,
    pub index_price: String,
    pub mark_price: String,
    pub open_interest: String,
    pub open_interest_limit: String,
    pub funding_clamp_small: String,
    pub funding_clamp_big: String,
    pub last_trade_price: String,
    pub current_funding_rate: String,
    pub funding_rate: String,
    pub funding_timestamp: i64,
    pub daily_base_token_volume: f64,
    pub daily_quote_token_volume: f64,
    pub daily_price_low: f64,
    pub daily_price_high: f64,
    pub daily_price_change: f64,
}
