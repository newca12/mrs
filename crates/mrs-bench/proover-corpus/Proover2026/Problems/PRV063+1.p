% Problem : Problems/PRV063+1.p
fof(c, conjecture, p6_topo(a), file('Problems/PRV063+1.p', c)).
fof(a5, axiom, ! [X]: (p5_topo(X) => p6_topo(X)), file('Problems/PRV063+1.p', a5)).
fof(a4, axiom, ! [X]: (p4_topo(X) => p5_topo(X)), file('Problems/PRV063+1.p', a4)).
fof(a3, axiom, ! [X]: (p3_topo(X) => p4_topo(X)), file('Problems/PRV063+1.p', a3)).
fof(a2, axiom, ! [X]: (p2_topo(X) => p3_topo(X)), file('Problems/PRV063+1.p', a2)).
fof(a1, axiom, ! [X]: (p1_topo(X) => p2_topo(X)), file('Problems/PRV063+1.p', a1)).
fof(a0, axiom, p1_topo(a), file('Problems/PRV063+1.p', a0)).
