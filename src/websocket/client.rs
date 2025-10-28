use color_eyre::Result;
use hyperliquid_rust_sdk::{BaseUrl, InfoClient, Message, Subscription};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

pub fn create_websocket_task(
    coin: String,
    tx: mpsc::UnboundedSender<(String, f64, f64, f64)>,
) -> JoinHandle<Result<()>> {
    tokio::spawn(async move {
        let mut client = InfoClient::new(None, Some(BaseUrl::Mainnet))
            .await
            .expect("Failed to create client");

        let (sender_channel, mut receiver_channel) = mpsc::unbounded_channel::<Message>();

        let _ = client
            .subscribe(
                Subscription::ActiveAssetCtx { coin: coin.clone() },
                sender_channel,
            )
            .await
            .expect("Subscription failed");

        while let Some(message) = receiver_channel.recv().await {
            match message {
                Message::ActiveAssetCtx(active_ctx) => {
                    handle_active_asset_ctx(active_ctx, &coin, &tx);
                }
                _ => {
                    // Handle other message types if needed
                }
            }
        }

        Ok(())
    })
}

fn handle_active_asset_ctx(
    active_ctx: hyperliquid_rust_sdk::ActiveAssetCtx,
    coin: &str,
    tx: &mpsc::UnboundedSender<(String, f64, f64, f64)>,
) {
    if let hyperliquid_rust_sdk::AssetCtx::Perps(perps_ctx) = &active_ctx.data.ctx {
        let funding = perps_ctx.funding.parse::<f64>().unwrap_or(0.0);
        let oi = perps_ctx.open_interest.parse::<f64>().unwrap_or(0.0);
        let price = perps_ctx.oracle_px.parse::<f64>().unwrap_or(0.0);
        let _ = tx.send((coin.to_string(), funding, oi, price));
    }
}
