% Problem : Problems/PRV060+1.p
fof(a1, axiom, ! [X]: ? [Y]: relB(X, Y), file('Problems/PRV060+1.p', a1)).
fof(b1, axiom, ! [X]: (midB1(X) => midB2(X)), file('Problems/PRV060+1.p', b1)).
fof(b2, axiom, ! [X]: (midB2(X) => midB3(X)), file('Problems/PRV060+1.p', b2)).
fof(c, conjecture, ? [X, Y]: relB(X, Y), file('Problems/PRV060+1.p', c)).
