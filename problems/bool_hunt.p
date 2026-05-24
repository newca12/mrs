% Boolean algebra: Huntington axiom
% Show that in a Boolean algebra (with complement, join, meet),
% complement of complement is identity.

fof(join_comm, axiom, ![X,Y]: join(X, Y) = join(Y, X)).
fof(join_assoc, axiom, ![X,Y,Z]: join(join(X, Y), Z) = join(X, join(Y, Z))).
fof(huntington, axiom, ![X,Y]: comp(comp(join(X, Y)), comp(join(X, comp(Y)))) = X).
fof(goal, conjecture, ![X]: comp(comp(X)) = X).
