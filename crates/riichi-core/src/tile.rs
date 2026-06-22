use serde::{Deserialize, Serialize};

/// 花色
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Suit {
    Man,    // 万子
    Pin,    // 筒子
    Sou,    // 索子
    Wind,   // 风牌
    Dragon, // 三元牌
}

/// 牌面数字（数牌 1-9，风牌 1-4，三元牌 1-3）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Rank(pub u8);

/// 一张牌的唯一标识，内部用 u8 (0-135)。
///
/// 编码方式：`suit_base + (rank - 1) * 4 + copy_index`
/// - 万子: 0-35, 筒子: 36-71, 索子: 72-107
/// - 风牌: 108-123 (4种 × 4张)
/// - 三元牌: 124-135 (3种 × 4张)
///
/// 同一种牌的 4 张副本共享相同的高位，仅低 2 位不同。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Tile(u8);

impl Tile {
    /// 获取底层 u8 值（用于排序、序列化等）
    pub fn raw(self) -> u8 {
        self.0
    }

    /// 从底层 u8 值创建（0-135）
    pub fn from_raw(raw: u8) -> Self {
        Self(raw)
    }
}

/// 牌种类（0-33），不区分副本。
///
/// 用于牌理判断（和了、听牌、向听数等）。
/// - 0-8: 万子 1-9
/// - 9-17: 筒子 1-9
/// - 18-26: 索子 1-9
/// - 27-30: 风牌（东南西北）
/// - 31-33: 三元牌（白發中）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TileType(pub u8);

impl TileType {
    // 常量：幺九牌类型
    pub const MAN1: TileType = TileType(0);
    pub const MAN9: TileType = TileType(8);
    pub const PIN1: TileType = TileType(9);
    pub const PIN9: TileType = TileType(17);
    pub const SOU1: TileType = TileType(18);
    pub const SOU9: TileType = TileType(26);
    pub const EAST: TileType = TileType(27);
    pub const SOUTH: TileType = TileType(28);
    pub const WEST: TileType = TileType(29);
    pub const NORTH: TileType = TileType(30);
    pub const HAKU: TileType = TileType(31);
    pub const HATSU: TileType = TileType(32);
    pub const CHUN: TileType = TileType(33);

    /// 幺九牌类型列表
    pub const YAOCHUUHAI: [TileType; 13] = [
        Self::MAN1,
        Self::MAN9,
        Self::PIN1,
        Self::PIN9,
        Self::SOU1,
        Self::SOU9,
        Self::EAST,
        Self::SOUTH,
        Self::WEST,
        Self::NORTH,
        Self::HAKU,
        Self::HATSU,
        Self::CHUN,
    ];

    /// 获取花色
    pub fn suit(self) -> Suit {
        match self.0 / 9 {
            0 => Suit::Man,
            1 => Suit::Pin,
            2 => Suit::Sou,
            _ => {
                if self.0 < 31 {
                    Suit::Wind
                } else {
                    Suit::Dragon
                }
            }
        }
    }

    /// 获取数字（数牌 1-9，风牌 1-4，三元牌 1-3）
    pub fn rank(self) -> Rank {
        let suit = self.suit();
        let base = match suit {
            Suit::Man => 0,
            Suit::Pin => 9,
            Suit::Sou => 18,
            Suit::Wind => 27,
            Suit::Dragon => 31,
        };
        Rank(self.0 - base + 1)
    }

    /// 是否为数牌
    pub fn is_number(self) -> bool {
        self.0 < 27
    }

    /// 是否为字牌（风牌 + 三元牌）
    pub fn is_honor(self) -> bool {
        self.0 >= 27
    }

    /// 是否为幺九牌（老头牌 + 字牌）
    pub fn is_yaochuuhai(self) -> bool {
        self.is_honor() || {
            let r = self.rank().0;
            r == 1 || r == 9
        }
    }

    /// 是否为老头牌（1 或 9 的数牌）
    pub fn is_terminal(self) -> bool {
        self.is_number() && {
            let r = self.rank().0;
            r == 1 || r == 9
        }
    }

    /// 是否为风牌
    pub fn is_wind(self) -> bool {
        (27..31).contains(&self.0)
    }

    /// 是否为三元牌
    pub fn is_dragon(self) -> bool {
        self.0 >= 31
    }

    /// 枚举全部 34 种牌类型
    pub fn all() -> [TileType; 34] {
        let mut arr = [TileType(0); 34];
        for (i, item) in arr.iter_mut().enumerate() {
            *item = TileType(i as u8);
        }
        arr
    }

    /// 同花色中下一张（如 1m → 2m），到 9 返回 None
    pub fn next_in_suit(self) -> Option<TileType> {
        if !self.is_number() {
            return None;
        }
        let rank = self.rank().0;
        if rank >= 9 {
            return None;
        }
        Some(TileType(self.0 + 1))
    }
}

