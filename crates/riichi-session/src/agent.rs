//! Pluggable player-agent boundary.
//!
//! The current debug AI still uses the channel adapter directly. This trait
//! reserves a stable seam for future heuristic, search-based, or external
//! Mortal-like agents without allowing an agent to mutate `GameState`.

use crate::channel::{PlayerAction, PlayerCommand, SessionEvent};
use riichi_core::player::PlayerId;
use std::future::Future;
use std::pin::Pin;
use tokio::sync::mpsc;

pub type AgentFuture<'a> = Pin<Box<dyn Future<Output = Option<PlayerAction>> + Send + 'a>>;

pub trait PlayerAgent: Send + Sync + 'static {
    fn player_id(&self) -> PlayerId;

    fn decide<'a>(&'a mut self, observation: SessionEvent) -> AgentFuture<'a>;
}

pub async fn run_player_agent(
    mut event_rx: mpsc::Receiver<SessionEvent>,
    action_tx: mpsc::Sender<PlayerCommand>,
    mut agent: Box<dyn PlayerAgent>,
) {
    while let Some(event) = event_rx.recv().await {
        let player = agent.player_id();
        if let Some(action) = agent.decide(event).await {
            if action_tx
                .send(PlayerCommand::new(player, action))
                .await
                .is_err()
            {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{run_player_agent, AgentFuture, PlayerAgent};
    use crate::channel::{PlayerAction, SessionEvent, TurnAction};
    use riichi_core::player::PlayerId;
    use riichi_core::tile::Tile;
    use tokio::sync::mpsc;
    use tokio::time::{timeout, Duration};

    struct TestAgent {
        player: PlayerId,
        tile: Tile,
    }

    impl TestAgent {
        fn new(player: PlayerId, tile: Tile) -> Self {
            Self { player, tile }
        }
    }

    impl PlayerAgent for TestAgent {
        fn player_id(&self) -> PlayerId {
            self.player
        }

        fn decide<'a>(&'a mut self, observation: SessionEvent) -> AgentFuture<'a> {
            let tile = self.tile;
            Box::pin(async move {
                match observation {
                    SessionEvent::ActionRequired { .. } => {
                        Some(PlayerAction::TurnAction(TurnAction::Discard(tile)))
                    }
                    _ => None,
                }
            })
        }
    }

    #[tokio::test]
    async fn runner_only_forwards_agent_actions() {
        let (event_tx, event_rx) = mpsc::channel(4);
        let (action_tx, mut action_rx) = mpsc::channel(4);
        let agent = Box::new(TestAgent::new(PlayerId(1), Tile::from_raw(0)));

        tokio::spawn(run_player_agent(event_rx, action_tx, agent));
        event_tx
            .send(SessionEvent::Error("无关事件".to_string()))
            .await
            .unwrap();
        assert!(timeout(Duration::from_millis(10), action_rx.recv())
            .await
            .is_err());

        event_tx
            .send(SessionEvent::ActionRequired {
                can_tsumo: false,
                can_riichi: false,
                riichi_options: Vec::new(),
                discard_options: vec![Tile::from_raw(0)],
                ankan_options: Vec::new(),
                kakan_options: Vec::new(),
                can_kyuushu: false,
            })
            .await
            .unwrap();
        let command = timeout(Duration::from_millis(100), action_rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(command.player, PlayerId(1));
        assert!(matches!(
            command.action,
            PlayerAction::TurnAction(TurnAction::Discard(_))
        ));
    }
}
