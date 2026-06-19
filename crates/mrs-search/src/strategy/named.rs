//! Named strategy schedules.
//!
//! A small registry mapping schedule names (as exposed by the `mrs --schedule
//! NAME` CLI flag) to `StrategySchedule` constructors. Adding a new schedule
//! is a 2-step change:
//!
//! 1. Implement a `pub fn my_schedule(total_time: Duration) -> StrategySchedule`
//!    in this module (or any module re-exported here).
//! 2. Add its name + constructor to [`by_name`].
//!
//! Built-in schedules:
//!
//! | Name      | Description                                                  |
//! |-----------|--------------------------------------------------------------|
//! | `casc`    | Default 12-strategy portfolio tuned for CASC-style budgets   |
//! |           | (30 s and up). Same as [`StrategySchedule::default_schedule`].|
//! |           | Strategies 1–9 cover KBO/LPO baseline search; strategies     |
//! |           | 10–11 target FEQ (equational) problems; strategy 12 targets  |
//! |           | ICU (unit equational) problems.                              |
//! | `fast`    | Single KBO `AgeWeight(5)` + `AllNegative` strategy. For      |
//! |           | sub-second ATP-query budgets (e.g. driving `mrs-proover`).   |
//! | `mini`    | Three-strategy portfolio: KBO `AgeWeight`, KBO               |
//! |           | `SmallestFirst`+no-AVATAR, LPO `AgeWeight`. Aimed at 1-5 s   |
//! |           | budgets where `fast` underperforms but the full 12-strategy  |
//! |           | rotation pays too much setup cost.                           |
//!
//! The CASC schedule name is `casc` (not `casc-30`): the same schedule is
//! intended for whatever CASC edition is current; per-edition tweaks should
//! be added as their own named schedules (e.g. `casc-31`) once they exist.

use std::time::Duration;

use crate::{LiteralSelection, SearchConfig, SelectionStrategy, TermOrdering};

use super::StrategySchedule;

/// All built-in schedule names, in the order they should be listed in
/// `--help` output.
pub const ALL: &[&str] = &[
    "casc", "casc_feq", "casc_fne", "casc_ueq", "casc_epr", "casc_eps", "casc_epu", "casc_icu",
    "fast", "mini", "ml", "ml_feq", "ml_fne", "ml_ueq", "ml_epr",
];

/// Look up a schedule by name. Returns `None` if the name is unknown.
pub fn by_name(name: &str, total_time: Duration, workers: usize) -> Option<StrategySchedule> {
    match name {
        "casc" | "default" => Some(StrategySchedule::default_schedule(total_time, workers)),
        "casc_feq" => Some(casc_feq(total_time, workers)),
        "casc_fne" => Some(casc_fne(total_time, workers)),
        "casc_ueq" => Some(casc_ueq(total_time, workers)),
        "casc_epr" => Some(casc_epr(total_time, workers)),
        "casc_eps" => Some(casc_eps(total_time, workers)),
        "casc_epu" => Some(casc_epu(total_time, workers)),
        "casc_icu" => Some(casc_icu(total_time, workers)),
        "fast" => Some(fast(total_time, workers)),
        "mini" => Some(mini(total_time, workers)),
        "ml" | "ml_feq" => Some(ml_feq(total_time, workers)),
        "ml_fne" => Some(ml_fne(total_time, workers)),
        "ml_ueq" => Some(ml_ueq(total_time, workers)),
        "ml_epr" => Some(ml_epr(total_time, workers)),
        _ => None,
    }
}

/// One KBO strategy for the full budget. Best for very short budgets where
/// 9-way setup overhead dominates the actual search time.
pub fn fast(total_time: Duration, _workers: usize) -> StrategySchedule {
    StrategySchedule {
        strategies: vec![(
            SearchConfig {
                time_limit: total_time,
                selection: SelectionStrategy::AgeWeight(5),
                literal_selection: LiteralSelection::AllNegative,
                ordering: TermOrdering::KBO,
                ..SearchConfig::default()
            },
            total_time,
        )],
    }
}

