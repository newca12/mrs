% Function-free EPR, satisfiable, with a clause that is NOT symmetric in its
% two variables. This is the shape that breaks restricted variable renaming:
% the 16 ground instances over 4 constants induce 16 distinct clauses, so a
% grounder that keeps one representative per variable-renaming orbit drops
% six constraints and can return a model that does not satisfy the problem.
% Used by `satisfied_model_is_certified_by_the_kernel`.
cnf(column_surjectivity, axiom,
    ( ~ group_element(Y)
    | product(X,e_4,Y)
    | product(X,e_3,Y)
    | product(X,e_2,Y)
    | product(X,e_1,Y)
    | ~ group_element(X) ) ).

cnf(product_idempotence, axiom,
    product(X,X,X) ).

cnf(e_1_is_an_element, axiom, group_element(e_1)).
cnf(e_2_is_an_element, axiom, group_element(e_2)).
cnf(e_3_is_an_element, axiom, group_element(e_3)).
cnf(e_4_is_an_element, axiom, group_element(e_4)).

cnf(e_1_differs_from_e_2, axiom, ~ equalish(e_1,e_2)).
cnf(e_1_is_e_1, axiom, equalish(e_1,e_1)).
cnf(e_2_is_e_2, axiom, equalish(e_2,e_2)).
