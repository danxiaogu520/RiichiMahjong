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
    let bottom_height = if f.area().height >= 40 {
        12
    } else if f.area().height >= 32 {
        8
    } else {
        0
    };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(21),
            Constraint::Length(bottom_height),
        ])
        .split(f.area());

    render_status(f, app, chunks[0]);
    render_board(f, app, chunks[1]);
    render_bottom(f, app, chunks[2]);
}

fn render_bottom(f: &mut Frame, app: &App, area: Rect) {
    if area.height < 3 {
        return;
    }
    if area.height < 8 {
        render_analysis(f, app, area);
        return;
    }
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(7), Constraint::Min(6)])
        .split(area);
    if app.show_analysis {
        render_analysis(f, app, chunks[0]);
    }
    if app.show_messages {
        render_messages(f, app, chunks[1]);
    }
}

fn render_analysis(f: &mut Frame, app: &App, area: Rect) {
    if let Some(info) = &app.tenpai_info {
        render_tenpai(f, app, info, area);
        return;
    }
    let lines =
        if app.analysis_options.is_empty() {
            vec![Line::from(Span::raw("暂无可分析的弃牌"))]
        } else {
            let mut lines = vec![Line::from("序号  弃牌      向听   进张总数   改良总数")];
            lines.extend(app.analysis_options.iter().take(5).enumerate().map(
                |(index, analysis)| {
                    Line::from(format!(
                        "{:>2}.   {:<6}   {:>2}      {:>3}张      {:>3}张",
                        index + 1,
                        format_tile_type(analysis.tile.tile_type()),
                        analysis.shanten,
                        analysis.acceptance_copies,
                        analysis.improvement_copies
                    ))
                },
            ));
            lines
        };
    let block = Block::default()
        .title("牌效分析")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    f.render_widget(
        Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn render_tenpai(f: &mut Frame, _app: &App, info: &riichi_engine::TenpaiInfo, area: Rect) {
    let mut lines = vec![Line::from("等待牌   山中剩余   状态")];
    for wait in &info.waits {
        let mut status = Vec::new();
        if info.is_furiten {
            status.push("振听");
        }
        if wait.is_no_yaku {
            status.push("无役");
        }
        lines.push(Line::from(format!(
            "{:<7} {:>3}张      {}",
            format_tile_type(wait.tile_type),
            wait.remaining,
            if status.is_empty() {
                "正常".to_string()
            } else {
                status.join("/")
            }
        )));
    }
    let block = Block::default()
        .title("听牌信息")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));
    f.render_widget(Paragraph::new(lines).block(block), area);
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
