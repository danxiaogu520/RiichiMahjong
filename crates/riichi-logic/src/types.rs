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

/// 判和上下文：包含判形、判役、算翻、算符所需的全部信息
#[derive(Debug, Clone)]
pub struct WinContext {
    // 基本条件
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
    /// 万、筒、索三种赤五的数量。
    pub red_fives: [u8; 3],
    /// 是否允许副露断幺九（食断）。
    pub kuitan: bool,
    /// 兼容旧配置字段。当前按具体和了牌逐张判役，偏听由该判定自然产生。
    pub atozuke: bool,
    // 风位
    pub seat_wind: TileType,
    pub field_wind: TileType,
    // 宝牌指示牌
    pub dora_indicators: Vec<TileType>,
    pub ura_dora_indicators: Vec<TileType>,
    // 副露
    pub melds: Vec<Meld>,
    // 计分参数
    pub dealer: usize,
    pub winner: usize,
    pub loser: Option<usize>,
    pub honba: u32,
    pub riichi_sticks: u32,
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

/// 一组面子
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mentsu {
    pub kind: MentsuKind,
    pub tile_type: TileType,
    pub is_open: bool,
}

/// 和了手牌类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandType {
    Standard,
    SevenPairs,
    Kokushi,
}

/// 和了手牌分解结果
#[derive(Debug, Clone)]
pub struct WinningHand {
    pub hand_type: HandType,
    pub jantai: TileType,
    pub mentsu: Vec<Mentsu>,
}

impl WinningHand {
    /// 收集所有牌类型（含雀头和面子中的每张牌）
    pub fn all_tiles(&self) -> Vec<TileType> {
        let mut tiles = Vec::new();
        tiles.push(self.jantai);
        tiles.push(self.jantai);
        for m in &self.mentsu {
            match m.kind {
                MentsuKind::Shuntsu => {
                    tiles.push(m.tile_type);
                    tiles.push(TileType(m.tile_type.0 + 1));
                    tiles.push(TileType(m.tile_type.0 + 2));
                }
                MentsuKind::Koutsu => {
                    if self.hand_type == HandType::SevenPairs {
                        tiles.push(m.tile_type);
                        tiles.push(m.tile_type);
                    } else {
                        tiles.push(m.tile_type);
                        tiles.push(m.tile_type);
                        tiles.push(m.tile_type);
                    }
                }
            }
        }
        tiles
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
//  听牌类型（analysis.rs 使用）
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
