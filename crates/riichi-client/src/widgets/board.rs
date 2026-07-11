use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use riichi_core::game::CallType;
use riichi_core::meld::{Meld, MeldKind};

use crate::app::App;
use crate::ui::{format_tile_type, tile_color};

pub fn render_board(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Length(5),
            Constraint::Length(5),
            Constraint::Length(8),
        ])
        .split(area);

    for i in 0..3 {
        render_opponent(f, app, 3 - i, chunks[i]);
    }
    render_human(f, app, chunks[3]);
}

fn render_opponent(f: &mut Frame, app: &App, player_idx: usize, area: Rect) {
    let name = app.player_name(player_idx);
    let points = app.points[player_idx];
    let melds_display = format_melds(&app.melds[player_idx]);

    let mut visible_discards = app.discards[player_idx].clone();
    if let Some((pending_player, pending_tile)) = app.pending_discard {
        if pending_player.0 == player_idx {
            visible_discards.push(pending_tile);
        }
    }
    let discards: Vec<Span> = visible_discards
        .iter()
        .enumerate()
        .flat_map(|(j, &t)| {
            let tt = t.tile_type();
            let style = Style::default().fg(tile_color(tt));
            let mut spans = vec![Span::styled(format!("{} ", format_tile_type(tt)), style)];
            if j > 0 && j % 8 == 0 {
                spans.insert(0, Span::raw("\n    "));
            }
            spans
        })
        .collect();

    let hand_count = if player_idx == 0 {
        app.hand_tiles.len()
    } else {
        app.hand_count
    };

    let line1 = Line::from(vec![
        Span::styled(
            format!("{}({})", name, player_idx),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" [{}] {}", points, melds_display),
            Style::default().fg(Color::White),
        ),
        Span::raw(format!("  手牌:{}张", hand_count)),
    ]);

    let line2 = Line::from({
        let mut spans = vec![Span::styled(
            "    牌河: ",
            Style::default().fg(Color::DarkGray),
        )];
        spans.extend(discards);
        spans
    });

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    let para = Paragraph::new(vec![line1, line2]).block(block);
    f.render_widget(para, area);
}

fn render_human(f: &mut Frame, app: &App, area: Rect) {
    let name = app.player_name(0);
    let points = app.points[0];
    let tiles = &app.hand_tiles;

    let mut hand_spans: Vec<Span> = Vec::new();
    for (i, &t) in tiles.iter().enumerate() {
        let tt = t.tile_type();

        let is_selected = i == app.selected;
        let style = if is_selected {
            Style::default()
                .fg(Color::Black)
                .bg(tile_color(tt))
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(tile_color(tt))
        };

        hand_spans.push(Span::styled(format!("{} ", format_tile_type(tt)), style));
    }

    let melds_display = format_melds(&app.melds[0]);

    let line1 = Line::from(vec![
        Span::styled(
            format!("{}({})", name, 0),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" [{}] {}", points, melds_display),
            Style::default().fg(Color::White),
        ),
    ]);

    let line2 = Line::from(hand_spans);

    let discards: Vec<Span> = app.discards[0]
        .iter()
        .flat_map(|&t| {
            let tt = t.tile_type();
            let style = Style::default().fg(tile_color(tt));
            vec![Span::styled(format!("{} ", format_tile_type(tt)), style)]
        })
        .collect();

    let line3 = Line::from({
        let mut spans = vec![Span::styled("牌河: ", Style::default().fg(Color::DarkGray))];
        spans.extend(discards);
        spans
    });

    let action_line = render_action_line(app);

    let block = Block::default()
        .title("你的手牌")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::White));

    let para = Paragraph::new(vec![line1, line2, line3, action_line]).block(block);
    f.render_widget(para, area);
}

fn format_melds(melds: &[Meld]) -> String {
    if melds.is_empty() {
        return String::new();
    }
    let mut result = String::from("副露:");
    for meld in melds {
        let kind = match meld.kind {
            MeldKind::Chi => "吃",
            MeldKind::Pon => "碰",
            MeldKind::Ankan => "暗杠",
            MeldKind::Minkan => "明杠",
            MeldKind::Kakan => "加杠",
        };
        result.push('[');
        result.push_str(kind);
        result.push(' ');
        for tile in &meld.tiles {
            result.push_str(&format_tile_type(tile.tile_type()));
        }
        result.push(']');
    }
    result
}

fn render_action_line(app: &App) -> Line<'static> {
    if !app.call_options.is_empty() {
        let mut spans = vec![Span::styled(
            match app.pending_discard {
                Some((player, tile)) => format!(
                    "响应: {}打出{} ",
                    player,
                    format_tile_type(tile.tile_type())
                ),
                None => "响应: ".to_string(),
            },
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )];
        for (index, option) in app.call_options.iter().enumerate() {
            let label = match &option.call_type {
                CallType::Ron => "荣和".to_string(),
                CallType::Pon { hand_tiles } => format!(
                    "碰({}{})",
                    format_tile_type(hand_tiles[0].tile_type()),
                    format_tile_type(hand_tiles[1].tile_type())
                ),
                CallType::Chi { hand_tiles } => format!(
                    "吃({}{})",
                    format_tile_type(hand_tiles[0].tile_type()),
                    format_tile_type(hand_tiles[1].tile_type())
                ),
                CallType::Minkan { hand_tiles } => format!(
                    "明杠({}{}{})",
                    format_tile_type(hand_tiles[0].tile_type()),
                    format_tile_type(hand_tiles[1].tile_type()),
                    format_tile_type(hand_tiles[2].tile_type())
                ),
            };
            let style = if index == app.call_selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Yellow)
            };
            spans.push(Span::styled(format!("[{}] ", label), style));
        }
        spans.push(Span::styled(
            "←→选择 Enter确认 P跳过",
            Style::default().fg(Color::DarkGray),
        ));
        return Line::from(spans);
    }

    if !matches!(app.phase, riichi_engine::game::GamePhase::ActionPhase) {
        return Line::from(Span::raw(""));
    }

    let mut spans = vec![Span::styled("操作: ", Style::default().fg(Color::White))];

    if app.can_tsumo {
        spans.push(Span::styled("[t]自摸 ", Style::default().fg(Color::Yellow)));
    }
    if app.can_riichi {
        spans.push(Span::styled("[r]立直 ", Style::default().fg(Color::Yellow)));
    }
    if !app.ankan_options.is_empty() {
        spans.push(Span::styled("[a]暗杠 ", Style::default().fg(Color::Yellow)));
    }
    if !app.kakan_options.is_empty() {
        spans.push(Span::styled("[k]加杠 ", Style::default().fg(Color::Yellow)));
    }
    if app.can_kyuushu {
        spans.push(Span::styled(
            "[9]九种九牌 ",
            Style::default().fg(Color::Yellow),
        ));
    }
    spans.push(Span::styled(
        "[←→]选择 [Enter]打牌",
        Style::default().fg(Color::DarkGray),
    ));

    Line::from(spans)
}
