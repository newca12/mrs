# SAT simplifier and selector autosearch review

Reviewed feature revision: `d3d71444` (`feat/sat-subsumption-resolution`).
The review repairs do not promote a selector, schedule, or SAT backend default.

## SAT simplification

The backend builds one sparse set of one-way literal matches, including both
equality orientations. Its ordinary and SR queries share one bounded encoding.
The encoding requires consistent source-variable bindings, one selected match
per source literal, and, in SR mode, exactly one complementary target occurrence
with no positive match using that occurrence. Returned choices are validated
before deletion. Deadline/cancellation/encoding exhaustion is a no-op, not
evidence for satisfiability. AVATAR-context subset guards remain at the callers.

Added differential checks against the separately implemented legacy clause
matcher for 54 bounded variable/constant/polarity combinations, and explicit
expired/cancelled-query tests. Existing tests cover equality orientation, wide
targets, assumption reuse, and an SR inference accepted by the proof kernel.
Removed a redundant SAT-mode assumption. This is an adaptation of Coutelier et
al., not a reproduction of their substitution-aware SAT solver: plain
subsumption follows MRS's existing bounded set-style matching contract, whereas
the paper defines multiset subsumption. Feature-vector/width guards may miss
set-style simplifications, but must never turn a miss into a positive verdict.

## Autosearch

The companion selector DSL stays data-only and opt-in. The review repairs:

- refuse certification credit when a prover exits unsuccessfully, even if its
  output says `self_check=Certified`;
- preserve `VerifiedBad`/`Unknown` audit verdicts instead of hiding them as an
  exit failure; only a successful `VerifiedGood` is credited;
- reserve memory for both a prover and its overlapping certificate subprocess;
- launch children in private Unix process groups and kill/reap the group on
  outer timeout/error, rather than leaving the certificate worker running.

The development/holdout split, selectors, binary hashes, limits, and copied
corpus remain identity-bound. External TPTP includes are path-bound, not
recursively hash-bound. The experiment is a local candidate-generation screen,
not a CASC score or promotion result. Full canonical cooperative campaigns are
not part of this review.

## Verification and separate follow-up

The baseline workspace suite passed. The repaired branch is gated by `cargo
check`, `cargo clippy --all -- -D warnings`, `cargo fmt --all --check`, and
`cargo test --workspace` in the Nix development environment before the local
merge. Compiler checks do not establish general search completeness.

The previously reproduced AVATAR kernel failures, BCE L-resolvent omission,
and index arity truncation are pre-existing at `3f575ac5`; they are separate
post-merge repairs. In particular, old self-check/strict results are not
independent evidence against those newly identified checker failure shapes.
