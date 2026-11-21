# Clause-Selection Autosearch

## Scope

`mrs-tools autosearch` is an experimental, local loop for searching the
given-clause selection policy while holding the selected base strategy's other
settings fixed. It searches the existing FIFO, weight, goal, unit, Horn, and
set-of-support queues, including their age/weight/goal interleavings. It does
not mutate or compile Rust source, change the inference calculus, or promote a
schedule automatically.

An optional seed file lets an LLM suggest selectors in the same small DSL. It
accepts **data only**, one expression per line; it never runs generated code.
That makes it useful for exploring configurations while keeping the prover and
the benchmark judge outside the model's control.

## Local workflow

Build the prover, strict auditor, and tool:

```bash
nix develop -c cargo build --release --bin mrs --bin mrs-proover --bin mrs_tools
```

Start with FNE strategy 12, two parallel one-worker prover processes, 2 GiB RSS
budgets, and five seconds per problem:

```bash
nix develop -c cargo run --release -p mrs-bench --bin mrs_tools -- autosearch \
  --corpus-dir crates/mrs-bench/problems/casc-30/FNE \
  --tptp-root "$HOME/TPTP-v10.0.0" \
  --schedule casc_fne --strategy 12 \
  --time 5 --train-sample 24 --max-candidates 40 \
  --jobs 2 --memory-mb 2048 \
  --output crates/mrs-bench/results/autosearch-fne-$(date +%Y%m%d)
```

`--train-sample 0` uses the full development split. `--seed-candidates FILE`
adds model-proposed expressions to the initial population; for example:

```text
age:3
goal:8
queue:age=1,weight=3,goal=1
queue:horn=2,age=1,weight=2
```

The accepted grammar is:

- `fifo`, `smallest`
- `age:N`, `goal:N` (`N` is a positive integer)
- `queue:age=N,weight=N,goal=N,unit=N,horn=N,sos=N`

Queue frequencies must be positive, queue names unique within an expression,
and the total frequency must fit in `u32`. Queue order is significant: it
defines the phase of the interleaving.

## Evaluation contract

- The corpus is hashed and split by the existing TPTP family-prefix policy.
  Development examples are sampled round-robin across families. The held-out
  families are not used to select mutations or the champion; keep them out of
  any LLM proposal prompts as well.
- Every candidate uses the same named schedule and exact base strategy, one
  search worker, the same time and memory limits, no cross-strategy sharing,
  and `mrs --self-check`. Only `Theorem`/`Unsatisfiable` answers with
  `self_check=Certified` count toward training fitness.
- Candidates are compared first by certified training coverage, then by median
  solve wall time. Mutations come from the best eight completed candidates so
  far. The fixed, seeded search is resumable with `--resume`;
  it refuses a changed corpus, binary, split, or option set.
- After search, the training winner and the declared baseline are run in
  randomized paired order on every held-out problem for at least two repeats.
  Emitted proofs are retained and passed to `mrs-proover --strict --only-mrs`;
  only `VerifiedGood` holdout proofs count as solves in `summary.json`.
- `--jobs` defaults to one process and cannot exceed half the detected physical
  cores (minimum one), because a self-checked run may overlap its search and
  certificate worker. Each child has an
  `MRS_MAX_MEMORY_MB` watchdog; Linux also applies `RLIMIT_AS` with 512 MiB of
  address-space headroom. The aggregate cap is checked against host and cgroup
  memory headroom. The defaults are conservative for the project's
  four-physical-core, 15-GiB WSL development host. Lower `--jobs` or
  `--memory-mb` if other work is running.

Artifacts are written incrementally to `runs.jsonl` and `state.json`, with the
selected-corpus manifest and binary digests in `experiment.json`. The selected
problem files are copied and hash-checked under `corpus/`, so a long run does
not read a changing source corpus. The external TPTP library path is recorded
but not recursively hashed; use an immutable, versioned TPTP tree and keep it
unchanged for the run and any resume. The host CPU model, physical-core count,
and Rust compiler version are also recorded.
Holdout stdout/stderr, strict-audit logs, and available proof files are retained
beneath `logs/` and `proofs/`; candidate rankings and paired results are in
`candidate-scores.json` and `summary.json`.

## Interpretation

This is a candidate-generation experiment on one local host, not a CASC score,
not an eight-worker cooperative-portfolio comparison, and not a schedule
promotion. The current search space tunes the existing queue interleaving; it
does not yet add arbitrary clause-feature scoring formulas. Use the paired,
strictly audited family holdout to decide whether a selector is worth a larger
cooperative test. A small or inconsistent delta should trigger more pairs or a
different held-out corpus, not promotion.
