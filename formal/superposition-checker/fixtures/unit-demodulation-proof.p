cnf(target,axiom,p(f(a)),file('input',target)).
cnf(equality,axiom,a=b,file('input',equality)).
cnf(negative,axiom,~p(f(b)),file('input',negative)).
cnf(rewritten,plain,p(f(b)),inference(demodulation,[status(thm)],[target,equality])).
cnf(bot,plain,$false,inference(resolution,[status(thm)],[rewritten,negative])).
