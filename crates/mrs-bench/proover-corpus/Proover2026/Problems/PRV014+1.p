% Problem : Problems/PRV014+1.p
fof(s0, axiom, ! [X0, X2]: ? [X1]: ! [X3]: ? [X4]: ! [X5]: r(f(c), X3), file('Problems/PRV014+1.p', s0)).
fof(s1, axiom, ! [X6, X7]: ~ p(b), file('Problems/PRV014+1.p', s1)).
fof(s2, axiom, ! [X8, X10]: ? [X9]: ~ ~ p(f(b)), file('Problems/PRV014+1.p', s2)).
fof(s3, axiom, ~ (q(c) & f(b) != c), file('Problems/PRV014+1.p', s3)).
fof(c, conjecture, (~ ~ ~ (q(c) & f(b) != c) | ! [X32, X33]: ? [X34]: t), file('Problems/PRV014+1.p', c)).
