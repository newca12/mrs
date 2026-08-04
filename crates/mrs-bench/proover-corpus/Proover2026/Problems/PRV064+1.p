% Problem : Problems/PRV064+1.p
fof(a1, axiom, ! [X]: (man(X) => mortal(X)), file('Problems/PRV064+1.p', a1)).
fof(a2, axiom, ! [X]: (mortal(X) => dies(X)), file('Problems/PRV064+1.p', a2)).
fof(a3, axiom, ! [X]: (dies(X) => finite_life(X)), file('Problems/PRV064+1.p', a3)).
fof(a4, axiom, ! [X]: (finite_life(X) => ~ eternal(X)), file('Problems/PRV064+1.p', a4)).
fof(a5, axiom, man(socrates), file('Problems/PRV064+1.p', a5)).
fof(c, conjecture, ~ eternal(socrates), file('Problems/PRV064+1.p', c)).
