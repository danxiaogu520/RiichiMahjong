use super::evaluate_win;
use crate::model::{
    ClosedGroup, MentsuKind, SettlementContext, WinInput, WinSituation, WinningHand, YakuName,
};
use riichi_core::meld::{Meld, MeldKind};
use riichi_core::player::PlayerId;
use riichi_core::tile::TileType;

struct TestContext {
    situation: WinSituation,
    melds: Vec<Meld>,
    settlement: SettlementContext,
}

impl std::ops::Deref for TestContext {
    type Target = WinSituation;

    fn deref(&self) -> &Self::Target {
        &self.situation
    }
}

impl std::ops::DerefMut for TestContext {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.situation
    }
}

fn context(melds: Vec<Meld>) -> TestContext {
    TestContext {
        situation: WinSituation {
            is_tsumo: false,
            is_riichi: false,
            is_double_riichi: false,
            is_ippatsu: false,
            is_rinshan: false,
            is_chankan: false,
            is_haitei: false,
            is_houtei: false,
            is_tenhou: false,
            is_chiihou: false,
            seat_wind: TileType::EAST,
            field_wind: TileType::EAST,
        },
        melds,
        settlement: SettlementContext {
            dealer: 0,
            winner: 1,
            loser: Some(0),
            pao_target: None,
            honba: 0,
            riichi_sticks: 0,
        },
    }
}

fn check_win(
    _all_tiles: &[riichi_core::tile::Tile],
    hand_types: &[TileType],
    ctx: &TestContext,
    is_furiten: bool,
    winning_tile: riichi_core::tile::Tile,
) -> Option<crate::model::WinResult> {
    let mut concealed = physical_tiles(hand_types);
    let index = concealed
        .iter()
        .rposition(|tile| tile.tile_type() == winning_tile.tile_type())?;
    concealed.remove(index);
    evaluate_win(WinInput {
        concealed_tiles: &concealed,
        melds: &ctx.melds,
        winning_tile,
        dora_indicators: &[],
        ura_dora_indicators: &[],
        situation: &ctx.situation,
        settlement: ctx.settlement,
        is_furiten,
    })
}

fn detect_yaku(
    hand: &WinningHand,
    ctx: &TestContext,
    winning_tile: riichi_core::tile::Tile,
) -> Vec<crate::model::YakuResult> {
    hand.winning_tile_placements(winning_tile.tile_type())
        .into_iter()
        .map(|placement| {
            crate::yaku::detect_yaku(hand, &ctx.situation, &ctx.melds, winning_tile, placement)
        })
        .max_by_key(|results| results.iter().map(|result| result.han).sum::<u8>())
        .unwrap_or_default()
}

#[test]
fn open_yakuhai_is_a_valid_yaku() {
    let haku = TileType::HAKU.with_copy(0);
    let meld = Meld::pon(
        vec![
            haku,
            TileType::HAKU.with_copy(1),
            TileType::HAKU.with_copy(2),
        ],
        haku,
        PlayerId(0),
    );
    let hand_types = vec![
        TileType(0),
        TileType(1),
        TileType(2),
        TileType(3),
        TileType(4),
        TileType(5),
        TileType(6),
        TileType(7),
        TileType(8),
        TileType(9),
        TileType(9),
    ];
    let all_tiles = hand_types
        .iter()
        .enumerate()
        .map(|(i, &tile_type)| tile_type.with_copy((i % 4) as u8))
        .chain(meld.tiles.iter().copied())
        .collect::<Vec<_>>();

    let result = check_win(
        &all_tiles,
        &hand_types,
        &context(vec![meld]),
        false,
        TileType(9).with_copy(2),
    );
    assert!(result.is_some());
}

#[test]
fn yaku_completed_by_this_wait_is_valid_under_partial_wait_rule() {
    let hand_types = vec![
        TileType(0),
        TileType(1),
        TileType(2),
        TileType(3),
        TileType(4),
        TileType(5),
        TileType(6),
        TileType(7),
        TileType(8),
        TileType(9),
        TileType(9),
        TileType::HAKU,
        TileType::HAKU,
        TileType::HAKU,
    ];
    let all_tiles = hand_types
        .iter()
        .enumerate()
        .map(|(i, &tile_type)| tile_type.with_copy((i % 4) as u8))
        .collect::<Vec<_>>();
    let result = check_win(
        &all_tiles,
        &hand_types,
        &context(Vec::new()),
        false,
        TileType::HAKU.with_copy(3),
    );
    assert!(result.is_some());
}

