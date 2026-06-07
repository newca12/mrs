% Proof : Problems/SYN050+1.p
%------------------------------------------------------------------------------
% File     : Vampire---5.0.1
% Problem  : SYN050+1 : TPTP v9.2.1. Released v2.0.0.
% Transfm  : none
% Format   : tptp:raw
% Command  : run_vampire %s %d THM

% Computer : n007.cluster.edu
% Model    : x86_64 x86_64
% CPU      : Intel(R) Xeon(R) CPU E5-2620 v4 2.10GHz
% Memory   : 8042.1875MB
% OS       : Linux 3.10.0-693.el7.x86_64
% CPULimit : 300s
% WCLimit  : 300s
% DateTime : Fri May  1 04:39:26 PM UTC 2026

% Result   : Theorem 0.45s 0.86s
% Output   : Refutation 0.45s
% Verified : 
% SZS Type : Refutation
%            Derivation depth      :   12
%            Number of leaves      :    4
% Syntax   : Number of formulae    :   27 (   9 unt;   2 def)
%            Number of atoms       :   80 (   0 equ)
%            Maximal formula atoms :    7 (   2 avg)
%            Number of connectives :   88 (  35   ~;  21   |;  20   &)
%                                         (   2 <=>;  10  =>;   0  <=;   0 <~>)
%            Maximal formula depth :    9 (   4 avg)
%            Maximal term depth    :    2 (   1 avg)
%            Number of predicates  :    7 (   6 usr;   3 prp; 0-1 aty)
%            Number of functors    :    1 (   1 usr;   0 con; 2-2 aty)
%            Number of variables   :   60 (   3 sgn  44   !;  16   ?)

% Comments : 
%------------------------------------------------------------------------------
fof(f1,conjecture,
    ( ! [X0,X1] :
      ? [X2] :
      ! [X3] :
        ( ( big_p(X0)
          & big_q(X1) )
       => ( big_r(X2)
          & big_s(X3) ) )
   => ? [X4,X5] :
        ( ( big_p(X4)
          & big_q(X5) )
       => ? [X6] : big_r(X6) ) ),
    file('/export/starexec/sandbox/benchmark/theBenchmark.p',pel20) ).

fof(f2,negated_conjecture,
    ~ ( ! [X0,X1] :
        ? [X2] :
        ! [X3] :
          ( ( big_p(X0)
            & big_q(X1) )
         => ( big_r(X2)
            & big_s(X3) ) )
     => ? [X4,X5] :
          ( ( big_p(X4)
            & big_q(X5) )
         => ? [X6] : big_r(X6) ) ),
    inference(negated_conjecture,[status(cth)],[f1]) ).

fof(f3,plain,
    ~ ( ! [X0,X1] :
        ? [X2] :
        ! [X3] :
          ( ( big_p(X0)
            & big_q(X1) )
         => big_r(X2) )
     => ? [X4,X5] :
          ( ( big_p(X4)
            & big_q(X5) )
         => ? [X6] : big_r(X6) ) ),
    inference(pure_predicate_removal,[],[f2]) ).

fof(f4,plain,
    ( ! [X4,X5] :
        ( ! [X6] : ~ big_r(X6)
        & big_p(X4)
        & big_q(X5) )
    & ! [X0,X1] :
      ? [X2] :
      ! [X3] :
        ( big_r(X2)
        | ~ big_p(X0)
        | ~ big_q(X1) ) ),
    inference(ennf_transformation,[],[f3]) ).

fof(f5,plain,
    ( ! [X4,X5] :
        ( ! [X6] : ~ big_r(X6)
        & big_p(X4)
        & big_q(X5) )
    & ! [X0,X1] :
      ? [X2] :
      ! [X3] :
        ( big_r(X2)
        | ~ big_p(X0)
        | ~ big_q(X1) ) ),
    inference(flattening,[],[f4]) ).

fof(f6,plain,
    ( ! [X0,X1] :
        ( ! [X2] : ~ big_r(X2)
        & big_p(X0)
        & big_q(X1) )
    & ! [X3,X4] :
      ? [X5] :
        ( big_r(X5)
        | ~ big_p(X3)
        | ~ big_q(X4) ) ),
    inference(rectify,[],[f5]) ).