impl std::fmt::Display for TileType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let rank = self.rank().0;
        match self.suit() {
            Suit::Man => write!(f, "{}m", rank),
            Suit::Pin => write!(f, "{}p", rank),
            Suit::Sou => write!(f, "{}s", rank),
            Suit::Wind | Suit::Dragon => {
                let z = if self.suit() == Suit::Dragon {
                    rank + 4
                } else {
                    rank
                };
                write!(f, "{}z", z)
            }
        }
    }
}

impl Tile {
    /// 获取牌种类（忽略副本）
    pub fn tile_type(self) -> TileType {
        TileType(self.0 / 4)
    }
}

impl TileType {
    /// 附上副本索引，生成一张实体牌
    pub fn with_copy(self, copy: u8) -> Tile {
        assert!(copy < 4, "copy must be 0-3");
        Tile(self.0 * 4 + copy)
    }
}

impl Tile {
    /// 通过花色、数字和副本索引 (0-3) 创建牌。
    pub fn new(suit: Suit, rank: Rank, copy: u8) -> Self {
        assert!(copy < 4, "copy index must be 0-3");
        let base = match suit {
            Suit::Man => 0,
            Suit::Pin => 36,
            Suit::Sou => 72,
            Suit::Wind => 108,
            Suit::Dragon => 124,
        };
        let max_rank = match suit {
            Suit::Wind => 4,
            Suit::Dragon => 3,
            _ => 9,
        };
        assert!(
            rank.0 >= 1 && rank.0 <= max_rank,
            "rank out of range for suit {:?}: {}",
            suit,
            rank.0
        );
        Tile(base + (rank.0 - 1) * 4 + copy)
    }

    /// 通过 tile_type 索引 (0-33) 和副本索引 (0-3) 创建牌。
    /// tile_type: 0-8=万子, 9-17=筒子, 18-26=索子, 27-30=风牌, 31-33=三元牌
    pub fn from_type_index(type_index: u8, copy: u8) -> Self {
        assert!(type_index < 34, "type_index must be 0-33");
        assert!(copy < 4, "copy must be 0-3");
        Tile(type_index * 4 + copy)
    }

    /// 获取花色
    pub fn suit(self) -> Suit {
        match self.0 / 36 {
            0 => Suit::Man,
            1 => Suit::Pin,
            2 => Suit::Sou,
            _ => {
                if self.0 < 124 {
                    Suit::Wind
                } else {
                    Suit::Dragon
                }
            }
        }
    }

    /// 获取数字
    pub fn rank(self) -> Rank {
        let suit = self.suit();
        let base = match suit {
            Suit::Man => 0,
            Suit::Pin => 36,
            Suit::Sou => 72,
            Suit::Wind => 108,
            Suit::Dragon => 124,
        };
        Rank((self.0 - base) / 4 + 1)
    }

    /// 获取副本索引 (0-3)
    pub fn copy_index(self) -> u8 {
        self.0 % 4
    }

    /// 获取 tile_type 索引 (0-33)，忽略副本。
    /// 用于判断两张牌是否为同一种。
    pub fn type_index(self) -> u8 {
        self.0 / 4
    }

    /// 判断两张牌是否为同一种（忽略副本索引）
    pub fn is_same_type(self, other: Tile) -> bool {
        self.type_index() == other.type_index()
    }

    /// 是否为数牌
    pub fn is_number(self) -> bool {
        self.0 < 108
    }

    /// 是否为字牌（风牌 + 三元牌）
    pub fn is_honor(self) -> bool {
        self.0 >= 108
    }

    /// 是否为幺九牌（老头牌 + 字牌）
    pub fn is_yaochuuhai(self) -> bool {
        if self.is_honor() {
            return true;
        }
        let r = self.rank().0;
        r == 1 || r == 9
    }

    /// 是否为老头牌（1 或 9 的数牌）
    pub fn is_terminal(self) -> bool {
        self.is_number() && (self.rank().0 == 1 || self.rank().0 == 9)
    }

    /// 是否为风牌
    pub fn is_wind(self) -> bool {
        self.0 >= 108 && self.0 < 124
    }

    /// 是否为三元牌
    pub fn is_dragon(self) -> bool {
        self.0 >= 124
    }

    /// 生成所有 136 张牌
    pub fn all_tiles() -> Vec<Tile> {
        (0..136).map(Tile).collect()
    }

    /// 是否为赤宝牌
    pub fn is_aka_dora(self) -> bool {
        self.rank() == Rank(5) && self.copy_index() == 0
    }

    /// 生成所有 34 种牌类型（每种取 copy 0）
    pub fn all_types() -> Vec<Tile> {
        (0..34).map(|i| Tile(i * 4)).collect()
    }
}

impl std::fmt::Display for Tile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let rank = self.rank().0;
        match self.suit() {
            Suit::Man => write!(f, "{}m", rank),
            Suit::Pin => write!(f, "{}p", rank),
            Suit::Sou => write!(f, "{}s", rank),
            // 字牌统一用 1z-7z：东1z 南2z 西3z 北4z 白5z 發6z 中7z
            Suit::Wind | Suit::Dragon => write!(
                f,
                "{}z",
                rank + if self.suit() == Suit::Dragon { 4 } else { 0 }
            ),
        }
    }
}

