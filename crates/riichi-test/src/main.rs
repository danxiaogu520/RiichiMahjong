mod parse;

use std::io::{self, Write};

use riichi_core::tile::TileType;
use riichi_logic::acceptance::{analyze_acceptance, analyze_discard, VisibleTiles};
use riichi_logic::analysis::{analyze_wait_tiles, is_winning};
use riichi_logic::dora::calculate_dora;
use riichi_logic::fu::calculate_fu;
use riichi_logic::scoring::calculate_points;
use riichi_logic::shanten::ShantenCalculator;
use riichi_logic::types::{TileCounts, WinContext};
use riichi_logic::win_check::{check_win, decompose_hand};

fn main() {
    println!("riichi-test — 交互式测试工具");
    println!("输入 help 查看可用命令, quit 退出\n");

    let mut calc = ShantenCalculator::new();
    let mut input = String::new();

    loop {
        print!("> ");
        io::stdout().flush().ok();
        input.clear();
        if io::stdin().read_line(&mut input).is_err() {
            break;
        }
        let line = input.trim();
        if line.is_empty() {
            continue;
        }

        let parts: Vec<&str> = line.splitn(2, ' ').collect();
        let cmd = parts[0];
        let args = parts.get(1).unwrap_or(&"");

        let result = match cmd {
            "help" | "h" => cmd_help(),
            "quit" | "q" | "exit" => break,
            "shanten" | "s" => cmd_shanten(&mut calc, args),
            "win" | "w" => cmd_win(args),
            "decompose" | "d" => cmd_decompose(args),
            "wait" | "wt" => cmd_wait(args),
            "dora" | "dr" => cmd_dora(args),
            "discard" | "dc" => cmd_discard(&mut calc, args),
            "acceptance" | "acc" => cmd_acceptance(&mut calc, args),
            "fu" => cmd_fu(args),
            "points" | "pt" => cmd_points(args),
            "check" | "ck" => cmd_check(args),
            _ => Err(format!("未知命令: '{}', 输入 help 查看帮助", cmd)),
        };

        match result {
            Ok(output) => println!("{}", output),
            Err(e) => println!("错误: {}", e),
        }
    }
}

fn cmd_help() -> Result<String, String> {
    Ok(r#"可用命令:
  shanten (s)  <手牌>              计算向听数 (13/14张)
  win (w)      <手牌>              判断是否和了形 (14张)
  decompose (d)<手牌>              分解手牌 (14张)
  wait (wt)    <手牌>              听牌分析 (13张)
  dora (dr)    <牌> --indicator <指示牌>  宝牌计算
  discard (dc) <手牌>              打牌分析 (14张, 选最优)
  acceptance (acc) <手牌>          进张分析 (13张)
  fu           <参数>              符数计算
  points (pt)  <翻> <符> <庄闲> <自摸荣和>  点数计算
  check (ck)   <手牌> <和了牌>     完整和了检查

手牌格式: "1m 2m 3m 4p 5p 6p 7s 8s 9s 1z 1z 1z 2z 2z"
字牌: 1z=东 2z=南 3z=西 4z=北 5z=白 6z=发 7z=中
重复: "1m×3" = 3张1m
quit (q) 退出"#
        .to_string())
}

fn cmd_shanten(calc: &mut ShantenCalculator, args: &str) -> Result<String, String> {
    let tiles = parse::parse_tiles(args)?;
    if tiles.len() != 13 && tiles.len() != 14 {
        return Err(format!("需要13或14张牌, 得到{}张", tiles.len()));
    }
    let shanten = calc.calculate(&tiles);
    Ok(format!("向听: {}", shanten))
}

fn cmd_win(args: &str) -> Result<String, String> {
    let tiles = parse::parse_tiles(args)?;
    if tiles.len() != 14 {
        return Err(format!("需要14张牌, 得到{}张", tiles.len()));
    }
    let mut counts = TileCounts::from_tiles(&tiles);
    let result = is_winning(&mut counts);
    Ok(format!("和了: {}", if result { "true" } else { "false" }))
}

