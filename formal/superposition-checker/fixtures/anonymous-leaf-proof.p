% Exact anonymous provenance is accepted only by matching a complete supplied premise.
cnf(positive,axiom,p(a),file('anonymous-leaf-problem.p',unknown),[]).
cnf(negative,axiom,~p(a),file('anonymous-leaf-problem.p',unknown),[]).
cnf(false,plain,$false,inference(resolution,[status(thm)],[positive,negative]),[]).
