% Proof : false-conjecture.p
cnf(np,axiom,~p,file('false-conjecture.p',np)).
fof(goal,conjecture,p,file('false-conjecture.p',goal)).
cnf(bot,plain,$false,inference(resolution,[status(thm)],[np,goal])).
