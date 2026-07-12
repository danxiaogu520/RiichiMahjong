use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use riichi_core::tile::{Suit, TileType};

use crate::app::App;
use crate::widgets::board::render_board;
use crate::widgets::status::render_status;

pub fn render(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(16),
            Constraint::Length(18),
        ])
        .split(f.area());

    render_status(f, app, chunks[0]);
    render_board(f, app, chunks[1]);
    render_bottom(f, app, chunks[2]);
}

fn render_bottom(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(5), Constraint::Min(8)])
        .split(area);
    render_analysis(f, app, chunks[0]);
    render_messages(f, app, chunks[1]);
}

fn render_analysis(f: &mut Frame, app: &App, area: Rect) {
    let text = if app.analysis_options.is_empty() {
        "暂无可分析的弃牌".to_string()
    } else {
        let options = app
            .analysis_options
            .iter()
            .take(8)
            .map(|analysis| {
                format!(
                    "{}: {}向听 进张{}张 改良{}张",
                    format_tile_type(analysis.tile.tile_type()),
                    analysis.shanten,
                    analysis.acceptance_copies,
                    analysis.improvement_copies
                )
            })
            .collect::<Vec<_>>();
        format!("最低向听弃牌：{}", options.join("  "))
    };
    let block = Block::default()
        .title("牌效分析")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    f.render_widget(
        Paragraph::new(Line::from(Span::raw(text)))
            .block(block)
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn render_messages(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title("消息")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let max_lines = area.height.saturating_sub(2) as usize;
    let skip = app.messages.len().saturating_sub(max_lines);
    let messages: Vec<Line> = app
        .messages
        .iter()
        .skip(skip)
        .map(|m| Line::from(Span::raw(m.clone())))
        .collect();

    let para = Paragraph::new(messages)
        .block(block)
        .wrap(Wrap { trim: false });
    f.render_widget(para, area);
}

pub fn format_tile_type(tt: TileType) -> String {
    let rank = tt.rank().0;
    match tt.suit() {
        Suit::Man => format!("{}m", rank),
        Suit::Pin => format!("{}p", rank),
        Suit::Sou => format!("{}s", rank),
        Suit::Wind => format!("{}z", rank),
        Suit::Dragon => format!("{}z", rank + 4),
    }
}

pub fn tile_color(tt: TileType) -> Color {
    match tt.suit() {
        Suit::Sou => Color::Red,
        Suit::Pin => Color::Green,
        Suit::Man => Color::Cyan,
        Suit::Wind | Suit::Dragon => Color::Yellow,
    }
}
