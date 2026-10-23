use std::collections::{HashMap, HashSet};

use mrs_core::clause::{Clause, ClauseSource};
use mrs_core::formula::Atom;
use mrs_core::symbol::SymbolId;
use mrs_core::term::Term;

/// Extracts all symbols from a clause.
fn clause_symbols(clause: &Clause, syms: &mut HashSet<SymbolId>) {
    for lit in &clause.literals {
        match &lit.atom {
            Atom::Pred(p, args) => {
                syms.insert(*p);
                for arg in args {
                    term_symbols(arg, syms);
                }
            }
            Atom::Eq(l, r) => {
                term_symbols(l, syms);
                term_symbols(r, syms);
            }
        }
    }
}

fn term_symbols(term: &Term, syms: &mut HashSet<SymbolId>) {
    let mut stack = vec![term];
    while let Some(t) = stack.pop() {
        if let Term::App(f, args) = t {
            syms.insert(*f);
            stack.extend(args.iter());
        }
    }
}

/// A generic item that can be selected by SInE.
pub trait SineItem {
    fn symbols(&self) -> HashSet<SymbolId>;
    fn is_conjecture(&self) -> bool;
}

impl SineItem for Clause {
    fn symbols(&self) -> HashSet<SymbolId> {
        let mut syms = HashSet::new();
        clause_symbols(self, &mut syms);
        syms
    }
    fn is_conjecture(&self) -> bool {
        match &self.source {
            ClauseSource::Input { role, .. } => {
                role == "conjecture" || role == "negated_conjecture"
            }
            _ => false,
        }
    }
}

#[derive(Clone)]
pub enum SineItemWrapper {
    Clause(Clause),
}

impl SineItem for SineItemWrapper {
    fn symbols(&self) -> HashSet<SymbolId> {
        match self {
            SineItemWrapper::Clause(c) => c.symbols(),
        }
    }
    fn is_conjecture(&self) -> bool {
        match self {
            SineItemWrapper::Clause(c) => c.is_conjecture(),
        }
    }
}

/// Why a SInE filter will not be applied to a clause set.
///
/// SInE restricts the search to a symbol-connected neighbourhood of the goal,
/// which makes it a *completeness-restricting* restriction: the search reports
/// `IncompletenessReason::SineFiltered` and can never claim saturation while
/// `SearchConfig::sine_tolerance` is set. That trade is only worth making when
/// the neighbourhood it found is actually a search space. These are the cases
/// where it is not, and where keeping the restriction active converts a full
/// budget into a fraction of a second of work.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SineSkip {
    /// The filter removed nothing, so it cannot pay for what it costs. This is
    /// the only case the pre-`SineSkip` guard recognised.
    NothingRemoved,
    /// The active set is exactly the conjecture seed set: the trigger phase
    /// grew it by nothing. The search then holds the goal and no premise, so no
    /// inference is available to it at all.
    NoGrowth,
    /// The clause set is smaller than [`SinePolicy::min_clauses`], so the
    /// restriction is not worth its cost whatever the filter would return.
    TooSmall,
    /// Fewer premises survived than a resolution step needs.
    TooFewPremises,
    /// More than `SinePolicy::max_removed_percent` of the clause set was
    /// removed, which starves the search of premises rather than focusing it.
    Starved,
}

impl SineSkip {
    /// Stable identifier for the `TRACE_SINE` diagnostic, so a sweep can group
    /// refusals without string matching.
    pub fn as_str(self) -> &'static str {
        match self {
            SineSkip::NothingRemoved => "nothing_removed",
            SineSkip::NoGrowth => "no_growth",
            SineSkip::TooSmall => "too_small",
            SineSkip::TooFewPremises => "too_few_premises",
            SineSkip::Starved => "starved",
        }
    }
}

