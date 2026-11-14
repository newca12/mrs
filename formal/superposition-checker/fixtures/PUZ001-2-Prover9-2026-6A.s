% Source: https://seetptp.tptp.org/file/TPTPWorld/TSTP/Solutions/PUZ/PUZ001-2/Prover9---2026-6A.UNS-CRf.s
% Archive SHA-256 before repository normalization: abfd0d5bfb9139930972d6aac3e391f25db7269841ba3799cb82b22abe2cb3be
% Repository normalization: remove trailing ASCII horizontal whitespace per line.
%------------------------------------------------------------------------------
% File     : Prover9---2026-6A
% Problem  : PUZ001-2 : TPTP v9.3.1. Released v1.0.0.
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
% DateTime : Sun Sep 27 08:22:42 AM UTC 2026

% Result   : Unsatisfiable 0.07s 5.54s
% Output   : CNFRefutation 0.07s
% Verified :
% SZS Type : Refutation
%            Derivation depth      :   10
%            Number of leaves      :   12
% Syntax   : Number of formulae    :   42 (  20 unt;   0 def)
%            Number of atoms       :   72 (  28 equ)
%            Maximal formula atoms :    4 (   1 avg)
%            Number of connectives :   56 (  26   ~;  30   |;   0   &)
%                                         (   0 <=>;   0  =>;   0  <=;   0 <~>)
%            Maximal formula depth :    5 (   2 avg)
%            Maximal term depth    :    2 (   1 avg)
%            Number of predicates  :    6 (   4 usr;   1 prp; 0-2 aty)
%            Number of functors    :    5 (   5 usr;   4 con; 0-1 aty)
%            Number of variables   :   22 (   0 sgn  10   !;   0   ?)

% Comments :
%------------------------------------------------------------------------------
fof(noone_else_lives_at_mansion,axiom,
    ! [VAR_0] :
      ( VAR_0 = charles
      | VAR_0 = butler
      | VAR_0 = aunt_agatha
      | ~ lives_at_dreadsbury(VAR_0) ),
    file('theBenchmark.p',noone_else_lives_at_mansion) ).

cnf(c_1,plain,
    ( A = charles
    | A = butler
    | A = aunt_agatha
    | ~ lives_at_dreadsbury(A) ),
    inference(clausify,[status(thm)],[noone_else_lives_at_mansion]) ).

fof(someone_in_mansion,axiom,
    lives_at_dreadsbury(someone),
    file('theBenchmark.p',someone_in_mansion) ).

cnf(c_5,plain,
    lives_at_dreadsbury(someone),
    inference(clausify,[status(thm)],[someone_in_mansion]) ).

fof(killer_poorer_than_victim,axiom,
    ! [VAR_0,VAR_1] :
      ( ~ richer(VAR_0,VAR_1)
      | ~ killed(VAR_0,VAR_1) ),
    file('theBenchmark.p',killer_poorer_than_victim) ).

cnf(c_6,plain,
    ( ~ richer(A,B)
    | ~ killed(A,B) ),
    inference(clausify,[status(thm)],[killer_poorer_than_victim]) ).

fof(butler_hates_poor_people,axiom,
    ! [VAR_0] :
      ( hates(butler,VAR_0)
      | richer(VAR_0,aunt_agatha) ),
    file('theBenchmark.p',butler_hates_poor_people) ).

cnf(c_7,plain,
    ( hates(butler,A)
    | richer(A,aunt_agatha) ),
    inference(clausify,[status(thm)],[butler_hates_poor_people]) ).

cnf(c_8,plain,
    ( hates(butler,A)
    | ~ killed(A,aunt_agatha) ),
    inference(resolve,[status(thm)],[c_6,c_7]) ).

fof(someone_killed_agatha,axiom,
    killed(someone,aunt_agatha),
    file('theBenchmark.p',someone_killed_agatha) ).