/// Three-strategy compact portfolio. Equal time slices.
pub fn mini(total_time: Duration, _workers: usize) -> StrategySchedule {
    let slice = total_time / 3;
    let last = total_time.saturating_sub(slice * 2);
    StrategySchedule {
        strategies: vec![
            (
                SearchConfig {
                    time_limit: slice,
                    selection: SelectionStrategy::AgeWeight(5),
                    literal_selection: LiteralSelection::AllNegative,
                    ordering: TermOrdering::KBO,
                    ..SearchConfig::default()
                },
                slice,
            ),
            (
                SearchConfig {
                    time_limit: slice,
                    selection: SelectionStrategy::SmallestFirst,
                    literal_selection: LiteralSelection::AllNegative,
                    ordering: TermOrdering::KBO,
                    max_term_weight: None,
                    use_avatar: false,
                    unit_only_resolution: false,
                    ..SearchConfig::default()
                },
                slice,
            ),
            (
                SearchConfig {
                    time_limit: last,
                    selection: SelectionStrategy::AgeWeight(5),
                    literal_selection: LiteralSelection::AllNegative,
                    ordering: TermOrdering::LPO,
                    ..SearchConfig::default()
                },
                last,
            ),
        ],
    }
}

/// A schedule that relies heavily on ML-guided selection, built on the CASC 11-strategy chassis.
pub fn ml_feq(total_time: Duration, _workers: usize) -> StrategySchedule {
    let ms = total_time.as_millis() as u64;
    let t1 = Duration::from_millis(ms * 14 / 100);
    let t2 = Duration::from_millis(ms * 10 / 100);
    let t3 = Duration::from_millis(ms * 10 / 100);
    let t4 = Duration::from_millis(ms * 9 / 100);
    let t5 = Duration::from_millis(ms * 9 / 100);
    let t6 = Duration::from_millis(ms * 10 / 100);
    let t7 = Duration::from_millis(ms * 14 / 100);
    let t8 = Duration::from_millis(ms * 9 / 100);
    let t9 = Duration::from_millis(ms * 9 / 100);
    let t10 = Duration::from_millis(ms * 4 / 100);
    let t11 = total_time
        .saturating_sub(t1)
        .saturating_sub(t2)
        .saturating_sub(t3)
        .saturating_sub(t4)
        .saturating_sub(t5)
        .saturating_sub(t6)
        .saturating_sub(t7)
        .saturating_sub(t8)
        .saturating_sub(t9)
        .saturating_sub(t10);

    StrategySchedule {
        strategies: vec![
            // s1: ML-Guided balanced exploration
            (
                SearchConfig {
                    time_limit: t1,
                    selection: SelectionStrategy::MlGuided {
                        ratio: 5,
                        alpha: 0.3,
                    },
                    literal_selection: LiteralSelection::AllNegative,
                    ordering: TermOrdering::KBO,
                    ..SearchConfig::default()
                },
                t1,
            ),
            // s2: ML-Guided deep chain proofs
            (
                SearchConfig {
                    time_limit: t2,
                    selection: SelectionStrategy::MlGuided {
                        ratio: 10,
                        alpha: 0.1,
                    },
                    literal_selection: LiteralSelection::AllNegative,
                    ordering: TermOrdering::KBO,
                    max_term_weight: None,
                    use_avatar: false,
                    unit_only_resolution: false,
                    ..SearchConfig::default()
                },
                t2,
            ),
            // s3: pure best-first (static fallback)
            (
                SearchConfig {
                    time_limit: t3,
                    selection: SelectionStrategy::SmallestFirst,
                    literal_selection: LiteralSelection::AllNegative,
                    ordering: TermOrdering::KBO,
                    ..SearchConfig::default()
                },
                t3,
            ),
            // s4: ML-Guided aggressive selection
            (
                SearchConfig {
                    time_limit: t4,
                    selection: SelectionStrategy::MlGuided {
                        ratio: 3,
                        alpha: 0.5,
                    },
                    literal_selection: LiteralSelection::MaxNegativeOrMaxPositive,
                    ordering: TermOrdering::KBO,
                    ..SearchConfig::default()
                },
                t4,
            ),
            // s5: unrestricted literal selection (for FEQ)
            (
                SearchConfig {
                    time_limit: t5,
                    selection: SelectionStrategy::AgeWeight(5),
                    literal_selection: LiteralSelection::All,
                    ordering: TermOrdering::KBO,
                    ..SearchConfig::default()
                },
                t5,
            ),
            // s6: FNE/definitional CNF proofs (static fallback)
            (
                SearchConfig {
                    time_limit: t6,
                    selection: SelectionStrategy::AgeWeight(5),
                    literal_selection: LiteralSelection::All,
                    ordering: TermOrdering::KBO,
                    max_term_weight: None,
                    use_avatar: false,
                    unit_only_resolution: false,
                    ..SearchConfig::default()
                },
                t6,
            ),
            // s7: ML-Guided LPO balanced
            (
                SearchConfig {
                    time_limit: t7,
                    selection: SelectionStrategy::MlGuided {
                        ratio: 5,
                        alpha: 0.3,
                    },
                    literal_selection: LiteralSelection::AllNegative,
                    ordering: TermOrdering::LPO,
                    ..SearchConfig::default()
                },
                t7,
            ),
            // s8: LPO goal-directed
            (
                SearchConfig {
                    time_limit: t8,
                    selection: SelectionStrategy::GoalDirected(10),
                    literal_selection: LiteralSelection::AllNegative,
                    ordering: TermOrdering::LPO,
                    ..SearchConfig::default()
                },
                t8,
            ),
            // s9: LPO best-first
            (
                SearchConfig {
                    time_limit: t9,
                    selection: SelectionStrategy::SmallestFirst,
                    literal_selection: LiteralSelection::AllNegative,
                    ordering: TermOrdering::LPO,
                    ..SearchConfig::default()
                },
                t9,
            ),
            // s10: ML-Guided FEQ-targeted KBO
            (
                SearchConfig {
                    time_limit: t10,
                    selection: SelectionStrategy::MlGuided {
                        ratio: 5,
                        alpha: 0.1,
                    },
                    literal_selection: LiteralSelection::All,
                    ordering: TermOrdering::KBO,
                    max_term_weight: Some(30),
                    use_avatar: false,
                    unit_only_resolution: false,
                    ..SearchConfig::default()
                },
                t10,
            ),
            // s11: FEQ-targeted LPO
            (
                SearchConfig {
                    time_limit: t11,
                    selection: SelectionStrategy::AgeWeight(5),
                    literal_selection: LiteralSelection::All,
                    ordering: TermOrdering::LPO,
                    max_term_weight: None,
                    use_avatar: false,
                    unit_only_resolution: false,
                    ..SearchConfig::default()
                },
                t11,
            ),
        ],
    }
}

