fof(fact_a,axiom,is_a_theorem(a)).
fof(fact_ab,axiom,is_a_theorem(implies(a,b))).
fof(condensed_detachment,axiom,
    ! [X,Y] : (is_a_theorem(Y) <= (is_a_theorem(X) & is_a_theorem(implies(X,Y))))).
fof(goal,conjecture,is_a_theorem(b)).
