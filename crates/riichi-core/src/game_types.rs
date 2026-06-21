use crate::meld::Meld;
use crate::player::PlayerId;
use crate::tile::{Tile, TileType};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GamePhase {
    DrawPhase,
    ActionPhase,
    ResponsePhase {
        discarded_tile: Tile,
        discarder: PlayerId,
    },
    ChankanResponse {
        kakan_tile: Tile,
        kakan_player: PlayerId,
        meld_index: usize,
    },
    RoundOver,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GameEvent {
    GameStarted { dealer: PlayerId },
    RoundStarted { round_number: u32, dealer: PlayerId },
    PlayerDrew { player: PlayerId, tile: Tile },
    PlayerDiscarded { player: PlayerId, tile: Tile },
    PlayerCalledPon { player: PlayerId, tiles: Vec<Tile>, from_player: PlayerId },
    PlayerCalledChi { player: PlayerId, tiles: Vec<Tile>, from_player: PlayerId },
    PlayerCalledMinkan { player: PlayerId, tiles: Vec<Tile>, from_player: PlayerId },
    PlayerCalledAnkan { player: PlayerId, tiles: Vec<Tile> },
    PlayerCalledKakan { player: PlayerId, tile: Tile, original_pon: Vec<Tile> },
    PlayerDeclaredRiichi { player: PlayerId },
    PlayerWon {
        player: PlayerId,
        is_tsumo: bool,
        points: i32,
        yaku_names: Vec<String>,
    },
    RoundEnded { reason: RoundEndReason },
    ExhaustiveDrawResult {
        tenpai: [bool; 4],
        payments: [i32; 4],
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RoundEndReason {
    ExhaustiveDraw,
    Win { winner: PlayerId, is_tsumo: bool },
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
    meld.tiles
        .iter()
        .filter(|t| **t != called)
        .map(|t| t.tile_type())
        .collect()
}
