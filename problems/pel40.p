% Pelletier #40
% (exists y, forall x, f(x,y) <=> f(x,x)) => ~(forall x, exists y, forall z, f(z,y) <=> ~f(z,x))

fof(goal, conjecture,
    (?[Y]: ![X]: (f(X,Y) <=> f(X,X)))
    =>
    ~(![X]: ?[Y]: ![Z]: (f(Z,Y) <=> ~f(Z,X)))
).
