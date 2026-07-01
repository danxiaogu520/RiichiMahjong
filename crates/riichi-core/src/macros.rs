#[macro_export]
macro_rules! tile_id {
    (1m) => { 0_u8 };
    (2m) => { 1_u8 };
    (3m) => { 2_u8 };
    (4m) => { 3_u8 };
    (5m) => { 4_u8 };
    (6m) => { 5_u8 };
    (7m) => { 6_u8 };
    (8m) => { 7_u8 };
    (9m) => { 8_u8 };

    (1p) => { 9_u8 };
    (2p) => { 10_u8 };
    (3p) => { 11_u8 };
    (4p) => { 12_u8 };
    (5p) => { 13_u8 };
    (6p) => { 14_u8 };
    (7p) => { 15_u8 };
    (8p) => { 16_u8 };
    (9p) => { 17_u8 };

    (1s) => { 18_u8 };
    (2s) => { 19_u8 };
    (3s) => { 20_u8 };
    (4s) => { 21_u8 };
    (5s) => { 22_u8 };
    (6s) => { 23_u8 };
    (7s) => { 24_u8 };
    (8s) => { 25_u8 };
    (9s) => { 26_u8 };

    (E) => { 27_u8 };
    (S) => { 28_u8 };
    (W) => { 29_u8 };
    (N) => { 30_u8 };
    (P) => { 31_u8 };
    (F) => { 32_u8 };
    (C) => { 33_u8 };

    (5mr) => { 34_u8 };
    (5pr) => { 35_u8 };
    (5sr) => { 36_u8 };

    (?) => { 37_u8 };

    ($first:tt, $($left:tt),*) => {
        [$crate::tile_id!($first), $($crate::tile_id!($left)),*]
    };

    ($($_:tt)*) => {
        ::std::compile_error!("invalid tile pattern")
    }
}

/// Macro for making const tile IDs in usize.
#[macro_export]
macro_rules! tile_index {
    ($s:tt) => {
        $crate::tile_id!($s) as usize
    };
    ($first:tt, $($left:tt),*) => {
        [$crate::tile_index!($first), $($crate::tile_index!($left)),*]
    };
}

/// Macro for making const [`Tile37`] values.
#[macro_export]
macro_rules! tile {
    ($s:tt) => {
        $crate::tile37::Tile37::new_unchecked($crate::tile_id!($s))
    };
    ($first:tt, $($left:tt),*) => {
        [$crate::tile!($first), $($crate::tile!($left)),*]
    };
}

/// Macro for matching a `u8` against const tile ID patterns.
#[allow(clippy::manual_range_patterns)]
#[macro_export]
macro_rules! tile_matches {
    ($o:expr, $($s:tt)|* $(|)?) => {
        matches!($o, $($crate::tile_id!($s))|*)
    };
}

/// Macro for making non-const [`Tile37`] values.
///
/// # Panics
/// Panics in debug mode if the input is not a valid tile.
#[macro_export]
macro_rules! tile_unchecked {
    ($($id:tt)*) => {{
        #[cfg(debug_assertions)]
        { $crate::tile37::Tile37::try_from($($id)*).unwrap() }
        #[cfg(not(debug_assertions))]
        { $crate::tile37::Tile37::new_unchecked(($($id)*) as u8) }
    }};
}

#[cfg(doctest)]
#[doc = "```rust,compile_fail"]
/// use riichi_core::tile_id;
///
/// let t = tile_id!(0m);
/// ```
struct _CompileFail;

#[cfg(test)]
mod tests {
    #[test]
    fn test_syntax() {
        assert_eq!(tile!(3s).index(), tile_index!(3s));
        assert_eq!(tile!(5sr).raw(), tile_id!(5sr));
        assert_eq!(tile!(5m).red().raw(), tile_id!(5mr));

        assert_eq!(tile_id![8m,], [tile_id!(8m)]);
        assert_eq!(tile_index![P,], [tile_index!(P)]);
        assert_eq!(tile![N,], [tile!(N)]);

        assert_eq!(
            tile_id![2p, 5pr, S],
            [tile_id!(2p), tile_id!(5pr), tile_id!(S)]
        );
        assert_eq!(
            tile_index![E, 6m, ?],
            [tile_index!(E), tile_index!(6m), tile_index!(?)]
        );
        assert_eq!(tile![1m, 2p, 9s], [tile!(1m), tile!(2p), tile!(9s)]);

        assert!(tile_matches!(tile!(E).raw(), 1m | E | ? | 5mr));
        assert!(!tile_matches!(tile!(3m).raw(), 1s | 7p | P));
    }

    #[test]
    fn test_completeness() {
        assert_eq!(tile!(1m).to_string(), "1m");
        assert_eq!(tile!(2m).to_string(), "2m");
        assert_eq!(tile!(3m).to_string(), "3m");
        assert_eq!(tile!(4m).to_string(), "4m");
        assert_eq!(tile!(5m).to_string(), "5m");
        assert_eq!(tile!(6m).to_string(), "6m");
        assert_eq!(tile!(7m).to_string(), "7m");
        assert_eq!(tile!(8m).to_string(), "8m");
        assert_eq!(tile!(9m).to_string(), "9m");

        assert_eq!(tile!(1p).to_string(), "1p");
        assert_eq!(tile!(2p).to_string(), "2p");
        assert_eq!(tile!(3p).to_string(), "3p");
        assert_eq!(tile!(4p).to_string(), "4p");
        assert_eq!(tile!(5p).to_string(), "5p");
        assert_eq!(tile!(6p).to_string(), "6p");
        assert_eq!(tile!(7p).to_string(), "7p");
        assert_eq!(tile!(8p).to_string(), "8p");
        assert_eq!(tile!(9p).to_string(), "9p");

        assert_eq!(tile!(1s).to_string(), "1s");
        assert_eq!(tile!(2s).to_string(), "2s");
        assert_eq!(tile!(3s).to_string(), "3s");
        assert_eq!(tile!(4s).to_string(), "4s");
        assert_eq!(tile!(5s).to_string(), "5s");
        assert_eq!(tile!(6s).to_string(), "6s");
        assert_eq!(tile!(7s).to_string(), "7s");
        assert_eq!(tile!(8s).to_string(), "8s");
        assert_eq!(tile!(9s).to_string(), "9s");

        assert_eq!(tile!(E).to_string(), "E");
        assert_eq!(tile!(S).to_string(), "S");
        assert_eq!(tile!(W).to_string(), "W");
        assert_eq!(tile!(N).to_string(), "N");
        assert_eq!(tile!(P).to_string(), "P");
        assert_eq!(tile!(F).to_string(), "F");
        assert_eq!(tile!(C).to_string(), "C");

        assert_eq!(tile!(5mr).to_string(), "5mr");
        assert_eq!(tile!(5pr).to_string(), "5pr");
        assert_eq!(tile!(5sr).to_string(), "5sr");

        assert_eq!(tile!(?).to_string(), "?");
    }
}
