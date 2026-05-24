% Pelletier problem #34 (Andrews challenge)
% ((?[X]: (![Y]: (p(X) <=> p(Y)))) <=> ((?[X]: q(X)) <=> (![Y]: q(Y))))
%   <=>
% ((?[X]: (![Y]: (q(X) <=> q(Y)))) <=> ((?[X]: p(X)) <=> (![Y]: p(Y))))
%
% This is one of the hardest propositional-style Pelletier problems.
% Deeply nested biconditionals with alternating quantifiers exercise
% both definitional CNF and search completeness.
% Status: Theorem (hard — may timeout with basic strategies)

fof(pel34, conjecture,
    (((?[X]: (![Y]: (p(X) <=> p(Y)))) <=> ((?[X]: q(X)) <=> (![Y]: q(Y))))
     <=>
     ((?[X]: (![Y]: (q(X) <=> q(Y)))) <=> ((?[X]: p(X)) <=> (![Y]: p(Y)))))).