#![allow(clippy::manual_range_patterns)]

use std::cmp::Ordering;
use std::error::Error;
use std::fmt;
use std::str::FromStr;

use crate::{tile, tile_id, tile_matches};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub const TILE_LABELS: [&str; 38] = [
    "1m", "2m", "3m", "4m", "5m", "6m", "7m", "8m", "9m", "1p", "2p", "3p", "4p", "5p", "6p", "7p",
    "8p", "9p", "1s", "2s", "3s", "4s", "5s", "6s", "7s", "8s", "9s", "E", "S", "W", "N", "P", "F",
    "C", "5mr", "5pr", "5sr", "?",
];

pub const DISCARD_URGENCY: [u8; 38] = [
    6, 5, 4, 3, 2, 3, 4, 5, 6, 6, 5, 4, 3, 2, 3, 4, 5, 6, 6, 5, 4, 3, 2, 3, 4, 5, 6, 7, 7, 7, 7, 7,
    7, 7, 1, 1, 1, 0,
];

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Tile37(u8);

#[derive(Debug)]
pub enum InvalidTile37 {
    Number(usize),
    String(String),
}

impl Tile37 {
    #[inline]
    #[must_use]
    pub fn new(id: u8) -> Self {
        debug_assert!(id <= 37, "invalid tile37 id: {id}");
        Self(id)
    }

    #[inline]
    #[must_use]
    pub const fn new_unchecked(id: u8) -> Self {
        Self(id)
    }

    #[inline]
    #[must_use]
    pub const fn raw(self) -> u8 {
        self.0
    }

    #[inline]
    #[must_use]
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    #[inline]
    #[must_use]
    pub const fn regular(self) -> Self {
        match self.0 {
            tile_id!(5mr) => tile!(5m),
            tile_id!(5pr) => tile!(5p),
            tile_id!(5sr) => tile!(5s),
            _ => self,
        }
    }

    #[inline]
    #[must_use]
    pub const fn red(self) -> Self {
        match self.0 {
            tile_id!(5m) => tile!(5mr),
            tile_id!(5p) => tile!(5pr),
            tile_id!(5s) => tile!(5sr),
            _ => self,
        }
    }

    #[inline]
    #[must_use]
    pub const fn is_red(self) -> bool {
        tile_matches!(self.0, 5mr | 5pr | 5sr)
    }

    #[inline]
    #[must_use]
    pub const fn is_honour(self) -> bool {
        tile_matches!(self.0, E | S | W | N | P | F | C)
    }

    #[inline]
    #[must_use]
    pub const fn is_terminal_or_honour(self) -> bool {
        tile_matches!(
            self.0,
            1m | 9m | 1p | 9p | 1s | 9s | E | S | W | N | P | F | C
        )
    }

    #[inline]
    #[must_use]
    pub const fn is_unknown(self) -> bool {
        self.0 >= tile_id!(?)
    }

    #[inline]
    #[must_use]
    pub const fn successor(self) -> Self {
        if self.is_unknown() {
            return self;
        }
        let tile = self.regular();
        let kind = tile.0 / 9;
        let num = tile.0 % 9;
        if kind < 3 {
            Self(kind * 9 + (num + 1) % 9)
        } else if num < 4 {
            Self(3 * 9 + (num + 1) % 4)
        } else {
            Self(3 * 9 + 4 + (num - 4 + 1) % 3)
        }
    }

    #[inline]
    #[must_use]
    pub const fn predecessor(self) -> Self {
        if self.is_unknown() {
            return self;
        }
        let tile = self.regular();
        let kind = tile.0 / 9;
        let num = tile.0 % 9;
        if kind < 3 {
            Self(kind * 9 + (num + 9 - 1) % 9)
        } else if num < 4 {
            Self(3 * 9 + (num + 4 - 1) % 4)
        } else {
            Self(3 * 9 + 4 + (num - 4 + 3 - 1) % 3)
        }
    }

    #[inline]
    #[must_use]
    pub const fn rotate_view(self) -> Self {
        if self.is_unknown() {
            return self;
        }
        let tile = self.regular();
        let tid = tile.0;
        let kind = tid / 9;
        let ret = match kind {
            0 => Self(tid + 9),
            1 => Self(tid - 9),
            _ => tile,
        };
        if self.is_red() {
            ret.red()
        } else {
            ret
        }
    }

    #[inline]
    #[must_use]
    pub fn cmp_discard_urgency(self, other: Self) -> Ordering {
        let l = self.0 as usize;
        let r = other.0 as usize;
        match DISCARD_URGENCY[l].cmp(&DISCARD_URGENCY[r]) {
            Ordering::Equal => r.cmp(&l),
            o => o,
        }
    }
}

impl Default for Tile37 {
    fn default() -> Self {
        tile!(?)
    }
}

impl TryFrom<u8> for Tile37 {
    type Error = InvalidTile37;

    fn try_from(v: u8) -> Result<Self, Self::Error> {
        Tile37::try_from(v as usize)
    }
}

impl TryFrom<usize> for Tile37 {
    type Error = InvalidTile37;

    fn try_from(v: usize) -> Result<Self, Self::Error> {
        if v >= 38 {
            Err(InvalidTile37::Number(v))
        } else {
            Ok(Self(v as u8))
        }
    }
}

impl TryFrom<Tile37> for u8 {
    type Error = InvalidTile37;

    fn try_from(v: Tile37) -> Result<Self, Self::Error> {
        Ok(v.0)
    }
}

