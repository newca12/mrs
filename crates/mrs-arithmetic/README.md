# mrs-arithmetic

Bounded, exact evaluation of TPTP integer and rational expressions. This fixes
the defined-word satisfiability gap: `$less(1,1)` must be false, not a predicate
whose interpretation a finite-model search is free to choose.

## Supported semantics

| Operations | Support |
| --- | --- |
| `$uminus`, `$abs`, `$sum`, `$difference`, `$product` | Exact integer/rational arithmetic |
| `$quotient` | Exact division; integer operands produce a rational |
| `$quotient_e`, `_t`, `_f` and corresponding `$remainder_*` | Euclidean, truncating, and floor division, including negative divisors |
| `$floor`, `$ceiling`, `$truncate`, `$round` | Preserve the operand sort; halfway rounding goes to the even integer |
| `$to_int`, `$to_rat` | Explicit conversions; no implicit mixing of `$int` and `$rat` |
| `$less`, `$lesseq`, `$greater`, `$greatereq`, numeric equality | Exact comparisons within one numeric sort |
| `$is_int`, `$is_rat`, numeric `$distinct` | Exact recognition and pairwise distinctness |

Values use `BigInt` and `BigRational`. Integral rationals remain rational: `2/1`
is not an integer-sorted `2`. Quoted user symbols such as `'$sum'` and `'1'` are
not interpreted as arithmetic tokens.

`$to_int` floors a negative nonintegral rational, while `$truncate` rounds toward
zero. This follows the v10 corpus regression `ARI525_1` (and the floor wording
in the TPTP arithmetic table). The table's separate truncation identity is
inconsistent for negative fractions; the corpus distinction is pinned in tests.

## Supported problem fragment

The AST evaluator handles closed, entirely evaluatable arithmetic/Boolean
formulas in TFF, TCF, FOF, and CNF. FOF/CNF arithmetic is a compatibility
extension; standard TPTP arithmetic belongs in typed languages. It also knows
the four order predicates' reflexive cases on identical nullary arguments,
under TPTP's well-typed arithmetic usage. It is not a global TFF type checker.

This is **not a complete arithmetic decision procedure**. In particular:

- General quantified/symbolic arithmetic is unsupported.
- Ground numeric terms nested inside arbitrary uninterpreted applications,
  such as `p($sum(1,2))`, are not normalized by this initial integration.
- Reals and `$to_real` are unsupported; they are never approximated with floats.
- Division/remainder by zero is unspecified, not assigned a convenient value.
- Unknown defined/system words, mismatched operand sorts, and unsupported
  conditional/let/higher-order constructs are not given arbitrary meanings.

The main and selected included formulas use the same lowering path. A supported
formula is discharged to a Boolean; an unevaluated interpreted formula is
counted as unsupported. Unsupported goals cause `GaveUp`. Unsupported axioms
or unresolved includes prohibit satisfiability claims, although a supported
subset can still supply a refutation. Low-level CNF/TCF adapters also refuse
interpreted content rather than interning it as ordinary atoms.

## Certification and resources

The strict proof kernel recomputes original source arithmetic before accepting
normalized input leaves. Model validation checks fixed standard arithmetic,
conjecture polarity, and the complete selected include closure. It does not
interpret `$less`, numbers, or arithmetic functions through finite tables.
An empty residual clause set gets an explicit nonempty model witness when its
uninterpreted signature is empty; if tautology elimination leaves source
symbols with no clauses, the prover omits an incomplete model certificate.

Arithmetic is shared trusted source-normalization code, **not covered by the
Verus decoded-IR theorem**. No new arithmetic inference rule or completeness
claim has been added to the general superposition calculus.

Default evaluation limits are 100,000 AST nodes, depth 256, 65,536 bits per
numeric component, and 10,000,000 estimated arithmetic work units. Callers can
also supply a deadline. Resource exhaustion returns `ResourceLimit`, never a
rounded, overflowing, or guessed value. Model include loading caps the complete
source/selected-text closure at 16 MiB, 4,096 file visits, and depth 64.

## Verification

```bash
nix develop -c cargo test -p mrs-arithmetic
nix develop -c cargo test -p mrs-proof-kernel --test arithmetic
nix develop -c cargo test -p mrs --test arithmetic_cli
nix develop -c cargo run -p mrs-arithmetic --example check_corpus -- /path/to/TPTP-v10.0.0/Problems/ARI
```

The corpus diagnostic evaluated 150 of the 700 v10 ARI files and matched all 150
declared statuses, with zero disagreements. The other 550 files are unsupported
or unclassified, not claimed as solves. This is evaluator semantic regression
testing, not a timed prover benchmark or a CASC score.

CLI regression tests cover the original wrong-satisfiability examples, exact
theorems with strict replay, standard-arithmetic models, negative conversions,
unsupported inputs, and selected includes. Unit tests exhaustively check small
signed integer quotients/remainders, rational division identities, ties-to-even,
arbitrary precision, sort separation, malformed values, and resource limits.

The final Nix-wrapped workspace run passed 1,675 tests in 80 suites, along with
`cargo check`, workspace all-target checking, formatting, canonical clippy, and
targeted arithmetic/parser/integration lints. Public `mrs-proover --strict`
audits also returned `VerifiedGood` for both an emitted rational-sum theorem
proof and the `ARI525_1` arithmetic counter-model.

All 150 supported ARI files were subsequently run through the debug prover with
`--hardware adaptive --workers 1 --strategy 1 --time 10 --certify-ordered --self-check`.
All 150 native statuses matched their headers, and all 150 emitted proof/model
outputs returned `VerifiedGood` from the public strict auditor. This is a local
semantic regression run, not a standardized performance measurement or CASC score.
