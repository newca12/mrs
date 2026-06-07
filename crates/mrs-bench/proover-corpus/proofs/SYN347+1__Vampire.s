% Proof : Problems/SYN347+1.p
%------------------------------------------------------------------------------
% File     : Vampire---5.0.1
% Problem  : SYN347+1 : TPTP v9.2.1. Released v2.0.0.
% Transfm  : none
% Format   : tptp:raw
% Command  : run_vampire %s %d THM

% Computer : n005.cluster.edu
% Model    : x86_64 x86_64
% CPU      : Intel(R) Xeon(R) CPU E5-2620 v4 2.10GHz
% Memory   : 8042.1875MB
% OS       : Linux 3.10.0-693.el7.x86_64
% CPULimit : 300s
% WCLimit  : 300s
% DateTime : Fri May  1 04:40:39 PM UTC 2026

% Result   : Theorem 0.98s 0.90s
% Output   : Refutation 0.98s
% Verified : 
% SZS Type : Refutation
%            Derivation depth      :   14
%            Number of leaves      :    8
% Syntax   : Number of formulae    :   48 (   5 unt;   5 def)
%            Number of atoms       :  205 (   0 equ)
%            Maximal formula atoms :   28 (   4 avg)
%            Number of connectives :  257 ( 100   ~; 105   |;  36   &)
%                                         (  12 <=>;   2  =>;   0  <=;   2 <~>)
%            Maximal formula depth :   13 (   5 avg)
%            Maximal term depth    :    2 (   1 avg)
%            Number of predicates  :    7 (   6 usr;   6 prp; 0-2 aty)
%            Number of functors    :    3 (   3 usr;   2 con; 0-2 aty)
%            Number of variables   :   69 (   0 sgn  51   !;  18   ?)

% Comments : 
%------------------------------------------------------------------------------
fof(f1,conjecture,
    ! [X0,X1] :
    ? [X2,X3] :
    ! [X4] :
      ( ( ( big_f(X2,X4)
        <=> big_f(X3,X4) )
      <=> big_f(X0,X1) )
      | ( big_f(X0,X4)
      <=> big_f(X1,X4) ) ),
    file('/export/starexec/sandbox/benchmark/theBenchmark.p',church_46_17_3) ).

fof(f2,negated_conjecture,
    ~ ! [X0,X1] :
      ? [X2,X3] :
      ! [X4] :
        ( ( ( big_f(X2,X4)
          <=> big_f(X3,X4) )
        <=> big_f(X0,X1) )
        | ( big_f(X0,X4)
        <=> big_f(X1,X4) ) ),
    inference(negated_conjecture,[status(cth)],[f1]) ).

fof(f3,plain,
    ? [X0,X1] :
    ! [X2,X3] :
    ? [X4] :
      ( ( ( big_f(X2,X4)
        <=> big_f(X3,X4) )
      <~> big_f(X0,X1) )
      & ( big_f(X0,X4)
      <~> big_f(X1,X4) ) ),
    inference(ennf_transformation,[],[f2]) ).

fof(f4,plain,
    ? [X0,X1] :
    ! [X2,X3] :
    ? [X4] :
      ( ( ~ big_f(X0,X1)
        | ( ( ~ big_f(X3,X4)
            | ~ big_f(X2,X4) )
          & ( big_f(X3,X4)
            | big_f(X2,X4) ) ) )
      & ( big_f(X0,X1)
        | ( ( big_f(X2,X4)
            | ~ big_f(X3,X4) )
          & ( big_f(X3,X4)
            | ~ big_f(X2,X4) ) ) )
      & ( ~ big_f(X1,X4)
        | ~ big_f(X0,X4) )
      & ( big_f(X1,X4)
        | big_f(X0,X4) ) ),
    inference(nnf_transformation,[],[f3]) ).

fof(f5,plain,
    ? [X0,X1] :
    ! [X2,X3] :
    ? [X4] :
      ( ( ~ big_f(X0,X1)
        | ( ( ~ big_f(X3,X4)
            | ~ big_f(X2,X4) )
          & ( big_f(X3,X4)
            | big_f(X2,X4) ) ) )
      & ( big_f(X0,X1)
        | ( ( big_f(X2,X4)
            | ~ big_f(X3,X4) )
          & ( big_f(X3,X4)
            | ~ big_f(X2,X4) ) ) )
      & ( ~ big_f(X1,X4)
        | ~ big_f(X0,X4) )
      & ( big_f(X1,X4)
        | big_f(X0,X4) ) ),
    inference(flattening,[],[f4]) ).

