include('Axioms/LAT001-0.ax').
cnf(equation_H21,axiom,
    join(meet(X,Y),meet(X,Z)) = meet(X,join(meet(Y,join(X,meet(Y,Z))),meet(Z,join(X,Y)))) ).

cnf(prove_H6,negated_conjecture,
    meet(a,join(meet(a,join(b,meet(a,c))),meet(c,join(a,b)))) != meet(a,join(b,meet(a,c))) ).