fn sequence(tile_type: TileType) -> ClosedGroup {
    ClosedGroup {
        kind: MentsuKind::Shuntsu,
        tile_type,
    }
}

fn triplet(tile_type: TileType) -> ClosedGroup {
    ClosedGroup {
        kind: MentsuKind::Koutsu,
        tile_type,
    }
}

fn yaku_han(yaku: &[crate::model::YakuResult], target: YakuName) -> Option<u8> {
    yaku.iter()
        .find(|result| result.yaku == target)
        .map(|result| result.han)
}

fn parse_hand(spec: &str) -> Vec<TileType> {
    let mut result = Vec::new();
    let mut digits = Vec::new();
    for ch in spec.chars() {
        match ch {
            '1'..='9' => digits.push(ch as u8 - b'0'),
            'm' | 'p' | 's' | 'z' => {
                let base = match ch {
                    'm' => 0,
                    'p' => 9,
                    's' => 18,
                    'z' => 27,
                    _ => unreachable!(),
                };
                result.extend(digits.drain(..).map(|rank| TileType(base + rank - 1)));
            }
            ' ' => {}
            _ => panic!("invalid test hand character: {ch}"),
        }
    }
    assert!(digits.is_empty(), "unfinished test hand segment: {spec}");
    result
}

fn physical_tiles(types: &[TileType]) -> Vec<riichi_core::tile::Tile> {
    let mut next_copy = [0u8; 34];
    types
        .iter()
        .map(|&tile_type| {
            let copy = next_copy[tile_type.0 as usize];
            next_copy[tile_type.0 as usize] += 1;
            tile_type.with_copy(copy)
        })
        .collect()
}

fn evaluate_closed(spec: &str, winning_tile: TileType, is_tsumo: bool) -> crate::model::WinResult {
    let hand_types = parse_hand(spec);
    assert_eq!(hand_types.len(), 14);
    let all_tiles = physical_tiles(&hand_types);
    let mut ctx = context(Vec::new());
    ctx.is_tsumo = is_tsumo;
    check_win(
        &all_tiles,
        &hand_types,
        &ctx,
        false,
        winning_tile.with_copy(3),
    )
    .expect("test hand should be a valid win")
}

#[test]
fn open_yaku_use_their_reduced_han_values() {
    let open_chi = |tiles: &[TileType]| {
        let actual: Vec<_> = tiles
            .iter()
            .enumerate()
            .map(|(i, &t)| t.with_copy(i as u8))
            .collect();
        Meld::chi(actual.clone(), actual[0], PlayerId(0))
    };

    // 副露混一色：2 翻。
    let honitsu_meld = open_chi(&[TileType(0), TileType(1), TileType(2)]);
    let honitsu_hand = WinningHand::Standard {
        pair: TileType::HAKU,
        groups: vec![
            sequence(TileType(3)),
            sequence(TileType(6)),
            triplet(TileType(0)),
        ],
    };
    let mut honitsu_ctx = context(vec![honitsu_meld]);
    honitsu_ctx.is_tsumo = true;
    let yaku = detect_yaku(&honitsu_hand, &honitsu_ctx, TileType(3).with_copy(0));
    assert_eq!(yaku_han(&yaku, YakuName::Honitsu), Some(2));

    // 副露一气通贯：1 翻。
    let ittsu_meld = open_chi(&[TileType(0), TileType(1), TileType(2)]);
    let ittsu_hand = WinningHand::Standard {
        pair: TileType(9),
        groups: vec![
            sequence(TileType(3)),
            sequence(TileType(6)),
            triplet(TileType(9)),
        ],
    };
    let ittsu_ctx = context(vec![ittsu_meld]);
    let yaku = detect_yaku(&ittsu_hand, &ittsu_ctx, TileType(3).with_copy(0));
    assert_eq!(yaku_han(&yaku, YakuName::Ittsu), Some(1));
}

#[test]
fn mortal_chiitoitsu_case_keeps_tanyao_and_fixed_25_fu() {
    let result = evaluate_closed("2255m445p667788s5p", TileType(13), false);
    assert_eq!(result.fu, 25);
    assert_eq!(result.total_han, 5);
    assert_eq!(
        yaku_han(&result.yaku_results, YakuName::Chiitoitsu),
        Some(2)
    );
    assert_eq!(yaku_han(&result.yaku_results, YakuName::Tanyao), Some(1));
    assert_eq!(yaku_han(&result.yaku_results, YakuName::AkaDora), Some(2));
}