fof(f6,plain,
    ( ? [X0,X1] :
      ! [X2,X3] :
      ? [X4] :
        ( ( ~ big_f(X0,X1)
          | ( ( ~ big_f(X3,X4)
              | ~ big_f(X2,X4) )
            & ( big_f(X3,X4)
              | big_f(X2,X4) ) ) )
        & ( big_f(X0,X1)
          | ( ( big_f(X2,X4)
              | ~ big_f(X3,X4) )
            & ( big_f(X3,X4)
              | ~ big_f(X2,X4) ) ) )
        & ( ~ big_f(X1,X4)
          | ~ big_f(X0,X4) )
        & ( big_f(X1,X4)
          | big_f(X0,X4) ) )
   => ! [X3,X2] :
      ? [X4] :
        ( ( ~ big_f(sK0,sK1)
          | ( ( ~ big_f(X3,X4)
              | ~ big_f(X2,X4) )
            & ( big_f(X3,X4)
              | big_f(X2,X4) ) ) )
        & ( big_f(sK0,sK1)
          | ( ( big_f(X2,X4)
              | ~ big_f(X3,X4) )
            & ( big_f(X3,X4)
              | ~ big_f(X2,X4) ) ) )
        & ( ~ big_f(sK1,X4)
          | ~ big_f(sK0,X4) )
        & ( big_f(sK1,X4)
          | big_f(sK0,X4) ) ) ),
    introduced(definition,[],[skolem_symbol_introduction]) ).

fof(f7,plain,
    ! [X2,X3] :
      ( ? [X4] :
          ( ( ~ big_f(sK0,sK1)
            | ( ( ~ big_f(X3,X4)
                | ~ big_f(X2,X4) )
              & ( big_f(X3,X4)
                | big_f(X2,X4) ) ) )
          & ( big_f(sK0,sK1)
            | ( ( big_f(X2,X4)
                | ~ big_f(X3,X4) )
              & ( big_f(X3,X4)
                | ~ big_f(X2,X4) ) ) )
          & ( ~ big_f(sK1,X4)
            | ~ big_f(sK0,X4) )
          & ( big_f(sK1,X4)
            | big_f(sK0,X4) ) )
     => ( ( ~ big_f(sK0,sK1)
          | ( ( ~ big_f(X3,sK2(X2,X3))
              | ~ big_f(X2,sK2(X2,X3)) )
            & ( big_f(X3,sK2(X2,X3))
              | big_f(X2,sK2(X2,X3)) ) ) )
        & ( big_f(sK0,sK1)
          | ( ( big_f(X2,sK2(X2,X3))
              | ~ big_f(X3,sK2(X2,X3)) )
            & ( big_f(X3,sK2(X2,X3))
              | ~ big_f(X2,sK2(X2,X3)) ) ) )
        & ( ~ big_f(sK1,sK2(X2,X3))
          | ~ big_f(sK0,sK2(X2,X3)) )
        & ( big_f(sK1,sK2(X2,X3))
          | big_f(sK0,sK2(X2,X3)) ) ) ),
    introduced(definition,[],[skolem_symbol_introduction]) ).

fof(f8,plain,
    ! [X2,X3] :
      ( ( ~ big_f(sK0,sK1)
        | ( ( ~ big_f(X3,sK2(X2,X3))
            | ~ big_f(X2,sK2(X2,X3)) )
          & ( big_f(X3,sK2(X2,X3))
            | big_f(X2,sK2(X2,X3)) ) ) )
      & ( big_f(sK0,sK1)
        | ( ( big_f(X2,sK2(X2,X3))
            | ~ big_f(X3,sK2(X2,X3)) )
          & ( big_f(X3,sK2(X2,X3))
            | ~ big_f(X2,sK2(X2,X3)) ) ) )
      & ( ~ big_f(sK1,sK2(X2,X3))
        | ~ big_f(sK0,sK2(X2,X3)) )
      & ( big_f(sK1,sK2(X2,X3))
        | big_f(sK0,sK2(X2,X3)) ) ),
    inference(skolemisation,[status(esa),new_symbols(skolem,[sK0,sK1,sK2])],[f5,f7,f6]) ).

