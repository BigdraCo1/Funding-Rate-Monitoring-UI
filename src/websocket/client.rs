use color_eyre::Result;
use futures::{SinkExt, StreamExt};
use hyperliquid_rust_sdk::{BaseUrl, InfoClient, Message, Subscription};
use serde_json::json;
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::{interval, timeout};
use tokio_tungstenite::{connect_async, tungstenite::Message as WsMessage};

use crate::request::coin_list_metadate_lighter;
use crate::third_party::lighter::api_path::LIGHTER_STREAM_URL;
use crate::third_party::lighter::data::MarketStatsMessage;

fn log_debug(msg: String) {
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open("/tmp/hype_debug.log")
    {
        let _ = writeln!(
            file,
            "[{}] {}",
            chrono::Local::now().format("%H:%M:%S"),
            msg
        );
    }
}

pub fn create_batch_websocket_task(
    coins: Vec<String>,
    tx: mpsc::UnboundedSender<(String, f64, f64, f64, u8)>,
    current_exchange: u8,
) -> JoinHandle<Result<()>> {
    tokio::spawn(async move {
        log_debug(format!(
            "create_batch_websocket_task called with exchange: {}",
            current_exchange
        ));
        match current_exchange {
            1 => {
                // Hyperliquid only
                log_debug("Starting Hyperliquid websocket".to_string());
                hyperliquid_websocket(coins, tx, 1).await
            }
            2 => {
                // Lighter only
                log_debug("Starting Lighter websocket".to_string());
                lighter_websocket(coins, tx, 2).await
            }
            3 => {
                // Both Hyperliquid and Lighter
                log_debug("Starting BOTH Hyperliquid and Lighter websockets".to_string());
                let tx_hl = tx.clone();
                let tx_lt = tx.clone();
                let coins_hl = coins.clone();
                let coins_lt = coins.clone();

                let hl_task =
                    tokio::spawn(async move { hyperliquid_websocket(coins_hl, tx_hl, 3).await });
                let lt_task =
                    tokio::spawn(async move { lighter_websocket(coins_lt, tx_lt, 3).await });

                // Wait for both to complete (or fail)
                let _ = tokio::try_join!(hl_task, lt_task);
                Ok(())
            }
            _ => {
                // Default to Hyperliquid
                log_debug(format!(
                    "Unknown exchange {}, defaulting to Hyperliquid",
                    current_exchange
                ));
                hyperliquid_websocket(coins, tx, 1).await
            }
        }
    })
}

async fn hyperliquid_websocket(
    coins: Vec<String>,
    tx: mpsc::UnboundedSender<(String, f64, f64, f64, u8)>,
    exchange: u8,
) -> Result<()> {
    log_debug(format!(
        "hyperliquid_websocket starting with {} coins, exchange={}",
        coins.len(),
        exchange
    ));
    let mut client = InfoClient::new(None, Some(BaseUrl::Mainnet))
        .await
        .expect("Failed to create Hyperliquid client");

    let (sender_channel, mut receiver_channel) = mpsc::unbounded_channel::<Message>();

    // Subscribe to all coins
    for coin in coins.iter() {
        let _ = client
            .subscribe(
                Subscription::ActiveAssetCtx { coin: coin.clone() },
                sender_channel.clone(),
            )
            .await
            .expect("Hyperliquid subscription failed");
    }

    // Handle messages from all subscriptions
    while let Some(message) = receiver_channel.recv().await {
        match message {
            Message::ActiveAssetCtx(active_ctx) => {
                handle_hyperliquid_message(active_ctx, &tx, exchange);
            }
            _ => {
                // Handle other message types if needed
            }
        }
    }

    Ok(())
}