#[test]
fn mortal_ryanpeiko_pinfu_case_selects_the_highest_decomposition() {
    let pinfu = evaluate_closed("666677778888m99p", TileType(7), false);
    assert_eq!(pinfu.fu, 30);
    assert_eq!(pinfu.total_han, 4);
    assert_eq!(yaku_han(&pinfu.yaku_results, YakuName::Ryanpeiko), Some(3));
    assert_eq!(yaku_han(&pinfu.yaku_results, YakuName::Pinfu), Some(1));

    let ryanpeiko = evaluate_closed("666677778888m99p", TileType(6), false);
    assert_eq!(ryanpeiko.fu, 40);
    assert_eq!(ryanpeiko.total_han, 3);
    assert_eq!(
        yaku_han(&ryanpeiko.yaku_results, YakuName::Ryanpeiko),
        Some(3)
    );
    assert_eq!(yaku_han(&ryanpeiko.yaku_results, YakuName::Pinfu), None);
}

#[test]
fn ron_sequence_completion_preserves_sanankou_for_highest_points() {
    // 和牌前：111m / 23m / 555p / 777s / 22p。
    // 荣和 1m 应放入 123m，保留三个暗刻；若放入 111m 则没有役。
    let result = evaluate_closed("111123m22555p777s", TileType::MAN1, false);

    assert_eq!(yaku_han(&result.yaku_results, YakuName::Sananko), Some(2));
    assert_eq!(result.fu, 50);
}

#[test]
fn ron_triplet_completion_does_not_count_as_concealed_triplet() {
    let result = evaluate_closed("111m22555p123s555z", TileType::HAKU, false);

    assert_eq!(
        yaku_han(&result.yaku_results, YakuName::YakuhaiSangen),
        Some(1)
    );
    assert_eq!(yaku_han(&result.yaku_results, YakuName::Sananko), None);
}

#[test]
fn tsumo_suuankou_tanki_is_double_yakuman() {
    let result = evaluate_closed("111m222p333s111z22z", TileType::SOUTH, true);

    assert_eq!(
        yaku_han(&result.yaku_results, YakuName::SuuankouTanki),
        Some(26)
    );
    assert_eq!(result.total_han, 26);
}

#[test]
fn tsumo_suuankou_shanpon_is_single_yakuman() {
    let result = evaluate_closed("111444m222p333s11z", TileType(3), true);

    assert_eq!(yaku_han(&result.yaku_results, YakuName::Suuankou), Some(13));
    assert_eq!(
        yaku_han(&result.yaku_results, YakuName::SuuankouTanki),
        None
    );
    assert_eq!(result.total_han, 13);
}

#[test]
fn all_open_sanshoku_patterns_are_detected() {
    let open_meld = |kind: MeldKind, tile_type: TileType| {
        let tiles: Vec<_> = (0..3).map(|copy| tile_type.with_copy(copy)).collect();
        match kind {
            MeldKind::Chi => {
                let called = tiles[0];
                Meld::chi(tiles, called, PlayerId(0))
            }
            _ => {
                let called = tiles[0];
                Meld::pon(tiles, called, PlayerId(0))
            }
        }
    };

    let sequence_melds = vec![
        Meld::chi(
            vec![
                TileType(0).with_copy(0),
                TileType(1).with_copy(0),
                TileType(2).with_copy(0),
            ],
            TileType(0).with_copy(0),
            PlayerId(0),
        ),
        Meld::chi(
            vec![
                TileType(9).with_copy(0),
                TileType(10).with_copy(0),
                TileType(11).with_copy(0),
            ],
            TileType(9).with_copy(0),
            PlayerId(1),
        ),
        Meld::chi(
            vec![
                TileType(18).with_copy(0),
                TileType(19).with_copy(0),
                TileType(20).with_copy(0),
            ],
            TileType(18).with_copy(0),
            PlayerId(2),
        ),
    ];
    let sequence_hand = WinningHand::Standard {
        pair: TileType(17),
        groups: vec![triplet(TileType(8))],
    };
    let yaku = detect_yaku(
        &sequence_hand,
        &context(sequence_melds),
        TileType(17).with_copy(0),
    );
    assert_eq!(yaku_han(&yaku, YakuName::SanshokuDoujun), Some(1));
    assert_eq!(yaku_han(&yaku, YakuName::Junchan), Some(2));

    let triplet_melds = vec![
        open_meld(MeldKind::Pon, TileType(1)),
        open_meld(MeldKind::Pon, TileType(10)),
        open_meld(MeldKind::Pon, TileType(19)),
    ];
    let triplet_hand = WinningHand::Standard {
        pair: TileType(13),
        groups: vec![sequence(TileType(0))],
    };
    let yaku = detect_yaku(
        &triplet_hand,
        &context(triplet_melds),
        TileType(13).with_copy(0),
    );
    assert_eq!(yaku_han(&yaku, YakuName::SanshokuDoukou), Some(2));
}