fof(f9,plain,
    ! [X2,X3] :
      ( big_f(sK1,sK2(X2,X3))
      | big_f(sK0,sK2(X2,X3)) ),
    inference(cnf_transformation,[],[f8]) ).

fof(f10,plain,
    ! [X2,X3] :
      ( ~ big_f(sK0,sK2(X2,X3))
      | ~ big_f(sK1,sK2(X2,X3)) ),
    inference(cnf_transformation,[],[f8]) ).

fof(f11,plain,
    ! [X2,X3] :
      ( big_f(sK0,sK1)
      | big_f(X3,sK2(X2,X3))
      | ~ big_f(X2,sK2(X2,X3)) ),
    inference(cnf_transformation,[],[f8]) ).

fof(f12,plain,
    ! [X2,X3] :
      ( big_f(sK0,sK1)
      | big_f(X2,sK2(X2,X3))
      | ~ big_f(X3,sK2(X2,X3)) ),
    inference(cnf_transformation,[],[f8]) ).

fof(f13,plain,
    ! [X2,X3] :
      ( ~ big_f(sK0,sK1)
      | big_f(X3,sK2(X2,X3))
      | big_f(X2,sK2(X2,X3)) ),
    inference(cnf_transformation,[],[f8]) ).

fof(f14,plain,
    ! [X2,X3] :
      ( ~ big_f(sK0,sK1)
      | ~ big_f(X3,sK2(X2,X3))
      | ~ big_f(X2,sK2(X2,X3)) ),
    inference(cnf_transformation,[],[f8]) ).

fof(f16,definition,
    ( spl3_1
  <=> ! [X2,X3] :
        ( big_f(X3,sK2(X2,X3))
        | ~ big_f(X2,sK2(X2,X3)) ) ),
    introduced(definition,[new_symbols(naming,[spl3_1])],[avatar_definition]) ).

fof(f17,plain,
    ( ! [X2,X3] :
        ( big_f(X3,sK2(X2,X3))
        | ~ big_f(X2,sK2(X2,X3)) )
    | ~ spl3_1 ),
    inference(avatar_component_clause,[],[f16]) ).

fof(f19,definition,
    ( spl3_2
  <=> big_f(sK0,sK1) ),
    introduced(definition,[new_symbols(naming,[spl3_2])],[avatar_definition]) ).

fof(f22,plain,
    ( spl3_1
    | spl3_2 ),
    inference(avatar_split_clause,[],[f11,f19,f16]) ).

fof(f24,definition,
    ( spl3_3
  <=> ! [X2,X3] :
        ( big_f(X2,sK2(X2,X3))
        | ~ big_f(X3,sK2(X2,X3)) ) ),
    introduced(definition,[new_symbols(naming,[spl3_3])],[avatar_definition]) ).

fof(f25,plain,
    ( ! [X2,X3] :
        ( big_f(X2,sK2(X2,X3))
        | ~ big_f(X3,sK2(X2,X3)) )
    | ~ spl3_3 ),
    inference(avatar_component_clause,[],[f24]) ).

fof(f26,plain,
    ( spl3_3
    | spl3_2 ),
    inference(avatar_split_clause,[],[f12,f19,f24]) ).

fof(f28,definition,
    ( spl3_4
  <=> ! [X2,X3] :
        ( big_f(X3,sK2(X2,X3))
        | big_f(X2,sK2(X2,X3)) ) ),
    introduced(definition,[new_symbols(naming,[spl3_4])],[avatar_definition]) ).

fof(f29,plain,
    ( ! [X2,X3] :
        ( big_f(X3,sK2(X2,X3))
        | big_f(X2,sK2(X2,X3)) )
    | ~ spl3_4 ),
    inference(avatar_component_clause,[],[f28]) ).

fof(f30,plain,
    ( spl3_4
    | ~ spl3_2 ),
    inference(avatar_split_clause,[],[f13,f19,f28]) ).

fof(f32,definition,
    ( spl3_5
  <=> ! [X2,X3] :
        ( ~ big_f(X3,sK2(X2,X3))
        | ~ big_f(X2,sK2(X2,X3)) ) ),
    introduced(definition,[new_symbols(naming,[spl3_5])],[avatar_definition]) ).

