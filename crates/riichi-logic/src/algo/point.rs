#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Payout {
    pub ron: i32,
    pub tsumo_non_dealer: i32,
    pub tsumo_dealer: i32,
}

impl Payout {
    #[must_use]
    pub fn lookup(is_dealer: bool, fu: u8, han: u8) -> Self {
        let (ron, tsumo_non_dealer, tsumo_dealer) = if is_dealer {
            match (fu, han) {
                (20, 2) | (40, 1) => (2000, 700, 0),
                (20, 3) | (40, 2) | (80, 1) => (3900, 1300, 0),
                (20, 4) | (40, 3) | (80, 2) => (7700, 2600, 0),
                (25, 2) | (50, 1) => (2400, 800, 0),
                (25, 3) | (50, 2) | (100, 1) => (4800, 1600, 0),
                (25, 4) | (50, 3) | (100, 2) => (9600, 3200, 0),
                (30, 1) => (1500, 500, 0),
                (30, 2) | (60, 1) => (2900, 1000, 0),
                (30, 3) | (60, 2) => (5800, 2000, 0),
                (30, 4) | (60, 3) => (11600, 3900, 0),
                (70, 1) => (3400, 1200, 0),
                (70, 2) => (6800, 2300, 0),
                (90, 1) => (4400, 1500, 0),
                (90, 2) => (8700, 2900, 0),
                (110, 1) => (5300, 1800, 0),
                (110, 2) => (10600, 3600, 0),
                (_, 5) | (40.., 4) | (70.., 3) => (12000, 4000, 0),
                (_, 6..=7) => (18000, 6000, 0),
                (_, 8..=10) => (24000, 8000, 0),
                (_, 11..=12) => (36000, 12000, 0),
                (_, 13..) => (48000, 16000, 0),
                _ => panic!("impossible: {fu} fu, {han} han"),
            }
        } else {
            match (fu, han) {
                (20, 2) | (40, 1) => (1300, 400, 700),
                (20, 3) | (40, 2) | (80, 1) => (2600, 700, 1300),
                (20, 4) | (40, 3) | (80, 2) => (5200, 1300, 2600),
                (25, 2) | (50, 1) => (1600, 400, 800),
                (25, 3) | (50, 2) | (100, 1) => (3200, 800, 1600),
                (25, 4) | (50, 3) | (100, 2) => (6400, 1600, 3200),
                (30, 1) => (1000, 300, 500),
                (30, 2) | (60, 1) => (2000, 500, 1000),
                (30, 3) | (60, 2) => (3900, 1000, 2000),
                (30, 4) | (60, 3) => (7700, 2000, 3900),
                (70, 1) => (2300, 600, 1200),
                (70, 2) => (4500, 1200, 2300),
                (90, 1) => (2900, 800, 1500),
                (90, 2) => (5800, 1500, 2900),
                (110, 1) => (3600, 900, 1800),
                (110, 2) => (7100, 1800, 3600),
                (_, 5) | (40.., 4) | (70.., 3) => (8000, 2000, 4000),
                (_, 6..=7) => (12000, 3000, 6000),
                (_, 8..=10) => (16000, 4000, 8000),
                (_, 11..=12) => (24000, 6000, 12000),
                (_, 13..) => (32000, 8000, 16000),
                _ => panic!("impossible: {fu} fu, {han} han"),
            }
        };
        Self {
            ron,
            tsumo_non_dealer,
            tsumo_dealer,
        }
    }

    #[inline]
    #[must_use]
    pub const fn yakuman(is_dealer: bool, count: i32) -> Self {
        if is_dealer {
            Self {
                ron: 48000 * count,
                tsumo_non_dealer: 16000 * count,
                tsumo_dealer: 0,
            }
        } else {
            Self {
                ron: 32000 * count,
                tsumo_non_dealer: 8000 * count,
                tsumo_dealer: 16000 * count,
            }
        }
    }

    #[inline]
    #[must_use]
    pub const fn tsumo_total(self, is_dealer: bool) -> i32 {
        if is_dealer {
            self.tsumo_non_dealer * 3
        } else {
            self.tsumo_non_dealer * 2 + self.tsumo_dealer
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_full_table_consistency() {
        let fus = [20, 25, 30, 40, 50, 60, 70, 80, 90, 100, 110];
        for &fu in &fus {
            for han in 1..=14 {
                if han == 1 && (fu < 30 || fu == 50) {
                    continue;
                }
                let base = if han >= 13 {
                    8000
                } else if han >= 11 {
                    6000
                } else if han >= 8 {
                    4000
                } else if han >= 6 {
                    3000
                } else if han >= 5 {
                    2000
                } else {
                    (fu as i32 * 2_i32.pow(2 + han)).min(2000)
                };
                let points = |mult: i32| (base * mult + 99) / 100 * 100;

                let p = Payout::lookup(false, fu as u8, han as u8);
                assert_eq!(p.tsumo_non_dealer, points(1), "{fu}/{han}");
                assert_eq!(p.tsumo_dealer, points(2), "{fu}/{han}");
                assert_eq!(p.ron, points(4), "{fu}/{han}");

                let p = Payout::lookup(true, fu as u8, han as u8);
                assert_eq!(p.tsumo_non_dealer, points(2), "{fu}/{han}");
                assert_eq!(p.ron, points(6), "{fu}/{han}");
            }
        }
    }

    #[test]
    fn test_specific_cases() {
        assert_eq!(
            Payout::lookup(false, 30, 1),
            Payout {
                ron: 1000,
                tsumo_non_dealer: 300,
                tsumo_dealer: 500
            }
        );
        assert_eq!(
            Payout::lookup(true, 40, 3),
            Payout {
                ron: 7700,
                tsumo_non_dealer: 2600,
                tsumo_dealer: 0
            }
        );
    }

    #[test]
    fn test_yakuman_values() {
        assert_eq!(
            Payout::yakuman(false, 1),
            Payout {
                ron: 32000,
                tsumo_non_dealer: 8000,
                tsumo_dealer: 16000
            }
        );
        assert_eq!(
            Payout::yakuman(true, 2),
            Payout {
                ron: 96000,
                tsumo_non_dealer: 32000,
                tsumo_dealer: 0
            }
        );
    }
}
