use crate::meld::Meld;
use crate::meld::MeldKind;
use crate::player::PlayerId;
use crate::tile::{Tile, TileType};
use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GameEvent {
    GameStarted {
        dealer: PlayerId,
    },
    RoundStarted {
        round_number: u32,
        dealer: PlayerId,
    },
    PlayerDrew {
        player: PlayerId,
        tile: Tile,
    },
    PlayerDiscarded {
        player: PlayerId,
        tile: Tile,
    },
    PlayerCalledPon {
        player: PlayerId,
        tiles: Vec<Tile>,
        from_player: PlayerId,
    },
    PlayerCalledChi {
        player: PlayerId,
        tiles: Vec<Tile>,
        from_player: PlayerId,
    },
    PlayerCalledMinkan {
        player: PlayerId,
        tiles: Vec<Tile>,
        from_player: PlayerId,
    },
    PlayerCalledAnkan {
        player: PlayerId,
        tiles: Vec<Tile>,
    },
    PlayerCalledKakan {
        player: PlayerId,
        tile: Tile,
        original_pon: Vec<Tile>,
    },
    PlayerDeclaredRiichi {
        player: PlayerId,
    },
    PlayerWon {
        player: PlayerId,
        is_tsumo: bool,
        points: i32,
        yaku_names: Vec<String>,
    },
    RoundEnded {
        reason: RoundEndReason,
    },
    ExhaustiveDrawResult {
        tenpai: [bool; 4],
        payments: [i32; 4],
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
