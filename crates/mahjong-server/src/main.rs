use std::io::{self, Write};

use mahjong_ai::analysis::analyze_discard;
use mahjong_ai::shanten::ShantenCalculator;
use mahjong_core::player::PlayerId;
use mahjong_core::tile::{Tile, TileType};
use mahjong_engine::action::{CallType, GameEvent, RoundEndReason, ResponseAction, TurnAction};
use mahjong_engine::game::{GamePhase, GameState};
use mahjong_engine::player::wind_display;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

// ═══════════════════════════════════════════════════════════════
//  ANSI 着色
// ═══════════════════════════════════════════════════════════════

/// 带花色着色的牌面显示
fn ct(tile: Tile) -> String {
    ctt(tile.tile_type())
}

/// 带花色着色的牌类型显示
fn ctt(tt: TileType) -> String {
    let code = match tt.suit() {
        mahjong_core::tile::Suit::Sou => "\x1b[31m",   // 索子 → 红
        mahjong_core::tile::Suit::Pin => "\x1b[32m",   // 筒子 → 绿
        mahjong_core::tile::Suit::Man => "\x1b[36m",   // 万子 → 青
        mahjong_core::tile::Suit::Wind | mahjong_core::tile::Suit::Dragon => "\x1b[33m", // 字牌 → 黄
    };
    format!("{}{}\x1b[0m", code, tt)
}

fn main() {
    let mut rng = StdRng::seed_from_u64(rand::thread_rng().gen());
    let mut game = GameState::new();
    let mut calc = ShantenCalculator::new();

    println!("╔══════════════════════════════════╗");
    println!("║       日麻 - 四人麻将            ║");
    println!("╚══════════════════════════════════╝");
    println!();

    while !game.is_game_over() {
        display_round_header(&game);
        game.start_round(&mut rng);

        let round_reason = play_round(&mut game, &mut rng, &mut calc);

        display_round_result(&game, &round_reason);
    }

    println!("\n═══════════ 全局終了 ═══════════");
    display_final_state(&game);
}

/// 进行一局，返回结束原因
fn play_round(game: &mut GameState, rng: &mut StdRng, calc: &mut ShantenCalculator) -> RoundEndReason {
    loop {
        match game.phase {
            GamePhase::DrawPhase => {
                if game.draw().is_err() {
                    return RoundEndReason::ExhaustiveDraw;
                }
                // display_state 延迟到 ActionPhase 统一显示
            }

            GamePhase::ActionPhase => {
                let player = game.current_player;
                if player == PlayerId(0) {
                    display_state(game);
                    let action = get_action_choice(game);
                    match game.execute_action(action) {
                        Ok(_) => {}
                        Err(e) => {
                            println!("  操作失败: {}", e);
                            continue;
                        }
                    }
                } else {
                    let hand = &game.players[player.0].hand;
                    let analysis = analyze_discard(calc, hand.tiles());
                    let best = analysis.iter()
                        .max_by_key(|a| a.acceptance + a.improvement)
                        .unwrap();
                    let tile = best.tile;
                    match game.execute_action(TurnAction::Discard(tile)) {
                        Ok(_) => {
                            let wind = game.players[player.0].wind;
                            println!(
                                "  {}({}) 打出 {}  (进张:{}, 向听:{})",
                                wind_display(wind),
                                player_name(player.0),
                                ct(tile),
                                best.acceptance,
                                best.shanten,
                            );
                        }
                        Err(_) => continue,
                    }
                }
            }

            GamePhase::ResponsePhase { .. } => {
                let call_options = game.get_call_options();
                if !call_options.is_empty() {
                    let human_options: Vec<_> = call_options
                        .iter()
                        .filter(|o| o.player == PlayerId(0))
                        .collect();

                    if !human_options.is_empty() {
                        display_call_options(&human_options);
                        let choice = get_call_choice(&human_options);
                        if let Some(action) = choice {
                            match game.execute_call(PlayerId(0), action) {
                                Ok(_) => {
                                    if matches!(game.phase, GamePhase::RoundOver) {
                                        return extract_round_end_reason(game);
                                    }
                                    continue;
                                }
                                Err(e) => {
                                    println!("  操作失败: {}", e);
                                }
                            }
                        }
                    }
                }

                // 所有人都跳过 → 通过 execute_call(Pass) 正式落河
                game.execute_call(PlayerId(0), ResponseAction::Pass).ok();
            }

            GamePhase::ChankanResponse { .. } => {
                let call_options = game.get_call_options();
                if !call_options.is_empty() {
                    let human_options: Vec<_> = call_options
                        .iter()
                        .filter(|o| o.player == PlayerId(0))
                        .collect();

                    if !human_options.is_empty() {
                        println!("  抢杠荣和！");
                        display_call_options(&human_options);
                        let choice = get_call_choice(&human_options);
                        if let Some(action) = choice {
                            match game.execute_call(PlayerId(0), action) {
                                Ok(_) => {
                                    if matches!(game.phase, GamePhase::RoundOver) {
                                        return extract_round_end_reason(game);
                                    }
                                    continue;
                                }
                                Err(e) => {
                                    println!("  操作失败: {}", e);
                                }
                            }
                        }
                    }
                }

                // 无人荣和 → 所有人 Pass，加杠成立，由 execute_chankan_call 处理岭上补摸
                game.execute_call(PlayerId(0), ResponseAction::Pass).ok();
                if matches!(game.phase, GamePhase::RoundOver) {
                    return extract_round_end_reason(game);
                }
            }

            GamePhase::RoundOver => {
                return extract_round_end_reason(game);
            }
        }

        if game.is_round_over() {
            return extract_round_end_reason(game);
        }
    }
}

