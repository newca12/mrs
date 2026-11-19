% Source: https://seetptp.tptp.org/file/TPTPWorld/TSTP/Solutions/SYN/SYN052-1/Prover9---2026-6A.UNS-CRf.s
% Archive SHA-256 before repository normalization: 39ac7bfda61201f47924cf9a6e800df9ff8dcb0f7f7eb59334eff5a386389a86
% Repository normalization: remove trailing ASCII horizontal whitespace per line.
%------------------------------------------------------------------------------
% File     : Prover9---2026-6A
% Problem  : SYN052-1 : TPTP v9.3.1. Released v1.0.0.
% Transfm  : none
% Format   : tptp:raw
% Command  : prover9 -casc 300 -f /export/starexec/sandbox/benchmark/theBenchmark.p

% Computer : n005.cluster.edu
% Model    : x86_64 x86_64
% CPU      : Intel(R) Xeon(R) CPU E5-2620 v4 2.10GHz
% Memory   : 8046.5625MB
% OS       : Linux 6.8.0-71-generic
% CPULimit : 300s
% WCLimit  : 300s
% DateTime : Sun Sep 27 09:36:52 AM UTC 2026

% Result   : Unsatisfiable 0.18s 0.46s
% Output   : CNFRefutation 0.18s
% Verified :
% SZS Type : Refutation
%            Derivation depth      :    5
%            Number of leaves      :    4
% Syntax   : Number of formulae    :   13 (   4 unt;   0 def)
%            Number of atoms       :   22 (   0 equ)
%            Maximal formula atoms :    2 (   1 avg)
%            Number of connectives :   18 (   9   ~;   9   |;   0   &)
%                                         (   0 <=>;   0  =>;   0  <=;   0 <~>)
%            Maximal formula depth :    4 (   2 avg)
%            Maximal term depth    :    1 (   1 avg)
%            Number of predicates  :    3 (   2 usr;   2 prp; 0-1 aty)
%            Number of functors    :    1 (   1 usr;   1 con; 0-0 aty)
%            Number of variables   :    7 (   4 sgn   3   !;   0   ?)

% Comments :
%------------------------------------------------------------------------------
fof(clause_1,axiom,
    ! [VAR_0] :
      ( ~ big_f(VAR_0)
      | p ),
    file('theBenchmark.p',clause_1) ).

cnf(c_1,plain,
    ( ~ big_f(A)
    | p ),
    inference(clausify,[status(thm)],[clause_1]) ).

fof(clause_2,axiom,
    ! [VAR_0] :
      ( ~ p
      | big_f(VAR_0) ),
    file('theBenchmark.p',clause_2) ).

cnf(c_2,plain,
    ( ~ p
    | big_f(A) ),
    inference(clausify,[status(thm)],[clause_2]) ).

fof(clause_3,axiom,
    ! [VAR_0] :
      ( p
      | big_f(VAR_0) ),
    file('theBenchmark.p',clause_3) ).

cnf(c_3,plain,
    ( p
    | big_f(A) ),
    inference(clausify,[status(thm)],[clause_3]) ).

fof(clause_4,axiom,
    ( ~ big_f(a)
    | ~ p ),
    file('theBenchmark.p',clause_4) ).

cnf(c_4,plain,
    ( ~ big_f(a)
    | ~ p ),
    inference(clausify,[status(thm)],[clause_4]) ).

cnf(c_10,plain,
    ( p
    | p ),
    inference(resolve,[status(thm)],[c_3,c_1]) ).

cnf(c_6,plain,
    p,
    inference(copy,[status(thm)],[c_10]) ).

cnf(c_7,plain,
    ~ big_f(a),
    inference(resolve,[status(thm)],[c_6,c_4]) ).

cnf(c_8,plain,
    big_f(A),
    inference(resolve,[status(thm)],[c_6,c_2]) ).

cnf(c_9,plain,
    $false,
    inference(resolve,[status(thm)],[c_8,c_7]) ).

%------------------------------------------------------------------------------
%----ORIGINAL SYSTEM OUTPUT
% 0.00/0.05  % Problem  : SYN052-1 : TPTP v9.3.1. Released v1.0.0.
% 0.00/0.06  % Command  : prover9 -casc 300 -f /export/starexec/sandbox/benchmark/theBenchmark.p
% 0.18/0.44  % Computer : n005.cluster.edu
% 0.18/0.44  % Model    : x86_64 x86_64
% 0.18/0.44  % CPU      : Intel(R) Xeon(R) CPU E5-2620 v4 @ 2.10GHz
% 0.18/0.44  % Memory   : 8046.5625MB
% 0.18/0.44  % OS       : Linux 6.8.0-71-generic
% 0.18/0.44  % CPULimit : 300
% 0.18/0.44  % WCLimit  : 300
% 0.18/0.44  % DateTime : Sat Sep 26 17:07:02 UTC 2026
% 0.18/0.45  % CPUTime  :
% 0.18/0.45  % Prover9 (64) version 2026-6A, July 2026, CASC-J13.
% 0.18/0.45  % Process 2614671 was started by sandbox on n005,
% 0.18/0.45  % Sat Sep 26 17:07:02 2026
% 0.18/0.45  % The command was "/export/starexec/sandbox/solver/bin/prover9 -casc 300 -f /export/starexec/sandbox/benchmark/theBenchmark.p".
% 0.18/0.45
% 0.18/0.45  % From the command line: assign(max_seconds, 300).
% 0.18/0.46
% 0.18/0.46  % SZS status Unsatisfiable for theBenchmark
% 0.18/0.46
% 0.18/0.46  % Proof 1 at 0.00 (+ 0.00) seconds.
% 0.18/0.46  % Length of proof is 8.
% 0.18/0.46  % Level of proof is 3.
% 0.18/0.46  % Maximum clause weight is 3.000.
% 0.18/0.46  % Given clauses 3.
% 0.18/0.46
% 0.18/0.46  % SZS output start CNFRefutation for theBenchmark
% See solution above
%------------------------------------------------------------------------------
