use crate::meld::Meld;
use crate::meld::MeldKind;
use crate::player::PlayerId;
use crate::tile::{Tile, TileType};
use serde::{Deserialize, Serialize};

/// The way a discard was made.  This is authoritative game history, not a
/// client intent: a server timeout can therefore produce an explicit
/// `Tsumogiri` event as well.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum DiscardKind {
    Tsumogiri,
    Tedashi,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum WinKind {
    Ron,
    Tsumo,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum CallKind {
    #[default]
    Chi,
    Pon,
    Minkan,
    Ankan,
    Kakan,
}

/// Immutable information needed to create the initial state of one hand.
/// The complete wall is stored instead of relying on a particular RNG
/// implementation, so old logs remain replayable after code changes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RoundSetup {
    pub round: u32,
    pub honba: u32,
    pub riichi_sticks: u32,
    pub event_start_id: u64,
    pub initial_points: [i32; 4],
    pub wall: Vec<Tile>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HanchanSetup {
    pub rules_version: String,
    pub rounds: Vec<RoundSetup>,
}

/// The authoritative, append-only Hanchan event log.
///
/// This type deliberately does not contain a materialized engine state.  The
/// engine may cache one alongside it, but the log remains the source of truth.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Hanchan {
    pub setup: HanchanSetup,
    pub events: Vec<EventEnvelope>,
}

impl Hanchan {
    pub fn new(setup: HanchanSetup) -> Self {
        Self {
            setup,
            events: Vec::new(),
        }
    }

    pub fn next_event_id(&self) -> u64 {
        self.events.len() as u64 + 1
    }

    pub fn append(&mut self, event: GameEvent) -> EventEnvelope {
        let envelope = EventEnvelope {
            event_id: self.next_event_id(),
            event,
        };
        self.events.push(envelope.clone());
        envelope
    }

    pub fn events_after(&self, event_id: u64) -> impl Iterator<Item = &EventEnvelope> {
        self.events
            .iter()
            .filter(move |envelope| envelope.event_id > event_id)
    }
}

