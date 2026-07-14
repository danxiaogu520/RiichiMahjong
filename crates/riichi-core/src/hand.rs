use serde::{Deserialize, Serialize};

use crate::tile::{Tile, TileType};

/// 手牌最大容量（13 张手牌 + 1 张摸牌 = 14）
const MAX_HAND: usize = 14;

/// 手牌，用 Vec 存储，按排序顺序维护。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hand {
    tiles: Vec<Tile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HandError {
    /// 要移除的牌不在手中
    TileNotFound(Tile),
    /// 手牌已满
    HandFull,
}

impl std::fmt::Display for HandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HandError::TileNotFound(t) => write!(f, "牌 {} 不在手中", t),
            HandError::HandFull => write!(f, "手牌已满"),
        }
    }
}

impl std::error::Error for HandError {}

impl Hand {
    /// 空手牌
    pub fn new() -> Self {
        Self { tiles: Vec::new() }
    }

    /// 从牌列表创建手牌（自动排序）
    pub fn from_tiles(tiles: &[Tile]) -> Self {
        let mut hand = Self::new();
        for &tile in tiles {
            hand.add(tile)
                .expect("Hand::from_tiles received more than 14 tiles");
        }
        hand
    }

    /// 当前手牌数量
    pub fn len(&self) -> usize {
        self.tiles.len()
    }

    /// 是否为空
    pub fn is_empty(&self) -> bool {
        self.tiles.is_empty()
    }

    /// 是否已满（14 张）
    pub fn is_full(&self) -> bool {
        self.tiles.len() >= MAX_HAND
    }

    /// 获取手牌切片（已排序）
    pub fn tiles(&self) -> &[Tile] {
        &self.tiles
    }

    /// 添加一张牌（插入到排序位置）
    pub fn add(&mut self, tile: Tile) -> Result<(), HandError> {
        if self.is_full() {
            return Err(HandError::HandFull);
        }
        let pos = self
            .tiles
            .iter()
            .position(|&t| t.raw() > tile.raw())
            .unwrap_or(self.tiles.len());
        self.tiles.insert(pos, tile);
        Ok(())
    }

    /// 移除一张牌
    pub fn remove(&mut self, tile: Tile) -> Result<(), HandError> {
        let pos = self
            .tiles
            .iter()
            .position(|&t| t == tile)
            .ok_or(HandError::TileNotFound(tile))?;
        self.tiles.remove(pos);
        Ok(())
    }

    /// 是否包含某张牌
    pub fn contains(&self, tile: Tile) -> bool {
        self.tiles.contains(&tile)
    }

    /// 统计某种牌的数量
    pub fn count_type(&self, tile_type: TileType) -> usize {
        self.tiles
            .iter()
            .filter(|t| t.tile_type() == tile_type)
            .count()
    }

    /// 获取指定索引的牌
    pub fn get(&self, index: usize) -> Option<Tile> {
        self.tiles.get(index).copied()
    }
}

impl Default for Hand {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for Hand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (i, tile) in self.tiles.iter().enumerate() {
            if i > 0 {
                write!(f, " ")?;
            }
            write!(f, "{}", tile)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{Hand, HandError};
    use crate::tile::{Tile, TileType};

    #[test]
    fn add_keeps_tiles_sorted_and_counts_by_type() {
        let mut hand = Hand::new();
        hand.add(Tile::from_raw(2)).unwrap();
        hand.add(Tile::from_raw(0)).unwrap();
        hand.add(Tile::from_raw(1)).unwrap();

        assert_eq!(
            hand.tiles(),
            &[Tile::from_raw(0), Tile::from_raw(1), Tile::from_raw(2)]
        );
        assert_eq!(hand.count_type(TileType::MAN1), 3);
    }

    #[test]
    fn add_returns_hand_full_instead_of_panicking() {
        let tiles = Tile::all_tiles();
        let mut hand = Hand::from_tiles(&tiles[..14]);

        assert_eq!(hand.add(tiles[14]), Err(HandError::HandFull));
    }
}
