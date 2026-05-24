% Pelletier #33
% forall x, (p(a) & (p(x) => p(b))) => p(c)  <=>
% forall x, (~p(a) | p(x) | p(c)) & (~p(a) | ~p(b) | p(c))

fof(goal, conjecture,
    (![X]: ((p(a) & (p(X) => p(b))) => p(c)))
    <=>
    (![X]: ((~p(a) | p(X) | p(c)) & (~p(a) | ~p(b) | p(c))))
).
