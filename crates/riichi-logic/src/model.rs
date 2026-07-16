//! Domain model shared by shape analysis, evaluation, and settlement.

use riichi_core::meld::Meld;
use riichi_core::tile::{Tile, TileType};
use serde::{Deserialize, Serialize};

// ═══════════════════════════════════════════════════════════════
//  役种定义
// ═══════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum YakuName {
    // 1翻役
    Riichi,
    MenzenTsumo,
    Ippatsu,
    Tanyao,
    Pinfu,
    Iipeiko,
    YakuhaiJikaze,
    YakuhaiBakaze,
    YakuhaiSangen,
    RinshanKaihou,
    Chankan,
    Haitei,
    Houtei,

    // 2翻役
    DoubleRiichi,
    Chiitoitsu,
    Toitoi,
    Sananko,
    Sankantsu,
    SanshokuDoukou,
    Shousangen,
    Honroutou,
    Honchantai,
    SanshokuDoujun,
    Ittsu,

    // 3翻役
    Honitsu,
    Junchan,
    Ryanpeiko,

    // 6翻役
    Chinitsu,

    // 役满
    Tenhou,
    Chiihou,
    Kokushi,
    Suuankou,
    Daisangen,
    Shousuushii,
    Tsuuiisou,
    Ryuuiisou,
    Chinroutou,
    ChuurenPoutou,
    Suukantsu,

    // 双倍役满
    Kokushi13,
    SuuankouTanki,
    Daisuushii,
    ChuurenPoutou9,

    // 宝牌（算翻阶段添加）
    Dora,
    AkaDora,
    UraDora,
}

// ═══════════════════════════════════════════════════════════════
//  役结果
// ═══════════════════════════════════════════════════════════════

/// 单个役结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YakuResult {
    pub yaku: YakuName,
    pub han: u8,
}

impl YakuResult {
    pub fn new(yaku: YakuName, han: u8) -> Self {
        Self { yaku, han }
    }
}

// ═══════════════════════════════════════════════════════════════
//  判和上下文
// ═══════════════════════════════════════════════════════════════

/// 和牌发生时的场况。项目只实现一套固定规则，因此这里不包含规则开关。
#[derive(Debug, Clone)]
pub struct WinSituation {
    pub is_tsumo: bool,
    pub is_riichi: bool,
    pub is_double_riichi: bool,
    pub is_ippatsu: bool,
    pub is_rinshan: bool,
    pub is_chankan: bool,
    pub is_haitei: bool,
    pub is_houtei: bool,
    /// 庄家第一巡、尚未打出任何牌时自摸。
    pub is_tenhou: bool,
    /// 闲家第一巡、本人尚未打出任何牌且未发生鸣牌时自摸。
    pub is_chiihou: bool,
    pub seat_wind: TileType,
    pub field_wind: TileType,
}

/// 一次和牌的结算席位与场棒信息。
#[derive(Debug, Clone, Copy)]
pub struct SettlementContext {
    pub dealer: usize,
    pub winner: usize,
    pub loser: Option<usize>,
    /// 责任支付者（包牌者），仅对役满和了结算生效。
    pub pao_target: Option<usize>,
    pub honba: u32,
    pub riichi_sticks: u32,
}

/// 判和的唯一输入。门清牌不包含和了牌；和了牌由 `winning_tile` 单独给出。
/// 所有派生表示（牌种计数、完整牌集合、门清状态）均在 logic 内构造。
#[derive(Debug, Clone, Copy)]
pub struct WinInput<'a> {
    pub concealed_tiles: &'a [Tile],
    pub melds: &'a [Meld],
    pub winning_tile: Tile,
    pub dora_indicators: &'a [TileType],
    pub ura_dora_indicators: &'a [TileType],
    pub situation: &'a WinSituation,
    pub settlement: SettlementContext,
    pub is_furiten: bool,
}

// ═══════════════════════════════════════════════════════════════
//  牌分解（用于高点法）
// ═══════════════════════════════════════════════════════════════

/// 面子种类
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MentsuKind {
    Shuntsu,
    Koutsu,
}

/// 门清部分分解出的一组面子。副露使用 `riichi_core::Meld` 表示，二者不混用。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClosedGroup {
    pub kind: MentsuKind,
    pub tile_type: TileType,
}

impl ClosedGroup {
    pub fn contains(&self, tile_type: TileType) -> bool {
        match self.kind {
            MentsuKind::Shuntsu => {
                self.tile_type.suit() == tile_type.suit()
                    && (self.tile_type.0..=self.tile_type.0 + 2).contains(&tile_type.0)
            }
            MentsuKind::Koutsu => self.tile_type == tile_type,
        }
    }
}

/// 和了牌在某个完整牌组分解中的归属。
///
/// 高点法不仅要枚举面子拆法，还要枚举和了牌究竟补成雀头还是哪一组面子；
/// 判役和计符必须使用同一个归属，不能各自从最终牌型猜测。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WinningTilePlacement {
    Pair,
    Group(usize),
    Special,
}

/// 和了手牌的真实形态。特殊牌型不再伪装成标准面子。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WinningHand {
    Standard {
        pair: TileType,
        groups: Vec<ClosedGroup>,
    },
    SevenPairs {
        pairs: [TileType; 7],
    },
    Kokushi {
        pair: TileType,
    },
}

