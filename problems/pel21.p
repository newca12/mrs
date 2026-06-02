% Pelletier problem #21
% ((?[X]: (p => f(X))) & (?[X]: (f(X) => p))) => ?[X]: (p <=> f(X))
% Note: this is NOT a theorem — it's actually invalid.
% The correct statement is that it IS a theorem in 2-valued logic.
% But interpreted carefully: the X in the conclusion need not be the same.
% Actually Pelletier 21 IS a theorem. Let's state it properly.
% Status: Theorem

fof(pel21, conjecture,
    ((?[X]: (p => f(X))) & (?[X]: (f(X) => p))) => ?[X]: (p <=> f(X))).