fof(f33,plain,
    ( ! [X2,X3] :
        ( ~ big_f(X3,sK2(X2,X3))
        | ~ big_f(X2,sK2(X2,X3)) )
    | ~ spl3_5 ),
    inference(avatar_component_clause,[],[f32]) ).

fof(f34,plain,
    ( spl3_5
    | ~ spl3_2 ),
    inference(avatar_split_clause,[],[f14,f19,f32]) ).

fof(f38,plain,
    ( ! [X0] : ~ big_f(X0,sK2(X0,X0))
    | ~ spl3_5 ),
    inference(factoring,[],[f33]) ).

fof(f40,plain,
    ( ! [X0] : big_f(X0,sK2(X0,X0))
    | ~ spl3_4
    | ~ spl3_5 ),
    inference(resolution,[],[f38,f29]) ).

fof(f41,plain,
    ( $false
    | ~ spl3_4
    | ~ spl3_5 ),
    inference(forward_subsumption_resolution,[],[f40,f38]) ).

fof(f42,plain,
    ( ~ spl3_4
    | ~ spl3_5 ),
    inference(avatar_contradiction_clause,[],[f41]) ).

fof(f50,plain,
    ( ! [X0] :
        ( ~ big_f(sK1,sK2(X0,sK0))
        | ~ big_f(X0,sK2(X0,sK0)) )
    | ~ spl3_1 ),
    inference(resolution,[],[f17,f10]) ).

fof(f54,plain,
    ( ~ big_f(sK1,sK2(sK1,sK0))
    | ~ spl3_1 ),
    inference(factoring,[],[f50]) ).

fof(f58,plain,
    ( big_f(sK0,sK2(sK1,sK0))
    | ~ spl3_1 ),
    inference(resolution,[],[f54,f9]) ).

fof(f59,plain,
    ( ~ big_f(sK0,sK2(sK1,sK0))
    | ~ spl3_1
    | ~ spl3_3 ),
    inference(resolution,[],[f54,f25]) ).

fof(f63,plain,
    ( $false
    | ~ spl3_1
    | ~ spl3_3 ),
    inference(forward_subsumption_resolution,[],[f59,f58]) ).

fof(f64,plain,
    ( ~ spl3_1
    | ~ spl3_3 ),
    inference(avatar_contradiction_clause,[],[f63]) ).

fof(s1,plain,
    ( spl3_1
    | spl3_2 ),
    inference(sat_conversion,[],[f22]) ).

fof(s2,plain,
    ( spl3_2
    | spl3_3 ),
    inference(sat_conversion,[],[f26]) ).

fof(s3,plain,
    ( ~ spl3_2
    | spl3_4 ),
    inference(sat_conversion,[],[f30]) ).

fof(s4,plain,
    ( ~ spl3_2
    | spl3_5 ),
    inference(sat_conversion,[],[f34]) ).

fof(s5,plain,
    ( ~ spl3_4
    | ~ spl3_5 ),
    inference(sat_conversion,[],[f42]) ).

fof(s7,plain,
    ( ~ spl3_1
    | ~ spl3_3 ),
    inference(sat_conversion,[],[f64]) ).

fof(s8,plain,
    ~ spl3_2,
    inference(rat,[],[s5,s3,s4]) ).

fof(s9,plain,
    spl3_3,
    inference(rat,[],[s2,s8]) ).

fof(s10,plain,
    spl3_1,
    inference(rat,[],[s1,s8]) ).

fof(s11,plain,
    $false,
    inference(rat,[],[s7,s9,s10]) ).

fof(f65,plain,
    $false,
    inference(avatar_sat_refutation,[],[s11]) ).

