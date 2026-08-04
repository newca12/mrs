% Problem : Problems/PRV011+1.p
fof(s0, axiom, ! [X0, X2]: ? [X1]: ! [X3]: ? [X4]: (q(f(c)) => a = a), file('Problems/PRV011+1.p', s0)).
fof(s1, axiom, ! [X5]: (! [X6]: t <=> (r(c, X5) => q(X5))), file('Problems/PRV011+1.p', s1)).
fof(s2, axiom, ! [X7, X9]: ? [X8]: p(a), file('Problems/PRV011+1.p', s2)).
fof(s3, axiom, ! [X10]: (~ p(a) => ~ r(X10, X10)), file('Problems/PRV011+1.p', s3)).
fof(s4, axiom, p(f(f(a))), file('Problems/PRV011+1.p', s4)).
fof(c, conjecture, (! [X0, X2]: ? [X1]: ! [X3]: ? [X4]: (q(f(c)) => a = a) | ! [X26]: ? [X27]: (t & t)), file('Problems/PRV011+1.p', c)).
