use serde::{Deserialize, Serialize};

use crate::tile::TileType;

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

/// 从玩家索引 (0-3) 获取自风 TileType
pub fn wind_from_index(index: usize) -> TileType {
    match index % 4 {
        0 => TileType::EAST,
        1 => TileType::SOUTH,
        2 => TileType::WEST,
        3 => TileType::NORTH,
        _ => unreachable!(),
    }
}

/// 下一个风位（东→南→西→北→东）
pub fn next_wind(wind: TileType) -> TileType {
    match wind {
        TileType::EAST => TileType::SOUTH,
        TileType::SOUTH => TileType::WEST,
        TileType::WEST => TileType::NORTH,
        TileType::NORTH => TileType::EAST,
        _ => panic!("不是风牌: {:?}", wind),
    }
}

/// 风牌的中文显示（东/南/西/北）
pub fn wind_display(wind: TileType) -> &'static str {
    match wind {
        TileType::EAST => "东",
        TileType::SOUTH => "南",
        TileType::WEST => "西",
        TileType::NORTH => "北",
        _ => "？",
    }
}
