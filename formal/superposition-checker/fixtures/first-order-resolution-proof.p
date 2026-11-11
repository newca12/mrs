cnf(a,axiom,p(X)|q(X),file('input',a)).
cnf(b,axiom,~p(a),file('input',b)).
cnf(c,axiom,~q(a),file('input',c)).
cnf(r1,plain,q(a),inference(resolution,[status(thm)],[a,b])).
cnf(bot,plain,$false,inference(resolution,[status(thm)],[r1,c])).
