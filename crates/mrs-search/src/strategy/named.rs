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
//! | `casc`    | Default 9-strategy portfolio tuned for CASC-style budgets    |
//! |           | (30s and up). Same as [`StrategySchedule::default_schedule`].|
//! | `fast`    | Single KBO `AgeWeight(5)` + `AllNegative` strategy. For      |
//! |           | sub-second ATP-query budgets (e.g. driving `mrs-proover`).   |
//! | `mini`    | Three-strategy portfolio: KBO `AgeWeight`, KBO               |
//! |           | `SmallestFirst`+no-AVATAR, LPO `AgeWeight`. Aimed at 1-5s    |
//! |           | budgets where `fast` underperforms but the full 9-strategy   |
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
pub const ALL: &[&str] = &["casc", "fast", "mini"];

/// Look up a schedule by name. Returns `None` if the name is unknown.
pub fn by_name(name: &str, total_time: Duration) -> Option<StrategySchedule> {
    match name {
        "casc" | "default" => Some(StrategySchedule::default_schedule(total_time)),
        "fast" => Some(fast(total_time)),
        "mini" => Some(mini(total_time)),
        _ => None,
    }
}

/// One KBO strategy for the full budget. Best for very short budgets where
/// 9-way setup overhead dominates the actual search time.
pub fn fast(total_time: Duration) -> StrategySchedule {
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
pub fn mini(total_time: Duration) -> StrategySchedule {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn casc_is_the_default() {
        let t = Duration::from_secs(30);
        let s = by_name("casc", t).expect("casc must exist");
        let d = StrategySchedule::default_schedule(t);
        assert_eq!(s.strategies.len(), d.strategies.len());
    }

    #[test]
    fn default_alias_works() {
        assert!(by_name("default", Duration::from_secs(5)).is_some());
    }

    #[test]
    fn fast_is_single_strategy() {
        let s = by_name("fast", Duration::from_secs(2)).unwrap();
        assert_eq!(s.strategies.len(), 1);
    }

    #[test]
    fn mini_is_three_strategies() {
        let s = by_name("mini", Duration::from_secs(3)).unwrap();
        assert_eq!(s.strategies.len(), 3);
    }

    #[test]
    fn unknown_returns_none() {
        assert!(by_name("nonexistent-schedule", Duration::from_secs(1)).is_none());
    }

    #[test]
    fn all_names_resolve() {
        for name in ALL {
            assert!(
                by_name(name, Duration::from_secs(1)).is_some(),
                "named schedule `{name}` is in ALL but by_name() doesn't know it",
            );
        }
    }
}
