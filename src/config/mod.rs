use ratatui::style::palette::tailwind;

pub const PALETTES: [tailwind::Palette; 4] = [
    tailwind::BLUE,
    tailwind::EMERALD,
    tailwind::INDIGO,
    tailwind::RED,
];

pub const INFO_TEXT: [&str; 2] = [
    "(Esc) quit | (↑/↓) move row | (←/→) move col",
    "(Shift + →/←) cycle color",
];

pub const ITEM_HEIGHT: usize = 2;
pub const POLL_DURATION_MS: u64 = 50;
pub const FUNDING_RATE_THRESHOLD: f64 = 0.000013;
pub const ERROR_POPUP_DURATION_MS: u64 = 1500;