#[test]
fn double_riichi_does_not_also_count_riichi() {
    let hand = WinningHand::Standard {
        pair: TileType(8),
        groups: vec![
            sequence(TileType(0)),
            sequence(TileType(3)),
            sequence(TileType(6)),
            sequence(TileType(1)),
        ],
    };
    let mut ctx = context(Vec::new());
    ctx.is_riichi = true;
    ctx.is_double_riichi = true;
    let yaku = detect_yaku(&hand, &ctx, TileType(1).with_copy(0));
    assert_eq!(yaku_han(&yaku, YakuName::DoubleRiichi), Some(2));
    assert_eq!(yaku_han(&yaku, YakuName::Riichi), None);
}

#[test]
fn thirteen_sided_kokushi_and_pure_chuuren_are_distinguished() {
    let kokushi_types: Vec<TileType> = TileType::YAOCHUUHAI
        .into_iter()
        .chain(std::iter::once(TileType::MAN1))
        .collect();
    let kokushi_tiles: Vec<_> = kokushi_types
        .iter()
        .enumerate()
        .map(|(index, &tile_type)| tile_type.with_copy((index % 4) as u8))
        .collect();
    let kokushi = check_win(
        &kokushi_tiles,
        &kokushi_types,
        &context(Vec::new()),
        false,
        TileType::MAN1.with_copy(3),
    )
    .expect("国士十三面应当和牌");
    assert!(yaku_han(&kokushi.yaku_results, YakuName::Kokushi13).is_some());

    let mut chuuren_types = Vec::new();
    for (rank, count) in [3, 1, 1, 1, 1, 1, 1, 1, 3].into_iter().enumerate() {
        chuuren_types.extend(std::iter::repeat_n(TileType(rank as u8), count));
    }
    chuuren_types.push(TileType(4));
    let chuuren_tiles: Vec<_> = chuuren_types
        .iter()
        .enumerate()
        .map(|(index, &tile_type)| tile_type.with_copy((index % 4) as u8))
        .collect();
    let pure = check_win(
        &chuuren_tiles,
        &chuuren_types,
        &context(Vec::new()),
        false,
        TileType(4).with_copy(3),
    )
    .expect("纯正九莲应当和牌");
    assert!(yaku_han(&pure.yaku_results, YakuName::ChuurenPoutou9).is_some());

    let regular = check_win(
        &chuuren_tiles,
        &chuuren_types,
        &context(Vec::new()),
        false,
        TileType(0).with_copy(3),
    )
    .expect("九莲宝灯应当和牌");
    assert!(yaku_han(&regular.yaku_results, YakuName::ChuurenPoutou).is_some());
    assert_eq!(
        yaku_han(&regular.yaku_results, YakuName::ChuurenPoutou9),
        None
    );
}

#[test]
fn yakuman_result_does_not_include_regular_yaku_or_dora() {
    let hand_types = vec![
        TileType(0),
        TileType(0),
        TileType(0),
        TileType(8),
        TileType(8),
        TileType(8),
        TileType(9),
        TileType(9),
        TileType(9),
        TileType(17),
        TileType(17),
        TileType(17),
        TileType(18),
        TileType(18),
    ];
    let all_tiles = hand_types
        .iter()
        .enumerate()
        .map(|(index, &tile_type)| tile_type.with_copy((index % 4) as u8))
        .collect::<Vec<_>>();
    let result = check_win(
        &all_tiles,
        &hand_types,
        &context(Vec::new()),
        false,
        TileType(18).with_copy(3),
    )
    .expect("清老头应当和牌");
    assert!(yaku_han(&result.yaku_results, YakuName::Chinroutou).is_some());
    assert_eq!(yaku_han(&result.yaku_results, YakuName::Honroutou), None);
    assert!(result.yaku_results.iter().all(|yaku| yaku.han >= 13));
}
