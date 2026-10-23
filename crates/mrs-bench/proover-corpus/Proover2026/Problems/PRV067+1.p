% Problem : Problems/PRV067+1.p
fof(a1, axiom, ! [X]: (p(X) | r(X)), file('Problems/PRV067+1.p', a1)).
fof(a2, axiom, ! [X]: (~ p(X) | t(X)), file('Problems/PRV067+1.p', a2)).
fof(c, conjecture, (r(a) | t(b)), file('Problems/PRV067+1.p', c)).