%------------------------------------------------------------------------------
%----ORIGINAL SYSTEM OUTPUT
% 0.00/0.12  % Problem    : SYN347+1 : TPTP v9.2.1. Released v2.0.0.
% 0.00/0.12  % Command    : run_vampire %s %d THM
% 0.15/0.32  % Computer   : n005.cluster.edu
% 0.15/0.32  % Model      : x86_64 x86_64
% 0.15/0.32  % CPU        : Intel(R) Xeon(R) CPU E5-2620 v4 @ 2.10GHz
% 0.15/0.32  % Memory     : 8042.1875MB
% 0.15/0.32  % OS         : Linux 3.10.0-693.el7.x86_64
% 0.15/0.32  % CPULimit   : 300
% 0.15/0.32  % WCLimit    : 300
% 0.15/0.32  % DateTime   : Fri May  1 06:01:45 EDT 2026
% 0.15/0.33  % CPUTime    : 
% 0.15/0.34  This is a FOF_THM_RFO_NEQ problem
% 0.15/0.35  Running first-order theorem proving
% 0.15/0.35  Running /export/starexec/sandbox/solver/bin/vampire --input_syntax tptp --proof tptp --output_axiom_names on --mode casc --cores 7 -m 16384 -t 300 /export/starexec/sandbox/benchmark/theBenchmark.p
% 0.47/0.63  % (19190)Detected formulas, will run a generic FOF schedule.
% 0.48/0.75  % (19197)dis-1011_1_sil=16000:fde=unused:s2agt=70:random_seed=1095278287:s2a=on:i=139:gtg=position_3000 on theBenchmark for (3000ds/139Mi)
% 0.48/0.75  % (19197)First to succeed.
% 0.48/0.75  % (19197)Solution written to "/export/starexec/sandbox/tmp/vampire-proof-19190"
% 0.48/0.78  % (19193)lrs+11_1_ncem=casc2026/models/loop8.pt:sil=128000:npcc=on:lma=off:spb=units:urr=ec_only:bce=on:s2agt=64:updr=off:random_seed=2943144412:i=134677:sd=20:aac=none:nm=16:ss=included:sgt=10_3000 on theBenchmark for (3000ds/134677Mi)
% 0.48/0.78  % (19196)dis-1010_2:3_sil=16000:sp=reverse_frequency:random_seed=131575963:i=119:av=off:ss=axioms_3000 on theBenchmark for (3000ds/119Mi)
% 0.48/0.78  % (19192)lrs+10_1_ncem=casc2026/models/loop8.pt:sil=128000:tgt=full:npcc=on:drc=off:sp=weighted_frequency:spb=goal:fd=preordered:foolp=on:random_seed=2169824752:i=141193_3000 on theBenchmark for (3000ds/141193Mi)
% 0.48/0.78  % (19198)dis-21_1_sil=8000:lcm=predicate:random_seed=755103160:st=5:avsq=on:i=129:avsqr=1,16:sd=3:aac=none:ep=RS:fsr=off:ss=included_3000 on theBenchmark for (3000ds/129Mi)
% 0.48/0.78  % (19195)lrs+1010_1_to=lpo:sil=32000:sos=on:spb=goal_then_units:bce=on:random_seed=2480095597:i=109:sd=1:ins=1:gsp=on:ss=axioms_3000 on theBenchmark for (3000ds/109Mi)
% 0.48/0.78  % (19194)lrs+1010_1_anc=all:sfv=off:to=kbo:ncem=casc2026/models/loop7.pt:sil=128000:npcc=on:prc=on:sos=all:bsr=unit_only:sac=on:random_seed=1085317587:i=141695:sd=1:nm=32:gsp=on:ss=included_3000 on theBenchmark for (3000ds/141695Mi)
% 0.48/0.78  % (19196)Also succeeded, but the first one will report.
% 0.48/0.78  % (19195)Also succeeded, but the first one will report.
% 0.48/0.78  % (19198)Also succeeded, but the first one will report.
% 0.98/0.90  % (19197)Refutation found. Thanks to Tanya!
% 0.98/0.90  % SZS status Theorem for theBenchmark
% 0.98/0.90  % SZS output start Proof for theBenchmark
% See solution above
% 0.98/0.90  % (19197)------------------------------
% 0.98/0.90  % (19197)Version: Vampire 5.0.1 (Release build, commit 1b9f22200 on 2026-04-29 16:18:29 +0200)
% 0.98/0.90  % (19197)Linked with Z3 4.14.0.0 3c47fd96cf5645d0c42b2c819d9e9a84380aa721 z3-4.8.4-9178-g3c47fd96c
% 0.98/0.90  % (19197)CaDiCaL version: 2.1.3
% 0.98/0.90  % (19197)Termination reason: Refutation
% 0.98/0.90  % (19197)Time elapsed: 0.002 s
% 0.98/0.90  % (19197)Peak memory usage: 81 MB
% 0.98/0.90  % (19197)Instructions burned: 3 (million)
% 0.98/0.90  % (19197)------------------------------
% 0.98/0.90  % (19197)------------------------------
% 0.98/0.90  % (19190)Success in time 0.266 s
%------------------------------------------------------------------------------

