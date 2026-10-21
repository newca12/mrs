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
DAG-dispatched inference rules include superposition and nullary-predicate
resolution. `check_nullary_resolution` postconditions validity preservation,
and `check_dag_resolution_step` composes that with the cited node parents and
conclusion. Pivots are positive-left/negative-right occurrences of the same
nullary predicate, with exactly one occurrence deleted from each parent and
exact left-context/right-context order, including duplicate literals. No
substitution, standardization or normalization is applied by this resolution
slice. Complete hygiene/resource refinement, conclusion comparison
modulo proved clause equivalences, an MRS interchange format, and c502 analysis
remain open. See `ASSURANCE.md` and
`supported-rules.json` for the current claim boundary.
