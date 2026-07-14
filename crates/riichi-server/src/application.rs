use crate::room::{RoomError, RoomManager, RoomPlayer};
use riichi_core::player::PlayerId;
use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoomInfo {
    pub id: String,
    pub players: Vec<RoomPlayer>,
    pub started: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
}

fn room_info(room: &crate::room::Room) -> RoomInfo {
    RoomInfo {
        id: room.id.clone(),
        players: room.players.iter().flatten().cloned().collect(),
        started: room.started,
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
}