fn cmd_decompose(args: &str) -> Result<String, String> {
    let tile_types = parse::parse_tile_types(args)?;
    if tile_types.len() != 14 {
        return Err(format!("需要14张牌, 得到{}张", tile_types.len()));
    }
    let hands = decompose_hand(&tile_types);
    if hands.is_empty() {
        return Ok("无有效分解".to_string());
    }
    let mut out = format!("{}种分解:\n", hands.len());
    for (i, hand) in hands.iter().enumerate() {
        out.push_str(&format!("  {}: {:?}\n", i + 1, hand));
    }
    Ok(out)
}

fn cmd_wait(args: &str) -> Result<String, String> {
    let tiles = parse::parse_tiles(args)?;
    if tiles.len() != 13 {
        return Err(format!("需要13张牌, 得到{}张", tiles.len()));
    }
    let waits = analyze_wait_tiles(&tiles);
    if waits.is_empty() {
        return Ok("未听牌".to_string());
    }
    let mut out = format!("听{}种牌:\n", waits.len());
    for w in &waits {
        out.push_str(&format!("  {} {:?}\n", w.tile_type, w.wait_types));
    }
    Ok(out)
}

fn cmd_dora(args: &str) -> Result<String, String> {
    let parts: Vec<&str> = args.split("--indicator").collect();
    if parts.len() != 2 {
        return Err("用法: dora <牌> --indicator <指示牌>".to_string());
    }
    let tiles = parse::parse_tiles(parts[0].trim())?;
    let indicators = parse::parse_tile_types(parts[1].trim())?;
    let result = calculate_dora(&tiles, &indicators, &[], false);
    Ok(format!(
        "宝牌: {}  赤宝牌: {}  里宝牌: {}  合计: {}",
        result.dora,
        result.aka_dora,
        result.ura_dora,
        result.total()
    ))
}

fn cmd_discard(calc: &mut ShantenCalculator, args: &str) -> Result<String, String> {
    let tiles = parse::parse_tiles(args)?;
    if tiles.len() != 14 {
        return Err(format!("需要14张牌, 得到{}张", tiles.len()));
    }
    let visible = VisibleTiles::new();
    let options = analyze_discard(calc, &tiles, &visible);
    if options.is_empty() {
        return Ok("无分析结果".to_string());
    }
    let mut out = String::new();
    for opt in &options {
        out.push_str(&format!(
            "打 {}: 向听{} 进张{}种{}张 改良{}种{}张\n",
            opt.tile,
            opt.shanten,
            opt.acceptance_types,
            opt.acceptance_copies,
            opt.improvement_types,
            opt.improvement_copies,
        ));
    }
    Ok(out)
}

fn cmd_acceptance(calc: &mut ShantenCalculator, args: &str) -> Result<String, String> {
    let tiles = parse::parse_tiles(args)?;
    if tiles.len() != 13 {
        return Err(format!("需要13张牌, 得到{}张", tiles.len()));
    }
    let visible = VisibleTiles::new();
    let (acceptance, improvement, shanten) = analyze_acceptance(calc, &tiles, &visible);
    let mut out = format!("向听: {}\n", shanten);
    if !acceptance.is_empty() {
        let total: usize = acceptance.iter().map(|a| a.copies).sum();
        out.push_str(&format!("进张: {}种{}张\n", acceptance.len(), total));
        for a in &acceptance {
            out.push_str(&format!("  {} ×{}\n", a.tile, a.copies));
        }
    }
    if !improvement.is_empty() {
        let total: usize = improvement.iter().map(|a| a.copies).sum();
        out.push_str(&format!("改良: {}种{}张\n", improvement.len(), total));
        for a in &improvement {
            out.push_str(&format!("  {} ×{}\n", a.tile, a.copies));
        }
    }
    Ok(out)
}

