use serde::{Deserialize, Serialize};

/// 临时本地测试默认思考时长：365 天，避免测试过程中被超时动作打断。
pub const DEFAULT_THINK_TIMEOUT_MS: u64 = 365 * 24 * 60 * 60 * 1000;

/// 一局游戏使用的规则配置。
///
/// 第一版先提供常见四人立直麻将的显式配置入口；旧逻辑仍使用默认值，
/// 后续迁移结算和途中流局时再逐项读取这些字段。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleConfig {
    pub starting_points: i32,
    pub red_fives: [u8; 3],
    pub kuitan: bool,
    /// 兼容旧配置字段。当前按每一张具体和了牌独立判役：有役的听牌可和，
    /// 无役的听牌不可和，不再做“和了前是否已有役”的时间推断。
    pub atozuke: bool,
    /// 是否启用部分规则中的双倍役满变体；默认关闭以对齐 Mortal。
    pub allow_double_yakuman: bool,
    pub allow_double_ron: bool,
    pub allow_triple_ron: bool,
    pub suucha_riichi_abort: bool,
    pub suukan_sanra_abort: bool,
    pub nagashi_mangan: bool,
    pub tobi: bool,
    /// 轮到玩家操作的超时时间（毫秒）。
    pub turn_timeout_ms: u64,
    /// 响应窗口的超时时间（毫秒）。
    pub response_timeout_ms: u64,
}

impl Default for RuleConfig {
    fn default() -> Self {
        Self {
            starting_points: 25_000,
            red_fives: [1, 1, 1],
            kuitan: true,
            atozuke: true,
            allow_double_yakuman: false,
            allow_double_ron: true,
            allow_triple_ron: true,
            suucha_riichi_abort: true,
            suukan_sanra_abort: true,
            nagashi_mangan: false,
            tobi: false,
            turn_timeout_ms: DEFAULT_THINK_TIMEOUT_MS,
            response_timeout_ms: DEFAULT_THINK_TIMEOUT_MS,
        }
    }
}
