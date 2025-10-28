use ratatui::style::{Color, palette::tailwind};

pub struct TableColors {
    pub buffer_bg: Color,
    pub header_bg: Color,
    pub header_fg: Color,
    pub row_fg: Color,
    pub selected_row_style_fg: Color,
    pub selected_column_style_fg: Color,
    pub selected_cell_style_fg: Color,
    pub normal_row_color: Color,
    pub alt_row_color: Color,
    pub footer_border_color: Color,
}

impl TableColors {
    pub const fn new(color: &tailwind::Palette) -> Self {
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

    pub fn funding_rate_color(&self, funding: f64) -> Color {
        use crate::config::FUNDING_RATE_THRESHOLD;

        if funding < 0.0 {
            Color::Red
        } else if funding > FUNDING_RATE_THRESHOLD {
            Color::Green
        } else {
            self.row_fg
        }
    }
}