fn cmd_fu(args: &str) -> Result<String, String> {
    let parts: Vec<&str> = args.split_whitespace().collect();
    if parts.len() < 5 {
        return Err("用法: fu <手牌> <tsumo|ron> <seat_wind> <field_wind>\n  例: fu 1m 1m 1m 2m 3m tsumo 1z 1z".to_string());
    }
    let hand_str = parts[..parts.len() - 3].join(" ");
    let win_type = parts[parts.len() - 3];
    let seat_str = parts[parts.len() - 2];
    let field_str = parts[parts.len() - 1];

    let tile_types = parse::parse_tile_types(&hand_str)?;
    if tile_types.len() != 14 {
        return Err(format!("手牌需要14张, 得到{}张", tile_types.len()));
    }

    let is_tsumo = match win_type {
        "tsumo" | "t" => true,
        "ron" | "r" => false,
        _ => return Err("和了方式: tsumo 或 ron".to_string()),
    };

    let seat_wind = parse::parse_tile_types(seat_str)?
        .into_iter()
        .next()
        .ok_or("需要自风")?;
    let field_wind = parse::parse_tile_types(field_str)?
        .into_iter()
        .next()
        .ok_or("需要场风")?;

    let hands = decompose_hand(&tile_types);
    let hand = hands.into_iter().next().ok_or("无法分解手牌")?;

    let fu = calculate_fu(&hand, &[], &[], is_tsumo, seat_wind, field_wind);
    Ok(format!("符: {}", fu))
}

fn cmd_points(args: &str) -> Result<String, String> {
    let parts: Vec<&str> = args.split_whitespace().collect();
    if parts.len() < 2 {
        return Err("用法: points <翻数> <符数> [庄家座位] [和了座位] [is_tsumo]\n  例: points 4 30    (默认闲家荣和)".to_string());
    }

    let han: u8 = parts[0].parse().map_err(|_| "翻数必须是数字")?;
    let fu: u32 = parts[1].parse().map_err(|_| "符数必须是数字")?;
    let dealer: usize = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
    let winner: usize = parts.get(3).and_then(|s| s.parse().ok()).unwrap_or(1);
    let is_tsumo = parts
        .get(4)
        .map(|s| *s == "t" || *s == "tsumo")
        .unwrap_or(false);

    let changes = calculate_points(han, fu, 0, winner, dealer, 0, 0, is_tsumo);
    let mut out = format!("{}翻{}符", han, fu);
    if winner == dealer {
        out.push_str(" (庄家)");
    } else {
        out.push_str(" (闲家)");
    }
    if is_tsumo {
        out.push_str(" 自摸:");
    } else {
        out.push_str(" 荣和:");
    }
    for (i, &c) in changes.iter().enumerate() {
        if c != 0 {
            out.push_str(&format!(" P{}={:+}", i, c));
        }
    }
    Ok(out)
}

fn cmd_check(args: &str) -> Result<String, String> {
    let parts: Vec<&str> = args.split_whitespace().collect();
    if parts.len() < 2 {
        return Err("用法: check <手牌13张> <和了牌>\n  例: check 1m 2m 3m 4m 5m 6m 7m 8m 9m 1p 2p 3p 4p 4p".to_string());
    }

    let hand_str = parts[..parts.len() - 1].join(" ");
    let win_tile_str = parts[parts.len() - 1];

    let hand_tiles = parse::parse_tiles(&hand_str)?;
    if hand_tiles.len() != 13 {
        return Err(format!("手牌需要13张, 得到{}张", hand_tiles.len()));
    }
    let win_tile = parse::parse_tiles(win_tile_str)?
        .into_iter()
        .next()
        .ok_or("需要和了牌")?;

    let mut all_tiles = hand_tiles.clone();
    all_tiles.push(win_tile);

    let hand_tile_types: Vec<TileType> = hand_tiles.iter().map(|t| t.tile_type()).collect();
    let ctx = WinContext {
        is_tsumo: true,
        is_riichi: false,
        is_double_riichi: false,
        is_ippatsu: false,
        is_rinshan: false,
        is_chankan: false,
        is_haitei: false,
        is_houtei: false,
        seat_wind: TileType::EAST,
        field_wind: TileType::EAST,
        melds: vec![],
        dora_indicators: vec![],
        ura_dora_indicators: vec![],
        dealer: 0,
        winner: 0,
        loser: None,
        honba: 0,
        riichi_sticks: 0,
    };

    match check_win(&all_tiles, &hand_tile_types, &ctx, false, win_tile) {
        Some(result) => {
            let mut out = format!("和了! {}翻{}符\n", result.total_han, result.fu);
            out.push_str("役种:\n");
            for y in &result.yaku_results {
                out.push_str(&format!("  {:?} ({}翻)\n", y.yaku, y.han));
            }
            out.push_str(&format!("点数: {:?}", result.points));
            Ok(out)
        }
        None => Ok("未和了".to_string()),
    }
}