cnf(c_9,plain,
    killed(someone,aunt_agatha),
    inference(clausify,[status(thm)],[someone_killed_agatha]) ).

fof(killer_hates_victim,axiom,
    ! [VAR_0,VAR_1] :
      ( hates(VAR_0,VAR_1)
      | ~ killed(VAR_0,VAR_1) ),
    file('theBenchmark.p',killer_hates_victim) ).

cnf(c_10,plain,
    ( hates(A,B)
    | ~ killed(A,B) ),
    inference(clausify,[status(thm)],[killer_hates_victim]) ).

fof(charles_and_agatha_hate_different_people,axiom,
    ! [VAR_0] :
      ( ~ hates(charles,VAR_0)
      | ~ hates(aunt_agatha,VAR_0) ),
    file('theBenchmark.p',charles_and_agatha_hate_different_people) ).

cnf(c_11,plain,
    ( ~ hates(charles,A)
    | ~ hates(aunt_agatha,A) ),
    inference(clausify,[status(thm)],[charles_and_agatha_hate_different_people]) ).

fof(agatha_likes_only_butler,axiom,
    ! [VAR_0] :
      ( hates(aunt_agatha,VAR_0)
      | VAR_0 = butler ),
    file('theBenchmark.p',agatha_likes_only_butler) ).

cnf(c_12,plain,
    ( hates(aunt_agatha,A)
    | A = butler ),
    inference(clausify,[status(thm)],[agatha_likes_only_butler]) ).

cnf(c_13,plain,
    ( hates(aunt_agatha,A)
    | butler = A ),
    inference(copy,[status(thm)],[c_12]) ).

fof(butler_and_agatha_hate_the_same_people,axiom,
    ! [VAR_0] :
      ( hates(butler,VAR_0)
      | ~ hates(aunt_agatha,VAR_0) ),
    file('theBenchmark.p',butler_and_agatha_hate_the_same_people) ).

cnf(c_14,plain,
    ( hates(butler,A)
    | ~ hates(aunt_agatha,A) ),
    inference(clausify,[status(thm)],[butler_and_agatha_hate_the_same_people]) ).

fof(noone_hates_everyone,axiom,
    ! [VAR_0] : ~ hates(VAR_0,every_one_but(VAR_0)),
    file('theBenchmark.p',noone_hates_everyone) ).

cnf(c_15,plain,
    ~ hates(A,every_one_but(A)),
    inference(clausify,[status(thm)],[noone_hates_everyone]) ).

fof(agatha_is_not_the_butler,axiom,
    aunt_agatha != butler,
    file('theBenchmark.p',agatha_is_not_the_butler) ).

cnf(c_16,plain,
    aunt_agatha != butler,
    inference(clausify,[status(thm)],[agatha_is_not_the_butler]) ).

cnf(c_17,plain,
    butler != aunt_agatha,
    inference(copy,[status(thm)],[c_16]) ).

fof(prove_agatha_killed_herself,axiom,
    ~ killed(aunt_agatha,aunt_agatha),
    file('theBenchmark.p',prove_agatha_killed_herself) ).

cnf(c_18,plain,
    ~ killed(aunt_agatha,aunt_agatha),
    inference(clausify,[status(thm)],[prove_agatha_killed_herself]) ).

cnf(c_21,plain,
    ( someone = charles
    | someone = butler
    | someone = aunt_agatha ),
    inference(resolve,[status(thm)],[c_1,c_5]) ).

cnf(c_23,plain,
    hates(someone,aunt_agatha),
    inference(resolve,[status(thm)],[c_10,c_9]) ).

cnf(c_26,plain,
    ~ hates(aunt_agatha,every_one_but(butler)),
    inference(resolve,[status(thm)],[c_14,c_15]) ).

cnf(c_28,plain,
    ~ killed(every_one_but(butler),aunt_agatha),
    inference(resolve,[status(thm)],[c_8,c_15]) ).

