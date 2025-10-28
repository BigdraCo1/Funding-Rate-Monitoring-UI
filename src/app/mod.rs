use color_eyre::Result;
use futures::future;
use std::process;
use std::sync::Arc;
use std::sync::Mutex;
use tokio::sync::mpsc;

use crate::request::coin_list_metadata;
use crate::ui::TuiApp;
use crate::websocket::create_websocket_task;

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

    fn set_exchange(&self, exchange: u8) {
        *self.current_exchange.lock().unwrap() = exchange;
    }

    fn get_exchange(&self) -> u8 {
        *self.current_exchange.lock().unwrap()
    }

    pub async fn run(&self) -> Result<()> {
        let (tx, rx) = mpsc::unbounded_channel::<(String, f64, f64, f64)>();
        let mut tasks = vec![];

        // Fetch and print coin metadata
        let coin = coin_list_metadata().await.unwrap(); // or handle the Result properly
        let coin_name_list: Vec<String> = coin
            .universe
            .iter()
            .map(|asset| asset.name.clone())
            .collect();

        // Create WebSocket subscription tasks
        for coin in coin_name_list.iter() {
            let task = create_websocket_task(coin.to_string(), tx.clone());
            tasks.push(task);
        }

        // Create UI task
        let ui_task = tokio::spawn(async move {
            let terminal = ratatui::init();
            let app = TuiApp::new(
                coin_name_list
                    .clone()
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
            );
            let app_result = app.run(terminal, rx);
            ratatui::restore();
            app_result
        });

        tasks.push(ui_task);

        // Run all tasks concurrently
        let results = future::join_all(tasks).await;

        // Check if any task failed
        for (i, result) in results.into_iter().enumerate() {
            match result {
                Ok(Ok(_)) => println!("Task {} completed successfully", i),
                Ok(Err(e)) => eprintln!("Task {} failed: {}", i, e),
                Err(e) => eprintln!("Task {} panicked: {}", i, e),
            }
        }
        Ok(())
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
