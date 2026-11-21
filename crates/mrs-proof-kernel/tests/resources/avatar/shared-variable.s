% Proof : satisfiable.p
cnf(np,axiom,~p(a),file('satisfiable.p',np)).
cnf(pb,axiom,p(b),file('satisfiable.p',pb)).
cnf(top,plain,p(X)|~p(X),inference(tautology,[status(thm)],[])).
cnf(split,plain,spl0_1|spl0_2,inference(avatar_split_clause,[status(esa),avatar_split([branch(0,spl0_1,[0]),branch(1,spl0_2,[1])],[])],[top])).
cnf(comp_p,plain,p(X)|~spl0_1,inference(avatar_component_clause,[status(esa),avatar_component(split,0,spl0_1)],[split])).
cnf(comp_q,plain,~p(X)|~spl0_2,inference(avatar_component_clause,[status(esa),avatar_component(split,1,spl0_2)],[split])).
cnf(empty_p,plain,~spl0_1,inference(resolution,[status(thm)],[comp_p,np])).
cnf(empty_q,plain,~spl0_2,inference(resolution,[status(thm)],[comp_q,pb])).
cnf(branch_p,plain,$false,inference(avatar_branch_refutation,[status(esa),avatar_context([spl0_1])],[empty_p])).
cnf(branch_q,plain,$false,inference(avatar_branch_refutation,[status(esa),avatar_context([spl0_2])],[empty_q])).
cnf(bot,plain,$false,inference(avatar_sat_refutation,[status(thm),avatar_sat_refutation([split],[branch_p,branch_q])],[split,branch_p,branch_q])).