async fn lighter_websocket(
    _coins: Vec<String>,
    tx: mpsc::UnboundedSender<(String, f64, f64, f64, u8)>,
    exchange: u8,
) -> Result<()> {
    log_debug(format!("lighter_websocket starting, exchange={}", exchange));

    // Fetch market mapping from API
    log_debug("Fetching Lighter market mapping...".to_string());
    let funding_rates = coin_list_metadate_lighter()
        .await
        .map_err(|e| color_eyre::eyre::eyre!("Failed to fetch Lighter coin list: {}", e))?;

    let mut market_map: HashMap<u8, String> = HashMap::new();
    for market in funding_rates {
        market_map.insert(market.market_id, market.symbol);
    }
    log_debug(format!(
        "Market map created with {} entries",
        market_map.len()
    ));

    // Reconnection loop with exponential backoff
    let mut reconnect_delay = Duration::from_secs(1);
    let max_reconnect_delay = Duration::from_secs(60);
    let mut attempt = 0;

    loop {
        attempt += 1;
        log_debug(format!("Connection attempt #{}", attempt));

        // Connect to Lighter WebSocket
        log_debug(format!(
            "Connecting to Lighter WebSocket: {}",
            LIGHTER_STREAM_URL
        ));

        let ws_result = connect_async(LIGHTER_STREAM_URL).await;

        let (ws_stream, _) = match ws_result {
            Ok(stream) => {
                log_debug("Connected to Lighter WebSocket".to_string());
                // Reset reconnect delay on successful connection
                reconnect_delay = Duration::from_secs(1);
                stream
            }
            Err(e) => {
                log_debug(format!(
                    "Lighter connection failed: {}, retrying in {:?}",
                    e, reconnect_delay
                ));
                tokio::time::sleep(reconnect_delay).await;
                // Exponential backoff
                reconnect_delay = std::cmp::min(reconnect_delay * 2, max_reconnect_delay);
                continue;
            }
        };

        let (mut write, mut read) = ws_stream.split();

        // Subscribe to market stats for all markets
        let subscribe_msg = json!({
            "type": "subscribe",
            "channel": "market_stats/all"
        });

        log_debug(format!(
            "Sending subscription: {}",
            subscribe_msg.to_string()
        ));
        if let Err(e) = write.send(WsMessage::Text(subscribe_msg.to_string())).await {
            log_debug(format!(
                "Failed to send subscription: {}, reconnecting...",
                e
            ));
            tokio::time::sleep(reconnect_delay).await;
            reconnect_delay = std::cmp::min(reconnect_delay * 2, max_reconnect_delay);
            continue;
        }
        log_debug("Successfully sent subscription to Lighter WebSocket".to_string());

        // Set up ping interval (30 seconds)
        let mut ping_interval = interval(Duration::from_secs(30));
        ping_interval.tick().await; // Skip the first immediate tick

        // Listen for messages
        log_debug("Listening for Lighter messages with health check enabled...".to_string());
        let should_reconnect;

        loop {
            tokio::select! {
                // Handle incoming messages with timeout
                message = timeout(Duration::from_secs(60), read.next()) => {
                    match message {
                        Ok(Some(Ok(WsMessage::Text(text)))) => {
                            log_debug(format!("Received text message: {} bytes", text.len()));
                            // Log first 500 chars of raw message for debugging
                            let preview = if text.len() > 500 {
                                format!("{}...", &text[..500])
                            } else {
                                text.clone()
                            };
                            log_debug(format!("Raw message preview: {}", preview));

                            if let Ok(parsed) = serde_json::from_str::<MarketStatsMessage>(&text) {
                                log_debug(format!(
                                    "Successfully parsed Lighter message with {} market stats",
                                    parsed.market_stats.len()
                                ));
                                handle_lighter_message(parsed, &tx, exchange, &market_map);
                            } else {
                                log_debug(format!("Failed to parse message as MarketStatsMessage. First 300 chars: {}", &text[..text.len().min(300)]));
                            }
                        }
                        Ok(Some(Ok(WsMessage::Binary(data)))) => {
                            log_debug(format!("Received binary message: {} bytes", data.len()));
                        }
                        Ok(Some(Ok(WsMessage::Ping(data)))) => {
                            log_debug("Received ping from server, sending pong".to_string());
                            if let Err(e) = write.send(WsMessage::Pong(data)).await {
                                log_debug(format!("Failed to send pong: {}, reconnecting...", e));
                                should_reconnect = true;
                                break;
                            }
                        }
                        Ok(Some(Ok(WsMessage::Pong(_)))) => {
                            log_debug("Received pong from server".to_string());
                        }
                        Ok(Some(Ok(WsMessage::Close(_)))) => {
                            log_debug("Received close frame from server, reconnecting...".to_string());
                            should_reconnect = true;
                            break;
                        }
                        Ok(Some(Err(e))) => {
                            log_debug(format!("Lighter WebSocket error: {}, reconnecting...", e));
                            should_reconnect = true;
                            break;
                        }
                        Ok(None) => {
                            log_debug("Lighter WebSocket stream ended, reconnecting...".to_string());
                            should_reconnect = true;
                            break;
                        }
                        Err(_) => {
                            log_debug("TIMEOUT: No message received within 60 seconds, reconnecting...".to_string());
                            should_reconnect = true;
                            break;
                        }
                        _ => {
                            log_debug("Received other message type".to_string());
                        }
                    }
                }
                // Send periodic pings
                _ = ping_interval.tick() => {
                    log_debug("⏰ PING: Sending ping to keep connection alive".to_string());
                    if let Err(e) = write.send(WsMessage::Ping(vec![])).await {
                        log_debug(format!("Failed to send ping: {}, reconnecting...", e));
                        should_reconnect = true;
                        break;
                    } else {
                        log_debug("✓ Ping sent successfully".to_string());
                    }
                }
            }
        }

        if should_reconnect {
            log_debug(format!("Reconnecting in {:?}...", reconnect_delay));
            tokio::time::sleep(reconnect_delay).await;
            // Exponential backoff
            reconnect_delay = std::cmp::min(reconnect_delay * 2, max_reconnect_delay);
        }
    }
}

