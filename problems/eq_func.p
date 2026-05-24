% Functional equation: f(f(f(a))) = a, f(f(f(f(f(a))))) = a |- f(a) = a
% gcd(3,5) = 1, so f(a) = a must hold.
% Status: Theorem

fof(ax1, axiom, f(f(f(a))) = a).
fof(ax2, axiom, f(f(f(f(f(a))))) = a).
fof(goal, conjecture, f(a) = a).
