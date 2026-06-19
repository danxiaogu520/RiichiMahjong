use serde::{Deserialize, Serialize};

use crate::tile::{Rank, Suit, Tile};

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
            hand.add(tile);
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
    pub fn add(&mut self, tile: Tile) {
        assert!(self.tiles.len() < MAX_HAND, "手牌已满，无法添加");
        let pos = self.tiles.iter().position(|&t| t.raw() > tile.raw()).unwrap_or(self.tiles.len());
        self.tiles.insert(pos, tile);
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

    /// 统计某种牌（按 type_index）的数量
    pub fn count_type(&self, type_index: u8) -> usize {
        self.tiles.iter().filter(|t| t.type_index() == type_index).count()
    }

    /// 统计某种牌（按花色和数字）的数量
    pub fn count(&self, suit: Suit, rank: Rank) -> usize {
        self.tiles
            .iter()
            .filter(|t| t.suit() == suit && t.rank() == rank)
            .count()
    }

    /// 获取指定索引的牌
    pub fn get(&self, index: usize) -> Option<Tile> {
        self.tiles.get(index).copied()
    }

    /// 排序手牌
    pub fn sort(&mut self) {
        self.tiles.sort_by_key(|t| t.raw());
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
    use super::*;

    #[test]
    fn test_empty_hand() {
        let hand = Hand::new();
        assert_eq!(hand.len(), 0);
        assert!(hand.is_empty());
        assert!(hand.tiles().is_empty());
    }

    #[test]
    fn test_add_and_sort() {
        let mut hand = Hand::new();
        // 故意乱序添加
        hand.add(Tile::new(Suit::Pin, Rank(3), 0));
        hand.add(Tile::new(Suit::Man, Rank(1), 0));
        hand.add(Tile::new(Suit::Sou, Rank(9), 0));
        hand.add(Tile::new(Suit::Man, Rank(5), 0));

        assert_eq!(hand.len(), 4);
        let tiles = hand.tiles();
        // 应该按 u8 值排序（万子 < 筒子 < 索子）
        assert!(tiles[0].raw() < tiles[1].raw());
        assert!(tiles[1].raw() < tiles[2].raw());
        assert!(tiles[2].raw() < tiles[3].raw());
    }

    #[test]
    fn test_remove() {
        let mut hand = Hand::new();
        let t1 = Tile::new(Suit::Man, Rank(1), 0);
        let t2 = Tile::new(Suit::Man, Rank(2), 0);
        let t3 = Tile::new(Suit::Man, Rank(3), 0);
        hand.add(t1);
        hand.add(t2);
        hand.add(t3);

        hand.remove(t2).unwrap();
        assert_eq!(hand.len(), 2);
        assert!(!hand.contains(t2));
        assert!(hand.contains(t1));
        assert!(hand.contains(t3));
    }

    #[test]
    fn test_remove_not_found() {
        let mut hand = Hand::new();
        let t = Tile::new(Suit::Man, Rank(1), 0);
        assert!(hand.remove(t).is_err());
    }

    #[test]
    fn test_contains() {
        let mut hand = Hand::new();
        let t = Tile::new(Suit::Dragon, Rank(3), 0); // 中
        assert!(!hand.contains(t));
        hand.add(t);
        assert!(hand.contains(t));
    }

    #[test]
    fn test_count_type() {
        let mut hand = Hand::new();
        hand.add(Tile::new(Suit::Man, Rank(5), 0));
        hand.add(Tile::new(Suit::Man, Rank(5), 1));
        hand.add(Tile::new(Suit::Man, Rank(5), 2));
        hand.add(Tile::new(Suit::Man, Rank(1), 0));

        // 5 万的 type_index = 4
        assert_eq!(hand.count_type(4), 3);
        assert_eq!(hand.count(Suit::Man, Rank(5)), 3);
        assert_eq!(hand.count(Suit::Man, Rank(1)), 1);
    }

    #[test]
    fn test_from_tiles() {
        let tiles = vec![
            Tile::new(Suit::Sou, Rank(1), 0),
            Tile::new(Suit::Man, Rank(9), 0),
            Tile::new(Suit::Pin, Rank(5), 0),
        ];
        let hand = Hand::from_tiles(&tiles);
        assert_eq!(hand.len(), 3);
        // 验证排序
        assert!(hand.tiles()[0].raw() < hand.tiles()[1].raw());
        assert!(hand.tiles()[1].raw() < hand.tiles()[2].raw());
    }

    #[test]
    fn test_display() {
        let mut hand = Hand::new();
        hand.add(Tile::new(Suit::Man, Rank(1), 0));
        hand.add(Tile::new(Suit::Pin, Rank(9), 0));
        hand.add(Tile::new(Suit::Wind, Rank(1), 0));
        let display = format!("{}", hand);
        assert_eq!(display, "1m 9p 1z");
    }
}