cnf(c_44,plain,
    butler = every_one_but(butler),
    inference(resolve,[status(thm)],[c_26,c_13]) ).

cnf(c_31,plain,
    every_one_but(butler) = butler,
    inference(copy,[status(thm)],[c_44]) ).

cnf(c_33,plain,
    ~ killed(butler,aunt_agatha),
    inference(paramod,[status(thm)],[c_31,c_28]) ).

cnf(c_35,plain,
    ( hates(charles,aunt_agatha)
    | someone = butler
    | someone = aunt_agatha ),
    inference(paramod,[status(thm)],[c_21,c_23]) ).

cnf(c_38,plain,
    ( ~ hates(aunt_agatha,aunt_agatha)
    | someone = butler
    | someone = aunt_agatha ),
    inference(resolve,[status(thm)],[c_35,c_11]) ).

cnf(c_45,plain,
    ( butler = aunt_agatha
    | someone = butler
    | someone = aunt_agatha ),
    inference(resolve,[status(thm)],[c_38,c_13]) ).

cnf(c_39,plain,
    ( someone = butler
    | someone = aunt_agatha ),
    inference(resolve,[status(thm)],[c_17,c_45]) ).

cnf(c_46,plain,
    ( killed(butler,aunt_agatha)
    | someone = aunt_agatha ),
    inference(paramod,[status(thm)],[c_39,c_9]) ).

cnf(c_40,plain,
    someone = aunt_agatha,
    inference(resolve,[status(thm)],[c_33,c_46]) ).

cnf(c_47,plain,
    killed(aunt_agatha,aunt_agatha),
    inference(paramod,[status(thm)],[c_40,c_9]) ).

cnf(c_43,plain,
    $false,
    inference(resolve,[status(thm)],[c_18,c_47]) ).

%------------------------------------------------------------------------------
%----ORIGINAL SYSTEM OUTPUT
% 0.00/0.02  % Problem  : PUZ001-2 : TPTP v9.3.1. Released v1.0.0.
% 0.00/0.03  % Command  : prover9 -casc 300 -f /export/starexec/sandbox/benchmark/theBenchmark.p
% 0.07/5.52  % Computer : n005.cluster.edu
% 0.07/5.52  % Model    : x86_64 x86_64
% 0.07/5.52  % CPU      : Intel(R) Xeon(R) CPU E5-2620 v4 @ 2.10GHz
% 0.07/5.52  % Memory   : 8046.5625MB
% 0.07/5.52  % OS       : Linux 6.8.0-71-generic
% 0.07/5.52  % CPULimit : 300
% 0.07/5.52  % WCLimit  : 300
% 0.07/5.52  % DateTime : Sat Sep 26 05:02:22 UTC 2026
% 0.07/5.52  % CPUTime  :
% 0.07/5.52  % Prover9 (64) version 2026-6A, July 2026, CASC-J13.
% 0.07/5.52  % Process 1251656 was started by sandbox on n005,
% 0.07/5.52  % Sat Sep 26 05:02:22 2026
% 0.07/5.52  % The command was "/export/starexec/sandbox/solver/bin/prover9 -casc 300 -f /export/starexec/sandbox/benchmark/theBenchmark.p".
% 0.07/5.53
% 0.07/5.53  % From the command line: assign(max_seconds, 300).
% 0.07/5.54
% 0.07/5.54  % SZS status Unsatisfiable for theBenchmark
% 0.07/5.54
% 0.07/5.54  % Proof 1 at 0.00 (+ 0.00) seconds.
% 0.07/5.54  % Length of proof is 26.
% 0.07/5.54  % Level of proof is 6.
% 0.07/5.54  % Maximum clause weight is 9.000.
% 0.07/5.54  % Given clauses 27.
% 0.07/5.54
% 0.07/5.54  % SZS output start CNFRefutation for theBenchmark
% See solution above
%------------------------------------------------------------------------------
