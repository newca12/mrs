% AC subset superposition regression: needs the partial-target shape.
%
% `prod(a, X) = b` has to rewrite `prod(prod(a,b), c)`, which has three AC
% arguments against the rule's two. The rigid whole-subterm AC unifier cannot
% express that: it aligns `a` against `prod(a,b)` and fails. Only the subset
% rule gets there, by consuming `{a, X}` out of `{a, b, c}` with `X := b`.
%
% Expected on `main`: GaveUp. Expected with the AC subset rule: Theorem — whose
% key node is an `ac_superposition` the strict kernel cannot replay, because
% `ac_superposition_replay` requires equal flattened arities (see UI-1 in
% docs/policies/unresolved-issues.md).

fof(comm, axiom, ![X, Y]: prod(X, Y) = prod(Y, X)).
fof(assoc, axiom, ![X, Y, Z]: prod(prod(X, Y), Z) = prod(X, prod(Y, Z))).
fof(rule, axiom, ![X]: prod(a, X) = b).
fof(goal, conjecture, prod(prod(a, b), c) = prod(c, b)).