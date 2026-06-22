use riichi_core::tile::{Tile, TileType};

pub fn parse_tile_types(input: &str) -> Result<Vec<TileType>, String> {
    let tokens = tokenize(input)?;
    let mut result = Vec::new();
    for token in tokens {
        let (tt, count) = parse_token(&token)?;
        for _ in 0..count {
            result.push(tt);
        }
    }
    Ok(result)
}

pub fn parse_tiles(input: &str) -> Result<Vec<Tile>, String> {
    let tokens = tokenize(input)?;
    let mut type_counts = [0u8; 34];
    let mut result = Vec::new();
    for token in tokens {
        let (tt, count) = parse_token(&token)?;
        for _ in 0..count {
            let copy = type_counts[tt.0 as usize];
            if copy >= 4 {
                return Err(format!("{}: 超过4张", tt));
            }
            result.push(tt.with_copy(copy));
            type_counts[tt.0 as usize] += 1;
        }
    }
    Ok(result)
}

fn tokenize(input: &str) -> Result<Vec<String>, String> {
    let input = input.trim();
    if input.is_empty() {
        return Err("输入为空".to_string());
    }

    let mut tokens = Vec::new();
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if chars[i].is_whitespace() {
            i += 1;
            continue;
        }

        if chars[i] == '×' || chars[i] == 'x' || chars[i] == 'X' {
            return Err(format!("位置 {}: 重复标记 '{}' 前缺少牌", i, chars[i]));
        }

        if i + 1 >= chars.len() {
            return Err(format!("位置 {}: 不完整的牌描述", i));
        }

        let rank = chars[i];
        let suit = chars[i + 1];

        if !rank.is_ascii_digit() {
            return Err(format!("位置 {}: 期望数字, 得到 '{}'", i, rank));
        }

        let _base = match suit {
            'm' | 'M' => 0u8,
            'p' | 'P' => 9u8,
            's' | 'S' => 18u8,
            'z' | 'Z' => 27u8,
            _ => return Err(format!("位置 {}: 未知花色 '{}', 期望 m/p/s/z", i + 1, suit)),
        };

        let rank_num = rank.to_digit(10).unwrap() as u8;
        if rank_num == 0 {
            return Err(format!("位置 {}: 牌面数字不能为0", i));
        }
        if suit == 'z' || suit == 'Z' {
            if rank_num > 7 {
                return Err(format!("位置 {}: 字牌范围 1z-7z, 得到 '{}z'", i, rank_num));
            }
        } else if rank_num > 9 {
            return Err(format!(
                "位置 {}: 数牌范围 1-9, 得到 '{}{}'",
                i, rank_num, suit
            ));
        }

        let tt_str = format!("{}{}", rank, suit);
        i += 2;

        let count = if i < chars.len() && (chars[i] == '×' || chars[i] == 'x' || chars[i] == 'X') {
            i += 1;
            if i >= chars.len() || !chars[i].is_ascii_digit() {
                return Err(format!("位置 {}: 重复标记后缺少数字", i));
            }
            let c = chars[i].to_digit(10).unwrap() as u8;
            i += 1;
            if c == 0 || c > 4 {
                return Err(format!("重复次数必须 1-4, 得到 {}", c));
            }
            c
        } else {
            1
        };

        tokens.push(format!(
            "{}{}",
            tt_str,
            if count > 1 {
                format!("×{}", count)
            } else {
                String::new()
            }
        ));
        for _ in 0..count {
            let _ = tokens.pop();
            tokens.push(tt_str.clone());
        }
    }

    Ok(tokens)
}

fn parse_token(token: &str) -> Result<(TileType, u8), String> {
    let chars: Vec<char> = token.chars().collect();
    if chars.len() != 2 {
        return Err(format!("无效牌描述: '{}'", token));
    }

    let rank = chars[0].to_digit(10).unwrap() as u8;
    let suit = chars[1];

    let base: u8 = match suit {
        'm' | 'M' => 0,
        'p' | 'P' => 9,
        's' | 'S' => 18,
        'z' | 'Z' => 27,
        _ => unreachable!(),
    };

    let index = base + (rank - 1);
    Ok((TileType(index), 1))
}