/// A stable position in a Hanchan event log.
///
/// Transport sequence numbers are intentionally kept separate from this
/// identifier.  The latter belongs to the game and survives reconnects.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EventEnvelope {
    pub event_id: u64,
    pub event: GameEvent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GamePhase {
    DrawPhase {
        player: PlayerId,
        position: DrawPosition,
    },
    ActionPhase {
        player: PlayerId,
        drawn_tile: Option<Tile>,
    },
    ResponsePhase {
        player: PlayerId,
        discarded_tile: Tile,
    },
    ChankanResponse {
        player: PlayerId,
        kan_tile: Tile,
        /// 被抢杠的杠种类：暗杠仅允许国士无双抢杠；加杠任意和牌可抢。
        #[serde(default)]
        kind: CallKind,
    },
    RoundOver,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum DrawPosition {
    LiveWall,
    Rinshan,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum GameEvent {
    /// Authoritative action-oriented events.
    Draw {
        player: PlayerId,
        tile: Tile,
    },
    Discard {
        player: PlayerId,
        tile: Tile,
        kind: DiscardKind,
    },
    Call {
        player: PlayerId,
        kind: CallKind,
        tiles: Vec<Tile>,
        called_tile: Option<Tile>,
        from_player: Option<PlayerId>,
        meld_index: Option<usize>,
    },
    Pass {
        player: PlayerId,
    },
    Riichi {
        player: PlayerId,
    },
    Win {
        winners: Vec<PlayerId>,
        tile: Tile,
        kind: WinKind,
        loser: Option<PlayerId>,
    },
    AbortiveDraw {
        player: Option<PlayerId>,
        reason: RoundEndReason,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RoundEndReason {
    ExhaustiveDraw,
    Win { winner: PlayerId, is_tsumo: bool },
    MultiWin { winners: Vec<PlayerId> },
    KyuushuKyuuhai,
    SuufonRenda,
    SuuchaRiichi,
    SuuKantsu,
}

#[derive(Debug, Clone)]
pub enum TurnAction {
    Discard(Tile),
    RiichiDiscard(Tile),
    Tsumo,
    Ankan(Tile),
    Kakan(usize, Tile),
    KyuushuKyuuhai,
}

#[derive(Debug, Clone)]
pub enum ResponseAction {
    Pass,
    Ron,
    Pon { hand_tiles: [Tile; 2] },
    Chi { hand_tiles: [Tile; 2] },
    Minkan { hand_tiles: [Tile; 3] },
}

#[derive(Debug, Clone)]
pub struct CallOption {
    pub player: PlayerId,
    pub call_type: CallType,
}

#[derive(Debug, Clone)]
pub enum CallType {
    Ron,
    Minkan { hand_tiles: [Tile; 3] },
    Pon { hand_tiles: [Tile; 2] },
    Chi { hand_tiles: [Tile; 2] },
}

#[derive(Debug, Clone)]
pub enum GameError {
    TileNotInHand(Tile),
    WallExhausted,
    NotYourTurn,
    InvalidAction(String),
}

impl std::fmt::Display for GameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GameError::TileNotInHand(t) => write!(f, "牌 {} 不在手中", t),
            GameError::WallExhausted => write!(f, "牌山已耗尽"),
            GameError::NotYourTurn => write!(f, "不是你的回合"),
            GameError::InvalidAction(msg) => write!(f, "无效操作: {}", msg),
        }
    }
}

impl std::error::Error for GameError {}

/// 从副露中提取食替禁打的牌类型
pub fn extract_kuikae_tiles(meld: &Meld) -> Vec<TileType> {
    let Some(called) = meld.called_tile else {
        return vec![];
    };
    let called_type = called.tile_type();
    let mut forbidden = vec![called_type];

    // 吃两面搭子时还禁止打出另一张筋牌：
    // 23 吃 1（123）禁 1、4；34 吃 2（234）禁 2、5；……；78 吃 9（789）禁 9、6。
    // 规则：所吃的牌与手中两张弃牌若还能组成另一组顺子，则该牌禁止立刻打出。
    // 低吃（如 56 吃 4）时另一张筋牌是 X+3（7）；高吃（56 吃 7）时是 X-3（4）；
    // 边张和坎张没有额外的筋食替，因此只禁止现物。
    if meld.kind == MeldKind::Chi {
        let mut hand_tiles: Vec<TileType> = meld
            .tiles
            .iter()
            .filter(|tile| **tile != called)
            .map(|tile| tile.tile_type())
            .collect();
        hand_tiles.sort_by_key(|tile| tile.0);

        if hand_tiles.len() == 2
            && hand_tiles[0].is_number()
            && hand_tiles[1].is_number()
            && hand_tiles[0].suit() == hand_tiles[1].suit()
            && hand_tiles[1].0 == hand_tiles[0].0 + 1
        {
            let first = hand_tiles[0].rank().0;
            let last = hand_tiles[1].rank().0;
            let called_rank = called_type.rank().0;
            if called_rank == first - 1 && last < 9 {
                // 低吃：如 56 吃 4，与手中 56 再组顺子的只有 7（X+3）
                forbidden.push(TileType(called_type.0 + 3));
            } else if called_rank == last + 1 && first > 1 {
                // 高吃：如 56 吃 7，与手中 56 再组顺子的只有 4（X-3）
                forbidden.push(TileType(called_type.0 - 3));
            }
        }
    }

    forbidden.sort_by_key(|tile| tile.0);
    forbidden.dedup();
    forbidden
}

#[cfg(test)]
mod tests {
    use super::extract_kuikae_tiles;
    use crate::meld::Meld;
    use crate::player::PlayerId;
    use crate::tile::Tile;

    // 索子 raw：1s=72.., 4s=84.., 5s=88.., 6s=92.., 7s=96.., 8s=100.., 9s=104..
    fn s(rank: u8) -> Tile {
        Tile::from_raw((17 + rank) * 4)
    }

    fn chi(tiles: Vec<Tile>, called: Tile) -> Meld {
        Meld::chi(tiles, called, PlayerId(0))
    }

    fn assert_forbidden(meld: &Meld, expected: &[u8]) {
        let mut got: Vec<u8> = extract_kuikae_tiles(meld)
            .into_iter()
            .map(|t| t.rank().0)
            .collect();
        got.sort_unstable();
        assert_eq!(got, expected, "meld {}", meld);
    }

    #[test]
    fn low_chi_forbids_called_and_one_above_the_meld() {
        // 56s 吃 4s（456）：禁 4、7
        assert_forbidden(&chi(vec![s(5), s(6), s(4)], s(4)), &[4, 7]);
        // 34s 吃 2s（234）：禁 2、5
        assert_forbidden(&chi(vec![s(3), s(4), s(2)], s(2)), &[2, 5]);
        // 78s 吃 6s（678）：禁 6、9
        assert_forbidden(&chi(vec![s(7), s(8), s(6)], s(6)), &[6, 9]);
        // 89s 吃 7s（789）：9 之上没有牌，只禁现物
        assert_forbidden(&chi(vec![s(8), s(9), s(7)], s(7)), &[7]);
    }

    #[test]
    fn high_chi_forbids_called_and_one_below_the_meld() {
        // 56s 吃 7s（567）：禁 7、4
        assert_forbidden(&chi(vec![s(5), s(6), s(7)], s(7)), &[4, 7]);
        // 67s 吃 8s（678）：禁 8、5
        assert_forbidden(&chi(vec![s(6), s(7), s(8)], s(8)), &[5, 8]);
        // 12s 吃 3s（123）：1 之下没有牌，只禁现物
        assert_forbidden(&chi(vec![s(1), s(2), s(3)], s(3)), &[3]);
    }

    #[test]
    fn mid_chi_and_kanchan_chi_only_forbid_the_called_tile() {
        // 46s 吃 5s（456 中张）：禁 5
        assert_forbidden(&chi(vec![s(4), s(6), s(5)], s(5)), &[5]);
        // 35s 吃 4s（345 中张）：禁 4
        assert_forbidden(&chi(vec![s(3), s(5), s(4)], s(4)), &[4]);
        // 碰只禁现物
        let pon = Meld::pon(vec![s(5), s(5), s(5)], s(5), PlayerId(0));
        assert_forbidden(&pon, &[5]);
    }
}
