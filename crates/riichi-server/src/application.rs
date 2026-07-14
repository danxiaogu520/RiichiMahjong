use crate::room::{RoomError, RoomManager, RoomPlayer};
use riichi_core::player::PlayerId;
use serde::Serialize;
use std::sync::{Arc, RwLock};
use tokio::sync::{mpsc, Mutex};

type EventReceiver = Arc<Mutex<mpsc::Receiver<riichi_session::SessionEvent>>>;

struct ActiveSession {
    control_tx: mpsc::Sender<riichi_session::SessionControl>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RoomInfo {
    pub id: String,
    pub players: Vec<RoomPlayerView>,
    pub started: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RoomPlayerView {
    pub id: PlayerId,
    pub nickname: String,
    pub ready: bool,
    pub connected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct JoinInfo {
    pub room: RoomInfo,
    pub player: PlayerId,
    pub token: String,
}

/// 网络入口使用的应用层门面。
///
/// 它只负责房间命令和状态广播所需的编排，不处理 HTTP/WebSocket 细节；
/// 这样终端、WebSocket 和未来的测试客户端可以共享同一套身份校验。
#[derive(Clone, Default)]
pub struct ServerApplication {
    rooms: Arc<RwLock<RoomManager>>,
    sessions: Arc<Mutex<std::collections::HashMap<String, ActiveSession>>>,
}

impl ServerApplication {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create_room(&self) -> RoomInfo {
        let mut rooms = self.rooms.write().expect("room manager lock poisoned");
        let id = rooms.create_room();
        room_info(rooms.room(&id).expect("new room must exist"))
    }

    pub fn join_room(
        &self,
        room_id: &str,
        nickname: impl Into<String>,
    ) -> Result<JoinInfo, RoomError> {
        let mut rooms = self.rooms.write().expect("room manager lock poisoned");
        let (player, token) = rooms.join(room_id, nickname)?;
        let room = room_info(rooms.room(room_id)?);
        Ok(JoinInfo {
            room,
            player,
            token,
        })
    }

    pub fn set_ready(
        &self,
        room_id: &str,
        token: &str,
        ready: bool,
    ) -> Result<RoomInfo, RoomError> {
        let mut rooms = self.rooms.write().expect("room manager lock poisoned");
        rooms
            .room_mut(room_id)?
            .set_ready_with_token(token, ready)?;
        Ok(room_info(rooms.room(room_id)?))
    }

    pub fn start_room(&self, room_id: &str) -> Result<RoomInfo, RoomError> {
        let mut rooms = self.rooms.write().expect("room manager lock poisoned");
        rooms.room_mut(room_id)?.start()?;
        Ok(room_info(rooms.room(room_id)?))
    }

    pub fn authenticate(&self, room_id: &str, token: &str) -> Result<PlayerId, RoomError> {
        let rooms = self.rooms.read().expect("room manager lock poisoned");
        rooms.room(room_id)?.player_by_token(token)
    }

    pub fn connect_player(&self, room_id: &str, token: &str) -> Result<PlayerId, RoomError> {
        let mut rooms = self.rooms.write().expect("room manager lock poisoned");
        rooms.room_mut(room_id)?.connect_by_token(token)
    }

    pub fn disconnect_player(&self, room_id: &str, token: &str) -> Result<PlayerId, RoomError> {
        let mut rooms = self.rooms.write().expect("room manager lock poisoned");
        rooms.room_mut(room_id)?.disconnect_by_token(token)
    }

    pub async fn launch_game(&self, room_id: &str) -> Result<RoomInfo, RoomError> {
        let room = {
            let mut rooms = self.rooms.write().expect("room manager lock poisoned");
            rooms.room_mut(room_id)?.start()?;
            room_info(rooms.room(room_id)?)
        };

        let mut pairs = Vec::new();
        for index in 0..4 {
            pairs.push(riichi_session::create_player_pair(PlayerId(index)));
        }
        let event_txs = std::array::from_fn(|index| pairs[index].0.event_tx.clone());
        let (action_tx, action_rx) = mpsc::channel(256);
        // 玩家连接建立后由 SessionControl 注入新的行动通道；这里不再
        // 为尚未连接的初始 pair 创建永久转发任务，避免重连后旧连接继续提交。
        drop(pairs);

        let (control_tx, control_rx) = mpsc::channel(32);
        let session = riichi_session::GameSession::new_with_control(
            event_txs,
            action_tx.clone(),
            action_rx,
            control_rx,
        );
        tokio::spawn(async move {
            let mut session = session;
            session.run().await;
        });
        self.sessions.lock().await.insert(
            room_id.to_string(),
            ActiveSession { control_tx },
        );
        Ok(room)
    }

    pub async fn session_channels(
        &self,
        room_id: &str,
        player: PlayerId,
    ) -> Result<(mpsc::Sender<riichi_session::PlayerCommand>, EventReceiver), RoomError> {
        let sessions = self.sessions.lock().await;
        let session = sessions.get(room_id).ok_or(RoomError::GameNotStarted)?;
        let (player_handle, client_handle) = riichi_session::create_player_pair(player);
        session
            .control_tx
            .send(riichi_session::SessionControl {
                player,
                event_tx: player_handle.event_tx,
                action_rx: player_handle.action_rx,
            })
            .await
            .map_err(|_| RoomError::GameNotStarted)?;
        Ok((
            client_handle.action_tx,
            Arc::new(Mutex::new(client_handle.event_rx)),
        ))
    }

    /// 对局结束后释放会话和房间，避免内存房间及控制通道永久保留。
    pub async fn finish_game(&self, room_id: &str) -> Result<(), RoomError> {
        self.sessions.lock().await.remove(room_id);
        self.rooms
            .write()
            .expect("room manager lock poisoned")
            .close_room(room_id)
            .map(|_| ())
    }
}

fn room_info(room: &crate::room::Room) -> RoomInfo {
    RoomInfo {
        id: room.id.clone(),
        players: room
            .players
            .iter()
            .flatten()
            .map(room_player_view)
            .collect(),
        started: room.started,
    }
}

fn room_player_view(player: &RoomPlayer) -> RoomPlayerView {
    RoomPlayerView {
        id: player.id,
        nickname: player.nickname.clone(),
        ready: player.ready,
        connected: player.connected,
    }
}

#[cfg(test)]
mod tests {
    use super::ServerApplication;
    use crate::room::RoomError;

    #[test]
    fn application_checks_token_before_changing_ready_state() {
        let app = ServerApplication::new();
        let room = app.create_room();
        let joined = app.join_room(&room.id, "玩家").unwrap();

        assert_eq!(
            app.set_ready(&room.id, "wrong-token", true),
            Err(RoomError::InvalidToken)
        );
        let updated = app.set_ready(&room.id, &joined.token, true).unwrap();
        assert!(updated.players[0].ready);
    }

    #[test]
    fn room_info_never_serializes_connection_tokens() {
        let app = ServerApplication::new();
        let room = app.create_room();
        let joined = app.join_room(&room.id, "玩家").unwrap();
        let encoded = serde_json::to_string(&joined.room).unwrap();

        assert!(!encoded.contains(&joined.token));
        assert!(encoded.contains("玩家"));
    }

    #[tokio::test]
    async fn game_cannot_start_before_all_four_players_are_ready() {
        let app = ServerApplication::new();
        let room = app.create_room();
        let joined = app.join_room(&room.id, "玩家").unwrap();

        assert_eq!(
            app.launch_game(&room.id).await,
            Err(RoomError::InvalidPlayer)
        );
        app.set_ready(&room.id, &joined.token, true).unwrap();
        assert!(
            !app.set_ready(&room.id, &joined.token, true)
                .unwrap()
                .started
        );
    }

    #[tokio::test]
    async fn finished_game_releases_session_and_room() {
        let app = ServerApplication::new();
        let room = app.create_room();
        let mut joined = Vec::new();
        for name in ["东", "南", "西", "北"] {
            joined.push(app.join_room(&room.id, name).unwrap());
        }
        for player in &joined {
            app.set_ready(&room.id, &player.token, true).unwrap();
        }

        app.launch_game(&room.id).await.unwrap();
        app.finish_game(&room.id).await.unwrap();

        assert_eq!(app.authenticate(&room.id, &joined[0].token), Err(RoomError::NotFound));
        assert!(matches!(
            app.session_channels(&room.id, riichi_core::player::PlayerId(0))
                .await,
            Err(RoomError::GameNotStarted)
        ));
    }
}
