% Function-free relational EPR satisfiability can be certified by the
% bounded ordered-resolution fragment. The certifier uses one fresh domain
% constant because this problem contains no explicit constants.
cnf(all_p, axiom, p(X)).
cnf(p_implies_not_q, axiom, ~p(X) | ~q(X)).