/// 从事件中提取最近一次 RoundEnded 的原因
fn extract_round_end_reason(game: &GameState) -> RoundEndReason {
    game.events
        .iter()
        .rev()
        .find_map(|e| {
            if let GameEvent::RoundEnded { reason } = e {
                Some(reason.clone())
            } else {
                None
            }
        })
        .unwrap_or(RoundEndReason::ExhaustiveDraw)
}

/// 场风+局数的显示
fn round_display(wind: TileType, round: u32) -> String {
    let wind_str = match wind {
        TileType::EAST => "东",
        TileType::SOUTH => "南",
        _ => "?",
    };
    let round_in_wind = ((round - 1) % 4) + 1;
    format!("{}{}局", wind_str, round_in_wind)
}

// ═══════════════════════════════════════════════════════════════
//  行动选择
// ═══════════════════════════════════════════════════════════════

/// 让人类玩家选择行动阶段的操作
///
/// 输入格式：
///   1-14  → 打出对应编号的牌（手牌 1-13，摸到的牌 14）
///   t     → 自摸
///   r     → 立直（然后选择宣言牌）
///   k     → 开杠（暗杠/加杠）
fn get_action_choice(game: &GameState) -> TurnAction {
    let me = &game.players[0];

    // 收集可用操作
    let can_tsumo = game.check_tsumo(PlayerId(0)).is_some();
    let can_riichi = game.can_declare_riichi(PlayerId(0));

    let ankan_opts = if me.is_riichi {
        game.get_riichi_ankan_options(PlayerId(0))
    } else {
        game.get_ankan_options(PlayerId(0))
    };
    let kakan_opts = game.get_kakan_options(PlayerId(0));
    let has_kan = !ankan_opts.is_empty() || !kakan_opts.is_empty();

    // 显示可用操作
    print!("  > ");
    if can_tsumo { print!("[\x1b[1mt\x1b[0m]自摸 "); }
    if can_riichi { print!("[\x1b[1mr\x1b[0m]立直 "); }
    if has_kan { print!("[\x1b[1mk\x1b[0m]杠 "); }
    println!("[\x1b[1m1-{}\x1b[0m]打牌", me.hand.len() + if game.drawn_tile.is_some() { 1 } else { 0 });

    loop {
        print!("  > ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        let input = input.trim().to_lowercase();

        // 数字 → 打牌
        if let Ok(n) = input.parse::<usize>() {
            let hand_len = me.hand.len();
            let total = hand_len + if game.drawn_tile.is_some() { 1 } else { 0 };
            if n >= 1 && n <= total {
                let tile = if n <= hand_len {
                    me.hand.tiles()[n - 1]
                } else {
                    game.drawn_tile.unwrap()
                };
                return TurnAction::Discard(tile);
            } else {
                println!("  无效编号，请输入 1-{}", total);
                continue;
            }
        }

        // 字母命令
        match input.as_str() {
            "t" if can_tsumo => return TurnAction::Tsumo,
            "t" => { println!("  无法自摸"); continue; }
            "r" if can_riichi => {
                let tile = get_riichi_discard_choice(game);
                return TurnAction::RiichiDiscard(tile);
            }
            "r" => { println!("  无法立直"); continue; }
            "k" if has_kan => {
                return get_kan_choice(game, &ankan_opts, &kakan_opts);
            }
            "k" => { println!("  没有可杠的牌"); continue; }
            _ => { println!("  输入 1-{} 打牌，或 t/r/k", me.hand.len() + if game.drawn_tile.is_some() {1} else {0}); }
        }
    }
}

/// 选择杠（暗杠或加杠）
fn get_kan_choice(
    game: &GameState,
    ankan_opts: &[Tile],
    kakan_opts: &[(usize, Tile)],
) -> TurnAction {
    let mut all_opts: Vec<String> = Vec::new();

    for &tile in ankan_opts {
        all_opts.push(format!("暗杠({})", ct(tile)));
    }
    for &(meld_idx, tile) in kakan_opts {
        let meld = &game.players[0].melds[meld_idx];
        all_opts.push(format!("加杠({} → {})", meld, ct(tile)));
    }

    if all_opts.len() == 1 {
        // 只有一个选项，直接执行
        if !ankan_opts.is_empty() {
            return TurnAction::Ankan(ankan_opts[0]);
        } else {
            let (meld_idx, tile) = kakan_opts[0];
            return TurnAction::Kakan(meld_idx, tile);
        }
    }

    // 多个选项
    print!("  杠: ");
    for (i, opt) in all_opts.iter().enumerate() {
        print!("[\x1b[1m{}\x1b[0m]{} ", i + 1, opt);
    }
    println!();

    loop {
        print!("  > ");
        io::stdout().flush().unwrap();
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        if let Ok(n) = input.trim().parse::<usize>() {
            if n >= 1 && n <= all_opts.len() {
                if n <= ankan_opts.len() {
                    return TurnAction::Ankan(ankan_opts[n - 1]);
                } else {
                    let (meld_idx, tile) = kakan_opts[n - ankan_opts.len() - 1];
                    return TurnAction::Kakan(meld_idx, tile);
                }
            }
        }
        println!("  无效选择");
    }
}

/// 选择立直宣言牌（必须是打出后能听牌的牌）
fn get_riichi_discard_choice(game: &GameState) -> mahjong_core::tile::Tile {
    use mahjong_yaku::analysis::analyze_wait_tiles;

    let me = &game.players[0];
    let hand = &me.hand;
    let forbidden = &me.forbidden;

    // 模拟 hand + drawn_tile = 14 张手牌
    let mut full_hand = hand.clone();
    if let Some(drawn) = game.drawn_tile {
        full_hand.add(drawn);
    }

    // 找出所有能听牌的打牌选项（从 14 张中选一张打出）
    let mut valid_tiles = Vec::new();
    for &tile in full_hand.tiles() {
        if forbidden.contains(&tile.tile_type()) {
            continue;
        }
        let mut simulated = full_hand.clone();
        simulated.remove(tile).ok();
        if !analyze_wait_tiles(simulated.tiles()).is_empty() {
            valid_tiles.push(tile);
        }
    }

    if valid_tiles.is_empty() {
        // 不应该发生（can_declare_riichi 已经检查过）
        panic!("没有能听牌的打牌选项");
    }

    println!("  立直宣言牌（打出后听牌）:");
    print!("  > ");
    for (i, &tile) in valid_tiles.iter().enumerate() {
        print!("{}\x1b[90m{}\x1b[0m ", ct(tile), i + 1);
    }
    println!();

    loop {
        print!("  选择: ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        let input = input.trim();

        match input.parse::<usize>() {
            Ok(n) if n >= 1 && n <= valid_tiles.len() => {
                return valid_tiles[n - 1];
            }
            _ => {
                println!("  无效输入。");
            }
        }
    }
}

/// 选择打出哪张牌
fn get_discard_choice(game: &GameState) -> mahjong_core::tile::Tile {
    let me = &game.players[0];
    let forbidden = &me.forbidden;

    if !forbidden.is_empty() {
        print!("  食替限制，不能打出: ");
        for &tt in forbidden {
            print!("{} ", tt);
        }
        println!();
    }

    // 立直后只能打出摸到的牌
    if me.is_riichi {
        if let Some(tile) = game.drawn_tile {
            println!("  立直中，只能打出摸到的 {}", tile);
            return tile;
        }
    }

    // 显示可打出的牌：手牌 + 自摸牌
    let hand_len = me.hand.len();
    let has_drawn = game.drawn_tile.is_some();
    let total = hand_len + if has_drawn { 1 } else { 0 };

    print!("  打出: ");
    for (i, tile) in me.hand.tiles().iter().enumerate() {
        print!("{}\x1b[90m{}\x1b[0m ", ct(*tile), i + 1);
    }
    if let Some(drawn) = game.drawn_tile {
        print!("\x1b[1m{}\x1b[0m\x1b[90m{}\x1b[0m(摸) ", ct(drawn), hand_len + 1);
    }
    println!();

    loop {
        print!("  打出哪张牌？(1-{}): ", total);
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        let input = input.trim();

        match input.parse::<usize>() {
            Ok(n) if n >= 1 && n <= total => {
                let tile = if n <= hand_len {
                    me.hand.tiles()[n - 1]
                } else {
                    game.drawn_tile.unwrap()
                };
                if forbidden.contains(&tile.tile_type()) {
                    println!("  食替：{} 不能立刻打出！", tile);
                    continue;
                }
                return tile;
            }
            _ => {
                println!("  无效输入，请输入 1 到 {} 的数字。", total);
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════
//  显示
// ═══════════════════════════════════════════════════════════════

/// 显示局开始头部信息
fn display_round_header(game: &GameState) {
    let wind = game.wind;
    let round = game.round + 1;
    let dealer = game.get_dealer();
    println!();
    println!(
        "  ╔══════════════════════════════════════════════════════╗"
    );
    println!(
        "  ║  {}  庄家：{:<6}                                  ║",
        round_display(wind, round),
        player_name(dealer.0)
    );
    println!(
        "  ╚══════════════════════════════════════════════════════╝"
    );
    println!();
}

/// 显示局结果
fn display_round_result(game: &GameState, reason: &RoundEndReason) {
    println!();
    println!("  ╔══════════════════════════════════════════════════════╗");
    match reason {
        RoundEndReason::Win { winner, is_tsumo } => {
            let win_type = if *is_tsumo { "自摸" } else { "荣和" };
            let winner_name = player_name(winner.0);
            if let Some(GameEvent::PlayerWon { yaku_names, points, .. }) =
                game.events.iter().rev().find(|e| matches!(e, GameEvent::PlayerWon { .. }))
            {
                println!(
                    "  ║  \x1b[1;33m{} {}\x1b[0m  {:>6}点                      ║",
                    winner_name, win_type, points.abs()
                );
                for name in yaku_names {
                    println!("  ║    · {:<48}║", name);
                }
            } else {
                println!("  ║  {} {}                                              ║", winner_name, win_type);
            }
        }
        RoundEndReason::ExhaustiveDraw => {
            println!("  ║  \x1b[33m荒牌流局\x1b[0m                                              ║");
            if let Some(GameEvent::ExhaustiveDrawResult { tenpai, payments }) =
                game.events.iter().rev().find(|e| matches!(e, GameEvent::ExhaustiveDrawResult { .. }))
            {
                for i in 0..4 {
                    let status = if tenpai[i] { "\x1b[32m听\x1b[0m" } else { "\x1b[90m未\x1b[0m" };
                    let pay_str = if payments[i] > 0 {
                        format!("+{}", payments[i])
                    } else if payments[i] < 0 {
                        format!("{}", payments[i])
                    } else {
                        String::new()
                    };
                    println!("  ║    {} {} {:>6}                                 ║", player_name(i), status, pay_str);
                }
            }
        }
        RoundEndReason::KyuushuKyuuhai => {
            println!("  ║  \x1b[33m九种九牌\x1b[0m — 途中流局                                ║");
        }
        RoundEndReason::SuufonRenda => {
            println!("  ║  \x1b[33m四风连打\x1b[0m — 途中流局                                ║");
        }
        RoundEndReason::SuuchaRiichi => {
            println!("  ║  \x1b[33m四家立直\x1b[0m — 途中流局                                ║");
        }
        RoundEndReason::SuuKantsu => {
            println!("  ║  \x1b[33m四杠散了\x1b[0m — 途中流局                                ║");
        }
    }

    // 点棒状况
    println!("  ╠══════════════════════════════════════════════════════╣");
    println!(
        "  ║  東:{:<7} 南:{:<7} 西:{:<7} 北:{:<7}  ║",
        game.players[0].points,
        game.players[1].points,
        game.players[2].points,
        game.players[3].points,
    );
    println!("  ╚══════════════════════════════════════════════════════╝");
    println!();
}

/// 显示当前游戏状态（人类玩家视角）
fn display_state(game: &GameState) {
    // ── 状态栏 ──
    let dora_str: Vec<String> = game.dora.iter().map(|tt| ctt(*tt)).collect();
    println!(
        "  宝牌: {}   残:{:<3} 供托:{}本  本场:{}",
        dora_str.join(" "),
        game.remaining_tiles(),
        game.riichi_sticks,
        game.honba,
    );
    println!();

    // ── 对手信息 ──
    for i in 1..4 {
        let p = &game.players[i];
        let name = player_name(i);
        let wind_ch = wind_display(p.wind);
        let riichi_tag = if p.is_riichi { " \x1b[31m[立直]\x1b[0m" } else { "" };

        println!(
            "  ┌─ {} {}({}) ──── 手牌:{:<2}{} ─┐",
            wind_ch, name, p.wind, p.hand.len(), riichi_tag
        );

        if !p.melds.is_empty() {
            let melds_str: Vec<String> = p.melds.iter().map(|m| m.to_string()).collect();
            println!("  │ 副露: {:<40}│", melds_str.join(" "));
        }

        // 牌河（每行 6 张）
        print_discards_in_grid(p, "  │");

        println!("  └──────────────────────────────────────────────────┘");
    }

    // ── 分隔线 ──
    println!();
    println!("  ═══════════════════════════════════════════════════");
    println!();

    // ── 自己的信息 ──
    let me = &game.players[0];
    let me_wind = wind_display(me.wind);

    if !me.melds.is_empty() {
        let melds_str: Vec<String> = me.melds.iter().map(|m| m.to_string()).collect();
        println!("  你({}) 副露: {}", me_wind, melds_str.join(" "));
    }

    // 手牌 + 摸到的牌
    print!("  你({}) 手牌({}张):  ", me_wind, me.hand.len());
    for tile in me.hand.tiles() {
        print!("{} ", ct(*tile));
    }
    if let Some(drawn) = game.drawn_tile {
        print!(" 摸:\x1b[1m{}\x1b[0m", ct(drawn));
    }
    println!();

    // 牌河
    print!("  你({}) 牌河:       ", me_wind);
    print_discards_flat(me);

    // 听牌提示（非立直、门前清时显示）
    if !me.is_riichi && me.is_menzen() {
        // 非立直时检查听牌（立直时必定听牌，不需要额外检查）
        let waits = mahjong_yaku::analysis::analyze_wait_tiles(me.hand.tiles());
        if !waits.is_empty() {
            let wait_str: Vec<String> = waits.iter().map(|w| ctt(w.tile_type)).collect();
            println!("  \x1b[32m听牌: {}\x1b[0m", wait_str.join(" "));
        }
    }

    if me.is_riichi {
        println!("  \x1b[31;1m★ 立直中 ★\x1b[0m");
    }

    println!();
    println!("  ──────────────────────────────────────────────────");
    println!();
}

/// 显示对手牌河（每行 6 张，带边框）
fn print_discards_in_grid(p: &mahjong_engine::player::Player, prefix: &str) {
    if p.discards.is_empty() {
        println!("{} 牌河: {:<40}│", prefix, "");
        return;
    }
    let lines: Vec<Vec<String>> = p.discards.chunks(6).map(|chunk| {
        chunk.iter().map(|t| {
            if p.riichi_declaration_tile == Some(*t) {
                format!("({})", ct(*t))
            } else {
                format!("{} ", ct(*t))
            }
        }).collect()
    }).collect();

    for (i, line) in lines.iter().enumerate() {
        let label = if i == 0 { "牌河:" } else { "     " };
        println!("{} {} {:<52}│", prefix, label, line.join(""));
    }
}

/// 显示自己牌河（无边框，扁平）
fn print_discards_flat(p: &mahjong_engine::player::Player) {
    if p.discards.is_empty() {
        println!();
        return;
    }
    for (i, t) in p.discards.iter().enumerate() {
        if i > 0 && i % 6 == 0 {
            println!();
            print!("                  ");
        }
        if p.riichi_declaration_tile == Some(*t) {
            print!("({}) ", ct(*t));
        } else {
            print!("{} ", ct(*t));
        }
    }
    println!();
}

/// 显示副露选项
fn display_call_options(options: &[&mahjong_engine::action::CallOption]) {
    print!("  > ");
    for (i, opt) in options.iter().enumerate() {
        match &opt.call_type {
            CallType::Ron => print!("[\x1b[1;33m{}\x1b[0m]荣和 ", i + 1),
            CallType::Minkan { hand_tiles } => {
                print!(
                    "[\x1b[1m{}\x1b[0m]大明杠({}{}{}) ",
                    i + 1,
                    ct(hand_tiles[0]), ct(hand_tiles[1]), ct(hand_tiles[2])
                );
            }
            CallType::Pon { hand_tiles } => {
                print!("[\x1b[1m{}\x1b[0m]碰({}{}) ", i + 1, ct(hand_tiles[0]), ct(hand_tiles[1]));
            }
            CallType::Chi { hand_tiles } => {
                print!("[\x1b[1m{}\x1b[0m]吃({}{}) ", i + 1, ct(hand_tiles[0]), ct(hand_tiles[1]));
            }
        }
    }
    println!("[\x1b[90mP\x1b[0m]跳过");
}

/// 获取副露选择
fn get_call_choice(options: &[&mahjong_engine::action::CallOption]) -> Option<ResponseAction> {
    loop {
        print!("  选择: ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        let input = input.trim();

        if input.eq_ignore_ascii_case("p") || input.is_empty() {
            return None;
        }

        match input.parse::<usize>() {
            Ok(n) if n >= 1 && n <= options.len() => {
                let opt = &options[n - 1];
                return match &opt.call_type {
                    CallType::Ron => Some(ResponseAction::Ron),
                    CallType::Minkan { hand_tiles } => Some(ResponseAction::Minkan {
                        hand_tiles: *hand_tiles,
                    }),
                    CallType::Pon { hand_tiles } => Some(ResponseAction::Pon {
                        hand_tiles: *hand_tiles,
                    }),
                    CallType::Chi { hand_tiles } => Some(ResponseAction::Chi {
                        hand_tiles: *hand_tiles,
                    }),
                };
            }
            _ => {
                println!("  无效输入。");
            }
        }
    }
}

/// 显示和了结果
fn display_score_result(changes: &[i32; 4], yaku_names: &[String], is_tsumo: bool) {
    let win_type = if is_tsumo { "自摸" } else { "荣和" };
    let total = changes.iter().sum::<i32>().abs();
    println!("  {} | {}点", win_type, total);
    for name in yaku_names {
        println!("    · {}", name);
    }
    println!(
        "  点棒变化: 东{} 南{} 西{} 北{}",
        format_change(changes[0]),
        format_change(changes[1]),
        format_change(changes[2]),
        format_change(changes[3]),
    );
}

fn format_change(n: i32) -> String {
    if n > 0 {
        format!("+{}", n)
    } else {
        format!("{}", n)
    }
}

/// 显示最终状态
fn display_final_state(game: &GameState) {
    println!();
    for i in 0..4 {
        let p = &game.players[i];
        let wind = p.wind;
        let role = if i == 0 { " ← 你" } else { "" };
        let points_str = format!("{}", p.points);
        print!(
            "  {}({}) {:<6}{}  手牌: ",
            wind_display(wind),
            player_name(i),
            points_str,
            role,
        );
        for tile in p.hand.tiles() {
            print!("{} ", ct(*tile));
        }
        if !p.melds.is_empty() {
            print!(" 副露:");
            for meld in &p.melds {
                print!(" {}", meld);
            }
        }
        println!();
    }
}

/// 获取玩家名称
fn player_name(index: usize) -> &'static str {
    match index {
        0 => "你",
        1 => "AI-南",
        2 => "AI-西",
        3 => "AI-北",
        _ => "???",
    }
}