impl FromStr for Tile37 {
    type Err = InvalidTile37;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        TILE_LABELS
            .iter()
            .position(|&label| label == s)
            .map(|i| Self(i as u8))
            .ok_or_else(|| InvalidTile37::String(s.to_owned()))
    }
}

impl fmt::Debug for Tile37 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl fmt::Display for Tile37 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(TILE_LABELS[self.0 as usize])
    }
}

impl Serialize for Tile37 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Tile37 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let tile = String::deserialize(deserializer)?
            .parse()
            .map_err(serde::de::Error::custom)?;
        Ok(tile)
    }
}

impl fmt::Display for InvalidTile37 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Number(n) => write!(f, "not a valid tile: {n}"),
            Self::String(s) => write!(f, "not a valid tile: \"{s}\""),
        }
    }
}

impl Error for InvalidTile37 {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_from_str() {
        assert!("E".parse::<Tile37>().is_ok());
        assert!("5mr".parse::<Tile37>().is_ok());
        assert!("?".parse::<Tile37>().is_ok());
        assert!("1m".parse::<Tile37>().is_ok());
        assert!("9s".parse::<Tile37>().is_ok());

        assert!("".parse::<Tile37>().is_err());
        assert!("0s".parse::<Tile37>().is_err());
        assert!("!".parse::<Tile37>().is_err());
    }

    #[test]
    fn test_convert_try_from() {
        for i in 0..38_u8 {
            assert!(Tile37::try_from(i).is_ok());
        }
        for v in [38_u8, 100, u8::MAX] {
            assert!(Tile37::try_from(v).is_err());
        }
    }

    #[test]
    fn test_successor_predecessor() {
        let tiles: Vec<Tile37> = TILE_LABELS
            .iter()
            .take(37)
            .map(|s| s.parse().unwrap())
            .collect();
        for tile in tiles {
            assert_eq!(tile.predecessor().successor(), tile.regular());
            assert_eq!(tile.successor().predecessor(), tile.regular());
        }
    }

    #[test]
    fn test_red_regular() {
        let (m5, p5, s5): (Tile37, Tile37, Tile37) = (
            "5m".parse().unwrap(),
            "5p".parse().unwrap(),
            "5s".parse().unwrap(),
        );
        let (mr5, pr5, sr5): (Tile37, Tile37, Tile37) = (
            "5mr".parse().unwrap(),
            "5pr".parse().unwrap(),
            "5sr".parse().unwrap(),
        );

        assert_eq!(m5.red(), mr5);
        assert_eq!(p5.red(), pr5);
        assert_eq!(s5.red(), sr5);
        assert_eq!(mr5.regular(), m5);
        assert_eq!(pr5.regular(), p5);
        assert_eq!(sr5.regular(), s5);

        assert!(mr5.is_red());
        assert!(pr5.is_red());
        assert!(sr5.is_red());
        assert!(!m5.is_red());
        assert!(!p5.is_red());
        assert!(!s5.is_red());
    }

    #[test]
    fn test_is_honour() {
        assert!(("E".parse::<Tile37>().unwrap()).is_honour());
        assert!(("C".parse::<Tile37>().unwrap()).is_honour());
        assert!(!("1m".parse::<Tile37>().unwrap()).is_honour());
    }

    #[test]
    fn test_is_terminal_or_honour() {
        assert!(("1m".parse::<Tile37>().unwrap()).is_terminal_or_honour());
        assert!(("9p".parse::<Tile37>().unwrap()).is_terminal_or_honour());
        assert!(("E".parse::<Tile37>().unwrap()).is_terminal_or_honour());
        assert!(!("5m".parse::<Tile37>().unwrap()).is_terminal_or_honour());
    }

    #[test]
    fn test_is_unknown() {
        assert!(("?".parse::<Tile37>().unwrap()).is_unknown());
        assert!(!("1m".parse::<Tile37>().unwrap()).is_unknown());
    }

    #[test]
    fn test_rotate_view() {
        let m5: Tile37 = "5m".parse().unwrap();
        let p5: Tile37 = "5p".parse().unwrap();
        let s5: Tile37 = "5s".parse().unwrap();
        let e: Tile37 = "E".parse().unwrap();

        assert_eq!(m5.rotate_view(), p5);
        assert_eq!(p5.rotate_view(), m5);
        assert_eq!(s5.rotate_view(), s5);
        assert_eq!(e.rotate_view(), e);
    }

    #[test]
    fn test_discard_urgency() {
        let e: Tile37 = "E".parse().unwrap();
        let m5: Tile37 = "5m".parse().unwrap();
        assert_eq!(e.cmp_discard_urgency(m5), Ordering::Greater);
        assert_eq!(m5.cmp_discard_urgency(e), Ordering::Less);
    }

    #[test]
    fn test_display() {
        assert_eq!(tile!(1m).to_string(), "1m");
        assert_eq!(tile!(5mr).to_string(), "5mr");
        assert_eq!(tile!(E).to_string(), "E");
        assert_eq!(tile!(?).to_string(), "?");
    }

    #[test]
    fn test_serde() {
        let t: Tile37 = "5mr".parse().unwrap();
        let json = serde_json::to_string(&t).unwrap();
        assert_eq!(json, "\"5mr\"");
        let t2: Tile37 = serde_json::from_str(&json).unwrap();
        assert_eq!(t, t2);
    }

    #[test]
    fn test_raw_index() {
        let t: Tile37 = "1m".parse().unwrap();
        assert_eq!(t.raw(), 0);
        assert_eq!(t.index(), 0);

        let t: Tile37 = "9s".parse().unwrap();
        assert_eq!(t.raw(), 26);
        assert_eq!(t.index(), 26);
    }
}
