use riichi_core::tile::{Tile, TileType};

pub fn parse_tile_types(input: &str) -> Result<Vec<TileType>, String> {
    let groups = parse_groups(input)?;
    let mut result = Vec::new();
    for (digits, suit) in groups {
        let base: u8 = match suit {
            'm' => 0,
            'p' => 9,
            's' => 18,
            'z' => 27,
            _ => return Err(format!("未知花色: {}", suit)),
        };
        for ch in digits.chars() {
            let rank = ch.to_digit(10).unwrap() as u8;
            if rank == 0 {
                return Err("牌面数字不能为0".into());
            }
            if suit == 'z' && rank > 7 {
                return Err(format!("字牌范围 1-7z, 得到 {}z", rank));
            }
            if suit != 'z' && rank > 9 {
                return Err(format!("数牌范围 1-9, 得到 {}{}", rank, suit));
            }
            result.push(TileType(base + rank - 1));
        }
    }
    Ok(result)
}

pub fn parse_tiles(input: &str) -> Result<Vec<Tile>, String> {
    let tile_types = parse_tile_types(input)?;
    let mut type_counts = [0u8; 34];
    let mut result = Vec::new();
    for tt in tile_types {
        let copy = type_counts[tt.0 as usize];
        if copy >= 4 {
            return Err(format!("{}: 超过4张", tt));
        }
        result.push(tt.with_copy(copy));
        type_counts[tt.0 as usize] += 1;
    }
    Ok(result)
}

fn parse_groups(input: &str) -> Result<Vec<(String, char)>, String> {
    let input = input.trim();
    if input.is_empty() {
        return Err("输入为空".into());
    }

    let chars: Vec<char> = input.chars().collect();
    let mut groups = Vec::new();
    let mut i = 0;

    while i < chars.len() {
        let mut digits = String::new();
        while i < chars.len() && chars[i].is_ascii_digit() {
            digits.push(chars[i]);
            i += 1;
        }
        if digits.is_empty() {
            return Err(format!("位置 {}: 期望数字", i));
        }
        if i >= chars.len() || !matches!(chars[i], 'm' | 'p' | 's' | 'z' | 'M' | 'P' | 'S' | 'Z') {
            return Err(format!("位置 {}: 数字后缺少花色字母 (m/p/s/z)", i));
        }
        let suit = chars[i].to_ascii_lowercase();
        i += 1;
        groups.push((digits, suit));
    }

    if groups.is_empty() {
        return Err("输入为空".into());
    }
    Ok(groups)
}
