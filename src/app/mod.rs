use crate::request::{coin_list_metadata, coin_list_metadate_lighter};
use crate::ui::TuiApp;
use crate::websocket::create_batch_websocket_task;
use color_eyre::Result;
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Arc;
use std::sync::Mutex;
use tokio::sync::mpsc;
use tokio::task::JoinSet;

fn log_debug(msg: String) {
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open("/tmp/hype_debug.log")
    {
        let _ = writeln!(
            file,
            "[{}] APP: {}",
            chrono::Local::now().format("%H:%M:%S"),
            msg
        );
    }
}

#[derive(Debug, Clone)]
pub struct App {
    current_exchange: Arc<Mutex<u8>>,
}

impl App {
    pub fn new() -> Self {
        Self {
            current_exchange: Arc::new(Mutex::new(1)),
        }
    }

    fn get_exchange(&self) -> u8 {
        *self.current_exchange.lock().unwrap()
    }

    async fn fetch_coin_list(exchange: u8) -> Result<Vec<String>> {
        match exchange {
            1 => {
                // Fetch full coin list from Hyperliquid
                let coin = coin_list_metadata().await.unwrap();
                let coins: Vec<String> = coin
                    .universe
                    .iter()
                    .map(|asset| asset.name.clone())
                    .collect();
                Ok(coins)
            }
            2 => {
                // Fetch lighter coin list
                let funding_rates = coin_list_metadate_lighter().await.unwrap();
                let coins: Vec<String> = funding_rates
                    .iter()
                    .map(|rate| rate.symbol.clone())
                    .collect();
                Ok(coins)
            }
            _ => {
                // Default: fetch full list
                let coin = coin_list_metadata().await.unwrap();
                let coins: Vec<String> = coin
                    .universe
                    .iter()
                    .map(|asset| asset.name.clone())
                    .collect();
                Ok(coins)
            }
        }
    }

    pub async fn run(&self) -> Result<()> {
        let (tx, rx) = mpsc::unbounded_channel::<(String, f64, f64, f64, u8)>();

        // Channel to communicate exchange changes from UI
        let (exchange_tx, mut exchange_rx) = mpsc::unbounded_channel::<u8>();

        // Channel to send coin list updates to UI
        let (coin_list_tx, coin_list_rx) = mpsc::unbounded_channel::<Vec<String>>();

        // Fetch initial coin metadata
        let initial_exchange = self.get_exchange();
        log_debug(format!("Initial exchange value: {}", initial_exchange));
        let all_coins = Self::fetch_coin_list(initial_exchange).await.unwrap();
        log_debug(format!(
            "Fetched {} coins for initial exchange {}",
            all_coins.len(),
            initial_exchange
        ));

        // Clone for the websocket management task
        let tx_clone = tx.clone();
        let coin_list_tx_clone = coin_list_tx.clone();
        let all_coins_for_ws = all_coins.clone();

        // Spawn a task to manage websocket subscriptions
        let ws_manager = tokio::spawn(async move {
            let mut join_set = JoinSet::new();
            let mut last_exchange = initial_exchange;
            let mut current_coins = all_coins_for_ws.clone();

            // Helper function to start websockets - inline the logic to avoid lifetime issues
            let start_websockets =
                |coins: Vec<String>,
                 exchange: u8,
                 tx: mpsc::UnboundedSender<(String, f64, f64, f64, u8)>| {
                    log_debug("Aborting all existing websocket tasks".to_string());
                    log_debug(format!(
                        "Creating new websocket task for exchange {}",
                        exchange
                    ));
                    let task = create_batch_websocket_task(coins, tx, exchange);
                    async move { task.await.unwrap_or_else(|e| Err(e.into())) }
                };

            // Start initial websockets
            log_debug(format!(
                "Starting initial websockets with exchange {}",
                last_exchange
            ));

            // Abort all existing tasks
            join_set.abort_all();
            // Drain aborted tasks to ensure they're fully stopped
            log_debug("Waiting for aborted tasks to finish...".to_string());
            while let Some(result) = join_set.join_next().await {
                log_debug(format!("Drained task: cancelled={}", result.is_err()));
            }
            log_debug("All old tasks stopped".to_string());

            let initial_task =
                start_websockets(current_coins.clone(), last_exchange, tx_clone.clone());
            join_set.spawn(initial_task);
            log_debug("New websocket task spawned".to_string());

            // Monitor for exchange changes
            loop {
                tokio::select! {
                    Some(new_exchange) = exchange_rx.recv() => {
                        log_debug(format!("Received exchange change: {} -> {}", last_exchange, new_exchange));
                        if new_exchange != last_exchange {
                            last_exchange = new_exchange;
                            log_debug(format!("Exchange changed, fetching coin list for exchange {}", new_exchange));

                            // Fetch new coin list based on exchange
                            match App::fetch_coin_list(new_exchange).await {
                                Ok(new_coins) => {
                                    log_debug(format!("Fetched {} coins for exchange {}", new_coins.len(), new_exchange));
                                    current_coins = new_coins.clone();
                                    // Send updated coin list to UI
                                    let _ = coin_list_tx_clone.send(new_coins.clone());

                                    // Restart websockets with new coin list and exchange
                                    log_debug(format!("Starting websockets for exchange {}", new_exchange));

                                    // Abort all existing tasks
                                    log_debug("Aborting all existing websocket tasks".to_string());
                                    join_set.abort_all();

                                    // Drain aborted tasks to ensure they're fully stopped
                                    log_debug("Waiting for aborted tasks to finish...".to_string());
                                    while let Some(result) = join_set.join_next().await {
                                        log_debug(format!("Drained task: cancelled={}", result.is_err()));
                                    }
                                    log_debug("All old tasks stopped".to_string());

                                    let new_task = start_websockets(current_coins.clone(), new_exchange, tx_clone.clone());
                                    join_set.spawn(new_task);
                                    log_debug("New websocket task spawned".to_string());
                                }
                                Err(e) => {
                                    log_debug(format!("Failed to fetch coin list: {:?}", e));
                                    // If fetch fails, keep using current coins
                                }
                            }
                        } else {
                            log_debug(format!("Exchange unchanged: {}", new_exchange));
                        }
                    }
                    Some(result) = join_set.join_next() => {
                        match result {
                            Ok(Ok(_)) => {}
                            Ok(Err(_)) => {}
                            Err(e) if e.is_cancelled() => {
                                // Task was cancelled, this is expected
                            }
                            Err(_) => {}
                        }
                    }
                    else => break,
                }
            }

            Ok::<(), color_eyre::Report>(())
        });

        // Get initial coin list for UI
        let initial_coin_list = all_coins.clone();

        // Create UI task with exchange sender
        let current_exchange_ui = Arc::clone(&self.current_exchange);
        let ui_task = tokio::spawn(async move {
            let terminal = ratatui::init();
            let app = TuiApp::new(
                initial_coin_list.clone(),
                current_exchange_ui,
                exchange_tx,
                initial_coin_list,
                coin_list_rx,
            );
            let app_result = app.run(terminal, rx);
            ratatui::restore();
            app_result
        });

        // Wait for UI to finish (user quits)
        let ui_result = ui_task.await;

        // Cancel websocket manager when UI exits
        ws_manager.abort();

        match ui_result {
            Ok(Ok(_)) => {}
            Ok(Err(_)) => {}
            Err(_) => {}
        }

        Ok(())
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