/// When to refuse a SInE filter.
#[derive(Debug, Clone, Copy)]
pub struct SinePolicy {
    /// At or below this many clauses SInE is not applied at all: on a clause
    /// set this small the restriction only costs completeness.
    pub min_clauses: usize,
    /// Refuse when fewer than this many premises survive. Two is the provable
    /// floor: a clause set with fewer than two clauses admits no inference, so
    /// searching it cannot produce a refutation.
    pub min_premises: usize,
    /// Refuse when the filter removes more than this percentage of the clause
    /// set.
    ///
    /// Defaults to 100, which disables the guard. That default is measured, not
    /// lazy: over 74 solved problems carrying a conjecture and more than 200
    /// clauses, SInE was applied on 8, and it removed up to **99.9 %** of the
    /// clause set on problems `mrs` solves — `CSR051-10` keeps 28 clauses of
    /// 44 219 and is a solved row. A percentage threshold low enough to catch
    /// the degenerate case would therefore switch SInE off exactly where it is
    /// earning its keep. Starvation is caught by `min_premises` and by
    /// `SineSkip::NoGrowth`, which are the criteria that separate the measured
    /// cases. Override with `MRS_SINE_MAX_REMOVED_PERCENT` to A/B a value.
    pub max_removed_percent: usize,
}

impl SinePolicy {
    /// Reads `MRS_SINE_MAX_REMOVED_PERCENT`, clamped to `100`, over the default
    /// policy. An unparsable or out-of-range value keeps the default rather than
    /// guessing.
    pub fn from_env() -> SinePolicy {
        let mut policy = SinePolicy::default();
        if let Some(value) = std::env::var("MRS_SINE_MAX_REMOVED_PERCENT")
            .ok()
            .as_deref()
            .and_then(parse_max_removed_percent)
        {
            policy.max_removed_percent = value;
        }
        policy
    }
}

fn parse_max_removed_percent(value: &str) -> Option<usize> {
    value
        .trim()
        .parse::<usize>()
        .ok()
        .filter(|&value| value <= 100)
}

impl Default for SinePolicy {
    fn default() -> Self {
        SinePolicy {
            min_clauses: 100,
            min_premises: 2,
            max_removed_percent: 100,
        }
    }
}

/// The outcome of asking whether SInE may be applied to a clause set.
#[derive(Debug)]
pub struct SineApplication<T> {
    /// The clause set the search should run on. Equal to the input, unfiltered,
    /// whenever `skip` is `Some`.
    pub items: Vec<T>,
    /// Whether the filter was applied and `items` is the restricted set.
    pub applied: bool,
    /// Why the filter was refused, if it was.
    pub skip: Option<SineSkip>,
    /// Clause count before filtering.
    pub before: usize,
    /// Clause count the filter selects. On a refusal this is still what the
    /// filter *would* have kept, so a diagnostic can report how far the filter
    /// was going to cut; `items` is the unfiltered set in that case.
    pub kept: usize,
    /// Conjecture clauses, which [`filter_items`] always keeps as seeds.
    pub seeds: usize,
}

/// Counts the conjecture clauses, which SInE seeds from.
fn seed_count<T: SineItem>(items: &[T]) -> usize {
    items.iter().filter(|item| item.is_conjecture()).count()
}

/// Applies a SInE filter unless the policy refuses it.
///
/// Refusing is always the safe direction: SInE restricts the search, so a
/// clause set that was not filtered is searched under strictly fewer
/// completeness restrictions than one that was. A refusal can cost a solved
/// problem, never a sound one.
pub fn apply_sine_filter<T: SineItem + Clone>(
    items: &[T],
    tolerance: f64,
    depth_limit: Option<usize>,
    policy: &SinePolicy,
) -> SineApplication<T> {
    let before = items.len();
    let seeds = seed_count(items);
    let refuse = |skip, kept| SineApplication {
        items: items.to_vec(),
        applied: false,
        skip: Some(skip),
        before,
        kept,
        seeds,
    };

    if before <= policy.min_clauses {
        return refuse(SineSkip::TooSmall, before);
    }

    let filtered = filter_items(items, tolerance, depth_limit);
    let kept = filtered.len();

    if kept == before {
        return refuse(SineSkip::NothingRemoved, kept);
    }
    // The filter always keeps the seeds, so a kept count at or below the seed
    // count means the trigger phase activated nothing beyond the goal itself.
    // The search then holds the goal and no premise, so no inference is
    // available to it at all and the whole budget is spent proving that.
    if kept <= seeds {
        return refuse(SineSkip::NoGrowth, kept);
    }
    if kept < policy.min_premises {
        return refuse(SineSkip::TooFewPremises, kept);
    }
    // Compare the exact ratio rather than a rounded-down integer percentage:
    // e.g. removing 99.5% must exceed a configured 99% limit. `u128` keeps
    // these products in range even when `usize` is at its maximum.
    let removed = before - kept;
    if (removed as u128) * 100 > (before as u128) * (policy.max_removed_percent as u128) {
        return refuse(SineSkip::Starved, kept);
    }

    SineApplication {
        items: filtered,
        applied: true,
        skip: None,
        before,
        kept,
        seeds,
    }
}

