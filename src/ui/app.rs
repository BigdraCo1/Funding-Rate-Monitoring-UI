use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Alignment, Constraint, Flex, Layout, Margin, Rect},
    style::{Modifier, Style, Stylize},
    text::Text,
    widgets::{
        Block, BorderType, Cell, Clear, HighlightSpacing, Paragraph, Row, Scrollbar,
        ScrollbarOrientation, ScrollbarState, Table, TableState,
    },
};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::Instant;

use crate::config::{ERROR_POPUP_DURATION_MS, INFO_TEXT, ITEM_HEIGHT, PALETTES, POLL_DURATION_MS};
use crate::data::CoinData;
use crate::ui::TableColors;

enum FundingRateRound {
    Hourly,
    QuadriHourly,
    OctaHourly,
    Daily,
    Monthly,
    Annually,
}

pub struct TuiApp {
    state: TableState,
    items: Vec<CoinData>,
    scroll_state: ScrollbarState,
    colors: TableColors,
    round: FundingRateRound,
    color_index: usize,
    symbol: bool,
    popup: bool,
    popup_message: String,
    exchange: u8,
    error_popup_timer: Option<tokio::time::Instant>,
}

impl TuiApp {
    pub fn new(coins: Vec<String>) -> Self {
        let items = coins.into_iter().map(CoinData::new).collect::<Vec<_>>();

        Self {
            state: TableState::default().with_selected(0),
            scroll_state: ScrollbarState::new((items.len().saturating_sub(1)) * ITEM_HEIGHT),
            colors: TableColors::new(&PALETTES[0]),
            round: FundingRateRound::Hourly,
            color_index: 0,
            items,
            symbol: false,
            popup: false,
            popup_message: String::new(),
            exchange: 1,
            error_popup_timer: None,
        }
    }

    fn update_coin(&mut self, coin: &str, funding: f64, open_interest: f64, oracle_price: f64) {
        if let Some(c) = self.items.iter_mut().find(|c| c.coin == coin) {
            c.update(funding, open_interest, oracle_price);
            self.update_scrollbar_size();
        }
    }

    pub fn get_exchange(&self) -> u8 {
        self.exchange
    }

