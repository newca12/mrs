% Problem : Problems/PRV062+1.p
fof(a1, axiom, p_ef(a), file('Problems/PRV062+1.p', a1)).
fof(a2, axiom, ~ p_ef(a), file('Problems/PRV062+1.p', a2)).
fof(a3, axiom, q_ef(a), file('Problems/PRV062+1.p', a3)).
fof(b1, axiom, ! [X]: (q_ef(X) => r_ef(X)), file('Problems/PRV062+1.p', b1)).
fof(c, conjecture, r_ef(a), file('Problems/PRV062+1.p', c)).
