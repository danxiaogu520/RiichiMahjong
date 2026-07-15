use crate::meld::Meld;
use crate::meld::MeldKind;
use crate::player::PlayerId;
use crate::tile::{Tile, TileType};
use serde::{Deserialize, Serialize};

/// The way a discard was made.  This is authoritative game history, not a
/// client intent: a server timeout can therefore produce an explicit
/// `Tsumogiri` event as well.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum DiscardKind {
    Tsumogiri,
    Tedashi,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum WinKind {
    Ron,
    Tsumo,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum CallKind {
    Chi,
    Pon,
    Minkan,
    Ankan,
    Kakan,
}

/// Immutable information needed to create the initial state of one hand.
/// The complete wall is stored instead of relying on a particular RNG
/// implementation, so old logs remain replayable after code changes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RoundSetup {
    pub round: u32,
    pub honba: u32,
    pub riichi_sticks: u32,
    pub event_start_id: u64,
    pub initial_points: [i32; 4],
    pub wall: Vec<Tile>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HanchanSetup {
    pub rules_version: String,
    pub rounds: Vec<RoundSetup>,
}

/// The authoritative, append-only Hanchan event log.
///
/// This type deliberately does not contain a materialized engine state.  The
/// engine may cache one alongside it, but the log remains the source of truth.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Hanchan {
    pub setup: HanchanSetup,
    pub events: Vec<EventEnvelope>,
}

impl Hanchan {
    pub fn new(setup: HanchanSetup) -> Self {
        Self {
            setup,
            events: Vec::new(),
        }
    }

    pub fn next_event_id(&self) -> u64 {
        self.events.len() as u64 + 1
    }

    pub fn append(&mut self, event: GameEvent) -> EventEnvelope {
        let envelope = EventEnvelope {
            event_id: self.next_event_id(),
            event,
        };
        self.events.push(envelope.clone());
        envelope
    }

    pub fn events_after(&self, event_id: u64) -> impl Iterator<Item = &EventEnvelope> {
        self.events
            .iter()
            .filter(move |envelope| envelope.event_id > event_id)
    }
}

/// A stable position in a Hanchan event log.
///
/// Transport sequence numbers are intentionally kept separate from this
/// identifier.  The latter belongs to the game and survives reconnects.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EventEnvelope {
    pub event_id: u64,
    pub event: GameEvent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GamePhase {
    DrawPhase {
        player: PlayerId,
        position: DrawPosition,
    },
    ActionPhase {
        player: PlayerId,
        drawn_tile: Option<Tile>,
    },
    ResponsePhase {
        player: PlayerId,
        discarded_tile: Tile,
    },
    ChankanResponse {
        player: PlayerId,
        kan_tile: Tile,
    },
    RoundOver,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum DrawPosition {
    LiveWall,
    Rinshan,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum GameEvent {
    /// Authoritative action-oriented events.
    Draw {
        player: PlayerId,
        tile: Tile,
    },
    Discard {
        player: PlayerId,
        tile: Tile,
        kind: DiscardKind,
    },
    Call {
        player: PlayerId,
        kind: CallKind,
        tiles: Vec<Tile>,
        called_tile: Option<Tile>,
        from_player: Option<PlayerId>,
        meld_index: Option<usize>,
    },
    Pass {
        player: PlayerId,
    },
    Riichi {
        player: PlayerId,
    },
    Win {
        winners: Vec<PlayerId>,
        tile: Tile,
        kind: WinKind,
        loser: Option<PlayerId>,
    },
    AbortiveDraw {
        player: Option<PlayerId>,
        reason: RoundEndReason,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RoundEndReason {
    ExhaustiveDraw,
    Win { winner: PlayerId, is_tsumo: bool },
    MultiWin { winners: Vec<PlayerId> },
    KyuushuKyuuhai,
    SuufonRenda,
    SuuchaRiichi,
    SuuKantsu,
}

#[derive(Debug, Clone)]
pub enum TurnAction {
    Discard(Tile),
    RiichiDiscard(Tile),
    Tsumo,
    Ankan(Tile),
    Kakan(usize, Tile),
    KyuushuKyuuhai,
}

#[derive(Debug, Clone)]
pub enum ResponseAction {
    Pass,
    Ron,
    Pon { hand_tiles: [Tile; 2] },
    Chi { hand_tiles: [Tile; 2] },
    Minkan { hand_tiles: [Tile; 3] },
}

#[derive(Debug, Clone)]
pub struct CallOption {
    pub player: PlayerId,
    pub call_type: CallType,
}

#[derive(Debug, Clone)]
pub enum CallType {
    Ron,
    Minkan { hand_tiles: [Tile; 3] },
    Pon { hand_tiles: [Tile; 2] },
    Chi { hand_tiles: [Tile; 2] },
}

#[derive(Debug, Clone)]
pub enum GameError {
    TileNotInHand(Tile),
    WallExhausted,
    NotYourTurn,
    InvalidAction(String),
}

impl std::fmt::Display for GameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GameError::TileNotInHand(t) => write!(f, "牌 {} 不在手中", t),
            GameError::WallExhausted => write!(f, "牌山已耗尽"),
            GameError::NotYourTurn => write!(f, "不是你的回合"),
            GameError::InvalidAction(msg) => write!(f, "无效操作: {}", msg),
        }
    }
}

impl std::error::Error for GameError {}

/// 从副露中提取食替禁打的牌类型
pub fn extract_kuikae_tiles(meld: &Meld) -> Vec<TileType> {
    let Some(called) = meld.called_tile else {
        return vec![];
    };
    let called_type = called.tile_type();
    let mut forbidden = vec![called_type];

    // 吃两面搭子时还禁止打出另一张筋牌：
    // 23 吃 1 禁 1、4；34 吃 2 禁 2、5；……；78 吃 9 禁 9、6。
    // 边张和坎张没有额外的筋食替，因此只禁止现物。
    if meld.kind == MeldKind::Chi {
        let mut hand_tiles: Vec<TileType> = meld
            .tiles
            .iter()
            .filter(|tile| **tile != called)
            .map(|tile| tile.tile_type())
            .collect();
        hand_tiles.sort_by_key(|tile| tile.0);

        if hand_tiles.len() == 2
            && hand_tiles[0].is_number()
            && hand_tiles[1].is_number()
            && hand_tiles[0].suit() == hand_tiles[1].suit()
            && hand_tiles[1].0 == hand_tiles[0].0 + 1
        {
            let first = hand_tiles[0].rank().0;
            let last = hand_tiles[1].rank().0;
            let called_rank = called_type.rank().0;
            if called_rank == first.saturating_sub(1) && first >= 2 {
                forbidden.push(TileType(called_type.0.saturating_sub(1)));
            } else if called_rank == last + 1 && last <= 8 {
                forbidden.push(TileType(called_type.0 + 1));
            }
        }
    }

    forbidden.sort_by_key(|tile| tile.0);
    forbidden.dedup();
    forbidden
}
