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

use crate::app::App;
use riichi_core::player::PlayerId;
use riichi_server::ai_client::run_ai_client;
use riichi_server::channel::{create_player_pair, ActionMsg};
use riichi_server::game::GameLoop;

#[tokio::main]
async fn main() -> io::Result<()> {
    let (p0_handle, p0_client) = create_player_pair(PlayerId(0));
    let (p1_handle, p1_client) = create_player_pair(PlayerId(1));
    let (p2_handle, p2_client) = create_player_pair(PlayerId(2));
    let (p3_handle, p3_client) = create_player_pair(PlayerId(3));

    let event_txs = [
        p0_handle.event_tx,
        p1_handle.event_tx,
        p2_handle.event_tx,
        p3_handle.event_tx,
    ];

    let (merged_tx, merged_rx) = mpsc::channel::<ActionMsg>(64);

    let tx0 = merged_tx.clone();
    let tx1 = merged_tx.clone();
    let tx2 = merged_tx.clone();
    let tx3 = merged_tx.clone();

    let mut r0 = p0_handle.action_rx;
    let mut r1 = p1_handle.action_rx;
    let mut r2 = p2_handle.action_rx;
    let mut r3 = p3_handle.action_rx;

    tokio::spawn(async move {
        while let Some(msg) = r0.recv().await {
            let _ = tx0.send(msg).await;
        }
    });
    tokio::spawn(async move {
        while let Some(msg) = r1.recv().await {
            let _ = tx1.send(msg).await;
        }
    });
    tokio::spawn(async move {
        while let Some(msg) = r2.recv().await {
            let _ = tx2.send(msg).await;
        }
    });
    tokio::spawn(async move {
        while let Some(msg) = r3.recv().await {
            let _ = tx3.send(msg).await;
        }
    });

    tokio::spawn(run_ai_client(p1_client));
    tokio::spawn(run_ai_client(p2_client));
    tokio::spawn(run_ai_client(p3_client));

    let mut game_loop = GameLoop::new(event_txs, merged_tx, merged_rx);
    tokio::spawn(async move {
        game_loop.run().await;
    });

    enable_raw_mode().map_err(|e| {
        io::Error::new(
            e.kind(),
            format!(
                "无法初始化终端 ({}). 请在真实的终端中运行, 而不是 IDE 内置终端或管道.",
                e
            ),
        )
    })?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(p0_client);
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

async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> io::Result<()> {
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

        if app.auto_play {
            app.tick_ai();
            if event::poll(std::time::Duration::from_millis(50))? {
                if let event::Event::Key(key) = event::read()? {
                    if key.code == crossterm::event::KeyCode::Char('h')
                        || key.code == crossterm::event::KeyCode::Char('q')
                    {
                        input::handle_input(app, key);
                    }
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
