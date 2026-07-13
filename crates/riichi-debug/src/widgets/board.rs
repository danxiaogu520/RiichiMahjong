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
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(if area.height >= 25 {
            [
                Constraint::Length(7),
                Constraint::Min(9),
                Constraint::Length(9),
            ]
        } else {
            [
                Constraint::Length(4),
                Constraint::Min(5),
                Constraint::Length(7),
            ]
        })
        .split(area);
    render_opponent(f, app, 2, rows[0]);

    let middle = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(29),
            Constraint::Min(20),
            Constraint::Length(29),
        ])
        .split(rows[1]);
    render_opponent(f, app, 3, middle[0]);
    render_center(f, app, middle[1]);
    render_opponent(f, app, 1, middle[2]);
    render_human(f, app, rows[2]);
}

fn player_style(app: &App, index: usize) -> Style {
    if app.current_player.0 == index {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    }
}

fn player_block(app: &App, index: usize, title: String) -> Block<'static> {
    let active = app.current_player.0 == index;
    Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(if active {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        })
}

fn render_opponent(f: &mut Frame, app: &App, index: usize, area: Rect) {
    let pstyle = player_style(app, index);
    let mut discards = Vec::new();
    for (j, &tile) in app.discards[index].iter().enumerate() {
        if j > 0 && j % 6 == 0 {
            discards.push(Span::raw("\n  "));
        }
        discards.push(Span::styled(
            display_tile(tile) + " ",
            tile_style(tile, pstyle.fg(tile_color(tile.tile_type()))),
        ));
    }
    if let Some((pending_player, pending_tile)) = app.pending_discard {
        if pending_player.0 == index {
            let river_len = app.discards[index].len();
            if river_len > 0 && river_len.is_multiple_of(6) {
                discards.push(Span::raw("\n  "));
            }
            discards.push(Span::styled(
                format!("[{}]", display_tile(pending_tile)),
                tile_style(
                    pending_tile,
                    Style::default()
                        .fg(Color::Yellow)
                        .bg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD),
                ),
            ));
        }
    }
    let melds = format_melds(&app.melds[index]);
    let title = format!("{}  {}点", app.player_name(index), app.points[index]);
    let lines = vec![
        Line::from(vec![
            Span::styled(
                if app.current_player.0 == index {
                    "▶ "
                } else {
                    "  "
                },
                pstyle,
            ),
            Span::styled(format!("手牌:{}张", app.hand_count_for(index)), pstyle),
            Span::styled(format!("  {}", melds), pstyle),
        ]),
        Line::from({
            let mut line = vec![Span::styled("牌河: ", pstyle)];
            line.extend(discards);
            line
        }),
    ];
    f.render_widget(
        Paragraph::new(lines).block(player_block(app, index, title)),
        area,
    );
}

