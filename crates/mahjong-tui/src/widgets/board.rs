use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use mahjong_engine::action::CallType;
use mahjong_engine::game::GamePhase;

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
    let p = &app.game.players[player_idx];
    let name = app.player_name(player_idx);
    let wind = format_tile_type(p.wind);

    let riichi_tag = if p.is_riichi { " [立直]" } else { "" };

    let melds_str: Vec<String> = p.melds.iter().map(|m| m.to_string()).collect();
    let melds_display = if melds_str.is_empty() {
        String::new()
    } else {
        format!("  副露:{}", melds_str.join(" "))
    };

    let discards: Vec<Span> = p
        .discards
        .iter()
        .enumerate()
        .flat_map(|(j, &t)| {
            let tt = t.tile_type();
            let is_riichi_tile = p.riichi_declaration_tile == Some(t);
            let style = if is_riichi_tile {
                Style::default().fg(tile_color(tt)).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(tile_color(tt))
            };
            let mut spans = vec![Span::styled(format!("{} ", format_tile_type(tt)), style)];
            if j > 0 && j % 8 == 0 {
                spans.insert(0, Span::raw("\n    "));
            }
            spans
        })
        .collect();

    let line1 = Line::from(vec![
        Span::styled(
            format!("{}({})", wind, name),
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" [{}]{}", p.points, riichi_tag),
            Style::default().fg(Color::White),
        ),
        Span::raw(format!("  手牌:{}张{}", p.hand.len(), melds_display)),
    ]);

    let line2 = Line::from({
        let mut spans = vec![Span::styled("    牌河: ", Style::default().fg(Color::DarkGray))];
        spans.extend(discards);
        spans
    });

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(if p.is_riichi { Color::Red } else { Color::DarkGray }));

    let para = Paragraph::new(vec![line1, line2]).block(block);
    f.render_widget(para, area);
}

fn render_human(f: &mut Frame, app: &App, area: Rect) {
    let p = &app.game.players[0];
    let name = app.player_name(0);
    let wind = format_tile_type(p.wind);

    let tiles = app.hand_tiles();

    let mut hand_spans: Vec<Span> = Vec::new();
    for (i, &t) in tiles.iter().enumerate() {
        let tt = t.tile_type();
        let is_drawn = app.game.drawn_tile == Some(t) && i == tiles.len() - 1;

        if is_drawn {
            hand_spans.push(Span::raw(" | "));
        }

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

    let riichi_tag = if p.is_riichi { " [立直]" } else { "" };
    let menzen = if p.is_menzen() { "门清" } else { "" };

    let melds_str: Vec<String> = p.melds.iter().map(|m| m.to_string()).collect();
    let melds_display = if melds_str.is_empty() {
        String::new()
    } else {
        format!("  副露:{}", melds_str.join(" "))
    };

    let line1 = Line::from(vec![
        Span::styled(
            format!("{}({})", wind, name),
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" [{}]{}{}{}", p.points, riichi_tag, menzen, melds_display),
            Style::default().fg(Color::White),
        ),
    ]);

    let line2 = Line::from(hand_spans);

    let discards: Vec<Span> = p
        .discards
        .iter()
        .flat_map(|&t| {
            let tt = t.tile_type();
            let is_riichi_tile = p.riichi_declaration_tile == Some(t);
            let style = if is_riichi_tile {
                Style::default().fg(tile_color(tt)).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(tile_color(tt))
            };
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

fn render_action_line(app: &App) -> Line<'static> {
    if !app.call_options.is_empty() {
        let mut spans = vec![Span::styled(
            "副露选择: ",
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )];

        for (i, opt) in app.call_options.iter().enumerate() {
            let label = match &opt.call_type {
                CallType::Ron => "荣和".to_string(),
                CallType::Pon { .. } => "碰".to_string(),
                CallType::Chi { .. } => "吃".to_string(),
                CallType::Minkan { .. } => "大明杠".to_string(),
            };
            let style = if i == app.call_selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Yellow)
            };
            spans.push(Span::styled(format!("[{}]{} ", i + 1, label), style));
        }
        spans.push(Span::styled("[P]跳过", Style::default().fg(Color::DarkGray)));
        return Line::from(spans);
    }

    if !matches!(app.game.phase, GamePhase::ActionPhase) {
        return Line::from(Span::raw(""));
    }

    let can_tsumo = app.game.check_tsumo(mahjong_core::player::PlayerId(0)).is_some();
    let can_riichi = app.game.can_declare_riichi(mahjong_core::player::PlayerId(0));

    let mut spans = vec![Span::styled("操作: ", Style::default().fg(Color::White))];

    if can_tsumo {
        spans.push(Span::styled("[t]自摸 ", Style::default().fg(Color::Yellow)));
    }
    if can_riichi {
        spans.push(Span::styled("[r]立直 ", Style::default().fg(Color::Yellow)));
    }
    spans.push(Span::styled(
        "[←→]选择 [Enter]打牌",
        Style::default().fg(Color::DarkGray),
    ));

    Line::from(spans)
}
