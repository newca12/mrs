cnf(target,axiom,p(f(a)),file('input',target)).
cnf(ab,axiom,a=b,file('input',ab)).
cnf(bc,axiom,b=c,file('input',bc)).
cnf(negative,axiom,~p(f(c)),file('input',negative)).
cnf(demodulated,plain,p(f(c)),inference(demodulation,[status(thm)],[target,ab,bc])).
cnf(bot,plain,$false,inference(resolution,[status(thm)],[demodulated,negative])).