fn render_center(f: &mut Frame, app: &App, area: Rect) {
    let dora = app
        .dora
        .iter()
        .map(|t| format_tile_type(*t))
        .collect::<Vec<_>>()
        .join(" ");
    let status = if app.is_ai_thinking() {
        "托管中：AI 思考中"
    } else if app.auto_play {
        "托管已开启"
    } else if app.needs_human_response() {
        "等待你的响应"
    } else if app.is_human_turn() {
        "轮到你打牌"
    } else {
        "AI 思考中"
    };
    let lines = vec![
        Line::from(format!(
            "{}  本场:{}  供托:{}",
            app.round_display(),
            app.honba,
            app.riichi_sticks
        )),
        Line::from(format!("剩余牌数:{}", app.remaining_tiles)),
        Line::from(format!("宝牌指示牌: {}", dora)),
        Line::from(Span::styled(
            status,
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
    ];
    f.render_widget(
        Paragraph::new(lines).block(Block::default().title("牌局状态").borders(Borders::ALL)),
        area,
    );
}

fn render_human(f: &mut Frame, app: &App, area: Rect) {
    let mut indices: Vec<usize> = (0..app.hand_tiles.len()).collect();
    let drawn_index = app
        .drawn_tile
        .and_then(|tile| app.hand_tiles.iter().position(|t| *t == tile));
    indices.sort_by_key(|&i| {
        if Some(i) == drawn_index {
            (1, u8::MAX, u8::MAX)
        } else {
            let t = app.hand_tiles[i].tile_type();
            (0, t.0, app.hand_tiles[i].copy_index())
        }
    });
    let hand = indices
        .iter()
        .enumerate()
        .flat_map(|(pos, &i)| {
            let tile = app.hand_tiles[i];
            let tt = tile.tile_type();
            let selected = if app.riichi_selecting {
                app.riichi_options
                    .get(app.riichi_selected)
                    .is_some_and(|selected| *selected == tile)
            } else {
                i == app.selected
            };
            let legal = if app.riichi_selecting {
                app.riichi_options.contains(&tile)
            } else {
                app.tile_is_discardable(tile)
            };
            let mut spans = Vec::new();
            if pos > 0 && Some(i) == drawn_index {
                spans.push(Span::raw("  "));
            }
            let mut style = if legal {
                Style::default().fg(tile_color(tt))
            } else {
                Style::default().fg(Color::DarkGray)
            };
            style = tile_style(tile, style);
            if selected && legal {
                style = style
                    .bg(Color::Yellow)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD);
            }
            let label = if Some(i) == drawn_index {
                format!("[{}] ", display_tile(tile))
            } else {
                display_tile(tile) + " "
            };
            if Some(i) == drawn_index {
                style = if selected && legal {
                    Style::default()
                        .bg(Color::LightYellow)
                        .fg(Color::Black)
                        .add_modifier(Modifier::BOLD)
                        .add_modifier(Modifier::UNDERLINED)
                } else {
                    if legal {
                        Style::default().fg(tile_color(tt))
                    } else {
                        Style::default().fg(Color::DarkGray)
                    }
                };
            }
            spans.push(Span::styled(label, style));
            spans
        })
        .collect::<Vec<_>>();
    let action = render_action_line(app);
    let mut lines = vec![
        Line::from(vec![
            Span::styled("▶ 你", player_style(app, 0)),
            Span::raw(format!("  {}点  ", app.points[0])),
            Span::raw(format_melds(&app.melds[0])),
        ]),
        Line::from(hand),
        Line::from({
            let mut line = vec![Span::styled("牌河: ", Style::default().fg(Color::DarkGray))];
            for (index, tile) in app.discards[0].iter().enumerate() {
                if index > 0 && index % 6 == 0 {
                    line.push(Span::raw("\n  "));
                }
                line.push(Span::styled(
                    display_tile(*tile) + " ",
                    tile_style(*tile, Style::default().fg(tile_color(tile.tile_type()))),
                ));
            }
            line
        }),
    ];
    lines.push(action);
    f.render_widget(
        Paragraph::new(lines).block(player_block(app, 0, "你的手牌".into())),
        area,
    );
}

fn format_melds(melds: &[Meld]) -> String {
    melds
        .iter()
        .map(|meld| {
            let kind = match meld.kind {
                MeldKind::Chi => "吃",
                MeldKind::Pon => "碰",
                MeldKind::Ankan => "暗杠",
                MeldKind::Minkan => "明杠",
                MeldKind::Kakan => "加杠",
            };
            format!(
                "[{} {}]",
                kind,
                meld.tiles
                    .iter()
                    .map(|t| display_tile(*t))
                    .collect::<String>()
            )
        })
        .collect::<Vec<_>>()
        .join("")
}

fn display_tile(tile: riichi_core::tile::Tile) -> String {
    let notation = format_tile_type(tile.tile_type());
    if tile.is_aka_dora() {
        format!("({})", notation)
    } else {
        notation
    }
}

fn tile_style(tile: riichi_core::tile::Tile, style: Style) -> Style {
    if tile.is_aka_dora() {
        style.fg(Color::LightRed).add_modifier(Modifier::BOLD)
    } else {
        style
    }
}

fn render_action_line(app: &App) -> Line<'static> {
    if !app.call_options.is_empty() {
        let mut spans = vec![Span::styled(
            format!(
                "响应: {} ",
                app.pending_discard
                    .map(|(_, t)| format_tile_type(t.tile_type()))
                    .unwrap_or_default()
            ),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )];
        for (index, option) in app.call_options.iter().enumerate() {
            let label = match &option.call_type {
                CallType::Ron => "荣和".into(),
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
                    "大明杠({}{}{})",
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
    if app.auto_play {
        return Line::from(Span::styled(
            "托管中：按 H 取消托管",
            Style::default().fg(Color::Yellow),
        ));
    }
    if app.riichi_selecting {
        return Line::from(Span::styled(
            "选择立直弃牌 ←→切换 Enter确认 Esc取消",
            Style::default().fg(Color::Yellow),
        ));
    }
    let mut spans = vec![Span::raw("操作: ")];
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
        "←→选择 Enter打牌",
        Style::default().fg(Color::DarkGray),
    ));
    Line::from(spans)
}
