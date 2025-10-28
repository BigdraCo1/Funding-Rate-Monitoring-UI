//! Integrated Ratatui + Hyperliquid Example
//!
//! Live table of Coin | Funding Rate | Open Interest
//! Updates via WebSocket subscriptions.

use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use futures::future;
use hyperliquid_rust_sdk::{BaseUrl, InfoClient, Message, Subscription};

use ratatui::{
    layout::{Constraint, Layout, Margin, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::Text,
    widgets::{
        Block, BorderType, Cell, HighlightSpacing, Paragraph, Row, Scrollbar,
        ScrollbarOrientation, ScrollbarState, Table, TableState,
    },
    DefaultTerminal, Frame,
};
use ratatui::style::palette::tailwind;
use tokio::sync::mpsc;

const PALETTES: [tailwind::Palette; 4] =
    [tailwind::BLUE, tailwind::EMERALD, tailwind::INDIGO, tailwind::RED];
const INFO_TEXT: [&str; 2] = [
    "(Esc) quit | (↑/↓) move row | (←/→) move col",
    "(Shift + →/←) cycle color",
];
const ITEM_HEIGHT: usize = 2;

#[derive(Clone, Debug)]
struct CoinData {
    coin: String,
    funding: f64,
    open_interest: f64,
}

struct TableColors {
    buffer_bg: Color,
    header_bg: Color,
    header_fg: Color,
    row_fg: Color,
    selected_row_style_fg: Color,
    selected_column_style_fg: Color,
    selected_cell_style_fg: Color,
    normal_row_color: Color,
    alt_row_color: Color,
    footer_border_color: Color,
}

impl TableColors {
    const fn new(color: &tailwind::Palette) -> Self {
        Self {
            buffer_bg: tailwind::SLATE.c950,
            header_bg: color.c900,
            header_fg: tailwind::SLATE.c200,
            row_fg: tailwind::SLATE.c200,
            selected_row_style_fg: color.c400,
            selected_column_style_fg: color.c400,
            selected_cell_style_fg: color.c600,
            normal_row_color: tailwind::SLATE.c950,
            alt_row_color: tailwind::SLATE.c900,
            footer_border_color: color.c400,
        }
    }
}

struct App {
    state: TableState,
    items: Vec<CoinData>,
    scroll_state: ScrollbarState,
    colors: TableColors,
    color_index: usize,
}

impl App {
    fn new(coins: Vec<String>) -> Self {
        let items = coins
            .into_iter()
            .map(|c| CoinData {
                coin: c,
                funding: 0.0,
                open_interest: 0.0,
            })
            .collect::<Vec<_>>();

        Self {
            state: TableState::default().with_selected(0),
            scroll_state: ScrollbarState::new((items.len().saturating_sub(1)) * ITEM_HEIGHT),
            colors: TableColors::new(&PALETTES[0]),
            color_index: 0,
            items,
        }
    }

    fn update_coin(&mut self, coin: &str, funding: f64, open_interest: f64) {
        if let Some(c) = self.items.iter_mut().find(|c| c.coin == coin) {
            c.funding = funding;
            c.open_interest = open_interest;
        }
    }

    fn next_row(&mut self) {
        let i = match self.state.selected() {
            Some(i) if i >= self.items.len() - 1 => 0,
            Some(i) => i + 1,
            None => 0,
        };
        self.state.select(Some(i));
        self.scroll_state = self.scroll_state.position(i * ITEM_HEIGHT);
    }

    fn previous_row(&mut self) {
        let i = match self.state.selected() {
            Some(0) => self.items.len() - 1,
            Some(i) => i - 1,
            None => 0,
        };
        self.state.select(Some(i));
        self.scroll_state = self.scroll_state.position(i * ITEM_HEIGHT);
    }

    fn next_column(&mut self) {
        self.state.select_next_column();
    }

    fn previous_column(&mut self) {
        self.state.select_previous_column();
    }

    fn next_color(&mut self) {
        self.color_index = (self.color_index + 1) % PALETTES.len();
    }

    fn previous_color(&mut self) {
        let count = PALETTES.len();
        self.color_index = (self.color_index + count - 1) % count;
    }

    fn set_colors(&mut self) {
        self.colors = TableColors::new(&PALETTES[self.color_index]);
    }

    fn run(
        mut self,
        mut terminal: DefaultTerminal,
        mut rx: mpsc::UnboundedReceiver<(String, f64, f64)>,
    ) -> Result<()> {
        loop {
            // Drain updates
            while let Ok((coin, funding, oi)) = rx.try_recv() {
                self.update_coin(&coin, funding, oi);
            }

            terminal.draw(|frame| self.draw(frame))?;

            if event::poll(std::time::Duration::from_millis(50))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        let shift = key.modifiers.contains(KeyModifiers::SHIFT);
                        match key.code {
                            KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                            KeyCode::Char('j') | KeyCode::Down => self.next_row(),
                            KeyCode::Char('k') | KeyCode::Up => self.previous_row(),
                            KeyCode::Char('l') | KeyCode::Right if shift => self.next_color(),
                            KeyCode::Char('h') | KeyCode::Left if shift => self.previous_color(),
                            KeyCode::Char('l') | KeyCode::Right => self.next_column(),
                            KeyCode::Char('h') | KeyCode::Left => self.previous_column(),
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    fn draw(&mut self, frame: &mut Frame) {
        let vertical = &Layout::vertical([Constraint::Min(5), Constraint::Length(4)]);
        let rects = vertical.split(frame.area());
        self.set_colors();
        self.render_table(frame, rects[0]);
        self.render_scrollbar(frame, rects[0]);
        self.render_footer(frame, rects[1]);
    }

    fn render_table(&mut self, frame: &mut Frame, area: Rect) {
        let header_style = Style::default()
            .fg(self.colors.header_fg)
            .bg(self.colors.header_bg);
        let selected_row_style = Style::default()
            .add_modifier(Modifier::REVERSED)
            .fg(self.colors.selected_row_style_fg);
        let selected_col_style = Style::default().fg(self.colors.selected_column_style_fg);
        let selected_cell_style = Style::default()
            .add_modifier(Modifier::REVERSED)
            .fg(self.colors.selected_cell_style_fg);

        let header = ["Coin", "Funding Rate", "Open Interest"]
            .into_iter()
            .map(Cell::from)
            .collect::<Row>()
            .style(header_style);

        let rows = self.items.iter().enumerate().map(|(i, c)| {
            let bg = if i % 2 == 0 {
                self.colors.normal_row_color
            } else {
                self.colors.alt_row_color
            };
            
            // Determine funding rate color
            let funding_color = if c.funding < 0.0 {
                Color::Red
            } else if c.funding > 0.000013 {
                Color::Green
            } else {
                self.colors.row_fg
            };

            if c.open_interest == 0.0 {
                return Row::new(vec![
                    Cell::from(c.coin.clone()),
                    Cell::from("N/A").style(Style::new().fg(self.colors.row_fg)),
                    Cell::from("N/A").style(Style::new().fg(self.colors.row_fg)),
                ])
                .style(Style::new().fg(self.colors.row_fg).bg(bg));
            }
            
            Row::new(vec![
                Cell::from(c.coin.clone()),
                Cell::from(format!("{:.6}", c.funding)).style(Style::new().fg(funding_color)),
                Cell::from(format!("{:.2}", c.open_interest)),
            ])
            .style(Style::new().fg(self.colors.row_fg).bg(bg))
        });

        // Use flexible constraints to fill the full width with equal column distribution
        let table = Table::new(rows, [
            Constraint::Fill(1),
            Constraint::Fill(1),
            Constraint::Fill(1),
        ])
        .header(header)
        .row_highlight_style(selected_row_style)
        .column_highlight_style(selected_col_style)
        .cell_highlight_style(selected_cell_style)
        .highlight_spacing(HighlightSpacing::Always)
        .bg(self.colors.buffer_bg);

        frame.render_stateful_widget(table, area, &mut self.state);
    }

    fn render_scrollbar(&mut self, frame: &mut Frame, area: Rect) {
        frame.render_stateful_widget(
            Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .begin_symbol(None)
                .end_symbol(None),
            area.inner(Margin {
                vertical: 1,
                horizontal: 1,
            }),
            &mut self.scroll_state,
        );
    }

    fn render_footer(&self, frame: &mut Frame, area: Rect) {
        let info_footer = Paragraph::new(Text::from_iter(INFO_TEXT))
            .style(Style::new().fg(self.colors.row_fg).bg(self.colors.buffer_bg))
            .centered()
            .block(
                Block::bordered()
                    .border_type(BorderType::Double)
                    .border_style(Style::new().fg(self.colors.footer_border_color)),
            );
        frame.render_widget(info_footer, area);
    }
}

#[tokio::main(flavor = "multi_thread", worker_threads = 5)]
async fn main() -> Result<()> {
    color_eyre::install()?;

    let coins = vec![
        "BTC",
        "ETH",
        "SOL",
        "AVAX",
        "DOGE",
        "MATIC",
        "ADA",
        "DOT",
        "XRP",
        "LTC",
        "BNB",
        "UNI",
        "LINK",
        "ATOM",
        "NEAR",
        "APT",
        "HYPE",
        "SUI",
        "ARB",
        "OP",
        "INJ",
        "TIA",
        "SEI",
        "WIF",
        "BONK",
        "PEPE",
        "SHIB",
        "JTO",
        "PYTH",
        "JUP",
        "RNDR",
        "FET",
        "TAO",
        "AAVE",
        "MKR",
        "COMP",
        "CRV",
        "LDO",
        "GMX",
        "BLUR",
        "APE",
        "ICP",
        "FIL",
        "STX",
        "IMX",
        "THETA",
        "MANA",
        "SAND",
        "AXS",
        "GALA",
        "BIO",
        "HPOS",
        "LOOM"
      ];
    let (tx, rx) = mpsc::unbounded_channel::<(String, f64, f64)>();

    let mut tasks = vec![];

    // Create WebSocket subscription tasks
    for coin in coins.clone() {
        let tx = tx.clone();
        let coin = coin.to_string();
        
        let task = tokio::spawn(async move {
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
                        if let hyperliquid_rust_sdk::AssetCtx::Perps(perps_ctx) = &active_ctx.data.ctx {
                            let funding = perps_ctx.funding.parse::<f64>().unwrap_or(0.0);
                            let oi = perps_ctx.open_interest.parse::<f64>().unwrap_or(0.0);
                            let _ = tx.send((coin.clone(), funding, oi));
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

            Ok::<(), color_eyre::Report>(())
        });

        tasks.push(task);
    }

    // Create UI task
    let ui_task = tokio::spawn(async move {
        let terminal = ratatui::init();
        let app = App::new(coins.into_iter().map(|s| s.to_string()).collect());
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