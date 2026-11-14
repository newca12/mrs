% Source: https://seetptp.tptp.org/file/TPTPWorld/TSTP/Solutions/SYN/SYN343-1/SPASS---3.9.UNS-Ref.s
% Archive SHA-256 before repository normalization: cd932cd27d76ab28ffcbacbf8ecb0941965e60aef524aeac6e1a4c83f23ce436
% Repository normalization: remove trailing ASCII horizontal whitespace per line.
%------------------------------------------------------------------------------
% File     : SPASS---3.9
% Problem  : SYN343-1 : TPTP v8.1.0. Released v1.2.0.
% Transfm  : none
% Format   : tptp
% Command  : run_spass %d %s

% Computer : n006.cluster.edu
% Model    : x86_64 x86_64
% CPU      : Intel(R) Xeon(R) CPU E5-2620 v4 2.10GHz
% Memory   : 8042.1875MB
% OS       : Linux 3.10.0-693.el7.x86_64
% CPULimit : 300s
% WCLimit  : 600s
% DateTime : Thu Jul 21 12:20:33 EDT 2022

% Result   : Unsatisfiable 0.19s 0.42s
% Output   : Refutation 0.19s
% Verified :
% SZS Type : Refutation
%            Derivation depth      :    2
%            Number of leaves      :    2
% Syntax   : Number of clauses     :    5 (   3 unt;   1 nHn;   5 RR)
%            Number of literals    :    7 (   0 equ;   3 neg)
%            Maximal clause size   :    2 (   1 avg)
%            Maximal term depth    :    1 (   1 avg)
%            Number of predicates  :    2 (   1 usr;   1 prp; 0-2 aty)
%            Number of functors    :    3 (   3 usr;   3 con; 0-0 aty)
%            Number of variables   :    0 (   0 sgn)

% Comments :
%------------------------------------------------------------------------------
cnf(1,axiom,
    ( f(a,u)
    | f(a,a) ),
    file('SYN343-1.p',unknown),
    [] ).

cnf(3,axiom,
    ( ~ f(a,a)
    | ~ f(u,v) ),
    file('SYN343-1.p',unknown),
    [] ).

cnf(4,plain,
    f(a,a),
    inference(con,[status(thm)],[1]),
    [iquote('0:Con:1.0')] ).

cnf(5,plain,
    ~ f(a,a),
    inference(con,[status(thm)],[3]),
    [iquote('0:Con:3.1')] ).

cnf(6,plain,
    $false,
    inference(mrr,[status(thm)],[5,4]),
    [iquote('0:MRR:5.0,4.0')] ).

%------------------------------------------------------------------------------
%----ORIGINAL SYSTEM OUTPUT
% 0.12/0.12  % Problem  : SYN343-1 : TPTP v8.1.0. Released v1.2.0.
% 0.12/0.13  % Command  : run_spass %d %s
% 0.13/0.34  % Computer : n006.cluster.edu
% 0.13/0.34  % Model    : x86_64 x86_64
% 0.13/0.34  % CPU      : Intel(R) Xeon(R) CPU E5-2620 v4 @ 2.10GHz
% 0.13/0.34  % Memory   : 8042.1875MB
% 0.13/0.34  % OS       : Linux 3.10.0-693.el7.x86_64
% 0.13/0.34  % CPULimit : 300
% 0.13/0.34  % WCLimit  : 600
% 0.13/0.34  % DateTime : Tue Jul 12 00:59:05 EDT 2022
% 0.13/0.35  % CPUTime  :
% 0.19/0.42
% 0.19/0.42  SPASS V 3.9
% 0.19/0.42  SPASS beiseite: Proof found.
% 0.19/0.42  % SZS status Theorem
% 0.19/0.42  Problem: /export/starexec/sandbox/benchmark/theBenchmark.p
% 0.19/0.42  SPASS derived 0 clauses, backtracked 0 clauses, performed 0 splits and kept 2 clauses.
% 0.19/0.42  SPASS allocated 63074 KBytes.
% 0.19/0.42  SPASS spent	0:00:00.06 on the problem.
% 0.19/0.42  		0:00:00.04 for the input.
% 0.19/0.42  		0:00:00.00 for the FLOTTER CNF translation.
% 0.19/0.42  		0:00:00.00 for inferences.
% 0.19/0.42  		0:00:00.00 for the backtracking.
% 0.19/0.42  		0:00:00.00 for the reduction.
% 0.19/0.42
% 0.19/0.42
% 0.19/0.42  Here is a proof with depth 0, length 5 :
% 0.19/0.42  % SZS output start Refutation
% See solution above
% 0.19/0.42  Formulae used in the proof : clause1 clause3
% 0.19/0.42
%------------------------------------------------------------------------------
