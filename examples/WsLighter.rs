use futures::{SinkExt, StreamExt};
use reqwest::get;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;
use tokio_tungstenite::{connect_async, tungstenite::Message};

#[derive(Debug, Deserialize)]
struct CoinSymbol {
    market_id: u8,
    symbol: String,
}

#[derive(Debug, Deserialize)]
struct ApiResponse {
    code: u16,
    funding_rates: Vec<FundingRate>,
}

#[derive(Debug, Deserialize)]
struct FundingRate {
    market_id: u8,
    exchange: String,
    symbol: String,
    rate: f64,
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // let file = File::open("coin.json").unwrap();
    // let reader = BufReader::new(file);
    // let mut parse_json: Vec<CoinSymbol> = serde_json::from_reader(reader)?;
    // parse_json.sort_by(|a, b| a.market_id.cmp(&b.market_id));
    // parse_json.dedup_by_key(|c| c.market_id);
    let req_url = "https://mainnet.zklighter.elliot.ai/api/v1/funding-rates";
    let info = reqwest::get(req_url).await?.text().await?;
    let mut parse_json: ApiResponse = serde_json::from_str(&info)?;
    parse_json
        .funding_rates
        .sort_by(|a, b| a.market_id.cmp(&b.market_id));
    parse_json.funding_rates.dedup_by_key(|c| c.market_id);
    println!("info : {:?}", parse_json.funding_rates);
    let mut market_map: HashMap<u8, String> = HashMap::new();
    for market in parse_json.funding_rates {
        market_map.insert(market.market_id, market.symbol);
    }
    let url = "wss://mainnet.zklighter.elliot.ai/stream";

    // Connect to WebSocket with TLS
    println!("Connecting to {}...", url);

    let (ws_stream, response) = connect_async(url).await.map_err(|e| {
        eprintln!("Connection failed: {}", e);
        e
    })?;

    println!("Connected! Response: {:?}", response.status());

    let (mut write, mut read) = ws_stream.split();

    // Subscribe message
    let subscribe_msg = json!({
        "type": "subscribe",
        "channel": "market_stats/all"
    });

    // Send subscription message
    println!("Sending subscription message...");
    write
        .send(Message::Text(subscribe_msg.to_string().into()))
        .await?;
    println!("Subscribed to market_stats/0");

    // Listen for messages
    println!("Listening for messages...\n");
    while let Some(message) = read.next().await {
        match message {
            Ok(Message::Text(text)) => match serde_json::from_str::<MarketStatsMessage>(&text) {
                Ok(parsed) => {
                    for (key, stats) in &parsed.market_stats {
                        let symbol = market_map
                            .get(&(stats.market_id as u8))
                            .cloned()
                            .unwrap_or_else(|| "Unknown".to_string());
                        println!(
                            "Market: {} | Symbol: {} | Current Funding Rate: {} | Funding Rate: {} | oi: {}",
                            key,
                            symbol,
                            stats.current_funding_rate,
                            stats.funding_rate,
                            stats.open_interest_limit
                        );
                    }
                }
                Err(e) => eprintln!("❌ Failed to parse JSON: {e}"),
            },
            Ok(Message::Binary(bin)) => {
                println!("Received binary data: {} bytes", bin.len());
            }
            Ok(Message::Ping(data)) => {
                println!("Received ping");
                // Automatically send pong
                if let Err(e) = write.send(Message::Pong(data)).await {
                    eprintln!("Error sending pong: {}", e);
                }
            }
            Ok(Message::Pong(_)) => {
                println!("Received pong");
            }
            Ok(Message::Close(frame)) => {
                println!("Connection closed: {:?}", frame);
                break;
            }
            Err(e) => {
                eprintln!("Error receiving message: {}", e);
                break;
            }
            _ => {}
        }
    }

    Ok(())
}
