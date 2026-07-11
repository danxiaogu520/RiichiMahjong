mod parse;

use std::io::{self, Write};

use riichi_core::tile::TileType;
use riichi_logic::acceptance::{analyze_acceptance, analyze_discard, VisibleTiles};
use riichi_logic::analysis::{analyze_wait_tiles, is_winning};
use riichi_logic::dora::calculate_dora;
use riichi_logic::fu::calculate_fu;
use riichi_logic::scoring::calculate_points;
use riichi_logic::shanten::ShantenCalculator;
use riichi_logic::types::TileCounts;
use riichi_logic::win_check::decompose_hand;

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
            "analyze" | "a" => cmd_analyze(&mut calc, args),
            "dora" | "dr" => cmd_dora(args),
            "fu" => cmd_fu(args),
            "points" | "pt" => cmd_points(args),
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
  analyze (a)  <手牌>               智能分析 (自动判断3n+1/3n+2)
  dora (dr)    <牌> --indicator <指示牌>  宝牌计算
  fu           <参数>              符数计算
  points (pt)  <翻> <符> [庄闲] [自摸荣和]  点数计算

手牌格式: "12345m445p45678s" 或 "12345m445p45678s1z"
  数字后跟花色字母: m=万 p=筒 s=索 z=字
  字牌: 1z=东 2z=南 3z=西 4z=北 5z=白 6z=发 7z=中
quit (q) 退出"#
        .to_string())
}

fn cmd_analyze(calc: &mut ShantenCalculator, args: &str) -> Result<String, String> {
    let tiles = parse::parse_tiles(args)?;
    let n = tiles.len();

    if n % 3 == 0 {
        return Err(format!("{}张牌无法分析 (3n长度)", n));
    }

    if n % 3 == 2 {
        analyze_3n2(calc, &tiles)
    } else {
        analyze_3n1(calc, &tiles)
    }
}

fn analyze_3n2(
    calc: &mut ShantenCalculator,
    tiles: &[riichi_core::tile::Tile],
) -> Result<String, String> {
    let tile_types: Vec<TileType> = tiles.iter().map(|t| t.tile_type()).collect();
    let mut counts = TileCounts::from_tiles(tiles);

    if is_winning(&mut counts) {
        let hands = decompose_hand(&tile_types);
        let mut out = format!("和了! ({}张)\n", tiles.len());
        if !hands.is_empty() {
            out.push_str("拆解:\n");
            for (i, hand) in hands.iter().enumerate() {
                out.push_str(&format!("  {}: ", i + 1));
                // 雀头
                out.push_str(&format!("{}{} ", fmt_tt(hand.jantai), fmt_tt(hand.jantai)));
                // 面子
                for m in &hand.mentsu {
                    match m.kind {
                        riichi_logic::types::MentsuKind::Shuntsu => {
                            let t0 = m.tile_type;
                            let t1 = TileType(t0.0 + 1);
                            let t2 = TileType(t0.0 + 2);
                            out.push_str(&format!("{}{}{} ", fmt_tt(t0), fmt_tt(t1), fmt_tt(t2)));
                        }
                        riichi_logic::types::MentsuKind::Koutsu => {
                            out.push_str(&format!(
                                "{}{}{} ",
                                fmt_tt(m.tile_type),
                                fmt_tt(m.tile_type),
                                fmt_tt(m.tile_type)
                            ));
                        }
                    }
                }
                // 手牌类型标注
                match hand.hand_type {
                    riichi_logic::types::HandType::Standard => {}
                    riichi_logic::types::HandType::SevenPairs => {
                        out.push_str("[七对子]");
                    }
                    riichi_logic::types::HandType::Kokushi => {
                        out.push_str("[国士无双]");
                    }
                }
                out.push('\n');
            }
        }
        return Ok(out);
    }

    let shanten = calc.calculate(tiles);
    let visible = VisibleTiles::new();
    let options = analyze_discard(calc, tiles, &visible);

    let mut out = format!("未和了  向听: {}\n", shanten);
    if !options.is_empty() {
        out.push_str("打牌分析:\n");
        for opt in &options {
            out.push_str(&format!(
                "  打 {}: 向听{} 进张{}种{}张 改良{}种{}张\n",
                opt.tile,
                opt.shanten,
                opt.acceptance_types,
                opt.acceptance_copies,
                opt.improvement_types,
                opt.improvement_copies,
            ));
        }
    }
    Ok(out)
}

fn analyze_3n1(
    calc: &mut ShantenCalculator,
    tiles: &[riichi_core::tile::Tile],
) -> Result<String, String> {
    let waits = analyze_wait_tiles(tiles);

    if !waits.is_empty() {
        let mut out = format!("听牌! ({}张)\n", tiles.len());
        out.push_str(&format!("听{}种牌:\n", waits.len()));
        for w in &waits {
            out.push_str(&format!("  {} {:?}\n", w.tile_type, w.wait_types));
        }
        return Ok(out);
    }

    let shanten = calc.calculate(tiles);
    let visible = VisibleTiles::new();
    let (acceptance, improvement, _) = analyze_acceptance(calc, tiles, &visible);

    let mut out = format!("未听牌  向听: {}\n", shanten);
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

fn fmt_tt(tt: TileType) -> String {
    let rank = tt.rank().0;
    match tt.suit() {
        riichi_core::tile::Suit::Man => format!("{}m", rank),
        riichi_core::tile::Suit::Pin => format!("{}p", rank),
        riichi_core::tile::Suit::Sou => format!("{}s", rank),
        riichi_core::tile::Suit::Wind | riichi_core::tile::Suit::Dragon => format!("{}z", rank),
    }
}

fn cmd_dora(args: &str) -> Result<String, String> {
    let parts: Vec<&str> = args.split("--indicator").collect();
    if parts.len() != 2 {
        return Err("用法: dora <牌> --indicator <指示牌>".into());
    }
    let tiles = parse::parse_tiles(parts[0].trim())?;
    let indicators = parse::parse_tile_types(parts[1].trim())?;
    let result = calculate_dora(&tiles, &indicators, &[], false, [1, 1, 1]);
    Ok(format!(
        "宝牌: {}  赤宝牌: {}  里宝牌: {}  合计: {}",
        result.dora,
        result.aka_dora,
        result.ura_dora,
        result.total()
    ))
}

fn cmd_fu(args: &str) -> Result<String, String> {
    let parts: Vec<&str> = args.split_whitespace().collect();
    if parts.len() < 5 {
        return Err("用法: fu <手牌> <tsumo|ron> <seat_wind> <field_wind>".into());
    }
    let hand_str = parts[..parts.len() - 3].join("");
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
        _ => return Err("和了方式: tsumo 或 ron".into()),
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
        return Err("用法: points <翻数> <符数> [庄家座位] [和了座位] [tsumo]".into());
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
