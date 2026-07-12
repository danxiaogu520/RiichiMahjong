use riichi_core::player::PlayerId;
use riichi_server::ai_client::run_ai_client;
use riichi_server::channel::{
    create_player_pair, ActionMsg, CallResponseMsg, PlayerAction, ServerEvent, TurnActionMsg,
};
use riichi_server::game::GameLoop;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() {
    let (p0_handle, mut p0_client) = create_player_pair(PlayerId(0));
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

    let mut hand_tiles: Vec<riichi_core::tile::Tile> = Vec::new();

    while let Some(event) = p0_client.event_rx.recv().await {
        match event {
            ServerEvent::StateUpdate {
                hand_tiles: h,
                remaining_tiles,
                ..
            } => {
                hand_tiles = h;
                println!("手牌: {:?}  剩余: {}", hand_tiles, remaining_tiles);
            }
            ServerEvent::ActionRequired { .. } => {
                let tile = hand_tiles
                    .last()
                    .copied()
                    .unwrap_or(riichi_core::tile::Tile::from_raw(0));
                println!("打出: {:?}", tile);
                let _ = p0_client
                    .action_tx
                    .send((
                        PlayerId(0),
                        PlayerAction::TurnAction(TurnActionMsg::Discard(tile)),
                    ))
                    .await;
            }
            ServerEvent::CallRequired { options } => {
                println!("副露选项: {:?}", options);
                let _ = p0_client
                    .action_tx
                    .send((
                        PlayerId(0),
                        PlayerAction::CallResponse(CallResponseMsg::Pass),
                    ))
                    .await;
            }
            ServerEvent::RoundResult {
                reason,
                win_details,
                point_changes,
                scores,
            } => {
                println!("本局结束：{}", reason);
                for detail in win_details {
                    println!("和牌明细：{}", detail);
                }
                println!("点棒变化：{:?}，结算点数：{:?}", point_changes, scores);
            }
            ServerEvent::GameOver { scores, ranking } => {
                println!("游戏结束: {:?}，排名: {:?}", scores, ranking);
                break;
            }
            ServerEvent::Error(message) => {
                eprintln!("动作错误: {}", message);
            }
        }
    }
}
