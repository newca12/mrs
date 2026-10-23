% Problem : Problems/PRV013+1.p
fof(s0, axiom, ! [X0, X2]: ? [X1, X3]: (f(c) = X2 & f(X0) = g(b, X2)), file('Problems/PRV013+1.p', s0)).
fof(s1, axiom, ! [X4, X5]: ? [X6]: (t & p(X4)), file('Problems/PRV013+1.p', s1)).
fof(s2, axiom, ! [X7, X9]: ? [X8]: ! [X10]: ? [X11, X12]: t, file('Problems/PRV013+1.p', s2)).
fof(s3, axiom, (? [X13]: p(f(X13)) => ! [X14, X15]: ? [X16]: t), file('Problems/PRV013+1.p', s3)).
fof(s4, axiom, ! [X17]: ? [X18]: q(g(a, b)), file('Problems/PRV013+1.p', s4)).
fof(c, conjecture, ((? [X13]: p(f(X13)) => ! [X14, X15]: ? [X16]: t) | ! [X19]: ? [X20]: t | ~ ~ q(a)), file('Problems/PRV013+1.p', c)).
