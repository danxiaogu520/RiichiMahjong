use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::App;

pub fn handle_input(app: &mut App, key: KeyEvent) {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        app.should_quit = true;
        return;
    }

    let tile_count = app.hand_tiles.len();

    match key.code {
        KeyCode::Char('q') => {
            app.should_quit = true;
        }
        KeyCode::Char('t') => {
            if app.can_tsumo {
                app.send_tsumo();
            } else {
                app.messages.push("无法自摸".to_string());
            }
        }
        KeyCode::Char('r') => {
            if app.can_riichi {
                app.riichi_selected = 0;
                app.riichi_selecting = true;
            } else {
                app.messages.push("无法立直".to_string());
            }
        }
        KeyCode::Char('a') => {
            if let Some(&tile) = app.ankan_options.first() {
                app.send_ankan(tile);
            }
        }
        KeyCode::Char('k') => {
            if let Some(&(index, tile)) = app.kakan_options.first() {
                app.send_kakan(index, tile);
            }
        }
        KeyCode::Char('9') => {
            if app.can_kyuushu {
                app.send_kyuushu();
            }
        }
        KeyCode::Left => {
            if app.riichi_selecting {
                app.riichi_selected = app.riichi_selected.saturating_sub(1);
            } else if app.call_options.is_empty() {
                if app.selected > 0 {
                    app.selected -= 1;
                }
            } else if app.call_selected > 0 {
                app.call_selected -= 1;
            }
        }
        KeyCode::Right => {
            if app.riichi_selecting {
                if app.riichi_selected + 1 < app.riichi_options.len() {
                    app.riichi_selected += 1;
                }
            } else if app.call_options.is_empty() {
                if app.selected < tile_count.saturating_sub(1) {
                    app.selected += 1;
                }
            } else if app.call_selected + 1 < app.call_options.len() {
                app.call_selected += 1;
            }
        }
        KeyCode::Enter => {
            if app.riichi_selecting {
                let tile = app.riichi_options.get(app.riichi_selected).copied();
                app.riichi_selecting = false;
                app.send_riichi_tile(tile);
            } else if app.selected < tile_count {
                let tile = app.hand_tiles[app.selected];
                app.send_discard(tile);
            }
        }
        KeyCode::Esc | KeyCode::Char('p') if app.riichi_selecting => {
            app.riichi_selecting = false;
        }
        KeyCode::Char(c) if c.is_ascii_digit() => {
            let n = c.to_digit(10).unwrap() as usize;
            if n >= 1 && n <= tile_count {
                let tile = app.hand_tiles[n - 1];
                app.send_discard(tile);
            }
        }
        _ => {}
    }
}

pub fn handle_call_input(app: &mut App, key: KeyEvent) {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        app.should_quit = true;
        return;
    }

    match key.code {
        KeyCode::Char('q') => {
            app.should_quit = true;
        }
        KeyCode::Char('p') | KeyCode::Esc => {
            app.send_call_pass();
        }
        KeyCode::Enter | KeyCode::Char('y') => {
            if let Some(option) = app.call_options.get(app.call_selected) {
                match &option.call_type {
                    riichi_core::game::CallType::Ron => app.send_call_ron(),
                    riichi_core::game::CallType::Pon { hand_tiles } => {
                        app.send_call_pon(*hand_tiles)
                    }
                    riichi_core::game::CallType::Chi { hand_tiles } => {
                        app.send_call_chi(*hand_tiles)
                    }
                    riichi_core::game::CallType::Minkan { hand_tiles } => {
                        app.send_call_minkan(*hand_tiles)
                    }
                }
            }
        }
        _ => {}
    }
}

pub fn handle_result_input(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => {
            app.should_quit = true;
        }
        KeyCode::Enter | KeyCode::Char(' ') => {
            app.show_result = false;
        }
        _ => {}
    }
}
