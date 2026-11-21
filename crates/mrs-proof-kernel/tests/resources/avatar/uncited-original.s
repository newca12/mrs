% Proof : satisfiable.p
% The digest and LRAT trace are valid for the forged stronger manifest.
cnf(np,axiom,~p(a),file('satisfiable.p',np)).
cnf(top,plain,p(a)|~p(a),inference(tautology,[status(thm)],[])).
cnf(split,plain,spl0_1|spl0_2,inference(avatar_split_clause,[status(esa),avatar_split([branch(0,spl0_1,[0]),branch(1,spl0_2,[1])],[])],[top])).
cnf(comp_p,plain,p(a)|~spl0_1,inference(avatar_component_clause,[status(esa),avatar_component(split,0,spl0_1)],[split])).
cnf(empty_p,plain,~spl0_1,inference(resolution,[status(thm)],[comp_p,np])).
cnf(branch_p,plain,$false,inference(avatar_branch_refutation,[status(esa),avatar_context([spl0_1])],[empty_p])).
cnf(bot,plain,$false,inference(avatar_sat_refutation,[status(thm),avatar_sat_refutation([split],[branch_p],sat_trace('lrat',2,'2c9cf8e78e2f3f11759d4e827bf96194bd7ac4b944983c1453c823e23ee6cb13',[1,2,3],[0,1],[[1,2],[-1],[]],'342030203320300a'))],[split,branch_p])).
