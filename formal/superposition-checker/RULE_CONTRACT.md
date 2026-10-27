# Supported superposition and resolution step contract (prototype)

The compiled wire-v1 artifact now invokes this rule from the CNF TPTP/MRS bridge;
see `INTERCHANGE.md` for the trusted decoding/normalization boundary. Step support
also includes **first-order binary resolution**: complementary positive-left/negative-right
occurrences of unified predicate or equality atoms under independent parent
standardization apart and a common simultaneous one-step unifier, with one pivot
removed per parent and left-context/right-context concatenation. Its theorem
`standardized_first_order_resolution_sound` and executable checker
`check_first_order_resolution_and_replay` prove universal validity preservation.
DAG support also includes **exact-atom resolution**: complementary positive-left/negative-right
occurrences of the identical predicate or equality atom, with exactly one pivot
removed per parent and left-context/right-context concatenation. Its theorem
`exact_atom_resolution_sound` and checker `check_exact_resolution` prove local
validity preservation without unification; DAG dispatch binds the actual cited
clauses. This includes unary/binary predicates and equality, not just nullary
predicates.

DAG support also includes **equality resolution** (reflexivity resolution):
from a clause with a negative equality literal `s != t` where `s` and `t` unify
under simultaneous unifier `sigma`, the conclusion removes the negative equality
literal and instantiates the remaining literals under `sigma`. Its theorem
`equality_resolution_sound` and checker `check_equality_resolution_and_replay`
prove universal validity preservation.

DAG support also includes **clause factoring** (positive/negative factoring):
from a clause `C | L1 | L2` where `L1` and `L2` have the same polarity and their
atoms unify under simultaneous unifier `sigma`, the conclusion removes `L2`
and instantiates the remaining literals under `sigma`. Its theorem
`clause_factoring_sound` and checker `check_clause_factoring_and_replay`
prove universal validity preservation.

DAG support also includes **equality factoring**:
from a clause `C | s = t | u = v` where `s` and `u` unify under simultaneous unifier
`sigma`, the conclusion derives `(C | t != v | s = t) sigma`. Its theorem
`equality_factoring_sound` and checker `check_equality_factoring_and_replay`
prove universal validity preservation.

DAG support also includes **subsumption resolution** (contextual literal elimination):
from target clause `C | L` and active clause `A | ~L'` where `L'` unifies with `L`
under unifier `sigma` and all other literals of `A sigma` match literals in `C`,
the conclusion removes `L` from the target clause without instantiating `C`. Its theorem
`subsumption_resolution_sound` and checker `check_subsumption_resolution_and_replay`
prove universal validity preservation.

DAG support also includes **clause weakening**:
from universally valid parent clause `C`, any clause `D` containing all literals
of `C` is derived without variable instantiation. Its theorem
`clause_weakening_sound` and checker `check_weakening_and_replay` prove universal
validity preservation.

DAG support also includes **demodulation** (unit equality rewriting):
from universally valid target clause `C` and unit positive equality parent `s = t`
(or `t = s`), replacing an occurrence of `s sigma` (or `t sigma`) at position `p`
in literal `k` of `C` by `t sigma` (or `s sigma`) derives `C` with literal `k`
rewritten, preserving all other literals and their positions. Its theorem
`demodulation_sound` and checker `check_demodulation_and_replay` prove universal
validity preservation.

DAG support also includes **clause instantiation**:
from universally valid parent clause `C` and arbitrary simultaneous substitution `sigma`,
the instantiated clause `C sigma` is derived. Its theorem `instantiation_sound`
(via `clause_valid_after_substitution`) and checker `check_instantiation_and_replay`
prove universal validity preservation.

DAG support also includes **clause condensation**:
from universally valid parent clause `C`, simultaneous substitution `sigma`,
and conclusion clause `D` containing all literals of `C sigma` (witnessed by
inclusions mapping each literal of `C` to an index in `D`), `D` is derived.
Its theorem `condensation_sound` and checker `check_condensation_and_replay`
prove universal validity preservation.

DAG support also includes **equality reflexivity**:
the unit equality clause `t = t` is derived as a universal tautology.
Its theorem `equality_reflexivity_sound` and checker `check_reflexivity_and_replay`
prove universal validity.

DAG support also includes **equality transitivity**:
from universally valid parent clause `C` containing a positive equality literal `s1 = s2` and unit positive
equality parent `t1 = t2`, the chained conclusion with that literal replaced by `u1 = u2` matching a transitive
orientation and all other literals preserved is derived. This covers both unit equality clauses and arbitrary
multi-literal clauses.
Its theorems `clause_transitivity_sound` and `equality_transitivity_sound` and checker `check_transitivity_and_replay`
prove universal validity preservation.

DAG support also includes **equality symmetry**:
from universally valid parent clause `C` containing an equality literal `s = t` (or `s != t`), the commuted
conclusion with that literal flipped to `t = s` (or `t != s`) and all other literals preserved is derived.
This covers both unit equality clauses and arbitrary multi-literal clauses.
Its theorems `clause_symmetry_sound` and `equality_symmetry_sound` and checker `check_symmetry_and_replay`
prove universal validity preservation.

DAG support also includes **clause excluded middle** (law of excluded middle):
for any first-order atom `A`, the binary clause `A | ~A` (or `~A | A`) is derived.
Its theorem `excluded_middle_sound` and checker `check_excluded_middle_and_replay`
prove universal validity.

