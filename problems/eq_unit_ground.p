% Ground unit-equality normalization path for certified EPR+Eq.
% a = b, p(a) |- p(b).  Unit ground equalities only; no function symbols.
% Exercises equality_normalization via expand_equality + ordered resolution.
% Status: Theorem

fof(ax_eq, axiom, a = b).
fof(ax_p, axiom, p(a)).
fof(goal, conjecture, p(b)).
