use riichi_core::game::{DrawPosition, GameEvent};
use riichi_core::player::Player;
use riichi_core::player::PlayerId;
use riichi_core::tile::{Tile, TileType};
use riichi_core::wall::Wall;
use serde::{Deserialize, Serialize};

pub use riichi_core::game::{extract_kuikae_tiles, GameError, GamePhase};

/// 麻将游戏状态核心结构体
///
/// 包含一局游戏的所有状态信息：
/// - 4 个玩家的完整状态（手牌、分数、副露、立直状态等）
/// - 场风、局数、本场数
/// - 立直棒数量（未被赢走的）
/// - 当前行动玩家
/// - 自摸牌缓冲区（摸到但未进手的牌）
/// - 牌山（剩余可摸的牌）
/// - 宝牌信息（宝牌、宝牌指示牌、里宝牌指示牌）
/// - 本局发生的事件列表
/// - 当前游戏阶段
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameState {
    /// 4 个玩家的完整状态
    pub players: [Player; 4],
    /// 当前场风（东场/南场/西场）
    pub wind: TileType,
    /// 当前局数（1-12，对应东一局到西四局）
    pub round: u32,
    /// 本场数（连庄时递增）
    pub honba: u32,
    /// 场上未被赢走的立直棒数量
    pub riichi_sticks: u32,
    /// 牌山（136 张牌的洗牌/摸牌/杠管理）
    pub wall: Wall,
    /// 宝牌列表（从宝牌指示牌推导出的宝牌类型）
    pub dora: Vec<TileType>,
    /// 宝牌指示牌列表（明面上的指示牌）
    pub dora_indicators: Vec<TileType>,
    /// 里宝牌指示牌列表（立直后才翻开的指示牌）
    pub ura_dora_indicators: Vec<TileType>,
    /// 本局发生的事件列表（用于回放和查询）
    pub events: Vec<GameEvent>,
    /// 从整场开始累积的事件历史，用于回放和断线恢复；不会随新局清空。
    pub history: Vec<GameEvent>,
    /// 本局开始时四家的点数，用于生成局末点棒变化。
    pub round_start_points: [i32; 4],
    /// 副露后当前玩家下一次出牌的食替禁牌。
    pub kuikae_forbidden: [Vec<TileType>; 4],
    /// 大三元/大四喜的责任支付者，按和了者座位记录。
    pub pao_targets: [Option<usize>; 4],
    /// 当前游戏阶段（摸牌/行动/响应/局结束）
    pub phase: GamePhase,
    /// 终局时、剩余立直棒加入前确定的最终排名。
    pub ranking_at_game_end: Option<[usize; 4]>,
}

#[derive(Debug, Clone, Copy)]
pub struct WaitInfo {
    pub tile_type: TileType,
    pub remaining: usize,
    pub is_no_yaku: bool,
}

#[derive(Debug, Clone)]
pub struct TenpaiInfo {
    pub waits: Vec<WaitInfo>,
    pub is_furiten: bool,
}

impl Default for GameState {
    fn default() -> Self {
        Self::new()
    }
}

impl GameState {
    /// 当前 phase 中拥有行动权或发起当前窗口的玩家。
    pub fn current_player(&self) -> Option<PlayerId> {
        match self.phase {
            GamePhase::DrawPhase { player, .. }
            | GamePhase::ActionPhase { player, .. }
            | GamePhase::ResponsePhase { player, .. }
            | GamePhase::ChankanResponse { player, .. } => Some(player),
            GamePhase::RoundOver => None,
        }
    }

    /// 当前行动阶段的摸牌；鸣牌后的行动阶段没有摸牌。
    pub fn drawn_tile(&self) -> Option<Tile> {
        match self.phase {
            GamePhase::ActionPhase { drawn_tile, .. } => drawn_tile,
            _ => None,
        }
    }

    pub fn draw_position(&self) -> Option<DrawPosition> {
        match self.phase {
            GamePhase::DrawPhase { position, .. } => Some(position),
            _ => None,
        }
    }
}
