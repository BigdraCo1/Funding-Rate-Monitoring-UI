use hyperliquid_rust_sdk::{InfoClient, BaseUrl};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Create client
    let mut client = InfoClient::new(None, Some(BaseUrl::Mainnet))
        .await
        .expect("Failed to create client");

    // Get meta info (perp markets)
    let info = client.meta().await.expect("Failed to get meta");

    println!("Available perpetual pairs:");
    for item in info.universe {
        println!("{}", item.name);
    }

    Ok(())
}
