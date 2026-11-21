# Term traversal and index cleanup — 2026-11-21

This is a local engine-efficiency study, not a CASC score, portfolio comparison,
or completeness result. The baseline is `c2f2f46b`. Shipped strategies, routing,
resource/pruning policies, SAT budgets and proof-checker semantics are unchanged.

## What the review selected

The current engine already has ID-only partner retrieval, negative demodulation
memoization, variable-maximum caching and several no-op substitution shortcuts.
Repeating those projects or replacing entire modules would not address the
remaining costs. The first slice targets repeated argument copying instead:

- `TermBank::map_app_args` reads immutable children by ID between callbacks.
  This is safe across arena growth without retaining pointers or eagerly
  cloning a node. It visits arguments in their original order, copies the
  argument list only on the first change and avoids interning an unchanged
  application. There is no unsafe code, new cache, or dynamic dispatch.
- Variable replacement, transitive substitution, flat matching substitution,
  chained matching substitution, renaming and demodulation RHS instantiation
  use that shared primitive. Flat substitution still does **not** chase a
  binding's replacement; transitive substitution still does. This distinction
  is important for rigid target variables and overlapping variable IDs.
- Literal demodulation updates its existing argument slots instead of creating
  two temporary argument vectors. A shallow rewrite path is held inline; an
  actual rewrite still owns the exact `Vec<usize>` witness path. Deep paths
  spill rather than truncate. A rewrite is recorded even if its resulting ID
  equals the starting ID, preserving the existing fixpoint/pass-limit behavior.
- STree unification visits only the matching symbol/arity and variable edges
  for a symbol query. Variable queries still visit every edge. The traversal
  order is unchanged; no candidate ordering or logical filter was tightened.
- Removed the demodulation memo's unused root-key/arity computation and corrected
  comments describing its storage and BTreeMap lookup costs.

## Profiling evidence

The fixture is the selected J13 `LAT141-1.p` plus the exact local TPTP v10.0.0
`LAT001-0.ax` bytes, frozen before measurement. This is not an identity claim
about the original J13 library. The host is an i7-10610U, four physical cores /
eight logical CPUs, NixOS WSL2; diagnostics use one worker.

Both builds use Rust 1.100.0, native target CPU, release fat LTO, one codegen
unit, debug information retained, and the same dependency lockfile. Sampling
with `perf record` was denied by the host, so Callgrind supplies instruction
attribution and **separate uninstrumented runs** supply timings.

At a 100-processed-clause ceiling, the baseline Callgrind run records
843,713,633 instructions. `subsumes_id_until` accounts for about 38% inclusively;
argument `SmallVec` cloning has substantial direct and descendant costs.
Recursive/inlined Callgrind rows overlap and must not be summed. This is why
the initial slice addresses substitution/renaming copies rather than solver
heuristics. The first stripped profiles and failed sampling attempts are not
used for per-function attribution.

## Measurement and verification

The complete artifacts are in `docs/results/perf/optimization-20261121/`.
Uninstrumented timing arms use one worker pinned to CPU 2, no sharing or LRS,
fixed 12-GiB address-space/RSS allowances and a 120-second safety guard. Each
pair passes the existing comparator's work-counter/selection-digest gate.
The larger-run pooled arms also match across both blocks, including retained
terms/proof clauses and forward-subsumption counts. Early refutation/saturation,
a different resource stop or a guard timeout is not a fixed-work speed result.

Ratios below are **candidate elapsed / baseline elapsed**; below 1 is faster.
All individual ratios, arguments, binary hashes and raw outputs are retained.

