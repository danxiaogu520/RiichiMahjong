use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::app::App;
use crate::ui::format_tile_type;

pub fn render_status(f: &mut Frame, app: &App, area: Rect) {
    let dora_str: Vec<String> = app.game.dora.iter().map(|tt| format_tile_type(*tt)).collect();

    let round = app.round_display();
    let remaining = app.game.remaining_tiles();
    let honba = app.game.honba;
    let riichi_sticks = app.game.riichi_sticks;

    let status_text = format!(
        "  {}  本场:{}  宝牌:{}  残:{}  供托:{}",
        round,
        honba,
        dora_str.join(" "),
        remaining,
        riichi_sticks,
    );

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let para = Paragraph::new(Line::from(Span::styled(
        status_text,
        Style::default().fg(Color::White),
    )))
    .block(block);

    f.render_widget(para, area);
}