/// A schedule optimized for FNE (First-Order No Equality).
/// Deep-chain resolution with SmallestFirst and no weight limits.
pub fn ml_fne(total_time: Duration, workers: usize) -> StrategySchedule {
    let workers = workers.max(1);
    let t_part = Duration::from_millis((total_time.as_millis() / workers as u128) as u64);
    let t_last = total_time.saturating_sub(t_part * (workers as u32 - 1));

    let mut strategies = Vec::new();
    for i in 0..workers {
        let t = if i == workers - 1 { t_last } else { t_part };
        let alpha = 0.1 + ((i % 10) as f32 * 0.05); // Cycle alphas
        let ratio = 3 + (i % 5) as u32; // Cycle ratios
        strategies.push((
            SearchConfig {
                time_limit: t,
                selection: SelectionStrategy::MlGuided { ratio, alpha },
                literal_selection: LiteralSelection::AllNegative,
                ordering: if i % 2 == 0 {
                    TermOrdering::KBO
                } else {
                    TermOrdering::LPO
                },
                max_term_weight: None,
                ..SearchConfig::default()
            },
            t,
        ));
    }
    StrategySchedule { strategies }
}

/// A schedule optimized for UEQ (Unit Equality).
/// No AVATAR, unit-only resolution, aggressive KBO/LPO.
pub fn ml_ueq(total_time: Duration, workers: usize) -> StrategySchedule {
    let workers = workers.max(1);
    let t_part = Duration::from_millis((total_time.as_millis() / workers as u128) as u64);
    let t_last = total_time.saturating_sub(t_part * (workers as u32 - 1));

    let mut strategies = Vec::new();
    for i in 0..workers {
        let t = if i == workers - 1 { t_last } else { t_part };
        let alpha = 0.1 + ((i % 10) as f32 * 0.05); // Cycle alphas
        let ratio = 3 + (i % 5) as u32; // Cycle ratios
        strategies.push((
            SearchConfig {
                time_limit: t,
                selection: SelectionStrategy::MlGuided { ratio, alpha },
                literal_selection: LiteralSelection::MaxNegativeOrMaxPositive,
                ordering: if i % 2 == 0 {
                    TermOrdering::KBO
                } else {
                    TermOrdering::LPO
                },
                use_avatar: false,
                unit_only_resolution: true,
                ..SearchConfig::default()
            },
            t,
        ));
    }
    StrategySchedule { strategies }
}

