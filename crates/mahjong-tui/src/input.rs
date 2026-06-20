use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use mahjong_engine::action::ResponseAction;

use crate::app::App;

pub fn handle_input(app: &mut App, key: KeyEvent) {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        app.should_quit = true;
        return;
    }

    let tiles = app.hand_tiles();
    let tile_count = tiles.len();

    match key.code {
        KeyCode::Char('q') => {
            app.should_quit = true;
        }
        KeyCode::Char('t') => {
            if app.game.check_tsumo(mahjong_core::player::PlayerId(0)).is_some() {
                app.execute_tsumo();
            } else {
                app.messages.push("无法自摸".to_string());
            }
        }
        KeyCode::Char('r') => {
            if app.game.can_declare_riichi(mahjong_core::player::PlayerId(0)) {
                app.execute_riichi();
            } else {
                app.messages.push("无法立直".to_string());
            }
        }
        KeyCode::Left => {
            if app.selected > 0 {
                app.selected -= 1;
            }
        }
        KeyCode::Right => {
            if app.selected < tile_count.saturating_sub(1) {
                app.selected += 1;
            }
        }
        KeyCode::Enter => {
            if app.selected < tile_count {
                let tile = tiles[app.selected];
                app.execute_discard(tile);
                if app.selected >= app.hand_tiles().len() && app.selected > 0 {
                    app.selected -= 1;
                }
            }
        }
        KeyCode::Char(c) if c.is_ascii_digit() => {
            let n = c.to_digit(10).unwrap() as usize;
            if n >= 1 && n <= tile_count {
                let tile = tiles[n - 1];
                app.execute_discard(tile);
                if app.selected >= app.hand_tiles().len() && app.selected > 0 {
                    app.selected -= 1;
                }
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

    let option_count = app.call_options.len();

    match key.code {
        KeyCode::Char('q') => {
            app.should_quit = true;
        }
        KeyCode::Left | KeyCode::Up => {
            if app.call_selected > 0 {
                app.call_selected -= 1;
            }
        }
        KeyCode::Right | KeyCode::Down => {
            if app.call_selected < option_count.saturating_sub(1) {
                app.call_selected += 1;
            }
        }
        KeyCode::Char('p') | KeyCode::Esc => {
            app.pass_call();
        }
        KeyCode::Enter => {
            if app.call_selected < option_count {
                let opt = &app.call_options[app.call_selected];
                let action = match &opt.call_type {
                    mahjong_engine::action::CallType::Ron => ResponseAction::Ron,
                    mahjong_engine::action::CallType::Pon { hand_tiles } => {
                        ResponseAction::Pon { hand_tiles: *hand_tiles }
                    }
                    mahjong_engine::action::CallType::Chi { hand_tiles } => {
                        ResponseAction::Chi { hand_tiles: *hand_tiles }
                    }
                    mahjong_engine::action::CallType::Minkan { hand_tiles } => {
                        ResponseAction::Minkan { hand_tiles: *hand_tiles }
                    }
                };
                app.execute_call(action);
            }
        }
        KeyCode::Char(c) if c.is_ascii_digit() => {
            let n = c.to_digit(10).unwrap() as usize;
            if n >= 1 && n <= option_count {
                let opt = &app.call_options[n - 1];
                let action = match &opt.call_type {
                    mahjong_engine::action::CallType::Ron => ResponseAction::Ron,
                    mahjong_engine::action::CallType::Pon { hand_tiles } => {
                        ResponseAction::Pon { hand_tiles: *hand_tiles }
                    }
                    mahjong_engine::action::CallType::Chi { hand_tiles } => {
                        ResponseAction::Chi { hand_tiles: *hand_tiles }
                    }
                    mahjong_engine::action::CallType::Minkan { hand_tiles } => {
                        ResponseAction::Minkan { hand_tiles: *hand_tiles }
                    }
                };
                app.execute_call(action);
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
