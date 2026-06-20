use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use mahjong_core::tile::{Suit, TileType};

use crate::app::App;
use crate::widgets::board::render_board;
use crate::widgets::status::render_status;

pub fn render(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(20),
            Constraint::Length(8),
        ])
        .split(f.area());

    render_status(f, app, chunks[0]);
    render_board(f, app, chunks[1]);
    render_bottom(f, app, chunks[2]);
}

fn render_bottom(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(40), Constraint::Length(50)])
        .split(area);

    render_messages(f, app, chunks[0]);
    render_analysis(f, app, chunks[1]);
}

fn render_messages(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title("消息")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let messages: Vec<Line> = app
        .messages
        .iter()
        .rev()
        .take(area.height.saturating_sub(2) as usize)
        .map(|m| Line::from(Span::raw(m.clone())))
        .collect();

    let para = Paragraph::new(messages).block(block).wrap(Wrap { trim: false });
    f.render_widget(para, area);
}

fn render_analysis(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title("打牌分析")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    if app.analysis.is_empty() {
        let para = Paragraph::new("无分析数据").block(block);
        f.render_widget(para, area);
        return;
    }

    let header = Line::from(vec![
        Span::styled("打牌", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw("  进张      改良    向听"),
    ]);

    let mut lines = vec![header];
    for (i, opt) in app.analysis.iter().enumerate() {
        let style = if i == 0 {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        let tile_str = format_tile_type(opt.tile.tile_type());
        let line = Line::from(vec![
            Span::styled(
                format!("{:>4}", tile_str),
                style,
            ),
            Span::styled(
                format!("  {:>2}种{:>2}张   {:>2}种{:>2}张  {:>2}",
                    opt.acceptance_types, opt.acceptance_copies,
                    opt.improvement_types, opt.improvement_copies,
                    opt.shanten),
                style,
            ),
        ]);
        lines.push(line);
    }

    let para = Paragraph::new(lines).block(block);
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