fof(f7,plain,
    ! [X3,X4] :
      ( ? [X5] :
          ( big_r(X5)
          | ~ big_p(X3)
          | ~ big_q(X4) )
     => ( big_r(sK0(X3,X4))
        | ~ big_p(X3)
        | ~ big_q(X4) ) ),
    introduced(definition,[],[skolem_symbol_introduction]) ).

fof(f8,plain,
    ( ! [X0,X1] :
        ( ! [X2] : ~ big_r(X2)
        & big_p(X0)
        & big_q(X1) )
    & ! [X3,X4] :
        ( big_r(sK0(X3,X4))
        | ~ big_p(X3)
        | ~ big_q(X4) ) ),
    inference(skolemisation,[status(esa),new_symbols(skolem,[sK0])],[f6,f7]) ).

fof(f9,plain,
    ! [X3,X4] :
      ( big_r(sK0(X3,X4))
      | ~ big_p(X3)
      | ~ big_q(X4) ),
    inference(cnf_transformation,[],[f8]) ).

fof(f10,plain,
    ! [X1] : big_q(X1),
    inference(cnf_transformation,[],[f8]) ).

fof(f11,plain,
    ! [X0] : big_p(X0),
    inference(cnf_transformation,[],[f8]) ).

fof(f12,plain,
    ! [X2] : ~ big_r(X2),
    inference(cnf_transformation,[],[f8]) ).

fof(f13,plain,
    ! [X0,X1] :
      ( ~ big_p(X0)
      | ~ big_q(X1) ),
    inference(resolution,[],[f9,f12]) ).

fof(f15,definition,
    ( spl1_1
  <=> ! [X1] : ~ big_q(X1) ),
    introduced(definition,[new_symbols(naming,[spl1_1])],[avatar_definition]) ).

fof(f16,plain,
    ( ! [X1] : ~ big_q(X1)
    | ~ spl1_1 ),
    inference(avatar_component_clause,[],[f15]) ).

fof(f18,definition,
    ( spl1_2
  <=> ! [X0] : ~ big_p(X0) ),
    introduced(definition,[new_symbols(naming,[spl1_2])],[avatar_definition]) ).

fof(f19,plain,
    ( ! [X0] : ~ big_p(X0)
    | ~ spl1_2 ),
    inference(avatar_component_clause,[],[f18]) ).

fof(f20,plain,
    ( spl1_1
    | spl1_2 ),
    inference(avatar_split_clause,[],[f13,f18,f15]) ).

fof(f21,plain,
    ( $false
    | ~ spl1_2 ),
    inference(resolution,[],[f19,f11]) ).

fof(f22,plain,
    ~ spl1_2,
    inference(avatar_contradiction_clause,[],[f21]) ).

fof(f23,plain,
    ( $false
    | ~ spl1_1 ),
    inference(resolution,[],[f16,f10]) ).

fof(f24,plain,
    ~ spl1_1,
    inference(avatar_contradiction_clause,[],[f23]) ).

fof(s1,plain,
    ( spl1_1
    | spl1_2 ),
    inference(sat_conversion,[],[f20]) ).

fof(s2,plain,
    ~ spl1_2,
    inference(sat_conversion,[],[f22]) ).

fof(s3,plain,
    ~ spl1_1,
    inference(sat_conversion,[],[f24]) ).

fof(s4,plain,
    $false,
    inference(rat,[],[s1,s2,s3]) ).

fof(f25,plain,
    $false,
    inference(avatar_sat_refutation,[],[s4]) ).

