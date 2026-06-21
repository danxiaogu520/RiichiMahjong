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
                app.send_riichi();
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
                let tile = app.hand_tiles[app.selected];
                app.send_discard(tile);
            }
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
            app.send_call_ron();
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
