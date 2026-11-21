% Proof : satisfiable.p
cnf(np,axiom,~p(a),file('satisfiable.p',np)).
cnf(top,plain,p(a)|~p(a),inference(tautology,[status(thm)],[])).
cnf(split_a,plain,spl0_1|spl0_2,inference(avatar_split_clause,[status(esa),avatar_split([branch(0,spl0_1,[0]),branch(1,spl0_2,[1])],[])],[top])).
cnf(split_b,plain,spl0_1|spl0_2,inference(avatar_split_clause,[status(esa),avatar_split([branch(0,spl0_1,[1]),branch(1,spl0_2,[0])],[])],[top])).
cnf(comp_a,plain,p(a)|~spl0_1,inference(avatar_component_clause,[status(esa),avatar_component(split_a,0,spl0_1)],[split_a])).
cnf(comp_b,plain,p(a)|~spl0_2,inference(avatar_component_clause,[status(esa),avatar_component(split_b,1,spl0_2)],[split_b])).
cnf(empty_a,plain,~spl0_1,inference(resolution,[status(thm)],[comp_a,np])).
cnf(empty_b,plain,~spl0_2,inference(resolution,[status(thm)],[comp_b,np])).
cnf(branch_a,plain,$false,inference(avatar_branch_refutation,[status(esa),avatar_context([spl0_1])],[empty_a])).
cnf(branch_b,plain,$false,inference(avatar_branch_refutation,[status(esa),avatar_context([spl0_2])],[empty_b])).
cnf(bot,plain,$false,inference(avatar_sat_refutation,[status(thm),avatar_sat_refutation([split_a,split_b],[branch_a,branch_b])],[split_a,split_b,branch_a,branch_b])).
