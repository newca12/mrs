% Source: https://seetptp.tptp.org/file/TPTPWorld/TSTP/Solutions/PUZ/PUZ001-1/Prover9---2026-6A.UNS-CRf.s
% Archive SHA-256 before repository normalization: 64ad5924638b8b900b9d4d453d5a9b32fa92891688b7eed5cf5f2b746f964be0
% Repository normalization: remove trailing ASCII horizontal whitespace per line.
%------------------------------------------------------------------------------
% File     : Prover9---2026-6A
% Problem  : PUZ001-1 : TPTP v9.3.1. Released v1.0.0.
% Transfm  : none
% Format   : tptp:raw
% Command  : prover9 -casc 300 -f /export/starexec/sandbox2/benchmark/theBenchmark.p

% Computer : n001.cluster.edu
% Model    : x86_64 x86_64
% CPU      : Intel(R) Xeon(R) CPU E5-2620 v4 2.10GHz
% Memory   : 8046.5625MB
% OS       : Linux 6.8.0-71-generic
% CPULimit : 300s
% WCLimit  : 300s
% DateTime : Sun Sep 27 08:22:41 AM UTC 2026

% Result   : Unsatisfiable 0.09s 0.39s
% Output   : CNFRefutation 0.09s
% Verified :
% SZS Type : Refutation
%            Derivation depth      :    5
%            Number of leaves      :   10
% Syntax   : Number of formulae    :   31 (  13 unt;   0 def)
%            Number of atoms       :   53 (   0 equ)
%            Maximal formula atoms :    3 (   1 avg)
%            Number of connectives :   47 (  25   ~;  22   |;   0   &)
%                                         (   0 <=>;   0  =>;   0  <=;   0 <~>)
%            Maximal formula depth :    5 (   2 avg)
%            Maximal term depth    :    1 (   1 avg)
%            Number of predicates  :    5 (   4 usr;   1 prp; 0-2 aty)
%            Number of functors    :    3 (   3 usr;   3 con; 0-0 aty)
%            Number of variables   :   16 (   0 sgn   8   !;   0   ?)

% Comments :
%------------------------------------------------------------------------------
fof(butler_hates_poor,axiom,
    ! [VAR_0] :
      ( hates(butler,VAR_0)
      | richer(VAR_0,agatha)
      | ~ lives(VAR_0) ),
    file('theBenchmark.p',butler_hates_poor) ).

cnf(c_1,plain,
    ( hates(butler,A)
    | richer(A,agatha)
    | ~ lives(A) ),
    inference(clausify,[status(thm)],[butler_hates_poor]) ).

fof(butler,axiom,
    lives(butler),
    file('theBenchmark.p',butler) ).

cnf(c_3,plain,
    lives(butler),
    inference(clausify,[status(thm)],[butler]) ).

fof(poorer_killer,axiom,
    ! [VAR_0,VAR_1] :
      ( ~ richer(VAR_0,VAR_1)
      | ~ killed(VAR_0,VAR_1) ),
    file('theBenchmark.p',poorer_killer) ).

cnf(c_5,plain,
    ( ~ richer(A,B)
    | ~ killed(A,B) ),
    inference(clausify,[status(thm)],[poorer_killer]) ).

cnf(c_7,plain,
    ( hates(butler,butler)
    | richer(butler,agatha) ),
    inference(resolve,[status(thm)],[c_1,c_3]) ).

cnf(c_10,plain,
    ( hates(butler,butler)
    | ~ killed(butler,agatha) ),
    inference(resolve,[status(thm)],[c_5,c_7]) ).

fof(different_hates,axiom,
    ! [VAR_0] :
      ( ~ hates(charles,VAR_0)
      | ~ hates(agatha,VAR_0) ),
    file('theBenchmark.p',different_hates) ).

cnf(c_12,plain,
    ( ~ hates(charles,A)
    | ~ hates(agatha,A) ),
    inference(clausify,[status(thm)],[different_hates]) ).

fof(no_one_hates_everyone,axiom,
    ! [VAR_0] :
      ( ~ hates(VAR_0,charles)
      | ~ hates(VAR_0,butler)
      | ~ hates(VAR_0,agatha) ),
    file('theBenchmark.p',no_one_hates_everyone) ).

cnf(c_13,plain,
    ( ~ hates(A,charles)
    | ~ hates(A,butler)
    | ~ hates(A,agatha) ),
    inference(clausify,[status(thm)],[no_one_hates_everyone]) ).

fof(agatha_hates_agatha,axiom,
    hates(agatha,agatha),
    file('theBenchmark.p',agatha_hates_agatha) ).

cnf(c_14,plain,
    hates(agatha,agatha),
    inference(clausify,[status(thm)],[agatha_hates_agatha]) ).

