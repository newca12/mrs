% Problem : Problems/PRV012+1.p
fof(s0, axiom, ! [X0, X2]: ? [X1, X3]: p(X1), file('Problems/PRV012+1.p', s0)).
fof(s1, axiom, ! [X4]: (? [X5]: r(X4, b) <=> ! [X6]: ? [X7]: t), file('Problems/PRV012+1.p', s1)).
fof(s2, axiom, ! [X8]: ? [X9]: r(X9, f(X9)), file('Problems/PRV012+1.p', s2)).
fof(s3, axiom, p(g(a, f(b))), file('Problems/PRV012+1.p', s3)).
fof(s4, axiom, ? [X10]: ! [X11]: ? [X12, X13]: p(g(b, X10)), file('Problems/PRV012+1.p', s4)).
fof(s5, axiom, ? [X14]: ((X14 = b => p(b)) | ! [X15]: p(X15)), file('Problems/PRV012+1.p', s5)).
fof(c, conjecture, (~ ~ ! [X4]: (? [X5]: r(X4, b) <=> ! [X6]: ? [X7]: t) | (r(f(g(c, c)), b) => ! [X32]: ? [X33]: q(X32))), file('Problems/PRV012+1.p', c)).
