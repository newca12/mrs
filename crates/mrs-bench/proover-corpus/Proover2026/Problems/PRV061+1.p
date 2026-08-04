% Problem : Problems/PRV061+1.p
fof(a1, axiom, p_dup(a), file('Problems/PRV061+1.p', a1)).
fof(ax1, axiom, p_dup(a), file('Problems/PRV061+1.p', a1)).
fof(ax2, axiom, p_dup(a), file('Problems/PRV061+1.p', a1)).
fof(b1, axiom, ! [X]: (p_dup(X) => q_dup(X)), file('Problems/PRV061+1.p', b1)).
fof(b2, axiom, ! [X]: (q_dup(X) => r_dup(X)), file('Problems/PRV061+1.p', b2)).
fof(c, conjecture, r_dup(a), file('Problems/PRV061+1.p', c)).
