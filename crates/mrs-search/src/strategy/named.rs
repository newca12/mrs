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
    "casc", "casc_feq", "casc_fne", "casc_ueq", "casc_epr", "fast", "mini",
];

/// Look up a schedule by name. Returns `None` if the name is unknown.
pub fn by_name(name: &str, total_time: Duration, workers: usize) -> Option<StrategySchedule> {
    match name {
        "casc" | "default" | "casc_feq" => {
            Some(StrategySchedule::default_schedule(total_time, workers))
        }
        "casc_fne" => Some(casc_fne(total_time, workers)),
        "casc_ueq" => Some(casc_ueq(total_time, workers)),
        "casc_epr" => Some(casc_epr(total_time, workers)),
        "fast" => Some(fast(total_time, workers)),
        "mini" => Some(mini(total_time, workers)),
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

/// A purely static schedule optimized for FNE (First-Order No Equality).
/// Drops paramodulation-heavy strategies and weight limits for deep chaining.
pub fn casc_fne(total_time: Duration, workers: usize) -> StrategySchedule {
    let workers = workers.max(1);
    let t_part = Duration::from_millis((total_time.as_millis() / workers as u128) as u64);
    let t_last = total_time.saturating_sub(t_part * (workers as u32 - 1));

    let mut strategies = Vec::new();
    for i in 0..workers {
        let t = if i == workers - 1 { t_last } else { t_part };
        let ratio = 3 + (i % 5) as u32;
        strategies.push((
            SearchConfig {
                time_limit: t,
                selection: if i % 3 == 0 {
                    SelectionStrategy::SmallestFirst
                } else {
                    SelectionStrategy::AgeWeight(ratio)
                },
                literal_selection: if i % 2 == 0 {
                    LiteralSelection::AllNegative
                } else {
                    LiteralSelection::MaxNegativeOrMaxPositive
                },
                ordering: if i % 4 < 2 {
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

/// A purely static schedule optimized for UEQ (Unit Equality).
/// Disables AVATAR and forces unit_only_resolution.
pub fn casc_ueq(total_time: Duration, workers: usize) -> StrategySchedule {
    let workers = workers.max(1);
    let t_part = Duration::from_millis((total_time.as_millis() / workers as u128) as u64);
    let t_last = total_time.saturating_sub(t_part * (workers as u32 - 1));

    let mut strategies = Vec::new();
    for i in 0..workers {
        let t = if i == workers - 1 { t_last } else { t_part };
        let ratio = 3 + (i % 5) as u32;
        strategies.push((
            SearchConfig {
                time_limit: t,
                selection: if i % 2 == 0 {
                    SelectionStrategy::SmallestFirst
                } else {
                    SelectionStrategy::AgeWeight(ratio)
                },
                literal_selection: LiteralSelection::MaxNegativeOrMaxPositive,
                ordering: if i % 4 < 2 {
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

/// A purely static schedule optimized for EPR (Effectively Propositional).
/// Extreme AVATAR SAT-splitting.
pub fn casc_epr(total_time: Duration, workers: usize) -> StrategySchedule {
    let workers = workers.max(1);
    let t_part = Duration::from_millis((total_time.as_millis() / workers as u128) as u64);
    let t_last = total_time.saturating_sub(t_part * (workers as u32 - 1));

    let mut strategies = Vec::new();
    for i in 0..workers {
        let t = if i == workers - 1 { t_last } else { t_part };
        let ratio = 5 + (i % 5) as u32;
        strategies.push((
            SearchConfig {
                time_limit: t,
                selection: if i % 3 == 0 {
                    SelectionStrategy::SmallestFirst
                } else {
                    SelectionStrategy::AgeWeight(ratio)
                },
                literal_selection: LiteralSelection::AllNegative,
                ordering: TermOrdering::KBO,
                use_avatar: true,
                max_term_weight: None,
                ..SearchConfig::default()
            },
            t,
        ));
    }
    StrategySchedule { strategies }
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
        for name in ["casc_fne", "casc_ueq", "casc_epr"] {
            let s = by_name(name, t, 8).unwrap();
            assert_eq!(s.strategies.len(), 8, "{name} should have 8 strategies");
            let total: Duration = s.strategies.iter().map(|(_, t)| *t).sum();
            assert_eq!(total, t, "{name} slices must sum to the budget");
        }
    }

    #[test]
    fn casc_feq_aliases_default() {
        let t = Duration::from_secs(30);
        let s = by_name("casc_feq", t, 1).unwrap();
        let d = StrategySchedule::default_schedule(t, 1);
        assert_eq!(s.strategies.len(), d.strategies.len());
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
