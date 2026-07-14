use riichi_core::player::PlayerId;
use std::collections::HashMap;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ROOM_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoomPlayer {
    pub id: PlayerId,
    pub nickname: String,
    pub token: String,
    pub ready: bool,
    pub connected: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoomError {
    NotFound,
    Full,
    Started,
    InvalidPlayer,
    InvalidToken,
    EmptyNickname,
}

impl fmt::Display for RoomError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::NotFound => "房间不存在",
            Self::Full => "房间已满",
            Self::Started => "游戏已经开始",
            Self::InvalidPlayer => "玩家座位无效",
            Self::InvalidToken => "连接凭证无效",
            Self::EmptyNickname => "昵称不能为空",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for RoomError {}

pub struct Room {
    pub id: String,
    pub players: [Option<RoomPlayer>; 4],
    pub started: bool,
}

impl Room {
    fn new(id: String) -> Self {
        Self {
            id,
            players: std::array::from_fn(|_| None),
            started: false,
        }
    }

    fn join(&mut self, nickname: String) -> Result<(PlayerId, String), RoomError> {
        if self.started {
            return Err(RoomError::Started);
        }
        if nickname.trim().is_empty() {
            return Err(RoomError::EmptyNickname);
        }
        let index = self
            .players
            .iter()
            .position(Option::is_none)
            .ok_or(RoomError::Full)?;
        let player = PlayerId(index);
        let token = format!(
            "{}-{}",
            self.id,
            NEXT_ROOM_ID.fetch_add(1, Ordering::Relaxed)
        );
        self.players[index] = Some(RoomPlayer {
            id: player,
            nickname,
            token: token.clone(),
            ready: false,
            connected: true,
        });
        Ok((player, token))
    }

    pub fn player(&self, player: PlayerId) -> Result<&RoomPlayer, RoomError> {
        self.players
            .get(player.0)
            .and_then(Option::as_ref)
            .ok_or(RoomError::InvalidPlayer)
    }

    pub fn set_ready(&mut self, player: PlayerId, ready: bool) -> Result<(), RoomError> {
        if self.started {
            return Err(RoomError::Started);
        }
        self.players
            .get_mut(player.0)
            .and_then(Option::as_mut)
            .ok_or(RoomError::InvalidPlayer)?
            .ready = ready;
        Ok(())
    }

    pub fn all_ready(&self) -> bool {
        self.players
            .iter()
            .all(|player| player.as_ref().is_some_and(|player| player.ready))
    }

    pub fn start(&mut self) -> Result<(), RoomError> {
        if self.started {
            return Err(RoomError::Started);
        }
        if !self.all_ready() {
            return Err(RoomError::InvalidPlayer);
        }
        self.started = true;
        Ok(())
    }

    pub fn reconnect(&mut self, token: &str) -> Result<PlayerId, RoomError> {
        let player = self
            .players
            .iter_mut()
            .flatten()
            .find(|player| player.token == token)
            .ok_or(RoomError::InvalidToken)?;
        player.connected = true;
        Ok(player.id)
    }
}

#[derive(Default)]
pub struct RoomManager {
    rooms: HashMap<String, Room>,
}

impl RoomManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create_room(&mut self) -> String {
        let id = format!(
            "{:06X}",
            NEXT_ROOM_ID.fetch_add(1, Ordering::Relaxed) % 0x1000000
        );
        self.rooms.insert(id.clone(), Room::new(id.clone()));
        id
    }

    pub fn room(&self, id: &str) -> Result<&Room, RoomError> {
        self.rooms.get(id).ok_or(RoomError::NotFound)
    }

    pub fn room_mut(&mut self, id: &str) -> Result<&mut Room, RoomError> {
        self.rooms.get_mut(id).ok_or(RoomError::NotFound)
    }

    pub fn join(
        &mut self,
        id: &str,
        nickname: impl Into<String>,
    ) -> Result<(PlayerId, String), RoomError> {
        self.room_mut(id)?.join(nickname.into())
    }

    pub fn close_room(&mut self, id: &str) -> Result<Room, RoomError> {
        self.rooms.remove(id).ok_or(RoomError::NotFound)
    }
}

#[cfg(test)]
mod tests {
    use super::{RoomError, RoomManager};

    #[test]
    fn four_players_get_stable_seats_and_only_ready_rooms_start() {
        let mut manager = RoomManager::new();
        let room_id = manager.create_room();
        let mut seats = Vec::new();
        for name in ["东", "南", "西", "北"] {
            seats.push(manager.join(&room_id, name).unwrap());
        }

        assert_eq!(seats[0].0 .0, 0);
        assert_eq!(seats[3].0 .0, 3);
        assert_eq!(manager.join(&room_id, "替补"), Err(RoomError::Full));

        for (player, _) in &seats {
            manager
                .room_mut(&room_id)
                .unwrap()
                .set_ready(*player, true)
                .unwrap();
        }
        manager.room_mut(&room_id).unwrap().start().unwrap();
        assert!(manager.room(&room_id).unwrap().started);
        assert_eq!(manager.join(&room_id, "观众"), Err(RoomError::Started));
    }

    #[test]
    fn reconnect_token_restores_the_original_seat() {
        let mut manager = RoomManager::new();
        let room_id = manager.create_room();
        let (player, token) = manager.join(&room_id, "玩家").unwrap();

        assert_eq!(
            manager.room_mut(&room_id).unwrap().reconnect(&token),
            Ok(player)
        );
        assert_eq!(
            manager.room_mut(&room_id).unwrap().reconnect("invalid"),
            Err(RoomError::InvalidToken)
        );
    }
}