pub fn filter_items<T: SineItem + Clone>(
    items: &[T],
    tolerance: f64,
    depth_limit: Option<usize>,
) -> Vec<T> {
    if items.is_empty() {
        return Vec::new();
    }

    let mut item_syms: Vec<HashSet<SymbolId>> = Vec::with_capacity(items.len());
    let mut sym_counts: HashMap<SymbolId, usize> = HashMap::new();

    let mut has_conjectures = false;

    for item in items {
        let syms = item.symbols();
        if item.is_conjecture() {
            has_conjectures = true;
        }
        for &s in &syms {
            *sym_counts.entry(s).or_insert(0) += 1;
        }
        item_syms.push(syms);
    }

    // If there are no conjectures, SInE can't start easily from the goal.
    // Return everything.
    if !has_conjectures {
        return items.to_vec();
    }

    // Map each symbol to the items it triggers
    let mut triggers: HashMap<SymbolId, Vec<usize>> = HashMap::new();

    for (i, syms) in item_syms.iter().enumerate() {
        if syms.is_empty() {
            continue;
        }
        // Find minimum generality in this item
        let min_g = syms.iter().map(|s| sym_counts[s]).min().unwrap() as f64;
        let threshold = min_g * tolerance;

        for &s in syms {
            if let Some(cnt) = sym_counts.get(&s)
                && (*cnt as f64) <= threshold
            {
                triggers.entry(s).or_default().push(i);
            }
        }
    }

    let mut active_items = HashSet::new();
    let mut active_syms = HashSet::new();
    let mut new_syms = HashSet::new();

    // Initialize with conjectures
    for (i, item) in items.iter().enumerate() {
        if item.is_conjecture() {
            active_items.insert(i);
            for &s in &item_syms[i] {
                if active_syms.insert(s) {
                    new_syms.insert(s);
                }
            }
        }
    }

    let mut depth = 0;
    while !new_syms.is_empty() {
        if let Some(dl) = depth_limit
            && depth >= dl
        {
            break;
        }
        depth += 1;

        let mut next_new_syms = HashSet::new();
        for s in new_syms {
            if let Some(triggered_items) = triggers.get(&s) {
                for &i in triggered_items {
                    if active_items.insert(i) {
                        // Added new item
                        for &new_s in &item_syms[i] {
                            if active_syms.insert(new_s) {
                                next_new_syms.insert(new_s);
                            }
                        }
                    }
                }
            }
        }
        new_syms = next_new_syms;
    }

    // Collect result
    let mut result = Vec::with_capacity(active_items.len());
    for (i, item) in items.iter().enumerate() {
        if active_items.contains(&i) {
            result.push(item.clone());
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use mrs_core::clause::{Clause, ClauseId, ClauseSource, Literal};
    use mrs_core::formula::Atom;
    use mrs_core::symbol::SymbolTable;

    #[test]
    fn test_sine_filter_clauses() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let q = syms.intern("q");

        // Conjecture clause containing q
        let c_conj = Clause::new(
            ClauseId(1),
            vec![Literal::pos(Atom::pred(q, vec![]))],
            ClauseSource::Input {
                name: "c1".into(),
                role: "conjecture".into(),
            },
        );

        // Axiom clause containing p and q (relational link)
        let c_ax1 = Clause::new(
            ClauseId(2),
            vec![
                Literal::pos(Atom::pred(p, vec![])),
                Literal::neg(Atom::pred(q, vec![])),
            ],
            ClauseSource::Input {
                name: "ax1".into(),
                role: "axiom".into(),
            },
        );

        // Unrelated axiom clause containing other symbols
        let r = syms.intern("r");
        let c_ax2 = Clause::new(
            ClauseId(3),
            vec![Literal::pos(Atom::pred(r, vec![]))],
            ClauseSource::Input {
                name: "ax2".into(),
                role: "axiom".into(),
            },
        );

        let clauses = vec![c_conj.clone(), c_ax1.clone(), c_ax2.clone()];

        // Filter with tolerance 2.0, depth 2
        let filtered = filter_items(&clauses, 2.0, Some(2));

        // We expect the conjecture and linked ax1 to be retained, but unrelated ax2 to be filtered out!
        let filtered_ids: Vec<u64> = filtered.iter().map(|c| c.id.0).collect();
        assert!(filtered_ids.contains(&1));
        assert!(filtered_ids.contains(&2));
        assert!(!filtered_ids.contains(&3));
    }

    /// Builds `axioms` unrelated axiom clauses plus one conjecture clause whose
    /// symbol is shared with none of them, which is the HWV090-1 shape: a
    /// single goal clause over a design whose goal symbols are too frequent to
    /// trigger anything.
    fn unlinked_problem(syms: &mut SymbolTable, axioms: usize) -> Vec<Clause> {
        let goal = syms.intern("goal_signal");
        let mut clauses = Vec::with_capacity(axioms + 1);
        for i in 0..axioms {
            let fresh = syms.intern(&format!("design_wire_{i}"));
            clauses.push(Clause::new(
                ClauseId(i as u64 + 1),
                vec![Literal::pos(Atom::pred(fresh, vec![]))],
                ClauseSource::Input {
                    name: format!("ax{i}"),
                    role: "axiom".into(),
                },
            ));
        }
        clauses.push(Clause::new(
            ClauseId(axioms as u64 + 1),
            vec![Literal::pos(Atom::pred(goal, vec![]))],
            ClauseSource::Input {
                name: "negated_conjecture".into(),
                role: "negated_conjecture".into(),
            },
        ));
        clauses
    }

    /// The failure this guard exists for: a goal clause that triggers nothing
    /// leaves the search holding the goal and no premise, so it derives nothing
    /// and exits immediately. SInE must be refused rather than applied.
    #[test]
    fn unlinked_goal_is_refused_as_no_growth() {
        let mut syms = SymbolTable::new();
        let clauses = unlinked_problem(&mut syms, 300);
        let application = apply_sine_filter(&clauses, 1.5, Some(3), &SinePolicy::default());
        assert_eq!(application.skip, Some(SineSkip::NoGrowth));
        assert!(!application.applied);
        assert_eq!(application.seeds, 1);
        // A refusal hands back the whole clause set, unfiltered.
        assert_eq!(application.items.len(), clauses.len());
    }

    /// A clause set below `min_clauses` is not worth restricting whatever the
    /// filter would return.
    #[test]
    fn small_clause_set_is_refused_as_too_small() {
        let mut syms = SymbolTable::new();
        let clauses = unlinked_problem(&mut syms, 4);
        let application = apply_sine_filter(&clauses, 1.5, Some(3), &SinePolicy::default());
        assert_eq!(application.skip, Some(SineSkip::TooSmall));
        assert_eq!(application.items.len(), clauses.len());
    }

    /// The pre-existing guard: a filter that removes nothing cannot pay for the
    /// completeness restriction it imposes, so it is switched off.
    ///
    /// Every clause carries the same single symbol, so every item's minimum
    /// generality is the total count and the trigger threshold covers all of
    /// them: the whole set is reachable from the goal and nothing is filtered.
    #[test]
    fn filter_that_removes_nothing_is_refused() {
        let mut syms = SymbolTable::new();
        let q = syms.intern("q");
        let mut clauses: Vec<Clause> = (0..200)
            .map(|i| {
                Clause::new(
                    ClauseId(i + 1),
                    vec![Literal::pos(Atom::pred(q, vec![]))],
                    ClauseSource::Input {
                        name: format!("c{i}"),
                        role: "axiom".into(),
                    },
                )
            })
            .collect();
        clauses.push(Clause::new(
            ClauseId(201),
            vec![Literal::neg(Atom::pred(q, vec![]))],
            ClauseSource::Input {
                name: "negated_conjecture".into(),
                role: "negated_conjecture".into(),
            },
        ));
        let application = apply_sine_filter(&clauses, 2.0, Some(5), &SinePolicy::default());
        assert_eq!(application.skip, Some(SineSkip::NothingRemoved));
        assert!(!application.applied);
    }

    /// The starvation guard the user asked for: a filter that keeps a workable
    /// neighbourhood is applied, and one that keeps almost nothing is refused
    /// even though it did grow past the seed set.
    #[test]
    fn starvation_threshold_gates_a_growing_filter() {
        let mut syms = SymbolTable::new();
        // One linked axiom and 400 unrelated ones. `min_premises = 2` is met
        // (goal + 1), so the decision falls to the removed percentage.
        let clauses = {
            let mut v = unlinked_problem(&mut syms, 400);
            let link = syms.intern("goal_signal");
            v.insert(
                0,
                Clause::new(
                    ClauseId(9000),
                    vec![
                        Literal::pos(Atom::pred(link, vec![])),
                        Literal::neg(Atom::pred(syms.intern("design_wire_0"), vec![])),
                    ],
                    ClauseSource::Input {
                        name: "link".into(),
                        role: "axiom".into(),
                    },
                ),
            );
            v
        };

        let lenient = SinePolicy {
            max_removed_percent: 100,
            ..SinePolicy::default()
        };
        let applied = apply_sine_filter(&clauses, 1.5, Some(3), &lenient);
        assert!(applied.applied, "expected the filter to be applied");
        assert!(applied.kept > applied.seeds);

        let strict = SinePolicy {
            max_removed_percent: 50,
            ..SinePolicy::default()
        };
        let refused = apply_sine_filter(&clauses, 1.5, Some(3), &strict);
        assert_eq!(refused.skip, Some(SineSkip::Starved));
        assert_eq!(refused.items.len(), clauses.len());
    }

    #[test]
    fn starvation_threshold_compares_fraction_without_rounding_down() {
        let mut syms = SymbolTable::new();
        // The goal, a linked axiom, and its premise survive: 3 of 200 clauses,
        // so 98.5% are removed. A 98% threshold must refuse this filter.
        let mut clauses = unlinked_problem(&mut syms, 198);
        let link = syms.intern("goal_signal");
        clauses.push(Clause::new(
            ClauseId(10_000),
            vec![
                Literal::pos(Atom::pred(link, vec![])),
                Literal::neg(Atom::pred(syms.intern("design_wire_0"), vec![])),
            ],
            ClauseSource::Input {
                name: "link".into(),
                role: "axiom".into(),
            },
        ));
        let policy = SinePolicy {
            max_removed_percent: 98,
            ..SinePolicy::default()
        };

        let application = apply_sine_filter(&clauses, 1.5, Some(3), &policy);
        assert_eq!(application.kept, 3);
        assert_eq!(application.skip, Some(SineSkip::Starved));
    }

    /// A filter that keeps a real neighbourhood must stay applied under the
    /// shipped policy, or the guard has disabled SInE everywhere and the
    /// divisions it was tuned for lose coverage.
    #[test]
    fn default_policy_keeps_a_workable_filter() {
        let mut syms = SymbolTable::new();
        let q = syms.intern("q");
        // A star-shaped problem: the goal links to a chain of premises, and a
        // large block of unrelated axioms is filtered away.
        let mut clauses = vec![Clause::new(
            ClauseId(1),
            vec![Literal::pos(Atom::pred(q, vec![]))],
            ClauseSource::Input {
                name: "negated_conjecture".into(),
                role: "negated_conjecture".into(),
            },
        )];
        let mut previous = q;
        for i in 0..20 {
            let next = syms.intern(&format!("link_{i}"));
            clauses.push(Clause::new(
                ClauseId(i + 2),
                vec![
                    Literal::neg(Atom::pred(previous, vec![])),
                    Literal::pos(Atom::pred(next, vec![])),
                ],
                ClauseSource::Input {
                    name: format!("chain{i}"),
                    role: "axiom".into(),
                },
            ));
            previous = next;
        }
        for i in 0..500 {
            clauses.push(Clause::new(
                ClauseId(1000 + i),
                vec![Literal::pos(Atom::pred(
                    syms.intern(&format!("junk_{i}")),
                    vec![],
                ))],
                ClauseSource::Input {
                    name: format!("junk{i}"),
                    role: "axiom".into(),
                },
            ));
        }

        let application = apply_sine_filter(&clauses, 2.0, Some(5), &SinePolicy::default());
        assert!(
            application.applied,
            "default policy refused a workable filter: {:?}",
            application.skip
        );
        assert!(application.kept < application.before);
    }

    /// The measured reason the percentage guard ships disabled.
    ///
    /// `CSR051-10` is a solved `mrs` row on which SInE keeps 28 clauses out of
    /// 44 219 — it removes 99.9 %. Any `max_removed_percent` below that would
    /// switch SInE off on a problem the division wins with it, so the default is
    /// 100 and the starvation criteria are `NoGrowth` and `min_premises`.
    /// Pinned so that raising the guard is a deliberate act with this number in
    /// front of the author, not a tightening of a literal.
    #[test]
    fn default_policy_does_not_refuse_a_99_percent_filter() {
        let mut syms = SymbolTable::new();
        let q = syms.intern("q");
        let mut clauses = vec![Clause::new(
            ClauseId(1),
            vec![Literal::pos(Atom::pred(q, vec![]))],
            ClauseSource::Input {
                name: "negated_conjecture".into(),
                role: "negated_conjecture".into(),
            },
        )];
        // 28 reachable premises, then a large unreachable block.
        let mut previous = q;
        for i in 0..27 {
            let next = syms.intern(&format!("live_{i}"));
            clauses.push(Clause::new(
                ClauseId(i + 2),
                vec![
                    Literal::neg(Atom::pred(previous, vec![])),
                    Literal::pos(Atom::pred(next, vec![])),
                ],
                ClauseSource::Input {
                    name: format!("chain{i}"),
                    role: "axiom".into(),
                },
            ));
            previous = next;
        }
        for i in 0..44_000 {
            clauses.push(Clause::new(
                ClauseId(100_000 + i),
                vec![Literal::pos(Atom::pred(
                    syms.intern(&format!("dead_{i}")),
                    vec![],
                ))],
                ClauseSource::Input {
                    name: format!("dead{i}"),
                    role: "axiom".into(),
                },
            ));
        }

        let application = apply_sine_filter(&clauses, 1.5, Some(3), &SinePolicy::default());
        assert!(
            application.applied,
            "default policy refused a 99.9% filter: {:?}",
            application.skip
        );
        let removed_percent = (application.before - application.kept) * 100 / application.before;
        assert!(
            removed_percent >= 99,
            "expected the fixture to remove >=99%, it removed {removed_percent}%"
        );
    }

    /// `MRS_SINE_MAX_REMOVED_PERCENT` must ignore values outside 0..=100 rather
    /// than clamping them into a policy nobody asked for. Keep this parsing
    /// check independent of process-global environment state: Rust tests run
    /// concurrently.
    #[test]
    fn max_removed_percent_env_override_is_bounded() {
        assert_eq!(parse_max_removed_percent("40"), Some(40));
        assert_eq!(parse_max_removed_percent(" 0 "), Some(0));
        assert_eq!(parse_max_removed_percent("100"), Some(100));
        assert_eq!(parse_max_removed_percent("250"), None);
        assert_eq!(parse_max_removed_percent("not-a-number"), None);
    }
}
