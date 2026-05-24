% Pelletier #38 (Challenge problem)
% forall x, (p(a) & (p(x) => exists y, p(y) & r(x,y))) =>
%   exists z w, p(z) & r(x,w) & r(w,z)
% IS EQUIVALENT TO
% forall x, (~p(a) | p(x) | exists z w, p(z) & r(x,w) & r(w,z)) &
%           (~p(a) | ~(exists y, p(y) & r(x,y)) | exists z w, p(z) & r(x,w) & r(w,z))

fof(goal, conjecture,
    (![X]: ((p(a) & (p(X) => (?[Y]: (p(Y) & r(X,Y))))) =>
            (?[Z,W]: (p(Z) & r(X,W) & r(W,Z)))))
    <=>
    (![X]: ((~p(a) | p(X) | (?[Z,W]: (p(Z) & r(X,W) & r(W,Z)))) &
            (~p(a) | ~(?[Y]: (p(Y) & r(X,Y))) | (?[Z,W]: (p(Z) & r(X,W) & r(W,Z))))))
).
