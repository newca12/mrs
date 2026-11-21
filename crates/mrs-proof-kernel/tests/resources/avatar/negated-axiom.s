% Proof : satisfiable.p
cnf(pb,axiom,p(b),file('satisfiable.p',pb)).
cnf(bot,plain,$false,inference(resolution,[status(thm)],[pb,inference(assume_negation,[status(cth)],[pb])])).
