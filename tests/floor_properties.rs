//! Property tests for the severity floor (RG-A5): the Wilson interval, the
//! level a cell allows, and the cap applied to a raised severity.
//!
//! `precision_floor.rs` checks the build-time table against an independent
//! re-implementation at fixed points. This file checks the invariants over
//! every input proptest draws: bounds stay inside [0, 1], the level is monotone
//! in TP, turning a TP into an FP never raises it, a cell that is not fresh
//! never lifts a rule, and the cap is idempotent and never raises a finding.
//!
//! "One more FP never raises the level" is NOT an invariant: the floor needs
//! n >= 10 for `warn`, so 9 TP and 0 FP is `note` while 9 TP and 1 FP (n = 10,
//! Wilson lower bound 0.596) is `warn`. A false positive adds a sample before
//! it lowers the bound. This is the pre-registered minimum n, not a defect;
//! `an_added_false_positive_can_lift_a_cell_over_the_minimum_n` pins it, and
//! the property below holds n fixed. (proptest found it on CI after passing
//! locally; the earlier form of this property was wrong.)

use proptest::prelude::*;
use realism_gate::Severity;
use realism_gate::rules::measured::{self, floor};

fn severity() -> impl Strategy<Value = Severity> {
    prop_oneof![
        Just(Severity::Note),
        Just(Severity::Warn),
        Just(Severity::Error)
    ]
}

/// Exhaustive over a grid, so the two level properties do not depend on which
/// inputs proptest draws.
#[test]
fn the_level_is_monotone_over_every_small_cell() {
    for tp in 0u32..=150 {
        for fp in 0u32..=150 {
            let here = floor::level(true, true, tp, fp);
            assert!(
                floor::level(true, true, tp + 1, fp) >= here,
                "tp+1 at {tp}/{fp}"
            );
            if tp >= 1 {
                assert!(
                    floor::level(true, true, tp - 1, fp + 1) <= here,
                    "tp->fp swap at {tp}/{fp}"
                );
            }
        }
    }
}

#[test]
fn an_added_false_positive_can_lift_a_cell_over_the_minimum_n() {
    assert_eq!(
        floor::level(true, true, 9, 0),
        floor::NOTE,
        "n = 9 is below the minimum n"
    );
    assert_eq!(
        floor::level(true, true, 9, 1),
        floor::WARN,
        "n = 10, lower bound above 0.50"
    );
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2_000))]

    #[test]
    fn wilson_bounds_bracket_the_point_estimate(n in 1u32..5_000, frac in 0.0f64..=1.0) {
        let k = ((f64::from(n) * frac).round() as u32).min(n);
        let (lo, hi) = floor::wilson(k, n);
        let p = f64::from(k) / f64::from(n);
        prop_assert!((0.0..=1.0).contains(&lo) && (0.0..=1.0).contains(&hi));
        prop_assert!(lo <= p + 1e-12 && p <= hi + 1e-12, "k={k} n={n} lo={lo} p={p} hi={hi}");
    }

    #[test]
    fn one_more_true_positive_never_lowers_the_level(tp in 0u32..400, fp in 0u32..400) {
        prop_assert!(floor::level(true, true, tp + 1, fp) >= floor::level(true, true, tp, fp));
    }

    #[test]
    fn turning_a_true_positive_into_a_false_positive_never_raises_the_level(tp in 1u32..400, fp in 0u32..400) {
        prop_assert!(floor::level(true, true, tp - 1, fp + 1) <= floor::level(true, true, tp, fp));
    }

    #[test]
    fn a_cell_that_is_not_fresh_or_not_measured_never_lifts_a_rule(tp in 0u32..10_000, fp in 0u32..10_000, measured in any::<bool>()) {
        prop_assert_eq!(floor::level(measured, false, tp, fp), floor::NOTE);
        prop_assert_eq!(floor::level(false, true, tp, fp), floor::NOTE);
    }

    #[test]
    fn the_level_respects_the_preregistered_minimum_n(tp in 0u32..40, fp in 0u32..40) {
        let level = floor::level(true, true, tp, fp);
        let n = tp + fp;
        if level == floor::ERROR {
            prop_assert!(n >= floor::ERROR_MIN_N && floor::wilson(tp, n).0 >= floor::ERROR_LOWER);
        }
        if level == floor::WARN {
            prop_assert!(n >= floor::WARN_MIN_N && floor::wilson(tp, n).0 >= floor::WARN_LOWER);
        }
    }

    #[test]
    fn the_cap_never_raises_and_is_idempotent(raised in severity(), cap in severity()) {
        let shipped = measured::capped(raised, cap);
        prop_assert!(shipped <= raised && shipped <= cap);
        prop_assert_eq!(measured::capped(shipped, cap), shipped);
        prop_assert!(shipped == raised || shipped == cap);
    }

    #[test]
    fn an_unknown_rule_or_language_ships_note(raised in severity(), rule in "[A-Z]{2}[0-9]{2,3}x", lang in "[a-z]{1,8}") {
        let table = measured::Table::new();
        prop_assert_eq!(measured::shipped_in(&table, &rule, &lang, raised), Severity::Note);
    }
}
