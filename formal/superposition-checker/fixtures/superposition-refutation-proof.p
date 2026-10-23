% Proof : superposition-refutation-problem.p
cnf(a,axiom,f(a)=b,file('superposition-refutation-problem.p',a)).
cnf(t,axiom,p(f(a)),file('superposition-refutation-problem.p',t)).
cnf(n,axiom,~p(b),file('superposition-refutation-problem.p',n)).
cnf(s,plain,p(b),inference(superposition,[status(thm)],[a,t])).
cnf(bot,plain,$false,inference(resolution,[status(thm)],[s,n])).
