% Simple equality: superposition into a subterm
% f(a) = b, g(b) = c |- g(f(a)) = c
% Status: Theorem

fof(ax1, axiom, f(a) = b).
fof(ax2, axiom, g(b) = c).
fof(goal, conjecture, g(f(a)) = c).
