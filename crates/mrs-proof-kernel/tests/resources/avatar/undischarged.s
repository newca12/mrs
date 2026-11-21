% Proof : satisfiable.p
cnf(np,axiom,~p(a),file('satisfiable.p',np)).
cnf(top,plain,p(a)|~p(a),inference(tautology,[status(thm)],[])).
cnf(split,plain,spl0_1|spl0_2,inference(avatar_split_clause,[status(esa),avatar_split([branch(0,spl0_1,[0]),branch(1,spl0_2,[1])],[])],[top])).
cnf(comp_p,plain,p(a)|~spl0_1,inference(avatar_component_clause,[status(esa),avatar_component(split,0,spl0_1)],[split])).
cnf(empty_p,plain,~spl0_1,inference(resolution,[status(thm)],[comp_p,np])).
cnf(branch_p,plain,$false,inference(avatar_branch_refutation,[status(esa),avatar_context([spl0_1])],[empty_p])).
