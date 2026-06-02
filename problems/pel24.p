% Pelletier problem #24 (simplified)
% ~(?[X]: (s(X) & q(X))) &
% (![X]: (p(X) => (q(X) | r(X)))) &
% (~(?[X]: (p(X))) => ?[X]: (q(X))) &
% (![X]: ((q(X) | r(X)) => s(X)))
% => ?[X]: (p(X) & r(X))
% Status: Theorem

fof(pel24, conjecture,
    ((~(?[X]: (s(X) & q(X))) &
      (![X]: (p(X) => (q(X) | r(X)))) &
      ((~(?[X]: p(X))) => (?[X]: q(X))) &
      (![X]: ((q(X) | r(X)) => s(X))))
     => (?[X]: (p(X) & r(X))))).
