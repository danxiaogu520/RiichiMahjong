use serde::{Deserialize, Serialize};

/// 一局游戏使用的规则配置。
///
/// 第一版先提供常见四人立直麻将的显式配置入口；旧逻辑仍使用默认值，
/// 后续迁移结算和途中流局时再逐项读取这些字段。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleConfig {
    pub starting_points: i32,
    pub red_fives: [u8; 3],
    pub kuitan: bool,
    pub atozuke: bool,
    pub allow_double_ron: bool,
    pub allow_triple_ron: bool,
    pub suucha_riichi_abort: bool,
    pub suukan_sanra_abort: bool,
    pub nagashi_mangan: bool,
    pub tobi: bool,
}

impl Default for RuleConfig {
    fn default() -> Self {
        Self {
            starting_points: 25_000,
            red_fives: [1, 1, 1],
            kuitan: true,
            atozuke: true,
            allow_double_ron: true,
            allow_triple_ron: true,
            suucha_riichi_abort: true,
            suukan_sanra_abort: true,
            nagashi_mangan: false,
            tobi: false,
        }
    }
}
