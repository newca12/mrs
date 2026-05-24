% Pelletier #25
% There exists an x such that p(x), AND for all x, (f(x) => ~g(x)) and
% (p(x) => g(x) & f(x)), THEN there exists x such that p(x) and ~f(x).

fof(premise1, axiom, ?[X]: p(X)).
fof(premise2, axiom, ![X]: (f(X) => ~g(X))).
fof(premise3, axiom, ![X]: (p(X) => (g(X) & f(X)))).
fof(goal, conjecture, ?[X]: (p(X) & ~f(X))).