fn handle_hyperliquid_message(
    active_ctx: hyperliquid_rust_sdk::ActiveAssetCtx,
    tx: &mpsc::UnboundedSender<(String, f64, f64, f64, u8)>,
    exchange: u8,
) {
    if let hyperliquid_rust_sdk::AssetCtx::Perps(perps_ctx) = &active_ctx.data.ctx {
        let coin = active_ctx.data.coin.clone();
        let funding = perps_ctx.funding.parse::<f64>().unwrap_or(0.0);
        let oi = perps_ctx.open_interest.parse::<f64>().unwrap_or(0.0);
        let price = perps_ctx.oracle_px.parse::<f64>().unwrap_or(0.0);
        let _ = tx.send((coin.clone(), funding, oi, price, exchange));
        log_debug(format!("Sent HL data: {} exchange={}", coin, exchange));
    }
}

fn handle_lighter_message(
    parsed: MarketStatsMessage,
    tx: &mpsc::UnboundedSender<(String, f64, f64, f64, u8)>,
    exchange: u8,
    market_map: &HashMap<u8, String>,
) {
    for (_key, stats) in parsed.market_stats {
        // Map market_id to symbol using the HashMap
        let symbol = market_map
            .get(&(stats.market_id as u8))
            .cloned()
            .unwrap_or_else(|| format!("UNKNOWN_{}", stats.market_id));
        let funding = stats.current_funding_rate.parse::<f64>().unwrap_or(0.0);
        let price = stats.mark_price.parse::<f64>().unwrap_or(0.0);
        let oi = (stats.open_interest.parse::<f64>().unwrap_or(0.0) / price) * 2.0f64;
        let _ = tx.send((symbol.clone(), funding, oi, price, exchange));
        log_debug(format!("Sent LT data: {} exchange={}", symbol, exchange));
    }
}