fof(agatha_hates_charles,axiom,
    hates(agatha,charles),
    file('theBenchmark.p',agatha_hates_charles) ).

cnf(c_15,plain,
    hates(agatha,charles),
    inference(clausify,[status(thm)],[agatha_hates_charles]) ).

fof(killer_hates_victim,axiom,
    ! [VAR_0,VAR_1] :
      ( hates(VAR_0,VAR_1)
      | ~ killed(VAR_0,VAR_1) ),
    file('theBenchmark.p',killer_hates_victim) ).

cnf(c_16,plain,
    ( hates(A,B)
    | ~ killed(A,B) ),
    inference(clausify,[status(thm)],[killer_hates_victim]) ).

fof(same_hates,axiom,
    ! [VAR_0] :
      ( hates(butler,VAR_0)
      | ~ hates(agatha,VAR_0) ),
    file('theBenchmark.p',same_hates) ).

cnf(c_17,plain,
    ( hates(butler,A)
    | ~ hates(agatha,A) ),
    inference(clausify,[status(thm)],[same_hates]) ).

fof(prove_neither_charles_nor_butler_did_it,axiom,
    ( killed(charles,agatha)
    | killed(butler,agatha) ),
    file('theBenchmark.p',prove_neither_charles_nor_butler_did_it) ).

cnf(c_18,plain,
    ( killed(charles,agatha)
    | killed(butler,agatha) ),
    inference(clausify,[status(thm)],[prove_neither_charles_nor_butler_did_it]) ).

cnf(c_19,plain,
    ~ hates(charles,agatha),
    inference(resolve,[status(thm)],[c_12,c_14]) ).

cnf(c_22,plain,
    hates(butler,charles),
    inference(resolve,[status(thm)],[c_17,c_15]) ).

cnf(c_23,plain,
    hates(butler,agatha),
    inference(resolve,[status(thm)],[c_17,c_14]) ).

cnf(c_30,plain,
    ( hates(charles,agatha)
    | killed(butler,agatha) ),
    inference(resolve,[status(thm)],[c_18,c_16]) ).

cnf(c_24,plain,
    killed(butler,agatha),
    inference(resolve,[status(thm)],[c_19,c_30]) ).

cnf(c_25,plain,
    hates(butler,butler),
    inference(resolve,[status(thm)],[c_24,c_10]) ).

cnf(c_31,plain,
    ( ~ hates(butler,butler)
    | ~ hates(butler,agatha) ),
    inference(resolve,[status(thm)],[c_22,c_13]) ).

cnf(c_32,plain,
    ~ hates(butler,butler),
    inference(resolve,[status(thm)],[c_23,c_31]) ).

cnf(c_29,plain,
    $false,
    inference(resolve,[status(thm)],[c_25,c_32]) ).

%------------------------------------------------------------------------------
%----ORIGINAL SYSTEM OUTPUT
% 0.00/0.03  % Problem  : PUZ001-1 : TPTP v9.3.1. Released v1.0.0.
% 0.00/0.04  % Command  : prover9 -casc 300 -f /export/starexec/sandbox2/benchmark/theBenchmark.p
% 0.09/0.36  % Computer : n001.cluster.edu
% 0.09/0.36  % Model    : x86_64 x86_64
% 0.09/0.36  % CPU      : Intel(R) Xeon(R) CPU E5-2620 v4 @ 2.10GHz
% 0.09/0.36  % Memory   : 8046.5625MB
% 0.09/0.36  % OS       : Linux 6.8.0-71-generic
% 0.09/0.37  % CPULimit : 300
% 0.09/0.37  % WCLimit  : 300
% 0.09/0.37  % DateTime : Sat Sep 26 05:08:12 UTC 2026
% 0.09/0.37  % CPUTime  :
% 0.09/0.37  % Prover9 (64) version 2026-6A, July 2026, CASC-J13.
% 0.09/0.37  % Process 962437 was started by sandbox2 on n001,
% 0.09/0.37  % Sat Sep 26 05:08:12 2026
% 0.09/0.37  % The command was "/export/starexec/sandbox2/solver/bin/prover9 -casc 300 -f /export/starexec/sandbox2/benchmark/theBenchmark.p".
% 0.09/0.37
% 0.09/0.37  % From the command line: assign(max_seconds, 300).
% 0.09/0.39
% 0.09/0.39  % SZS status Unsatisfiable for theBenchmark
% 0.09/0.39
% 0.09/0.39  % Proof 1 at 0.00 (+ 0.00) seconds.
% 0.09/0.39  % Length of proof is 18.
% 0.09/0.39  % Level of proof is 4.
% 0.09/0.39  % Maximum clause weight is 9.000.
% 0.09/0.39  % Given clauses 16.
% 0.09/0.39
% 0.09/0.39  % SZS output start CNFRefutation for theBenchmark
% See solution above
%------------------------------------------------------------------------------
