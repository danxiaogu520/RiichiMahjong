//! 本项目唯一使用的雀魂风格四人立直麻将规则。

/// 本地测试/服务器默认思考时长。
pub const DEFAULT_THINK_TIMEOUT_MS: u64 = 365 * 24 * 60 * 60 * 1000;
pub const STARTING_POINTS: i32 = 25_000;
pub const TURN_TIMEOUT_MS: u64 = DEFAULT_THINK_TIMEOUT_MS;
pub const RESPONSE_TIMEOUT_MS: u64 = DEFAULT_THINK_TIMEOUT_MS;
