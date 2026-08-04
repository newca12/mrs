% Problem : Problems/PRV018+1.p
fof(a1, axiom, a = b, file('Problems/PRV018+1.p', a1)).
fof(a2, axiom, b = c, file('Problems/PRV018+1.p', a2)).
fof(a3, axiom, h(a) = d, file('Problems/PRV018+1.p', a3)).
fof(a4, axiom, ! [X]: (p(X) => q(h(X))), file('Problems/PRV018+1.p', a4)).
fof(a5, axiom, p(c), file('Problems/PRV018+1.p', a5)).
fof(c, conjecture, q(d), file('Problems/PRV018+1.p', c)).
