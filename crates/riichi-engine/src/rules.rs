//! 本项目唯一使用的雀魂风格四人立直麻将规则。

/// 本地测试/服务器默认思考时长。
pub const DEFAULT_THINK_TIMEOUT_MS: u64 = 365 * 24 * 60 * 60 * 1000;
pub const STARTING_POINTS: i32 = 25_000;
pub const RED_FIVES: [u8; 3] = [1, 1, 1];
pub const KUITAN: bool = true;
pub const ALLOW_DOUBLE_YAKUMAN: bool = true;
pub const NAGASHI_MANGAN: bool = true;
pub const TURN_TIMEOUT_MS: u64 = DEFAULT_THINK_TIMEOUT_MS;
pub const RESPONSE_TIMEOUT_MS: u64 = DEFAULT_THINK_TIMEOUT_MS;

/// 规则常量：允许双响和三响，不启用三家和了流局。
pub const ALLOW_DOUBLE_RON: bool = true;
pub const ALLOW_TRIPLE_RON: bool = true;
