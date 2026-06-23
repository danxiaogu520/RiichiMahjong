use riichi_core::meld::MeldKind;
use riichi_core::player::{wind_from_index, PlayerId};
use riichi_core::player_state::Player;
use riichi_core::tile::TileType;
use riichi_core::wall::Wall;

use crate::game::{GamePhase, GameState};

impl GameState {
    /// 创建新的游戏状态（默认值）
    ///
    /// 初始化为东一局、0 本场、无立直棒、空牌山
    pub fn new() -> Self {
        Self {
            players: [
                Player::new(PlayerId(0), wind_from_index(0)),
                Player::new(PlayerId(1), wind_from_index(1)),
                Player::new(PlayerId(2), wind_from_index(2)),
                Player::new(PlayerId(3), wind_from_index(3)),
            ],
            current_player: PlayerId(0),
            wind: TileType::EAST,
            events: Vec::new(),
            phase: GamePhase::ActionPhase,
            drawn_tile: None,
            round: 0,
            honba: 0,
            riichi_sticks: 0,
            wall: Wall::empty(),
            dora: Vec::new(),
            dora_indicators: Vec::new(),
            ura_dora_indicators: Vec::new(),
        }
    }

    /// 获取当前庄家
    ///
    /// 庄家 = 自风为东风的玩家 = (round - 1) % 4
    pub fn get_dealer(&self) -> PlayerId {
        PlayerId((self.round.saturating_sub(1) as usize) % 4)
    }

    /// 获取当前场上所有杠的数量（暗杠 + 加杠 + 大明杠）
    ///
    /// 用于四杠散了判定和岭上牌管理
    pub fn get_kan_count(&self) -> usize {
        self.players
            .iter()
            .map(|player| {
                player
                    .melds
                    .iter()
                    .filter(|meld| {
                        meld.kind == MeldKind::Ankan
                            || meld.kind == MeldKind::Kakan
                            || meld.kind == MeldKind::Minkan
                    })
                    .count()
            })
            .sum()
    }

    /// 从宝牌指示牌推导出宝牌
    ///
    /// 规则：指示牌的下一张（循环自增）
    /// - 数牌：1-8 → +1，9 → 1
    /// - 风牌：东南西北 → 东南西北（循环）
    /// - 三元牌：白发中 → 白发中（循环）
    pub(crate) fn dora_from_indicator(indicator: TileType) -> TileType {
        if indicator.is_number() {
            let rank = indicator.rank().0;
            if rank < 9 {
                TileType(indicator.0 + 1)
            } else {
                TileType(indicator.0 - 8) // 9 → 1
            }
        } else {
            // 字牌：风牌 27-30 循环，三元牌 31-33 循环
            let base = if indicator.is_wind() { 27 } else { 31 };
            let size = if indicator.is_wind() { 4 } else { 3 };
            TileType(base + (indicator.0 - base + 1) % size)
        }
    }

    /// 杠后翻开新的宝牌指示牌
    ///
    /// 最多可追加 4 次（共 5 组宝牌）
    /// 每次杠后从王牌区取下一张指示牌
    pub(crate) fn reveal_dora_indicator(&mut self) {
        let kan_count = self.get_kan_count();
        if kan_count > 0 && kan_count <= 5 && self.dora.len() < 5 {
            let indicator = self.wall.dora_indicator(kan_count).tile_type();
            self.dora_indicators.push(indicator);
            self.dora.push(Self::dora_from_indicator(indicator));
            // 里宝牌指示牌（立直后才可见）
            self.ura_dora_indicators
                .push(self.wall.ura_dora_indicator(kan_count).tile_type());
        }
    }
}
