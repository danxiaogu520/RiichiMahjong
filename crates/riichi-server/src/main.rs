use tokio::sync::mpsc;
use riichi_server::channel::{PlayerAction, ServerEvent};
use riichi_server::game_loop::GameLoop;

#[tokio::main]
async fn main() {
    let (event_tx, mut event_rx) = mpsc::channel::<ServerEvent>(64);
    let (action_tx, action_rx) = mpsc::channel::<PlayerAction>(64);

    let mut game_loop = GameLoop::new(event_tx, action_rx);

    tokio::spawn(async move {
        game_loop.run().await;
    });

    while let Some(event) = event_rx.recv().await {
        match event {
            ServerEvent::StateUpdate { hand_tiles, remaining_tiles, .. } => {
                println!("手牌: {:?}  剩余: {}", hand_tiles, remaining_tiles);
            }
            ServerEvent::ActionRequired { can_tsumo, can_riichi } => {
                println!("行动: tsumo={} riichi={}", can_tsumo, can_riichi);
                let tile = riichi_core::tile::Tile::from_raw(0);
                let _ = action_tx.send(PlayerAction::TurnAction(
                    riichi_server::channel::TurnActionMsg::Discard(tile),
                )).await;
            }
            ServerEvent::CallRequired { options } => {
                println!("副露选项: {:?}", options);
                let _ = action_tx.send(PlayerAction::CallResponse(
                    riichi_server::channel::CallResponseMsg::Pass,
                )).await;
            }
            ServerEvent::GameOver { scores } => {
                println!("游戏结束: {:?}", scores);
                break;
            }
        }
    }
}
