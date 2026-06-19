use mahjong_core::player::PlayerId;
use mahjong_core::tile::Tile;
use serde::{Deserialize, Serialize};

/// 玩家在行动阶段可以执行的操作
#[derive(Debug, Clone)]
pub enum TurnAction {
    /// 打出一张牌
    Discard(Tile),
    /// 立直打牌：宣告立直并打出一张能使手牌听牌的牌
    RiichiDiscard(Tile),
    /// 自摸和
    Tsumo,
    /// 暗杠（手中 4 张相同牌）
    Ankan(Tile),
    /// 加杠（已有明刻 + 手中第 4 张）
    Kakan(usize, Tile),
    /// 九种九牌宣告（途中流局）
    KyuushuKyuuhai,
}

/// 玩家对他人打出的牌可以做出的回应
#[derive(Debug, Clone)]
pub enum ResponseAction {
    /// 过
    Pass,
    /// 荣和
    Ron,
    /// 碰
    Pon { hand_tiles: [Tile; 2] },
    /// 吃
    Chi { hand_tiles: [Tile; 2] },
    /// 大明杠（手中 3 张 + 他家打出 1 张）
    Minkan { hand_tiles: [Tile; 3] },
}

/// 副露选项
#[derive(Debug, Clone)]
pub struct CallOption {
    pub player: PlayerId,
    pub call_type: CallType,
}

/// 可执行的副露类型
#[derive(Debug, Clone)]
pub enum CallType {
    Ron,
    Minkan { hand_tiles: [Tile; 3] },
    Pon { hand_tiles: [Tile; 2] },
    Chi { hand_tiles: [Tile; 2] },
}

/// 游戏事件，由引擎处理操作后返回
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GameEvent {
    /// 游戏开始
    GameStarted { dealer: PlayerId },
    /// 一局开始
    RoundStarted { round_number: u32, dealer: PlayerId },
    /// 玩家摸牌
    PlayerDrew { player: PlayerId, tile: Tile },
    /// 玩家打牌
    PlayerDiscarded { player: PlayerId, tile: Tile },
    /// 玩家碰
    PlayerCalledPon { player: PlayerId, tiles: Vec<Tile>, from_player: PlayerId },
    /// 玩家吃
    PlayerCalledChi { player: PlayerId, tiles: Vec<Tile>, from_player: PlayerId },
    /// 玩家大明杠
    PlayerCalledMinkan { player: PlayerId, tiles: Vec<Tile>, from_player: PlayerId },
    /// 玩家暗杠
    PlayerCalledAnkan { player: PlayerId, tiles: Vec<Tile> },
    /// 玩家加杠
    PlayerCalledKakan { player: PlayerId, tile: Tile, original_pon: Vec<Tile> },
    /// 玩家立直
    PlayerDeclaredRiichi { player: PlayerId },
    /// 玩家和了
    PlayerWon {
        player: PlayerId,
        is_tsumo: bool,
        points: i32,
        yaku_names: Vec<String>,
    },
    /// 一局结束
    RoundEnded { reason: RoundEndReason },
    /// 荒牌流局结果（听牌状态与罚符支付）
    ExhaustiveDrawResult {
        tenpai: [bool; 4],
        payments: [i32; 4],
    },
}

/// 一局结束原因
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RoundEndReason {
    /// 荒牌流局（牌山摸完）
    ExhaustiveDraw,
    /// 和了
    Win { winner: PlayerId, is_tsumo: bool },
    /// 九种九牌（途中流局）
    KyuushuKyuuhai,
    /// 四风连打（途中流局）
    SuufonRenda,
    /// 四家立直（途中流局）
    SuuchaRiichi,
    /// 四杠散了（途中流局）
    SuuKantsu,
}