DAG support also includes **equality congruence**:
from universally valid parent clause `C` containing a positive equality literal `s = t`, the congruent conclusion
with that literal replaced by `C[s] = C[t]` (or `C[t] = C[s]`) and all other literals preserved is derived.
This covers both unit equality clauses and arbitrary multi-literal clauses.
`C` is a nonempty single-hole context formed recursively from unary functions
and either argument of binary functions. Function symbols and all off-path
sibling terms must agree exactly; no substitution or simultaneous multi-hole
replacement is implicit. Nested contexts such as `h(g(c, f(_)), d)` are supported.
Its theorems `clause_congruence_sound` and `equality_congruence_sound` and checker `check_congruence_and_replay`
prove universal validity preservation via `oriented_congruence_preserves_value`.
The existing clause-structure preflight bounds syntax before recursive replay;
this extension does not claim exact cumulative comparison-work accounting.

DAG support also includes **clause tautology**:
any clause containing a reflexive equality literal `s = s` or a complementary pair
of literals `A` and `~A` is derived.
Its theorem `clause_tautology_sound` and checker `check_tautology_and_replay`
prove universal validity.

This document fixes the mathematical rule represented by
`SuperpositionCertificate` in `first_order_core.rs`. It is an **unordered
first-order superposition rule**: no KBO/LPO maximality or literal-selection
condition is required. Those are restrictions used by particular ordered
calculi for search properties, not premises needed for semantic soundness of an
individual inference. This contract does not claim completeness or termination
of the unordered calculus.

## Inputs and notation

Clauses are universally closed finite disjunctions of literals. A positive
source literal must be an equality `s = t`, oriented from `s` to `t`, with `s`
not a variable. The target is any literal `L` and one argument-slot/term-path
position `p` at which a subterm `u` occurs. The target literal may have either
polarity and may itself be equality. Equality is interpreted as identity.

The two parent clauses are independently standardized apart by finite,
simultaneous variable renamings `rho_s` and `rho_t`:

* every variable occurring in each parent has an explicit variable image;
* each map is injective on the variables of its parent; and
* the two image sets are disjoint.

The executable checker represents these renamings as supplied finite maps and
checks the conditions above. It does not choose fresh variable IDs itself.
Additional bindings outside a parent's variable set have no effect on the rule.

## Unifier and inference

The certificate supplies a finite substitution `sigma` with unique domain
keys. It is a **simultaneous, one-step substitution**: each variable occurrence
is replaced by its listed right-hand-side term, and variables inside that
right-hand side are not recursively substituted during this application. No
MGU property is required. In particular, target-parent variables may be bound,
and cyclic right-hand sides are meaningful under this declared one-step
convention; this checker is not claiming to compute or validate a recursively
closed MGU.

The certificate is accepted only if, after both freshenings and application of
the same `sigma` to the entire standardized parents,

```text
sigma(rho_s(s)) = sigma(rho_t(u)).
```

The conclusion is built in this exact sequence:

1. every source-parent literal except the selected equality, in original order,
   after `rho_s` and then `sigma`;
2. every target-parent literal after `rho_t` and then `sigma`, in original
   order, with the selected occurrence in `L` replaced by
   `sigma(rho_s(t))`.

Consequently the unifier is applied to **all** literals in both parents before
the selected target term is rewritten. The checker compares the supplied
conclusion against this sequence exactly. Literal permutation and alpha-
equivalence of the conclusion are not accepted by this prototype.

## Why these premises are sound

Independent standardization and substitution produce instances of the
universally closed parents, so both instantiated clauses remain valid. The
unifier equation makes the selected source left side and target subterm equal
in every interpretation and valuation. Since equality is identity,
congruence permits replacing that occurrence by the instantiated right side.
The disjunction of the remaining source literals, remaining target literals,
and rewritten target literal is therefore entailed by the parents. This is the
semantic argument formalized by `standardized_superposition_sound` and
`universally_closed_superposition_sound`; neither target-variable rigidity nor
ordered-calculus maximality is a soundness premise here.

## Prototype boundary

The executable checker validates the finite syntax, hygiene conditions,
supplied unifier equation, selected path, and exact replayed conclusion.
`replay_semantic_premises` now exposes the semantic premises on successful
checking, and `accepted_replay_preserves_validity` connects exact replay to
`standardized_superposition_sound`. The executable replay API postconditions
universal validity preservation. `check_dag_superposition_step` additionally
binds the certificate to its cited nodes and proves local DAG entailment.

The DAG contract consumes universally valid parents, not their truth at one
valuation, because substitutions can select different parent instances.
Whole-DAG IR acceptance is now proved by `certify_dag`: acceptance implies no
well-formed model of the supplied universally closed input clauses. Exact input
membership is checked; no runtime semantic-validity test is required.
The proof DAG also dispatches nullary and exact-atom resolution,
standardized-apart first-order resolution, equality resolution, clause and
equality factoring, subsumption resolution, weakening, demodulation, clause
instantiation, clause condensation, equality reflexivity, equality transitivity, equality symmetry, clause excluded middle, and equality congruence. Each rule's replay is bound to cited parents and conclusion and
has a universal-validity preservation postcondition. The nullary-resolution
slice uses positive-left/negative-right occurrences of the same nullary
predicate, deletes exactly one occurrence from each parent and preserves exact
left-context/right-context order, including duplicates. Complete
hygiene/resource refinement, conclusion comparison modulo proved clause
remain open. See `ASSURANCE.md` and
`supported-rules.json` for the current claim boundary.
