//! The severity floor, as plain arithmetic with no dependencies.
//!
//! `build.rs` includes this file to derive the shipped severity table from
//! `data/measured.json` at build time, and `measured.rs` includes it so the
//! runtime record (the k/n and interval a finding prints) is computed by the
//! same code. The policy (P2R preregistration §5, Amendment 2):
//!
//! * `error` only when a **fresh** cell has a Wilson 95% lower bound of at
//!   least 0.80 with n >= 16;
//! * `warn` when a fresh cell has a lower bound of at least 0.50 with n >= 10;
//! * `note` otherwise, including every in-sample cell and every unmeasured one.
//!
//! n is the number of decided labels, TP + FP; an `unclear` label is not
//! counted either way.

/// z for a two-sided 95% interval, as committed in the preregistration.
pub const Z: f64 = 1.959964;
/// The `error` floor: fresh lower bound and minimum n.
pub const ERROR_LOWER: f64 = 0.80;
pub const ERROR_MIN_N: u32 = 16;
/// The `warn` floor: fresh lower bound and minimum n.
pub const WARN_LOWER: f64 = 0.50;
pub const WARN_MIN_N: u32 = 10;
/// Below this n a cell is printed as a count, never as a rate.
pub const COUNT_ONLY_BELOW: u32 = 5;

/// The level the floor allows: 0 = note, 1 = warn, 2 = error. Kept numeric so
/// the build script needs no crate type.
pub type Level = u8;
pub const NOTE: Level = 0;
pub const WARN: Level = 1;
pub const ERROR: Level = 2;

/// The Wilson 95% score interval for `k` successes in `n` trials. `n == 0`
/// gives the uninformative `[0, 1]`.
pub fn wilson(k: u32, n: u32) -> (f64, f64) {
    if n == 0 {
        return (0.0, 1.0);
    }
    let n_f = f64::from(n);
    let p = f64::from(k) / n_f;
    let z2 = Z * Z;
    let denom = 1.0 + z2 / n_f;
    let centre = p + z2 / (2.0 * n_f);
    let margin = Z * ((p * (1.0 - p) + z2 / (4.0 * n_f)) / n_f).sqrt();
    let lo = ((centre - margin) / denom).max(0.0);
    let hi = ((centre + margin) / denom).min(1.0);
    (lo, hi)
}

/// The level one cell allows. Only a measured, fresh cell can allow more
/// than `note`.
pub fn level(measured: bool, fresh: bool, tp: u32, fp: u32) -> Level {
    if !measured || !fresh {
        return NOTE;
    }
    let n = tp + fp;
    let (lo, _) = wilson(tp, n);
    if n >= ERROR_MIN_N && lo >= ERROR_LOWER {
        ERROR
    } else if n >= WARN_MIN_N && lo >= WARN_LOWER {
        WARN
    } else {
        NOTE
    }
}