/// A schedule optimized for EPR (Effectively Propositional).
/// Extreme AVATAR SAT-splitting.
pub fn ml_epr(total_time: Duration, workers: usize) -> StrategySchedule {
    let workers = workers.max(1);
    let t_part = Duration::from_millis((total_time.as_millis() / workers as u128) as u64);
    let t_last = total_time.saturating_sub(t_part * (workers as u32 - 1));

    let mut strategies = Vec::new();
    for i in 0..workers {
        let t = if i == workers - 1 { t_last } else { t_part };
        let alpha = 0.2 + ((i % 10) as f32 * 0.05); // Cycle alphas
        let ratio = 5;
        strategies.push((
            SearchConfig {
                time_limit: t,
                selection: SelectionStrategy::MlGuided { ratio, alpha },
                literal_selection: LiteralSelection::AllNegative,
                ordering: TermOrdering::KBO,
                use_avatar: true,
                ..SearchConfig::default()
            },
            t,
        ));
    }
    StrategySchedule { strategies }
}

/// A helper function to build a CASC strategy schedule using a mathematically optimized priority sequence.
fn build_casc_schedule(
    total_time: Duration,
    workers: usize,
    order: &[usize; 15],
) -> StrategySchedule {
    let workers = workers.max(1);
    let t_part = Duration::from_millis((total_time.as_millis() / workers as u128) as u64);
    let t_last = total_time.saturating_sub(t_part * (workers as u32 - 1));

    let base_configs: Vec<SearchConfig> =
        super::StrategySchedule::_all_strategies(Duration::ZERO, 0)
            .strategies
            .into_iter()
            .map(|(c, _)| c)
            .collect();

    let mut strategies = Vec::new();
    for i in 0..workers {
        let t = if i == workers - 1 { t_last } else { t_part };
        let idx = order[i % 15] - 1;
        let mut cfg = base_configs[idx].clone();
        cfg.time_limit = t;
        strategies.push((cfg, t));
    }
    StrategySchedule { strategies }
}

/// A purely static schedule optimized for FNE (First-Order No Equality).
/// Tunes the portfolio according to CASC-30 priority sweeps.
pub fn casc_fne(total_time: Duration, workers: usize) -> StrategySchedule {
    build_casc_schedule(
        total_time,
        workers,
        &[11, 12, 8, 4, 10, 2, 1, 3, 5, 6, 7, 9, 13, 14, 15],
    )
}

/// A purely static schedule optimized for FEQ (First-Order with Equality).
/// Tunes the portfolio according to CASC-30 priority sweeps.
pub fn casc_feq(total_time: Duration, workers: usize) -> StrategySchedule {
    build_casc_schedule(
        total_time,
        workers,
        &[8, 12, 1, 11, 10, 4, 14, 6, 13, 15, 2, 3, 5, 7, 9],
    )
}

