% Transitivity of equality
% a = b, b = c |- a = c
% Status: Theorem

fof(ax1, axiom, a = b).
fof(ax2, axiom, b = c).
fof(goal, conjecture, a = c).
