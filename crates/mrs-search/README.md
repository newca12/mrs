# mrs-search

Proof search engine with given-clause loop and strategy portfolio.

## Given-clause loop

The Otter-style loop maintains two sets of clauses — *processed* (already used for inference) and *unprocessed* (waiting) — and iterates:

1. Select the best clause from *unprocessed* according to `SelectionStrategy`.
2. Generate all inferences with *processed* clauses.
3. Simplify and filter new clauses (demodulation, subsumption).
4. Move the selected clause to *processed*; add new clauses to *unprocessed*.
5. Stop on empty clause (refutation), saturation, timeout, or resource limit.

## Strategy portfolio

`StrategySchedule::default_schedule(total_time)` runs 9 strategies serially, each with a fresh `SearchState`. Strategies vary clause selection, literal selection, and term ordering. The first refutation found wins.

## Key API

```rust
// Run a single strategy
search(state: &mut SearchState, config: SearchConfig) -> SearchResult

enum SearchResult { Refutation(ClauseId), Saturated, Timeout, ResourceOut }

// Portfolio
StrategySchedule::default_schedule(total_time: Duration) -> StrategySchedule

// Configuration
SearchConfig {
    time_limit: Duration,
    max_clauses: usize,          // default 50 000
    selection: SelectionStrategy, // AgeWeight(n), SmallestFirst, Fifo, …
    literal_selection: LiteralSelection,
    ordering: TermOrdering,
}
```

## Dependencies

`mrs-core`, `mrs-calculus`, `mrs-index`
