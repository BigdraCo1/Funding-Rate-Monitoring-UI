use hyperliquid_rust_sdk::{BaseUrl, InfoClient, Message, Subscription};
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let coin = "BTC";

    let mut client = InfoClient::new(None, Some(BaseUrl::Mainnet)).await?;

    // Keep unbounded_channel as required by the API
    let (sender_channel, mut receiver_channel) = mpsc::unbounded_channel::<Message>();

    client
        .subscribe(
            Subscription::Bbo {
                coin: coin.to_string(),
            },
            sender_channel,
        )
        .await?;

    let mut total_latency = 0u64;
    let mut count = 0u64;
    let mut min_latency = u64::MAX;
    let mut max_latency = 0u64;

    // Faster time source - calculate offset once
    let epoch_offset = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let start_instant = std::time::Instant::now();

    while let Some(message) = receiver_channel.recv().await {
        // Use Instant for faster timing
        let now_ms = epoch_offset + start_instant.elapsed().as_millis() as u64;

        if let Message::Bbo(bbo) = message {
            let latency = now_ms - bbo.data.time;
            total_latency += latency;
            count += 1;
            min_latency = min_latency.min(latency);
            max_latency = max_latency.max(latency);

            // Print every 100 messages to reduce I/O overhead
            if count % 100 == 0 {
                println!(
                    "[{coin}] latency: {} ms | avg: {} ms | count: {} | min: {} ms | max: {} ms",
                    latency,
                    total_latency / count,
                    count,
                    min_latency,
                    max_latency
                );
            }
        }
    }

    Ok(())
}
