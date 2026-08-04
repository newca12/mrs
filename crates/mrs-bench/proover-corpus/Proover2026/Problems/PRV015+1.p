% Problem : Problems/PRV015+1.p
fof(s0, axiom, ! [X0, X2]: ? [X1]: ~ (X2 != X1), file('Problems/PRV015+1.p', s0)).
fof(s1, axiom, ! [X3]: (? [X4]: r(g(X4, b), g(X4, c)) => ! [X5]: ? [X6]: b = a), file('Problems/PRV015+1.p', s1)).
fof(s2, axiom, ! [X7, X9]: ? [X8]: ! [X10]: ? [X11, X12]: t, file('Problems/PRV015+1.p', s2)).
fof(s3, axiom, p(a), file('Problems/PRV015+1.p', s3)).
fof(s4, axiom, (! [X13]: p(f(f(X13))) | t), file('Problems/PRV015+1.p', s4)).
fof(c, conjecture, ((! [X31]: ? [X32]: t => (t & p(g(b, b)))) | ~ (! [X31]: ? [X32]: t => (t & p(g(b, b)))) | q(c)), file('Problems/PRV015+1.p', c)).
