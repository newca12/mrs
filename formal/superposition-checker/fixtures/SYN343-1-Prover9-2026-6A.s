% Source: https://seetptp.tptp.org/file/TPTPWorld/TSTP/Solutions/SYN/SYN343-1/Prover9---2026-6A.UNS-CRf.s
% Archive SHA-256 before repository normalization: a39372f9b113e504401adb071a32940660919e7f0f86ddd1423004e8ae890df9
% Repository normalization: remove trailing ASCII horizontal whitespace per line.
%------------------------------------------------------------------------------
% File     : Prover9---2026-6A
% Problem  : SYN343-1 : TPTP v9.3.1. Released v1.2.0.
% Transfm  : none
% Format   : tptp:raw
% Command  : prover9 -casc 300 -f /export/starexec/sandbox2/benchmark/theBenchmark.p

% Computer : n016.cluster.edu
% Model    : x86_64 x86_64
% CPU      : Intel(R) Xeon(R) CPU E5-2620 v4 2.10GHz
% Memory   : 8046.5625MB
% OS       : Linux 6.8.0-71-generic
% CPULimit : 300s
% WCLimit  : 300s
% DateTime : Sun Sep 27 09:37:38 AM UTC 2026

% Result   : Unsatisfiable 0.12s 0.37s
% Output   : CNFRefutation 0.12s
% Verified :
% SZS Type : Refutation
%            Derivation depth      :    3
%            Number of leaves      :    2
% Syntax   : Number of formulae    :    7 (   3 unt;   0 def)
%            Number of atoms       :   11 (   0 equ)
%            Maximal formula atoms :    2 (   1 avg)
%            Number of connectives :    9 (   5   ~;   4   |;   0   &)
%                                         (   0 <=>;   0  =>;   0  <=;   0 <~>)
%            Maximal formula depth :    5 (   2 avg)
%            Maximal term depth    :    1 (   1 avg)
%            Number of predicates  :    2 (   1 usr;   1 prp; 0-2 aty)
%            Number of functors    :    1 (   1 usr;   1 con; 0-0 aty)
%            Number of variables   :    6 (   3 sgn   3   !;   0   ?)

% Comments :
%------------------------------------------------------------------------------
fof(clause1,axiom,
    ! [VAR_0] :
      ( f(a,a)
      | f(a,VAR_0) ),
    file('theBenchmark.p',clause1) ).

cnf(c_1,plain,
    ( f(a,a)
    | f(a,A) ),
    inference(clausify,[status(thm)],[clause1]) ).

fof(clause3,axiom,
    ! [VAR_0,VAR_1] :
      ( ~ f(VAR_0,VAR_1)
      | ~ f(a,a) ),
    file('theBenchmark.p',clause3) ).

cnf(c_3,plain,
    ( ~ f(A,B)
    | ~ f(a,a) ),
    inference(clausify,[status(thm)],[clause3]) ).

cnf(c_4,plain,
    f(a,a),
    inference(factor,[status(thm)],[c_1]) ).

cnf(c_6,plain,
    ~ f(a,a),
    inference(factor,[status(thm)],[c_3]) ).

cnf(c_5,plain,
    $false,
    inference(resolve,[status(thm)],[c_4,c_6]) ).

%------------------------------------------------------------------------------
%----ORIGINAL SYSTEM OUTPUT
% 0.00/0.03  % Problem  : SYN343-1 : TPTP v9.3.1. Released v1.2.0.
% 0.00/0.04  % Command  : prover9 -casc 300 -f /export/starexec/sandbox2/benchmark/theBenchmark.p
% 0.08/0.35  % Computer : n016.cluster.edu
% 0.08/0.35  % Model    : x86_64 x86_64
% 0.08/0.35  % CPU      : Intel(R) Xeon(R) CPU E5-2620 v4 @ 2.10GHz
% 0.08/0.35  % Memory   : 8046.5625MB
% 0.08/0.35  % OS       : Linux 6.8.0-71-generic
% 0.08/0.35  % CPULimit : 300
% 0.08/0.35  % WCLimit  : 300
% 0.08/0.35  % DateTime : Sat Sep 26 17:41:30 UTC 2026
% 0.08/0.35  % CPUTime  :
% 0.08/0.35  % Prover9 (64) version 2026-6A, July 2026, CASC-J13.
% 0.08/0.35  % Process 1398365 was started by sandbox2 on n016,
% 0.08/0.35  % Sat Sep 26 17:41:30 2026
% 0.08/0.35  % The command was "/export/starexec/sandbox2/solver/bin/prover9 -casc 300 -f /export/starexec/sandbox2/benchmark/theBenchmark.p".
% 0.12/0.36
% 0.12/0.36  % From the command line: assign(max_seconds, 300).
% 0.12/0.37
% 0.12/0.37  % SZS status Unsatisfiable for theBenchmark
% 0.12/0.37
% 0.12/0.37  % Proof 1 at 0.00 (+ 0.00) seconds.
% 0.12/0.37  % Length of proof is 4.
% 0.12/0.37  % Level of proof is 2.
% 0.12/0.37  % Maximum clause weight is 6.000.
% 0.12/0.37  % Given clauses 0.
% 0.12/0.37
% 0.12/0.37  % SZS output start CNFRefutation for theBenchmark
% See solution above
%------------------------------------------------------------------------------
