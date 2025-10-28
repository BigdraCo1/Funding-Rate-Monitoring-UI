//! Integrated Ratatui + Hyperliquid Example
//!
//! Live table of Coin | Funding Rate | Open Interest
//! Updates via WebSocket subscriptions.

pub mod app;
pub mod config;
pub mod data;
pub mod request;
pub mod third_party;
pub mod ui;
pub mod websocket;

use crate::app::App;
use color_eyre::Result;

#[tokio::main(flavor = "multi_thread", worker_threads = 10)]
async fn main() -> Result<()> {
    color_eyre::install()?;

    let app = App::new();
    app.run().await
}
