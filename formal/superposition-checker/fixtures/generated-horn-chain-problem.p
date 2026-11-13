cnf(seed_fact,axiom,seed(f(a))).
cnf(seed_rule,axiom,~seed(X)|middle(X)).
cnf(middle_rule,axiom,~middle(f(Y))|last(Y)).
cnf(last_negative,axiom,~last(a)).