impl WinningHand {
    pub fn pair(&self) -> TileType {
        match self {
            Self::Standard { pair, .. } | Self::Kokushi { pair } => *pair,
            Self::SevenPairs { pairs } => pairs[0],
        }
    }

    pub fn groups(&self) -> &[ClosedGroup] {
        match self {
            Self::Standard { groups, .. } => groups,
            Self::SevenPairs { .. } | Self::Kokushi { .. } => &[],
        }
    }

    /// 枚举当前牌组分解下所有可能的和了牌归属。
    ///
    /// 同一种牌可能同时出现在刻子和顺子中。此时每种归属都必须独立判役、
    /// 计符和计点，再由评估层选择得点最高者。
    pub fn winning_tile_placements(&self, winning_tile: TileType) -> Vec<WinningTilePlacement> {
        match self {
            Self::Standard { pair, groups } => {
                let mut placements = Vec::new();
                if *pair == winning_tile {
                    placements.push(WinningTilePlacement::Pair);
                }
                placements.extend(
                    groups
                        .iter()
                        .enumerate()
                        .filter(|(_, group)| group.contains(winning_tile))
                        .map(|(index, _)| WinningTilePlacement::Group(index)),
                );
                placements
            }
            Self::SevenPairs { pairs } => {
                if pairs.contains(&winning_tile) {
                    vec![WinningTilePlacement::Pair]
                } else {
                    Vec::new()
                }
            }
            Self::Kokushi { pair } => {
                if *pair == winning_tile {
                    vec![WinningTilePlacement::Pair]
                } else if winning_tile.is_yaochuuhai() {
                    vec![WinningTilePlacement::Special]
                } else {
                    Vec::new()
                }
            }
        }
    }

    /// 收集所有牌类型（含雀头和面子中的每张牌）
    pub fn all_tiles(&self) -> Vec<TileType> {
        match self {
            Self::Standard { pair, groups } => {
                let mut tiles = vec![*pair; 2];
                for group in groups {
                    match group.kind {
                        MentsuKind::Shuntsu => {
                            tiles.extend([
                                group.tile_type,
                                TileType(group.tile_type.0 + 1),
                                TileType(group.tile_type.0 + 2),
                            ]);
                        }
                        MentsuKind::Koutsu => tiles.extend([group.tile_type; 3]),
                    }
                }
                tiles
            }
            Self::SevenPairs { pairs } => pairs.iter().flat_map(|&pair| [pair; 2]).collect(),
            Self::Kokushi { pair } => {
                let mut tiles = TileType::YAOCHUUHAI.to_vec();
                tiles.push(*pair);
                tiles
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════
//  宝牌结果
// ═══════════════════════════════════════════════════════════════

/// 宝牌计算结果
#[derive(Debug, Clone, Default)]
pub struct DoraResult {
    pub dora: u8,
    pub aka_dora: u8,
    pub ura_dora: u8,
}

impl DoraResult {
    pub fn total(&self) -> u8 {
        self.dora + self.aka_dora + self.ura_dora
    }

    pub fn is_empty(&self) -> bool {
        self.total() == 0
    }
}

// ═══════════════════════════════════════════════════════════════
//  和了结果
// ═══════════════════════════════════════════════════════════════

/// 和了最终结果
#[derive(Debug, Clone)]
pub struct WinResult {
    pub yaku_results: Vec<YakuResult>,
    pub total_han: u8,
    pub fu: u32,
    pub points: [i32; 4],
}

// ═══════════════════════════════════════════════════════════════
//  听牌类型（shape.rs 使用）
// ═══════════════════════════════════════════════════════════════

/// 听牌类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaitType {
    Ryanmen,
    Kanchan,
    Penchan,
    Shanpon,
    Tanki,
}

/// 听牌信息：某张牌的所有可能听牌类型
#[derive(Debug, Clone)]
pub struct WaitTileInfo {
    pub tile_type: TileType,
    pub wait_types: Vec<WaitType>,
}

/// 完整听牌分析结果
pub type WaitInfo = Vec<WaitTileInfo>;

// ═══════════════════════════════════════════════════════════════
//  工具类型
// ═══════════════════════════════════════════════════════════════

/// 牌种计数数组的包装体
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TileCounts([u8; 34]);

impl TileCounts {
    pub fn new() -> Self {
        Self([0; 34])
    }

    pub fn from_tiles(tiles: &[Tile]) -> Self {
        let mut counts = [0u8; 34];
        for tile in tiles {
            counts[tile.tile_type().0 as usize] += 1;
        }
        Self(counts)
    }

    pub fn get(&self, t: TileType) -> u8 {
        self.0[t.0 as usize]
    }

    pub fn set(&mut self, t: TileType, n: u8) {
        self.0[t.0 as usize] = n;
    }

    pub fn inc(&mut self, t: TileType) {
        self.0[t.0 as usize] += 1;
    }

    pub fn dec(&mut self, t: TileType) {
        self.0[t.0 as usize] -= 1;
    }

    pub fn is_empty(&self) -> bool {
        self.0.iter().all(|&c| c == 0)
    }

    pub fn inner(&self) -> &[u8; 34] {
        &self.0
    }

    pub fn inner_mut(&mut self) -> &mut [u8; 34] {
        &mut self.0
    }
}

impl Default for TileCounts {
    fn default() -> Self {
        Self::new()
    }
}
