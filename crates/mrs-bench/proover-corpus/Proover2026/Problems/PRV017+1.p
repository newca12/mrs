% Problem : Problems/PRV017+1.p
fof(a1, axiom, ! [X]: (greek(X) => human(X)), file('Problems/PRV017+1.p', a1)).
fof(a2, axiom, ! [X]: (human(X) => (mortal(X) | immortal(X))), file('Problems/PRV017+1.p', a2)).
fof(a3, axiom, ! [X]: (immortal(X) => god(X)), file('Problems/PRV017+1.p', a3)).
fof(a4, axiom, greek(socrates), file('Problems/PRV017+1.p', a4)).
fof(a5, axiom, ~ god(socrates), file('Problems/PRV017+1.p', a5)).
fof(c, conjecture, mortal(socrates), file('Problems/PRV017+1.p', c)).
