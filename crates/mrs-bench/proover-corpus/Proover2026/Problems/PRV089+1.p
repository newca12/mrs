% Problem : Problems/PRV089+1.p
fof(a1, axiom, ? [Y]: p(Y), file('Problems/PRV089+1.p', a1)).
fof(a2, axiom, ! [X]: q(X), file('Problems/PRV089+1.p', a2)).
fof(c, conjecture, ? [Z]: (p(Z) & q(Z)), file('Problems/PRV089+1.p', c)).
