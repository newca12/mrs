% Problem : Problems/PRV059+1.p
fof(a1, axiom, ! [X]: ? [Y]: relA(X, Y), file('Problems/PRV059+1.p', a1)).
fof(b1, axiom, ! [X]: (midA1(X) => midA2(X)), file('Problems/PRV059+1.p', b1)).
fof(b2, axiom, ! [X]: (midA2(X) => midA3(X)), file('Problems/PRV059+1.p', b2)).
fof(c, conjecture, ? [X, Y]: relA(X, Y), file('Problems/PRV059+1.p', c)).
