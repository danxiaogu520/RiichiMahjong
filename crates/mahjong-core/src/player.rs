use serde::{Deserialize, Serialize};

/// 玩家标识符（0-3）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PlayerId(pub usize);

impl PlayerId {
    /// 下一个玩家
    pub fn next(self) -> PlayerId {
        PlayerId((self.0 + 1) % 4)
    }
}

impl std::fmt::Display for PlayerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            0 => write!(f, "东"),
            1 => write!(f, "南"),
            2 => write!(f, "西"),
            3 => write!(f, "北"),
            _ => write!(f, "P{}", self.0),
        }
    }
}
