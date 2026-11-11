cnf(ab,axiom,a=b,file('input',ab)).
cnf(bc,axiom,b=c,file('input',bc)).
cnf(not_ac,axiom,a!=c,file('input',not_ac)).
cnf(ac,plain,a=c,inference(transitivity,[status(thm)],[ab,bc])).
cnf(bot,plain,$false,inference(resolution,[status(thm)],[ac,not_ac])).
