mod app;
mod input;
mod ui;
mod widgets;

use std::io;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use tokio::sync::mpsc;

use riichi_server::channel::{PlayerAction, ServerEvent};
use riichi_server::game_loop::GameLoop;
use crate::app::App;

#[tokio::main]
async fn main() -> io::Result<()> {
    let (event_tx, event_rx) = mpsc::channel::<ServerEvent>(64);
    let (action_tx, action_rx) = mpsc::channel::<PlayerAction>(64);

    let mut game_loop = GameLoop::new(event_tx, action_rx);
    tokio::spawn(async move {
        game_loop.run().await;
    });

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(event_rx, action_tx);
    let res = run_app(&mut terminal, &mut app).await;

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("Error: {}", err);
    }

    if app.game_over {
        println!("游戏结束！");
        println!("最终点数:");
        for i in 0..4 {
            let name = app.player_name(i);
            println!("  {}: {} 点", name, app.scores[i]);
        }
    }

    Ok(())
}

async fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) -> io::Result<()> {
    loop {
        app.process_server_events().await;

        terminal.draw(|f| ui::render(f, app))?;

        if app.should_quit {
            return Ok(());
        }

        if app.show_result {
            if event::poll(std::time::Duration::from_millis(100))? {
                if let event::Event::Key(key) = event::read()? {
                    input::handle_result_input(app, key);
                }
            }
            continue;
        }

        if app.is_human_turn() || app.needs_human_response() {
            if event::poll(std::time::Duration::from_millis(100))? {
                if let event::Event::Key(key) = event::read()? {
                    if app.needs_human_response() {
                        input::handle_call_input(app, key);
                    } else {
                        input::handle_input(app, key);
                    }
                }
            }
        } else {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    }
}
