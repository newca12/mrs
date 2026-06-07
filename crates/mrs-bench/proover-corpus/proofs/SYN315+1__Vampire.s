% Proof : Problems/SYN315+1.p
%------------------------------------------------------------------------------
% File     : Vampire---5.0.1
% Problem  : SYN315+1 : TPTP v9.2.1. Released v2.0.0.
% Transfm  : none
% Format   : tptp:raw
% Command  : run_vampire %s %d THM

% Computer : n018.cluster.edu
% Model    : x86_64 x86_64
% CPU      : Intel(R) Xeon(R) CPU E5-2620 v4 2.10GHz
% Memory   : 8042.1875MB
% OS       : Linux 3.10.0-693.el7.x86_64
% CPULimit : 300s
% WCLimit  : 300s
% DateTime : Fri May  1 04:40:30 PM UTC 2026

% Result   : Theorem 0.72s 0.90s
% Output   : Refutation 0.72s
% Verified : 
% SZS Type : Refutation
%            Derivation depth      :   12
%            Number of leaves      :    7
% Syntax   : Number of formulae    :   39 (   5 unt;   5 def)
%            Number of atoms       :  113 (   0 equ)
%            Maximal formula atoms :   16 (   2 avg)
%            Number of connectives :  124 (  50   ~;  44   |;  16   &)
%                                         (  10 <=>;   3  =>;   0  <=;   1 <~>)
%            Maximal formula depth :    9 (   4 avg)
%            Maximal term depth    :    2 (   1 avg)
%            Number of predicates  :    8 (   7 usr;   7 prp; 0-1 aty)
%            Number of functors    :    1 (   1 usr;   0 con; 1-1 aty)
%            Number of variables   :   25 (   0 sgn  19   !;   6   ?)

