% Problem : Problems/PRV001+1.p
fof(a1, axiom, ~ ! [X]: ? [Y]: ! [Z]: (p(X, Y, Z) | ? [X]: ! [W]: (q(X, Y, Z, W) & ? [Y]: r(X, Y, Z, W))), file('Problems/PRV001+1.p', a1)).
fof(c, conjecture, ~ ! [X]: ? [Y]: ! [Z]: (p(X, Y, Z) | ? [X]: ! [W]: (q(X, Y, Z, W) & ? [Y]: r(X, Y, Z, W))), file('Problems/PRV001+1.p', c)).
