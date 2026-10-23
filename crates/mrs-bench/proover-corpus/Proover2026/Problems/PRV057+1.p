% Problem : Problems/PRV057+1.p
fof(a0, axiom, p1_cs(a), file('Problems/PRV057+1.p', a0)).
fof(a1, axiom, ! [X]: (p1_cs(X) => p2_cs(X)), file('Problems/PRV057+1.p', a1)).
fof(a2, axiom, ! [X]: (p2_cs(X) => p3_cs(X)), file('Problems/PRV057+1.p', a2)).
fof(a3, axiom, ! [X]: (p3_cs(X) => p4_cs(X)), file('Problems/PRV057+1.p', a3)).
fof(a4, axiom, ! [X]: (p4_cs(X) => p5_cs(X)), file('Problems/PRV057+1.p', a4)).
fof(a5, axiom, ! [X]: (p5_cs(X) => p6_cs(X)), file('Problems/PRV057+1.p', a5)).
fof(c, conjecture, p6_cs(a), file('Problems/PRV057+1.p', c)).