    fn update_exchange(&mut self, exchange: u8) {
        self.exchange = exchange;
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

    fn select_row(&mut self, ch: String) -> Result<()> {
        let row = self
            .items
            .iter()
            .enumerate()
            .filter(|c| c.1.has_data())
            .position(|c| c.1.coin.starts_with(&ch))
            .ok_or_else(|| color_eyre::eyre::eyre!("No coin found starting with '{}'", ch))?;

        self.state.select(Some(row));
        self.scroll_state = self.scroll_state.position(row * ITEM_HEIGHT);
        Ok(())
    }

    fn previous_row(&mut self) {
        let i = match self.state.selected() {
            Some(0) => 0,
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

    fn sort_collumn(&mut self) {
        if let Some(selected_col) = self.state.selected_column() {
            match selected_col {
                0 => self.items.sort_by(|a, b| a.coin.cmp(&b.coin)),
                1 => self.items.sort_by(|a, b| {
                    b.funding
                        .partial_cmp(&a.funding)
                        .unwrap_or(std::cmp::Ordering::Equal)
                }),
                2 => {
                    if !self.symbol {
                        self.items.sort_by(|a, b| {
                            b.open_interest
                                .partial_cmp(&a.open_interest)
                                .unwrap_or(std::cmp::Ordering::Equal)
                        })
                    } else {
                        self.items.sort_by(|a, b| {
                            (b.open_interest * b.oracle_price)
                                .partial_cmp(&(a.open_interest * a.oracle_price))
                                .unwrap_or(std::cmp::Ordering::Equal)
                        })
                    }
                }
                _ => {}
            }
        }
    }

    fn next_round(&mut self) {
        self.round = match self.round {
            FundingRateRound::Hourly => FundingRateRound::QuadriHourly,
            FundingRateRound::QuadriHourly => FundingRateRound::OctaHourly,
            FundingRateRound::OctaHourly => FundingRateRound::Daily,
            FundingRateRound::Daily => FundingRateRound::Monthly,
            FundingRateRound::Monthly => FundingRateRound::Annually,
            FundingRateRound::Annually => FundingRateRound::Hourly,
        };
    }

    fn update_scrollbar_size(&mut self) {
        let items_with_data = self.items.iter().filter(|c| c.has_data()).count();
        self.scroll_state = self
            .scroll_state
            .content_length((items_with_data.saturating_sub(1)) * ITEM_HEIGHT);
    }

    fn toggle_symbol(&mut self) {
        self.symbol = !self.symbol;
    }

    fn toggle_popup(&mut self) {
        self.popup = !self.popup;
    }

    pub fn run(
        mut self,
        mut terminal: DefaultTerminal,
        mut rx: mpsc::UnboundedReceiver<(String, f64, f64, f64)>,
    ) -> Result<()> {
        loop {
            // Drain updates
            while let Ok((coin, funding, oi, price)) = rx.try_recv() {
                self.update_coin(&coin, funding, oi, price);
            }

            terminal.draw(|frame| self.draw(frame))?;

            if event::poll(Duration::from_millis(POLL_DURATION_MS))? {
                // Drain ALL events, not just one
                while event::poll(Duration::from_millis(0))? {
                    match event::read()? {
                        Event::Key(key) if key.kind == KeyEventKind::Press => {
                            let shift = key.modifiers.contains(KeyModifiers::SHIFT);
                            if !self.popup {
                                match key.code {
                                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                                    KeyCode::Char('j') | KeyCode::Down => self.next_row(),
                                    KeyCode::Char('k') | KeyCode::Up => self.previous_row(),
                                    KeyCode::Char('l') | KeyCode::Right if shift => {
                                        self.next_color()
                                    }
                                    KeyCode::Char('h') | KeyCode::Left if shift => {
                                        self.previous_color()
                                    }
                                    KeyCode::Char('l') | KeyCode::Right => self.next_column(),
                                    KeyCode::Char('h') | KeyCode::Left => self.previous_column(),
                                    KeyCode::Char('r') => self.next_round(),
                                    KeyCode::Char('t') => self.toggle_symbol(),
                                    KeyCode::Char('s') => {
                                        self.update_exchange(0u8);
                                    }
                                    KeyCode::Enter => self.sort_collumn(),
                                    KeyCode::Char('/') => {
                                        // clear popup message
                                        self.popup_message.clear();
                                        self.toggle_popup()
                                    }
                                    _ => {}
                                }
                            } else {
                                match key.code {
                                    KeyCode::Char('/') => self.toggle_popup(),
                                    KeyCode::Backspace => {
                                        let _ = self.popup_message.pop();
                                    }
                                    KeyCode::Char(c) => self.popup_message.push(c),
                                    KeyCode::Enter => {
                                        self.state = TableState::default().with_selected(0);
                                        self.toggle_popup();
                                        let result = self.select_row(self.popup_message.clone());
                                        if result.is_err() {
                                            self.error_popup_timer = Some(Instant::now());
                                        }
                                        self.popup_message.clear();
                                    }
                                    _ => {}
                                }
                            }
                        }
                        // Explicitly ignore mouse events and other event types
                        Event::Mouse(_)
                        | Event::Resize(_, _)
                        | Event::FocusGained
                        | Event::FocusLost
                        | Event::Paste(_) => {}
                        _ => {}
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
        if self.popup {
            self.render_popup(frame);
        }
        if let Some(error_popup_timer) = self.error_popup_timer {
            if error_popup_timer.elapsed().as_millis() > ERROR_POPUP_DURATION_MS.into() {
                self.error_popup_timer = None;
            } else {
                self.render_popup_not_found(frame);
            }
        }
    }

    fn render_popup(&mut self, frame: &mut Frame) {
        let area = frame.area();
        let block = Block::bordered().title("Popup");
        let area = self.popup_area(area, 60, 20);
        frame.render_widget(Clear, area);
        let paragraph = Paragraph::new(self.popup_message.as_str())
            .block(Block::bordered().title("Search"))
            .style(Style::default())
            .alignment(Alignment::Center);
        frame.render_widget(paragraph, area);
        frame.render_widget(block, area);
    }

    fn render_popup_not_found(&mut self, frame: &mut Frame) {
        let area = frame.area();
        let block = Block::bordered().title("Popup");
        let area = self.popup_area(area, 40, 20);
        frame.render_widget(Clear, area);
        let paragraph = Paragraph::new("Not found")
            .block(Block::bordered().title("Search"))
            .style(Style::default())
            .alignment(Alignment::Center);
        frame.render_widget(paragraph, area);
        frame.render_widget(block, area);
    }

    fn popup_area(&self, area: Rect, percent_x: u16, percent_y: u16) -> Rect {
        let vertical = Layout::vertical([Constraint::Percentage(percent_y)]).flex(Flex::Center);
        let horizontal = Layout::horizontal([Constraint::Percentage(percent_x)]).flex(Flex::Center);
        let [area] = vertical.areas(area);
        let [area] = horizontal.areas(area);
        area
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

        let header_funding_rate_display = match self.round {
            FundingRateRound::Hourly => "Funding Rate (Hourly)",
            FundingRateRound::QuadriHourly => "Funding Rate (4-Hourly)",
            FundingRateRound::OctaHourly => "Funding Rate (8-Hourly)",
            FundingRateRound::Daily => "Funding Rate (Daily)",
            FundingRateRound::Monthly => "Funding Rate (Monthly)",
            FundingRateRound::Annually => "Funding Rate (Annually)",
        };

        let header: Row<'_> = ["Coin", header_funding_rate_display, "Open Interest"]
            .into_iter()
            .map(Cell::from)
            .collect::<Row>()
            .style(header_style);

        let rows = self
            .items
            .iter()
            .enumerate()
            .filter(|(_, c)| c.has_data())
            .map(|(i, c)| {
                let bg = if i % 2 == 0 {
                    self.colors.normal_row_color
                } else {
                    self.colors.alt_row_color
                };

                let funding_color = self.colors.funding_rate_color(c.funding);

                let mut funding_display = c.funding;
                let mut open_interest_display: String;

                match self.round {
                    FundingRateRound::Hourly => {}
                    FundingRateRound::QuadriHourly => {
                        funding_display = c.funding * 4.0;
                    }
                    FundingRateRound::OctaHourly => {
                        funding_display = c.funding * 8.0;
                    }
                    FundingRateRound::Daily => {
                        funding_display = c.funding * 24.0;
                    }
                    FundingRateRound::Monthly => {
                        funding_display = c.funding * 24.0 * 30.0;
                    }
                    FundingRateRound::Annually => {
                        funding_display = c.funding * 24.0 * 365.0;
                    }
                }

                if self.symbol {
                    let oi_usd = c.open_interest * c.oracle_price;
                    if oi_usd >= 1_000_000_000.0 {
                        open_interest_display = format!("${:.2}B", oi_usd / 1_000_000_000.0);
                    } else if oi_usd >= 1_000_000.0 {
                        open_interest_display = format!("${:.2}M", oi_usd / 1_000_000.0);
                    } else if oi_usd >= 1_000.0 {
                        open_interest_display = format!("${:.2}K", oi_usd / 1_000.0);
                    } else {
                        open_interest_display = format!("${:.2}", oi_usd);
                    }
                } else {
                    open_interest_display = format!("{} {}", c.open_interest, c.coin);
                }

                Row::new(vec![
                    Cell::from(c.coin.clone()),
                    Cell::from(format!("{:.6}%", funding_display * 100.0))
                        .style(Style::new().fg(funding_color)),
                    Cell::from(open_interest_display),
                ])
                .style(Style::new().fg(self.colors.row_fg).bg(bg))
            });

        let table = Table::new(
            rows,
            [
                Constraint::Fill(1),
                Constraint::Fill(1),
                Constraint::Fill(1),
            ],
        )
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
        let info_footer = Paragraph::new(format!("{:?}{:?}", INFO_TEXT, self.exchange))
            .style(
                Style::new()
                    .fg(self.colors.row_fg)
                    .bg(self.colors.buffer_bg),
            )
            .centered()
            .block(
                Block::bordered()
                    .border_type(BorderType::Double)
                    .border_style(Style::new().fg(self.colors.footer_border_color)),
            );
        frame.render_widget(info_footer, area);
    }
}
