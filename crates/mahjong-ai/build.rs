use std::fs::File;
use std::io::Write;

const MAX_SHT: u8 = 14;

#[derive(Clone, Copy)]
struct Delta {
    a: i32,
    b: i32,
    c: i32,
    h: i32,
    m: i32,
}

fn chmin(x: &mut u8, y: u8) {
    if *x > y {
        *x = y;
    }
}

fn dp<const N: usize>(hand: &[i32; N], deltas: &[Delta]) -> [u8; 10] {
    let mut table = vec![vec![vec![vec![[MAX_SHT; 5]; 2]; 5]; 5]; N + 1];
    table[0][0][0][0][0] = 0;

    for n in 0..N {
        for delta in deltas {
            let a_max = 4 - delta.a;
            if a_max < 0 {
                continue;
            }
            for a in 0..=a_max as usize {
                let b_max = std::cmp::min(4 - delta.b, a as i32);
                if b_max < 0 {
                    continue;
                }
                for b in 0..=b_max as usize {
                    let h_max = 1 - delta.h;
                    if h_max < 0 {
                        continue;
                    }
                    for h in 0..=h_max as usize {
                        let m_max = 4 - delta.m;
                        if m_max < 0 {
                            continue;
                        }
                        for m in 0..=m_max as usize {
                            let tmp = table[n][a][b][h][m];
                            if tmp == MAX_SHT {
                                continue;
                            }
                            let na = b + delta.b as usize;
                            let nc = delta.c as usize;
                            let nh = h + delta.h as usize;
                            let nm = m + delta.m as usize;
                            let cost =
                                std::cmp::max(a as i32 + delta.a - hand[n], 0) as u8;
                            chmin(&mut table[n + 1][na][nc][nh][nm], tmp + cost);
                        }
                    }
                }
            }
        }
    }

    let mut sht = [0u8; 10];
    for m in 0..5 {
        sht[m] = table[N][0][0][0][m];
        sht[m + 5] = table[N][0][0][1][m];
    }
    sht
}

fn deal<const N: usize>(n: usize, hand: &mut [i32; N], results: &mut Vec<[i32; N]>) {
    if n >= N {
        results.push(*hand);
        return;
    }
    for i in 0..=4 {
        hand[n] = i;
        deal::<N>(n + 1, hand, results);
    }
}

fn main() {
    // Generate index_s.bin (number suits, 9 tiles)
    {
        let number_deltas = vec![
            Delta { a: 0, b: 0, c: 0, h: 0, m: 0 },
            Delta { a: 1, b: 1, c: 1, h: 0, m: 1 },
            Delta { a: 2, b: 2, c: 2, h: 0, m: 2 },
            Delta { a: 3, b: 0, c: 0, h: 0, m: 1 },
            Delta { a: 4, b: 1, c: 1, h: 0, m: 2 },
            Delta { a: 2, b: 0, c: 0, h: 1, m: 0 },
            Delta { a: 3, b: 1, c: 1, h: 1, m: 1 },
            Delta { a: 4, b: 2, c: 2, h: 1, m: 2 },
        ];

        let mut hands = Vec::new();
        let mut hand = [0i32; 9];
        deal::<9>(0, &mut hand, &mut hands);

        let out_dir = std::env::var("OUT_DIR").unwrap();
        let path = format!("{}/index_s.bin", out_dir);
        let mut file = File::create(&path).unwrap();

        for h in &hands {
            let dist = dp::<9>(h, &number_deltas);
            file.write_all(&dist).unwrap();
        }

        println!("cargo:warning=index_s.bin: {} entries", hands.len());
    }

    // Generate index_h.bin (honor tiles, 7 tiles)
    {
        let honor_deltas = vec![
            Delta { a: 0, b: 0, c: 0, h: 0, m: 0 },
            Delta { a: 3, b: 0, c: 0, h: 0, m: 1 },
            Delta { a: 2, b: 0, c: 0, h: 1, m: 0 },
        ];

        let mut hands = Vec::new();
        let mut hand = [0i32; 7];
        deal::<7>(0, &mut hand, &mut hands);

        let out_dir = std::env::var("OUT_DIR").unwrap();
        let path = format!("{}/index_h.bin", out_dir);
        let mut file = File::create(&path).unwrap();

        for h in &hands {
            let dist = dp::<7>(h, &honor_deltas);
            file.write_all(&dist).unwrap();
        }

        println!("cargo:warning=index_h.bin: {} entries", hands.len());
    }
}