/// A purely static schedule optimized for UEQ (Unit Equality).
/// Tunes the portfolio according to CASC-30 priority sweeps.
pub fn casc_ueq(total_time: Duration, workers: usize) -> StrategySchedule {
    build_casc_schedule(
        total_time,
        workers,
        &[11, 4, 2, 14, 8, 6, 1, 3, 5, 7, 9, 10, 12, 13, 15],
    )
}

/// A schedule optimized for EPR Unsatisfiable (EPU).
/// Greedy order: s6 (+9/10), s2 (+1/10) — 100% coverage at 2 cores.
/// Tunes the portfolio according to CASC-30 priority sweeps.
pub fn casc_epu(total_time: Duration, workers: usize) -> StrategySchedule {
    build_casc_schedule(
        total_time,
        workers,
        &[6, 2, 1, 3, 4, 5, 7, 8, 9, 10, 11, 12, 13, 14, 15],
    )
}

/// A schedule optimized for EPR Satisfiable (EPS).
/// Greedy order: s2 (+22/24), s6 (+2/24) — 100% coverage at 2 cores.
/// Tunes the portfolio according to CASC-30 priority sweeps.
pub fn casc_eps(total_time: Duration, workers: usize) -> StrategySchedule {
    build_casc_schedule(
        total_time,
        workers,
        &[2, 6, 1, 3, 4, 5, 7, 8, 9, 10, 11, 12, 13, 14, 15],
    )
}

/// A schedule optimized for EPR (Effectively Propositional) — used as
/// a fallback for both EPS and EPU when a specific schedule isn't selected.
/// Prioritises s6 first (better for EPU; EPS route uses casc_eps directly).
pub fn casc_epr(total_time: Duration, workers: usize) -> StrategySchedule {
    build_casc_schedule(
        total_time,
        workers,
        &[6, 2, 1, 3, 4, 5, 7, 8, 9, 10, 11, 12, 13, 14, 15],
    )
}

/// A purely static schedule optimized for ICU (Intensional Unit Equality).
/// Tunes the portfolio according to CASC-30 priority sweeps.
pub fn casc_icu(total_time: Duration, workers: usize) -> StrategySchedule {
    build_casc_schedule(
        total_time,
        workers,
        &[12, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 14, 15],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn casc_is_the_default() {
        let t = Duration::from_secs(30);
        let s = by_name("casc", t, 1).expect("casc must exist");
        let d = StrategySchedule::default_schedule(t, 1);
        assert_eq!(s.strategies.len(), d.strategies.len());
    }

    #[test]
    fn default_alias_works() {
        assert!(by_name("default", Duration::from_secs(5), 1).is_some());
    }

    #[test]
    fn fast_is_single_strategy() {
        let s = by_name("fast", Duration::from_secs(2), 1).unwrap();
        assert_eq!(s.strategies.len(), 1);
    }

    #[test]
    fn mini_is_three_strategies() {
        let s = by_name("mini", Duration::from_secs(3), 1).unwrap();
        assert_eq!(s.strategies.len(), 3);
    }

    #[test]
    fn unknown_returns_none() {
        assert!(by_name("nonexistent-schedule", Duration::from_secs(1), 1).is_none());
    }

    #[test]
    fn division_schedules_scale_with_workers() {
        let t = Duration::from_secs(8);
        for name in [
            "casc_fne", "casc_feq", "casc_ueq", "casc_epr", "casc_eps", "casc_epu", "casc_icu",
            "ml_fne", "ml_ueq", "ml_epr",
        ] {
            let s = by_name(name, t, 8).unwrap();
            assert_eq!(s.strategies.len(), 8, "{name} should have 8 strategies");
            let total: Duration = s.strategies.iter().map(|(_, t)| *t).sum();
            assert_eq!(total, t, "{name} slices must sum to the budget");
        }
    }

    #[test]
    fn all_names_resolve() {
        for name in ALL {
            assert!(
                by_name(name, Duration::from_secs(1), 1).is_some(),
                "named schedule `{name}` is in ALL but by_name() doesn't know it",
            );
        }
    }
}