| Workload / cap | Randomized pairs | Mean ratio | Range | Faster candidate pairs |
|---|---:|---:|---:|---:|
| LAT141 / 200 | 5, seed 42 | 1.039 | 0.885–1.112 | 1/5 |
| Ring / 100 | 5, seed 42 | 0.994 | 0.954–1.032 | 3/5 |
| LAT141 / 400, initial block | 5, seed 123 | 0.976 | 0.853–1.241 | 4/5 |
| LAT141 / 400, replication | 5, seed 456 | 0.947 | 0.880–0.984 | 5/5 |
| LAT141 / 400, pooled | 10 | 0.961 | 0.853–1.241 | 9/10 |

The larger-run replication was added because the initial timings disagreed.
The pooled mean pair ratio corresponds to about 3.9% lower elapsed time, but
the short lattice screen is slower and the ring screen is essentially neutral.
This is **not a general wall-clock speedup claim** or a significance result.
It is a local efficiency/clarity improvement with mixed timing evidence; it
does not promote a schedule, algorithm or experimental policy. No memory
reduction is claimed: retained logical structures are unchanged.

At the same 100-clause profiled work, instruction count falls from 843,713,633
to 807,807,102 (**4.26%**). Both profiles select the same digest and have the
same processed/generated/passive/discard/memo counters and retained structures.
The 400-clause timing arms likewise share one digest, generate 8,185 clauses
and retain 1,276,030 terms / 60,970 proof clauses. These are diagnostics, not
certified-problem coverage or competition results.

Regression coverage includes independent legacy substitution/renaming oracles,
flat-versus-chained binding cases, unchanged IDs, forced arena growth, shared
subterms, arities through 512, exact index result order before/after removal,
and a 512-argument predicate with a spilled 13-entry rewrite path. Existing
AVATAR, index recall, inference, deadline and independent proof replay tests
remain required.

The final Nix-wrapped gates passed: `cargo check`,
`cargo clippy --all -- -D warnings`, `cargo fmt --all --check` and
`cargo test --workspace`, including doctests. The extra
`cargo clippy -p mrs-core -p mrs-index -p mrs-calculus --all-targets -- -D warnings`
also passed after four pre-existing FVI test-style warnings were cleaned up.
The final log is `cargo-final-verification.log.gz`; the earlier required-gate
log is retained separately. Formal sources, pins and fixtures were not changed;
this slice did not rerun Verus or expand its verified scope.

The rebuilt strict checker certifies both the preserved baseline and newly
produced candidate ring proofs as `VerifiedGood`; their input identities and
checker hash are retained. The complete stdout bytes differ only in the
elapsed-time footer. The SZS proof sections are byte-identical, with SHA-256
`fecb458b585666310a93a3786007d4390432ac0e1180158f385221d684fe7fcb`.
The unchanged derivation has 94 nodes / 12,954 proof bytes. This uncapped solve
is a proof regression, **not** a speed comparison: the earlier baseline solve
overlapped compilation. The capped timing arms intentionally stop before any
refutation.

## Next optimization priorities, not implemented here

1. **Allocation-free failure diagnostics in inference matching.** The profile
   includes formatting of rejected unification attempts. Keep the public
   diagnostic API, but investigate a shared compact internal failure result for
   hot callers that only need success/failure. Do not duplicate matching logic.
2. **Substitution work, not only copies.** Repeated flat application and occurs
   checks remain expensive. Trail-based matching or term-result memoization
   needs explicit substitution-lifetime/rigidity contracts and differential
   proof tests before implementation.
3. **Flattening and trie scratch space.** Measure reusable bounded buffers and
   child-skip metadata; preserve variable normalization, recall and exact
   candidate order. The current selective traversal is only a small slice.
4. **Proof/history retention and worker preparation.** Measure duplicate owning
   clause/witness memory before migrating to shared immutable storage. Preserve
   authoritative first-registration semantics and citation closure.
5. **Deadline overhead.** Clock checks are visible in the profile. Do not simply
   remove or thin them: any batching change needs adversarial cancellation and
   bounded-overrun evidence.

These are larger contract-bearing projects, not permission to promote a new
algorithm, cache policy, pruning rule or schedule from a local throughput test.
