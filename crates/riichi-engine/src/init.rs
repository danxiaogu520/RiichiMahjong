use riichi_core::meld::MeldKind;
use riichi_core::player::{wind_from_index, PlayerId};
use riichi_core::player_state::Player;
use riichi_core::tile::TileType;
use riichi_core::wall::Wall;

use crate::game::{GamePhase, GameState};

impl GameState {
    pub fn new() -> Self {
        Self {
            players: [
                Player::new(PlayerId(0), wind_from_index(0)),
                Player::new(PlayerId(1), wind_from_index(1)),
                Player::new(PlayerId(2), wind_from_index(2)),
                Player::new(PlayerId(3), wind_from_index(3)),
            ],
            current_player: PlayerId(0),
            wind: TileType::EAST,
            events: Vec::new(),
            phase: GamePhase::ActionPhase,
            drawn_tile: None,
            round: 0,
            honba: 0,
            riichi_sticks: 0,
            wall: Wall::empty(),
            dora: Vec::new(),
            dora_indicators: Vec::new(),
            ura_dora_indicators: Vec::new(),
        }
    }

    pub fn get_dealer(&self) -> PlayerId {
        PlayerId((self.round.saturating_sub(1) as usize) % 4)
    }

    pub fn get_kan_count(&self) -> usize {
        self.players
            .iter()
            .map(|player| {
                player
                    .melds
                    .iter()
                    .filter(|meld| {
                        meld.kind == MeldKind::Ankan
                            || meld.kind == MeldKind::Kakan
                            || meld.kind == MeldKind::Minkan
                    })
                    .count()
            })
            .sum()
    }

    pub(crate) fn dora_from_indicator(indicator: TileType) -> TileType {
        if indicator.is_number() {
            let rank = indicator.rank().0;
            if rank < 9 {
                TileType(indicator.0 + 1)
            } else {
                TileType(indicator.0 - 8)
            }
        } else {
            let base = if indicator.is_wind() { 27 } else { 31 };
            let size = if indicator.is_wind() { 4 } else { 3 };
            TileType(base + (indicator.0 - base + 1) % size)
        }
    }

    pub(crate) fn reveal_dora_indicator(&mut self) {
        let kan_count = self.get_kan_count();
        if kan_count > 0 && kan_count <= 5 && self.dora.len() < 5 {
            let indicator = self.wall.dora_indicator(kan_count).tile_type();
            self.dora_indicators.push(indicator);
            self.dora.push(Self::dora_from_indicator(indicator));
            self.ura_dora_indicators
                .push(self.wall.ura_dora_indicator(kan_count).tile_type());
        }
    }
}