%------------------------------------------------------------------------------
%----ORIGINAL SYSTEM OUTPUT
% 0.00/0.12  % Problem    : SYN050+1 : TPTP v9.2.1. Released v2.0.0.
% 0.00/0.12  % Command    : run_vampire %s %d THM
% 0.15/0.33  % Computer   : n007.cluster.edu
% 0.15/0.33  % Model      : x86_64 x86_64
% 0.15/0.33  % CPU        : Intel(R) Xeon(R) CPU E5-2620 v4 @ 2.10GHz
% 0.15/0.33  % Memory     : 8042.1875MB
% 0.15/0.33  % OS         : Linux 3.10.0-693.el7.x86_64
% 0.15/0.33  % CPULimit   : 300
% 0.15/0.33  % WCLimit    : 300
% 0.15/0.33  % DateTime   : Fri May  1 05:43:39 EDT 2026
% 0.15/0.33  % CPUTime    : 
% 0.15/0.34  This is a FOF_THM_RFO_NEQ problem
% 0.15/0.34  Running first-order theorem proving
% 0.15/0.34  Running /export/starexec/sandbox/solver/bin/vampire --input_syntax tptp --proof tptp --output_axiom_names on --mode casc --cores 7 -m 16384 -t 300 /export/starexec/sandbox/benchmark/theBenchmark.p
% 0.43/0.59  % (18497)Detected formulas, will run a generic FOF schedule.
% 0.45/0.71  % (18529)dis-21_1_sil=8000:lcm=predicate:random_seed=2461932608:st=5:avsq=on:i=129:avsqr=1,16:sd=3:aac=none:ep=RS:fsr=off:ss=included_3000 on theBenchmark for (3000ds/129Mi)
% 0.45/0.71  % (18529)First to succeed.
% 0.45/0.71  % (18529)Solution written to "/export/starexec/sandbox/tmp/vampire-proof-18497"
% 0.45/0.74  % (18524)lrs+11_1_ncem=casc2026/models/loop8.pt:sil=128000:npcc=on:lma=off:spb=units:urr=ec_only:bce=on:s2agt=64:updr=off:random_seed=602447858:i=134677:sd=20:aac=none:nm=16:ss=included:sgt=10_3000 on theBenchmark for (3000ds/134677Mi)
% 0.45/0.74  % (18526)lrs+1010_1_to=lpo:sil=32000:sos=on:spb=goal_then_units:bce=on:random_seed=1535932293:i=109:sd=1:ins=1:gsp=on:ss=axioms_3000 on theBenchmark for (3000ds/109Mi)
% 0.45/0.74  % (18523)lrs+10_1_ncem=casc2026/models/loop8.pt:sil=128000:tgt=full:npcc=on:drc=off:sp=weighted_frequency:spb=goal:fd=preordered:foolp=on:random_seed=4257667277:i=141193_3000 on theBenchmark for (3000ds/141193Mi)
% 0.45/0.74  % (18527)dis-1010_2:3_sil=16000:sp=reverse_frequency:random_seed=3313800542:i=119:av=off:ss=axioms_3000 on theBenchmark for (3000ds/119Mi)
% 0.45/0.74  % (18525)lrs+1010_1_anc=all:sfv=off:to=kbo:ncem=casc2026/models/loop7.pt:sil=128000:npcc=on:prc=on:sos=all:bsr=unit_only:sac=on:random_seed=67665008:i=141695:sd=1:nm=32:gsp=on:ss=included_3000 on theBenchmark for (3000ds/141695Mi)
% 0.45/0.74  % (18528)dis-1011_1_sil=16000:fde=unused:s2agt=70:random_seed=3087309552:s2a=on:i=139:gtg=position_3000 on theBenchmark for (3000ds/139Mi)
% 0.45/0.74  % (18527)Also succeeded, but the first one will report.
% 0.45/0.74  % (18528)Also succeeded, but the first one will report.
% 0.45/0.74  % (18526)Also succeeded, but the first one will report.
% 0.45/0.86  % (18529)Refutation found. Thanks to Tanya!
% 0.45/0.86  % SZS status Theorem for theBenchmark
% 0.45/0.86  % SZS output start Proof for theBenchmark
% See solution above
% 0.45/0.86  % (18529)------------------------------
% 0.45/0.86  % (18529)Version: Vampire 5.0.1 (Release build, commit 1b9f22200 on 2026-04-29 16:18:29 +0200)
% 0.45/0.86  % (18529)Linked with Z3 4.14.0.0 3c47fd96cf5645d0c42b2c819d9e9a84380aa721 z3-4.8.4-9178-g3c47fd96c
% 0.45/0.86  % (18529)CaDiCaL version: 2.1.3
% 0.45/0.86  % (18529)Termination reason: Refutation
% 0.45/0.86  % (18529)Time elapsed: 0.001 s
% 0.45/0.86  % (18529)Peak memory usage: 81 MB
% 0.45/0.86  % (18529)Instructions burned: 1 (million)
% 0.45/0.86  % (18529)------------------------------
% 0.45/0.86  % (18529)------------------------------
% 0.45/0.86  % (18497)Success in time 0.271 s
%------------------------------------------------------------------------------

