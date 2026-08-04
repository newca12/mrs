% Problem : Problems/PRV019+1.p
fof(a1, axiom, ! [X]: (p(X) | q(X)), file('Problems/PRV019+1.p', a1)).
fof(a2, axiom, ! [X]: (p(X) => r(X)), file('Problems/PRV019+1.p', a2)).
fof(a3, axiom, ! [X]: (q(X) => r(X)), file('Problems/PRV019+1.p', a3)).
fof(c, conjecture, ! [X]: r(X), file('Problems/PRV019+1.p', c)).
