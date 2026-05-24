
This represents approximately **388ms per formula** on average, which is extraordinarily slow. Normal parsing should be < 1ms per formula.

### Root Causes

1. **Excessive Backtracking**: The `alt()` combinator tries alternatives sequentially, causing redundant parsing
2. **Speculative Parsing in THF**: `thf_parenthesized` tries formula parsing, then backtracks to try type parsing
3. **Repeated `ws` Calls**: Whitespace parsing is called between every token
4. **Deep Recursion**: Deeply nested formulas create stack pressure
5. **Memory Allocation**: `Vec` growth during parsing causes reallocations
6. **No Memoization**: Same subexpressions may be parsed multiple times during backtracking

---

## Optimization Strategies

### Phase 1: Quick Wins (Estimated 2-5x improvement)

#### 1.1 Optimize `alt()` Order by Frequency

Reorder alternatives in `alt()` to put most common cases first (atomic formulas, simple quantified formulas before rare non-classical operators).

#### 1.2 Use Peek-Based Dispatch

Instead of trying each alternative, peek at the first character to dispatch directly to the correct parser.

#### 1.3 Reduce Whitespace Parsing

Combine multiple `ws` calls using tuples.

### Phase 2: Eliminate Speculative Parsing (Estimated 3-10x improvement)

#### 2.1 Fix THF Parenthesized Backtracking

Use lookahead to determine formula vs type instead of trying both.

#### 2.2 Committed Parsing After Keywords

After parsing `$ite`, `$let`, etc., commit immediately with `cut_err`.

### Phase 3: Structural Optimizations (Estimated 2-5x improvement)

#### 3.1 Inline Hot Paths
#### 3.2 Avoid Intermediate Allocations (fold during parsing)
#### 3.3 Pre-allocate Vectors

### Phase 4: Advanced Optimizations

#### 4.1 Parallel Formula Parsing
#### 4.2 Lazy AST Construction
#### 4.3 Memoization / Packrat Parsing

### Phase 5: Architecture Changes

#### 5.1 Two-Pass Parsing (tokenize then parse)
#### 5.2 Streaming Parser (process formulas one at a time)

---

## Implementation Priority

| Phase | Effort | Impact | Priority |
|-------|--------|--------|----------|
| 1.1 Reorder alt() | Low | Medium | P0 |
| 1.2 Peek dispatch | Medium | High | P0 |
| 2.1 Fix THF backtrack | High | Very High | P0 |
| 3.2 Avoid allocations | Medium | Medium | P2 |
| 4.1 Parallel parsing | High | High | P2 |

---

## Profiling Strategy

1. **Flamegraph**: `cargo flamegraph --example parse_file`
2. **Add timing instrumentation** to identify hot paths
3. **Count parser invocations** to find excessive backtracking

## Expected Outcomes

| Scenario | Current | Target |
|----------|---------|--------|
| ITP237_2.p | 75 min | ~1-2 min |
| Throughput | 0.8 f/s | 30-50 f/s |