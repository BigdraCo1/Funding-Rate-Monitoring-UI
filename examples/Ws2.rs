use hyperliquid_rust_sdk::{InfoClient, BaseUrl, Subscription, Message};
use tokio::sync::mpsc;
use futures::future;

#[tokio::main(flavor = "multi_thread", worker_threads = 5)]
async fn main() -> anyhow::Result<()> {
    let mut tasks = vec![];

    // Example: subscribe to 10 different coins
    let coins = vec![
        "BTC",
    ];

    for coin in coins {
        let coin = coin.to_string();
        let task = tokio::spawn(async move {
            // Each task has its own client + channel
            let mut client = InfoClient::new(None, Some(BaseUrl::Mainnet))
                .await
                .expect("Failed to create client");

            let (sender_channel, mut receiver_channel) = mpsc::unbounded_channel::<Message>();

            let resp = client.subscribe(
                Subscription::L2Book { coin: coin.clone() },
                sender_channel,
            )
            .await
            .expect("Subscription failed");

            println!("[{coin}] Subscribed: {:#?}", resp);

            while let Some(message) = receiver_channel.recv().await {
                match message {
                    Message::L2Book(l2_book) => {
                        match &l2_book.data {
                            hyperliquid_rust_sdk::L2BookData { coin, time, levels } => {
                                println!("Coin: {}", coin);
                                println!("Time: {}", time);
                                println!("Bids: {:?}", levels[0]);
                                println!("Asks: {:?}", levels[1]);
                            }
                        }
                        
                    }
                    Message::Trades(trades) => {
                        for t in trades.data {
                            println!("[{coin}] Trade: price={} size={}", t.px, t.sz);
                        }
                    }
                    _ => {
                        println!("[{coin}] Other message: {:#?}", message);
                    }
                }
            }
            
        });

        tasks.push(task);
    }

    // Run all ws tasks concurrently
    future::join_all(tasks).await;

    Ok(())
}
