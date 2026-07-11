use riichi_core::game::GameEvent;
use riichi_core::player::Player;
use riichi_core::player::PlayerId;
use riichi_core::tile::{Tile, TileType};
use riichi_core::wall::Wall;
use serde::{Deserialize, Serialize};

use crate::rules::RuleConfig;

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
    /// 当前牌局使用的规则配置。
    pub rules: RuleConfig,
    /// 4 个玩家的完整状态
    pub players: [Player; 4],
    /// 当前场风（东场/南场）
    pub wind: TileType,
    /// 当前局数（1-8，对应东一局到南四局）
    pub round: u32,
    /// 本场数（连庄时递增）
    pub honba: u32,
    /// 场上未被赢走的立直棒数量
    pub riichi_sticks: u32,
    /// 当前行动玩家
    pub current_player: PlayerId,
    /// 自摸牌缓冲区：刚从牌山摸到、尚未进手的牌
    /// 摸牌后存在于缓冲区中，手牌保持 3n+1 张
    /// 玩家行动时决定去向：打出（不进手）、自摸/暗杠/加杠（先提交到手牌再操作）
    pub drawn_tile: Option<Tile>,
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
    /// 副露后当前玩家下一次出牌的食替禁牌。
    pub kuikae_forbidden: [Vec<TileType>; 4],
    /// 当前游戏阶段（摸牌/行动/响应/局结束）
    pub phase: GamePhase,
}

impl Default for GameState {
    fn default() -> Self {
        Self::new()
    }
}
