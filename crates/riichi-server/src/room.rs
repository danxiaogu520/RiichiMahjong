use riichi_core::player::PlayerId;
use std::collections::HashMap;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ROOM_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoomPlayer {
    pub id: PlayerId,
    pub nickname: String,
    pub token: Option<String>,
    pub ready: bool,
    pub connected: bool,
    pub controller: SeatController,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeatController {
    Human,
    AiFill,
    AiTakeover,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoomError {
    NotFound,
    Full,
    Started,
    InvalidPlayer,
    NotAllReady,
    InvalidToken,
    EmptyNickname,
    GameNotStarted,
    NotRoomOwner,
    InvalidAiCount,
    NoHumanPlayers,
}

impl fmt::Display for RoomError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::NotFound => "房间不存在",
            Self::Full => "房间已满",
            Self::Started => "游戏已经开始",
            Self::InvalidPlayer => "玩家座位无效",
            Self::NotAllReady => "所有玩家准备后才能开始",
            Self::InvalidToken => "连接凭证无效",
            Self::EmptyNickname => "昵称不能为空",
            Self::GameNotStarted => "游戏尚未开始",
            Self::NotRoomOwner => "只有房主可以执行此操作",
            Self::InvalidAiCount => "AI 数量必须在 0 到 3 之间且能组成四个座位",
            Self::NoHumanPlayers => "至少需要一名真人玩家",
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for RoomError {}

pub struct Room {
    pub id: String,
    pub players: [Option<RoomPlayer>; 4],
    pub started: bool,
    pub owner: Option<PlayerId>,
    connection_generations: [u64; 4],
}

impl Room {
    fn new(id: String) -> Self {
        Self {
            id,
            players: std::array::from_fn(|_| None),
            started: false,
            owner: None,
            connection_generations: [0; 4],
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
        let token = new_token();
        self.players[index] = Some(RoomPlayer {
            id: player,
            nickname,
            token: Some(token.clone()),
            ready: false,
            connected: true,
            controller: SeatController::Human,
        });
        if self.owner.is_none() {
            self.owner = Some(player);
        }
        Ok((player, token))
    }

    pub fn player(&self, player: PlayerId) -> Result<&RoomPlayer, RoomError> {
        self.players
            .get(player.0)
            .and_then(Option::as_ref)
            .ok_or(RoomError::InvalidPlayer)
    }

    pub fn player_by_token(&self, token: &str) -> Result<PlayerId, RoomError> {
        self.players
            .iter()
            .flatten()
            .find(|player| player.token.as_deref() == Some(token))
            .map(|player| player.id)
            .ok_or(RoomError::InvalidToken)
    }

    pub fn connect_by_token(&mut self, token: &str) -> Result<PlayerId, RoomError> {
        let player = self
            .players
            .iter_mut()
            .flatten()
            .find(|player| player.token.as_deref() == Some(token))
            .ok_or(RoomError::InvalidToken)?;
        if player.controller == SeatController::AiFill {
            return Err(RoomError::InvalidToken);
        }
        if player.controller == SeatController::AiTakeover {
            player.controller = SeatController::Human;
            self.connection_generations[player.id.0] =
                self.connection_generations[player.id.0].wrapping_add(1);
        }
        player.connected = true;
        Ok(player.id)
    }

    pub fn disconnect_by_token(&mut self, token: &str) -> Result<(PlayerId, u64), RoomError> {
        let player = self
            .players
            .iter_mut()
            .flatten()
            .find(|player| player.token.as_deref() == Some(token))
            .ok_or(RoomError::InvalidToken)?;
        if player.controller != SeatController::Human {
            return Err(RoomError::InvalidToken);
        }
        player.connected = false;
        self.connection_generations[player.id.0] =
            self.connection_generations[player.id.0].wrapping_add(1);
        Ok((player.id, self.connection_generations[player.id.0]))
    }

    pub fn set_ready(&mut self, player: PlayerId, ready: bool) -> Result<(), RoomError> {
        if self.started {
            return Err(RoomError::Started);
        }
        let room_player = self
            .players
            .get_mut(player.0)
            .and_then(Option::as_mut)
            .ok_or(RoomError::InvalidPlayer)?;
        if room_player.controller != SeatController::Human {
            return Err(RoomError::InvalidPlayer);
        }
        room_player.ready = ready;
        Ok(())
    }

    pub fn set_ready_with_token(
        &mut self,
        token: &str,
        ready: bool,
    ) -> Result<PlayerId, RoomError> {
        let player = self
            .players
            .iter()
            .flatten()
            .find(|player| player.token.as_deref() == Some(token))
            .map(|player| player.id)
            .ok_or(RoomError::InvalidToken)?;
        self.set_ready(player, ready)?;
        Ok(player)
    }

    pub fn all_ready(&self) -> bool {
        self.players.iter().all(|player| {
            player
                .as_ref()
                .is_some_and(|player| player.controller != SeatController::Human || player.ready)
        })
    }

    pub fn set_ai_count(
        &mut self,
        requester: PlayerId,
        mut ai_count: usize,
    ) -> Result<(), RoomError> {
        if self.started {
            return Err(RoomError::Started);
        }
        if self.owner != Some(requester) {
            return Err(RoomError::NotRoomOwner);
        }
        let human_count = self
            .players
            .iter()
            .flatten()
            .filter(|player| player.controller == SeatController::Human)
            .count();
        if ai_count > 3 || human_count + ai_count > 4 {
            return Err(RoomError::InvalidAiCount);
        }

        for player in &mut self.players {
            if player
                .as_ref()
                .is_some_and(|player| player.controller == SeatController::AiFill)
            {
                *player = None;
            }
        }

        let empty_seats = self
            .players
            .iter()
            .filter(|player| player.is_none())
            .count();
        if ai_count > empty_seats {
            return Err(RoomError::InvalidAiCount);
        }
        for index in (0..self.players.len()).rev() {
            if ai_count == 0 {
                break;
            }
            if self.players[index].is_none() {
                self.players[index] = Some(RoomPlayer {
                    id: PlayerId(index),
                    nickname: "AI".to_string(),
                    token: None,
                    ready: true,
                    connected: true,
                    controller: SeatController::AiFill,
                });
                ai_count -= 1;
            }
        }
        Ok(())
    }

    pub fn ai_players(&self) -> Vec<PlayerId> {
        self.players
            .iter()
            .flatten()
            .filter(|player| {
                matches!(
                    player.controller,
                    SeatController::AiFill | SeatController::AiTakeover
                )
            })
            .map(|player| player.id)
            .collect()
    }

    pub fn start(&mut self, requester: PlayerId) -> Result<(), RoomError> {
        if self.started {
            return Err(RoomError::Started);
        }
        if self.owner != Some(requester) {
            return Err(RoomError::NotRoomOwner);
        }
        if !self
            .players
            .iter()
            .flatten()
            .any(|player| player.controller == SeatController::Human)
        {
            return Err(RoomError::NoHumanPlayers);
        }
        if self.players.iter().any(Option::is_none) {
            return Err(RoomError::NotAllReady);
        }
        if !self.all_ready() {
            return Err(RoomError::NotAllReady);
        }
        self.started = true;
        Ok(())
    }

    pub fn reconnect(&mut self, token: &str) -> Result<PlayerId, RoomError> {
        self.connect_by_token(token)
    }

    pub fn mark_ai_takeover(&mut self, player: PlayerId) -> Result<(), RoomError> {
        let room_player = self
            .players
            .get_mut(player.0)
            .and_then(Option::as_mut)
            .ok_or(RoomError::InvalidPlayer)?;
        if room_player.controller != SeatController::Human || room_player.connected {
            return Err(RoomError::InvalidPlayer);
        }
        room_player.controller = SeatController::AiTakeover;
        room_player.connected = true;
        Ok(())
    }

    pub fn connection_generation(&self, player: PlayerId) -> Result<u64, RoomError> {
        self.players
            .get(player.0)
            .and_then(Option::as_ref)
            .ok_or(RoomError::InvalidPlayer)?;
        Ok(self.connection_generations[player.0])
    }
}

fn new_token() -> String {
    let mut bytes = [0u8; 32];
    rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut bytes);
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
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
        manager
            .room_mut(&room_id)
            .unwrap()
            .start(seats[0].0)
            .unwrap();
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

    #[test]
    fn connection_lifecycle_updates_only_the_token_owner() {
        let mut manager = RoomManager::new();
        let room_id = manager.create_room();
        let (player, token) = manager.join(&room_id, "玩家").unwrap();
        assert!(
            manager
                .room(&room_id)
                .unwrap()
                .player(player)
                .unwrap()
                .connected
        );

        manager
            .room_mut(&room_id)
            .unwrap()
            .disconnect_by_token(&token)
            .unwrap();
        assert!(
            !manager
                .room(&room_id)
                .unwrap()
                .player(player)
                .unwrap()
                .connected
        );
        manager
            .room_mut(&room_id)
            .unwrap()
            .connect_by_token(&token)
            .unwrap();
        assert!(
            manager
                .room(&room_id)
                .unwrap()
                .player(player)
                .unwrap()
                .connected
        );
    }

    #[test]
    fn owner_can_configure_zero_to_three_ai_seats() {
        let mut manager = RoomManager::new();
        let room_id = manager.create_room();
        let (owner, owner_token) = manager.join(&room_id, "房主").unwrap();

        manager
            .room_mut(&room_id)
            .unwrap()
            .set_ai_count(owner, 3)
            .unwrap();

        let room = manager.room(&room_id).unwrap();
        assert_eq!(room.owner, Some(owner));
        assert_eq!(room.ai_players().len(), 3);
        assert_eq!(room.player_by_token(&owner_token).unwrap(), owner);
    }

    #[test]
    fn non_owner_cannot_change_ai_count_or_start_room() {
        let mut manager = RoomManager::new();
        let room_id = manager.create_room();
        let (owner, _) = manager.join(&room_id, "房主").unwrap();
        let (guest, _) = manager.join(&room_id, "玩家").unwrap();

        assert_eq!(
            manager.room_mut(&room_id).unwrap().set_ai_count(guest, 2),
            Err(RoomError::NotRoomOwner)
        );
        assert_eq!(
            manager.room_mut(&room_id).unwrap().start(guest),
            Err(RoomError::NotRoomOwner)
        );
        assert_ne!(owner, guest);
    }

    #[test]
    fn one_human_and_three_ai_can_start_after_human_ready() {
        let mut manager = RoomManager::new();
        let room_id = manager.create_room();
        let (owner, _) = manager.join(&room_id, "房主").unwrap();
        manager
            .room_mut(&room_id)
            .unwrap()
            .set_ai_count(owner, 3)
            .unwrap();
        manager
            .room_mut(&room_id)
            .unwrap()
            .set_ready(owner, true)
            .unwrap();

        manager.room_mut(&room_id).unwrap().start(owner).unwrap();
        assert!(manager.room(&room_id).unwrap().started);
    }
}
