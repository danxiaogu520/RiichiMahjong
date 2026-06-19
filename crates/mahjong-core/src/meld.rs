use serde::{Deserialize, Serialize};

use crate::player::PlayerId;
use crate::tile::Tile;

/// 副露种类
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MeldKind {
    /// 吃 — 从上家打出的牌组成顺子
    Chi,
    /// 碰 — 从任意他家打出的牌组成刻子
    Pon,
    /// 暗杠 — 手中 4 张相同牌
    Ankan,
    /// 明杠 — 从他家打出的牌，手中有 3 张
    Minkan,
    /// 加杠 — 已有明刻，手中第 4 张
    Kakan,
}

/// 一组副露
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Meld {
    /// 副露种类
    pub kind: MeldKind,
    /// 副露中的所有牌（3 张或 4 张）
    pub tiles: Vec<Tile>,
    /// 从他家拿的牌（暗杠时为 None）
    pub called_tile: Option<Tile>,
    /// 来源玩家（暗杠时为 None）
    pub from_player: Option<PlayerId>,
}

impl Meld {
    /// 创建吃副露
    pub fn chi(tiles: Vec<Tile>, called_tile: Tile, from_player: PlayerId) -> Self {
        Self {
            kind: MeldKind::Chi,
            tiles,
            called_tile: Some(called_tile),
            from_player: Some(from_player),
        }
    }

    /// 创建碰副露
    pub fn pon(tiles: Vec<Tile>, called_tile: Tile, from_player: PlayerId) -> Self {
        Self {
            kind: MeldKind::Pon,
            tiles,
            called_tile: Some(called_tile),
            from_player: Some(from_player),
        }
    }

    /// 创建暗杠
    pub fn ankan(tiles: Vec<Tile>) -> Self {
        Self {
            kind: MeldKind::Ankan,
            tiles,
            called_tile: None,
            from_player: None,
        }
    }

    /// 创建明杠
    pub fn minkan(tiles: Vec<Tile>, called_tile: Tile, from_player: PlayerId) -> Self {
        Self {
            kind: MeldKind::Minkan,
            tiles,
            called_tile: Some(called_tile),
            from_player: Some(from_player),
        }
    }

    /// 是否为暗副露（不影响门清）
    pub fn is_concealed(&self) -> bool {
        self.kind == MeldKind::Ankan
    }

    /// 是否为明副露（破坏门清）
    pub fn is_open(&self) -> bool {
        !self.is_concealed()
    }
}

impl std::fmt::Display for Meld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self.kind {
            MeldKind::Chi => "吃",
            MeldKind::Pon => "碰",
            MeldKind::Ankan => "暗杠",
            MeldKind::Minkan => "明杠",
            MeldKind::Kakan => "加杠",
        };
        write!(f, "[{}", label)?;
        for tile in &self.tiles {
            write!(f, " {}", tile)?;
        }
        write!(f, "]")
    }
}