% Comments : 
%------------------------------------------------------------------------------
fof(f1,conjecture,
    ? [X0] :
    ! [X1] :
      ( ( big_f(X0)
      <=> p )
     => ( big_f(X1)
      <=> p ) ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',church_46_2_1) ).

fof(f2,negated_conjecture,
    ~ ? [X0] :
      ! [X1] :
        ( ( big_f(X0)
        <=> p )
       => ( big_f(X1)
        <=> p ) ),
    inference(negated_conjecture,[status(cth)],[f1]) ).

fof(f3,plain,
    ! [X0] :
    ? [X1] :
      ( ( big_f(X1)
      <~> p )
      & ( big_f(X0)
      <=> p ) ),
    inference(ennf_transformation,[],[f2]) ).

fof(f4,plain,
    ! [X0] :
    ? [X1] :
      ( ( ~ p
        | ~ big_f(X1) )
      & ( p
        | big_f(X1) )
      & ( big_f(X0)
        | ~ p )
      & ( p
        | ~ big_f(X0) ) ),
    inference(nnf_transformation,[],[f3]) ).

fof(f5,plain,
    ! [X0] :
    ? [X1] :
      ( ( ~ p
        | ~ big_f(X1) )
      & ( p
        | big_f(X1) )
      & ( big_f(X0)
        | ~ p )
      & ( p
        | ~ big_f(X0) ) ),
    inference(flattening,[],[f4]) ).

fof(f6,plain,
    ! [X0] :
      ( ? [X1] :
          ( ( ~ p
            | ~ big_f(X1) )
          & ( p
            | big_f(X1) )
          & ( big_f(X0)
            | ~ p )
          & ( p
            | ~ big_f(X0) ) )
     => ( ( ~ p
          | ~ big_f(sK0(X0)) )
        & ( p
          | big_f(sK0(X0)) )
        & ( big_f(X0)
          | ~ p )
        & ( p
          | ~ big_f(X0) ) ) ),
    introduced(definition,[],[skolem_symbol_introduction]) ).

fof(f7,plain,
    ! [X0] :
      ( ( ~ p
        | ~ big_f(sK0(X0)) )
      & ( p
        | big_f(sK0(X0)) )
      & ( big_f(X0)
        | ~ p )
      & ( p
        | ~ big_f(X0) ) ),
    inference(skolemisation,[status(esa),new_symbols(skolem,[sK0])],[f5,f6]) ).

fof(f8,plain,
    ! [X0] :
      ( p
      | ~ big_f(X0) ),
    inference(cnf_transformation,[],[f7]) ).

fof(f9,plain,
    ! [X0] :
      ( big_f(X0)
      | ~ p ),
    inference(cnf_transformation,[],[f7]) ).

fof(f10,plain,
    ! [X0] :
      ( p
      | big_f(sK0(X0)) ),
    inference(cnf_transformation,[],[f7]) ).

fof(f11,plain,
    ! [X0] :
      ( ~ p
      | ~ big_f(sK0(X0)) ),
    inference(cnf_transformation,[],[f7]) ).

fof(f13,definition,
    ( spl1_1
  <=> ! [X0] : ~ big_f(X0) ),
    introduced(definition,[new_symbols(naming,[spl1_1])],[avatar_definition]) ).

fof(f14,plain,
    ( ! [X0] : ~ big_f(X0)
    | ~ spl1_1 ),
    inference(avatar_component_clause,[],[f13]) ).

fof(f16,definition,
    ( spl1_2
  <=> p ),
    introduced(definition,[new_symbols(naming,[spl1_2])],[avatar_definition]) ).

fof(f18,plain,
    ( spl1_1
    | spl1_2 ),
    inference(avatar_split_clause,[],[f8,f16,f13]) ).

fof(f21,definition,
    ( spl1_3
  <=> ! [X0] : big_f(X0) ),
    introduced(definition,[new_symbols(naming,[spl1_3])],[avatar_definition]) ).

fof(f22,plain,
    ( ! [X0] : big_f(X0)
    | ~ spl1_3 ),
    inference(avatar_component_clause,[],[f21]) ).

fof(f23,plain,
    ( ~ spl1_2
    | spl1_3 ),
    inference(avatar_split_clause,[],[f9,f21,f16]) ).

fof(f25,definition,
    ( spl1_4
  <=> ! [X0] : big_f(sK0(X0)) ),
    introduced(definition,[new_symbols(naming,[spl1_4])],[avatar_definition]) ).

fof(f26,plain,
    ( ! [X0] : big_f(sK0(X0))
    | ~ spl1_4 ),
    inference(avatar_component_clause,[],[f25]) ).

fof(f27,plain,
    ( spl1_4
    | spl1_2 ),
    inference(avatar_split_clause,[],[f10,f16,f25]) ).

fof(f29,definition,
    ( spl1_5
  <=> ! [X0] : ~ big_f(sK0(X0)) ),
    introduced(definition,[new_symbols(naming,[spl1_5])],[avatar_definition]) ).

fof(f30,plain,
    ( ! [X0] : ~ big_f(sK0(X0))
    | ~ spl1_5 ),
    inference(avatar_component_clause,[],[f29]) ).

fof(f31,plain,
    ( spl1_5
    | ~ spl1_2 ),
    inference(avatar_split_clause,[],[f11,f16,f29]) ).

fof(f32,plain,
    ( $false
    | ~ spl1_3
    | ~ spl1_5 ),
    inference(resolution,[],[f30,f22]) ).

fof(f33,plain,
    ( ~ spl1_3
    | ~ spl1_5 ),
    inference(avatar_contradiction_clause,[],[f32]) ).

fof(f36,plain,
    ( $false
    | ~ spl1_1
    | ~ spl1_4 ),
    inference(resolution,[],[f26,f14]) ).

fof(f37,plain,
    ( ~ spl1_1
    | ~ spl1_4 ),
    inference(avatar_contradiction_clause,[],[f36]) ).

fof(s1,plain,
    ( spl1_1
    | spl1_2 ),
    inference(sat_conversion,[],[f18]) ).

fof(s2,plain,
    ( ~ spl1_2
    | spl1_3 ),
    inference(sat_conversion,[],[f23]) ).

fof(s3,plain,
    ( spl1_2
    | spl1_4 ),
    inference(sat_conversion,[],[f27]) ).

fof(s4,plain,
    ( ~ spl1_2
    | spl1_5 ),
    inference(sat_conversion,[],[f31]) ).

fof(s5,plain,
    ( ~ spl1_3
    | ~ spl1_5 ),
    inference(sat_conversion,[],[f33]) ).

fof(s7,plain,
    ( ~ spl1_1
    | ~ spl1_4 ),
    inference(sat_conversion,[],[f37]) ).

fof(s8,plain,
    ~ spl1_2,
    inference(rat,[],[s5,s2,s4]) ).

fof(s9,plain,
    spl1_4,
    inference(rat,[],[s3,s8]) ).

fof(s10,plain,
    spl1_1,
    inference(rat,[],[s1,s8]) ).

fof(s11,plain,
    $false,
    inference(rat,[],[s7,s9,s10]) ).

fof(f38,plain,
    $false,
    inference(avatar_sat_refutation,[],[s11]) ).

%------------------------------------------------------------------------------
%----ORIGINAL SYSTEM OUTPUT
% 0.00/0.12  % Problem    : SYN315+1 : TPTP v9.2.1. Released v2.0.0.
% 0.00/0.12  % Command    : run_vampire %s %d THM
% 0.15/0.32  % Computer   : n018.cluster.edu
% 0.15/0.32  % Model      : x86_64 x86_64
% 0.15/0.32  % CPU        : Intel(R) Xeon(R) CPU E5-2620 v4 @ 2.10GHz
% 0.15/0.32  % Memory     : 8042.1875MB
% 0.15/0.32  % OS         : Linux 3.10.0-693.el7.x86_64
% 0.15/0.33  % CPULimit   : 300
% 0.15/0.33  % WCLimit    : 300
% 0.15/0.33  % DateTime   : Fri May  1 05:59:17 EDT 2026
% 0.15/0.33  % CPUTime    : 
% 0.15/0.34  This is a FOF_THM_RFO_NEQ problem
% 0.15/0.35  Running first-order theorem proving
% 0.15/0.35  Running /export/starexec/sandbox2/solver/bin/vampire --input_syntax tptp --proof tptp --output_axiom_names on --mode casc --cores 7 -m 16384 -t 300 /export/starexec/sandbox2/benchmark/theBenchmark.p
% 0.46/0.64  % (4815)Detected formulas, will run a generic FOF schedule.
% 0.48/0.75  % (4823)dis-21_1_sil=8000:lcm=predicate:random_seed=2882031750:st=5:avsq=on:i=129:avsqr=1,16:sd=3:aac=none:ep=RS:fsr=off:ss=included_3000 on theBenchmark for (3000ds/129Mi)
% 0.48/0.75  % (4823)First to succeed.
% 0.48/0.75  % (4823)Solution written to "/export/starexec/sandbox2/tmp/vampire-proof-4815"
% 0.48/0.77  % (4817)lrs+10_1_ncem=casc2026/models/loop8.pt:sil=128000:tgt=full:npcc=on:drc=off:sp=weighted_frequency:spb=goal:fd=preordered:foolp=on:random_seed=720114470:i=141193_3000 on theBenchmark for (3000ds/141193Mi)
% 0.48/0.77  % (4818)lrs+11_1_ncem=casc2026/models/loop8.pt:sil=128000:npcc=on:lma=off:spb=units:urr=ec_only:bce=on:s2agt=64:updr=off:random_seed=3469753349:i=134677:sd=20:aac=none:nm=16:ss=included:sgt=10_3000 on theBenchmark for (3000ds/134677Mi)
% 0.48/0.77  % (4819)lrs+1010_1_anc=all:sfv=off:to=kbo:ncem=casc2026/models/loop7.pt:sil=128000:npcc=on:prc=on:sos=all:bsr=unit_only:sac=on:random_seed=3481551694:i=141695:sd=1:nm=32:gsp=on:ss=included_3000 on theBenchmark for (3000ds/141695Mi)
% 0.48/0.77  % (4821)dis-1010_2:3_sil=16000:sp=reverse_frequency:random_seed=1402904765:i=119:av=off:ss=axioms_3000 on theBenchmark for (3000ds/119Mi)
% 0.48/0.77  % (4822)dis-1011_1_sil=16000:fde=unused:s2agt=70:random_seed=2610424943:s2a=on:i=139:gtg=position_3000 on theBenchmark for (3000ds/139Mi)
% 0.48/0.78  % (4821)Also succeeded, but the first one will report.
% 0.48/0.78  % (4822)Also succeeded, but the first one will report.
% 0.48/0.79  % (4820)lrs+1010_1_to=lpo:sil=32000:sos=on:spb=goal_then_units:bce=on:random_seed=598311872:i=109:sd=1:ins=1:gsp=on:ss=axioms_3000 on theBenchmark for (3000ds/109Mi)
% 0.48/0.79  % (4820)Also succeeded, but the first one will report.
% 0.72/0.90  % (4823)Refutation found. Thanks to Tanya!
% 0.72/0.90  % SZS status Theorem for theBenchmark
% 0.72/0.90  % SZS output start Proof for theBenchmark
% See solution above
% 0.72/0.90  % (4823)------------------------------
% 0.72/0.90  % (4823)Version: Vampire 5.0.1 (Release build, commit 1b9f22200 on 2026-04-29 16:18:29 +0200)
% 0.72/0.90  % (4823)Linked with Z3 4.14.0.0 3c47fd96cf5645d0c42b2c819d9e9a84380aa721 z3-4.8.4-9178-g3c47fd96c
% 0.72/0.90  % (4823)CaDiCaL version: 2.1.3
% 0.72/0.90  % (4823)Termination reason: Refutation
% 0.72/0.90  % (4823)Time elapsed: 0.001 s
% 0.72/0.90  % (4823)Peak memory usage: 81 MB
% 0.72/0.90  % (4823)Instructions burned: 1 (million)
% 0.72/0.90  % (4823)------------------------------
% 0.72/0.90  % (4823)------------------------------
% 0.72/0.90  % (4815)Success in time 0.261 s
%------------------------------------------------------------------------------

