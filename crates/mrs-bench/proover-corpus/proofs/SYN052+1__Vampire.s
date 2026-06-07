% Proof : Problems/SYN052+1.p
%------------------------------------------------------------------------------
% File     : Vampire---5.0.1
% Problem  : SYN052+1 : TPTP v9.2.1. Released v2.0.0.
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
% DateTime : Fri May  1 04:39:27 PM UTC 2026

% Result   : Theorem 0.49s 0.93s
% Output   : Refutation 0.49s
% Verified : 
% SZS Type : Refutation
%            Derivation depth      :   13
%            Number of leaves      :    6
% Syntax   : Number of formulae    :   38 (   5 unt;   4 def)
%            Number of atoms       :  103 (   0 equ)
%            Maximal formula atoms :    8 (   2 avg)
%            Number of connectives :  109 (  44   ~;  39   |;  13   &)
%                                         (   9 <=>;   3  =>;   0  <=;   1 <~>)
%            Maximal formula depth :    7 (   3 avg)
%            Maximal term depth    :    1 (   1 avg)
%            Number of predicates  :    7 (   6 usr;   6 prp; 0-1 aty)
%            Number of functors    :    1 (   1 usr;   1 con; 0-0 aty)
%            Number of variables   :   25 (   0 sgn  21   !;   4   ?)

% Comments : 
%------------------------------------------------------------------------------
fof(f1,conjecture,
    ( ! [X0] :
        ( p
      <=> big_f(X0) )
   => ( p
    <=> ! [X1] : big_f(X1) ) ),
    file('/export/starexec/sandbox/benchmark/theBenchmark.p',pel22) ).

fof(f2,negated_conjecture,
    ~ ( ! [X0] :
          ( p
        <=> big_f(X0) )
     => ( p
      <=> ! [X1] : big_f(X1) ) ),
    inference(negated_conjecture,[status(cth)],[f1]) ).

fof(f3,plain,
    ( ( p
    <~> ! [X1] : big_f(X1) )
    & ! [X0] :
        ( p
      <=> big_f(X0) ) ),
    inference(ennf_transformation,[],[f2]) ).

fof(f4,plain,
    ( ( ? [X1] : ~ big_f(X1)
      | ~ p )
    & ( ! [X1] : big_f(X1)
      | p )
    & ! [X0] :
        ( ( p
          | ~ big_f(X0) )
        & ( big_f(X0)
          | ~ p ) ) ),
    inference(nnf_transformation,[],[f3]) ).

fof(f5,plain,
    ( ( ? [X1] : ~ big_f(X1)
      | ~ p )
    & ( ! [X1] : big_f(X1)
      | p )
    & ! [X0] :
        ( ( p
          | ~ big_f(X0) )
        & ( big_f(X0)
          | ~ p ) ) ),
    inference(flattening,[],[f4]) ).

fof(f6,plain,
    ( ( ? [X0] : ~ big_f(X0)
      | ~ p )
    & ( ! [X1] : big_f(X1)
      | p )
    & ! [X2] :
        ( ( p
          | ~ big_f(X2) )
        & ( big_f(X2)
          | ~ p ) ) ),
    inference(rectify,[],[f5]) ).

fof(f7,plain,
    ( ? [X0] : ~ big_f(X0)
   => ~ big_f(sK0) ),
    introduced(definition,[],[skolem_symbol_introduction]) ).

fof(f8,plain,
    ( ( ~ big_f(sK0)
      | ~ p )
    & ( ! [X1] : big_f(X1)
      | p )
    & ! [X2] :
        ( ( p
          | ~ big_f(X2) )
        & ( big_f(X2)
          | ~ p ) ) ),
    inference(skolemisation,[status(esa),new_symbols(skolem,[sK0])],[f6,f7]) ).

fof(f9,plain,
    ! [X2] :
      ( big_f(X2)
      | ~ p ),
    inference(cnf_transformation,[],[f8]) ).

fof(f10,plain,
    ! [X2] :
      ( p
      | ~ big_f(X2) ),
    inference(cnf_transformation,[],[f8]) ).

fof(f11,plain,
    ! [X1] :
      ( big_f(X1)
      | p ),
    inference(cnf_transformation,[],[f8]) ).

fof(f12,plain,
    ( ~ big_f(sK0)
    | ~ p ),
    inference(cnf_transformation,[],[f8]) ).

fof(f14,definition,
    ( spl1_1
  <=> p ),
    introduced(definition,[new_symbols(naming,[spl1_1])],[avatar_definition]) ).

fof(f18,definition,
    ( spl1_2
  <=> ! [X2] : big_f(X2) ),
    introduced(definition,[new_symbols(naming,[spl1_2])],[avatar_definition]) ).

fof(f19,plain,
    ( ! [X2] : big_f(X2)
    | ~ spl1_2 ),
    inference(avatar_component_clause,[],[f18]) ).

fof(f20,plain,
    ( ~ spl1_1
    | spl1_2 ),
    inference(avatar_split_clause,[],[f9,f18,f14]) ).

fof(f22,definition,
    ( spl1_3
  <=> ! [X2] : ~ big_f(X2) ),
    introduced(definition,[new_symbols(naming,[spl1_3])],[avatar_definition]) ).

fof(f23,plain,
    ( ! [X2] : ~ big_f(X2)
    | ~ spl1_3 ),
    inference(avatar_component_clause,[],[f22]) ).

fof(f24,plain,
    ( spl1_3
    | spl1_1 ),
    inference(avatar_split_clause,[],[f10,f14,f22]) ).

fof(f25,plain,
    ( spl1_1
    | spl1_2 ),
    inference(avatar_split_clause,[],[f11,f18,f14]) ).

fof(f27,definition,
    ( spl1_4
  <=> big_f(sK0) ),
    introduced(definition,[new_symbols(naming,[spl1_4])],[avatar_definition]) ).

fof(f29,plain,
    ( ~ big_f(sK0)
    | spl1_4 ),
    inference(avatar_component_clause,[],[f27]) ).

fof(f30,plain,
    ( ~ spl1_1
    | ~ spl1_4 ),
    inference(avatar_split_clause,[],[f12,f27,f14]) ).

fof(f31,plain,
    ( $false
    | ~ spl1_2
    | ~ spl1_3 ),
    inference(forward_subsumption_resolution,[],[f23,f19]) ).

fof(f32,plain,
    ( ~ spl1_2
    | ~ spl1_3 ),
    inference(avatar_contradiction_clause,[],[f31]) ).

fof(f33,plain,
    ( $false
    | ~ spl1_2
    | spl1_4 ),
    inference(forward_subsumption_resolution,[],[f29,f19]) ).

fof(f34,plain,
    ( ~ spl1_2
    | spl1_4 ),
    inference(avatar_contradiction_clause,[],[f33]) ).

fof(s1,plain,
    ( ~ spl1_1
    | spl1_2 ),
    inference(sat_conversion,[],[f20]) ).

fof(s2,plain,
    ( spl1_1
    | spl1_3 ),
    inference(sat_conversion,[],[f24]) ).

fof(s3,plain,
    ( spl1_1
    | spl1_2 ),
    inference(sat_conversion,[],[f25]) ).

fof(s4,plain,
    ( ~ spl1_1
    | ~ spl1_4 ),
    inference(sat_conversion,[],[f30]) ).

fof(s5,plain,
    ( ~ spl1_2
    | ~ spl1_3 ),
    inference(sat_conversion,[],[f32]) ).

fof(s6,plain,
    ( ~ spl1_2
    | spl1_4 ),
    inference(sat_conversion,[],[f34]) ).

fof(s7,plain,
    spl1_1,
    inference(rat,[],[s5,s2,s3]) ).

fof(s8,plain,
    ~ spl1_4,
    inference(rat,[],[s4,s7]) ).

fof(s9,plain,
    spl1_2,
    inference(rat,[],[s1,s7]) ).

fof(s10,plain,
    $false,
    inference(rat,[],[s6,s8,s9]) ).

fof(f35,plain,
    $false,
    inference(avatar_sat_refutation,[],[s10]) ).

%------------------------------------------------------------------------------
%----ORIGINAL SYSTEM OUTPUT
% 0.00/0.12  % Problem    : SYN052+1 : TPTP v9.2.1. Released v2.0.0.
% 0.00/0.12  % Command    : run_vampire %s %d THM
% 0.17/0.33  % Computer   : n007.cluster.edu
% 0.17/0.33  % Model      : x86_64 x86_64
% 0.17/0.33  % CPU        : Intel(R) Xeon(R) CPU E5-2620 v4 @ 2.10GHz
% 0.17/0.33  % Memory     : 8042.1875MB
% 0.17/0.33  % OS         : Linux 3.10.0-693.el7.x86_64
% 0.17/0.33  % CPULimit   : 300
% 0.17/0.33  % WCLimit    : 300
% 0.17/0.33  % DateTime   : Fri May  1 05:42:54 EDT 2026
% 0.17/0.33  % CPUTime    : 
% 0.17/0.35  This is a FOF_THM_RFO_NEQ problem
% 0.17/0.35  Running first-order theorem proving
% 0.17/0.35  Running /export/starexec/sandbox/solver/bin/vampire --input_syntax tptp --proof tptp --output_axiom_names on --mode casc --cores 7 -m 16384 -t 300 /export/starexec/sandbox/benchmark/theBenchmark.p
% 0.49/0.64  % (14659)Detected formulas, will run a generic FOF schedule.
% 0.49/0.73  % (14679)dis-1011_1_sil=16000:fde=unused:s2agt=70:random_seed=1335229187:s2a=on:i=139:gtg=position_3000 on theBenchmark for (3000ds/139Mi)
% 0.49/0.73  % (14679)First to succeed.
% 0.49/0.73  % (14679)Solution written to "/export/starexec/sandbox/tmp/vampire-proof-14659"
% 0.49/0.76  % (14677)lrs+1010_1_to=lpo:sil=32000:sos=on:spb=goal_then_units:bce=on:random_seed=3666238390:i=109:sd=1:ins=1:gsp=on:ss=axioms_3000 on theBenchmark for (3000ds/109Mi)
% 0.49/0.77  % (14674)lrs+10_1_ncem=casc2026/models/loop8.pt:sil=128000:tgt=full:npcc=on:drc=off:sp=weighted_frequency:spb=goal:fd=preordered:foolp=on:random_seed=3568404722:i=141193_3000 on theBenchmark for (3000ds/141193Mi)
% 0.49/0.77  % (14677)Also succeeded, but the first one will report.
% 0.49/0.77  % (14680)dis-21_1_sil=8000:lcm=predicate:random_seed=3499702659:st=5:avsq=on:i=129:avsqr=1,16:sd=3:aac=none:ep=RS:fsr=off:ss=included_3000 on theBenchmark for (3000ds/129Mi)
% 0.49/0.77  % (14680)Also succeeded, but the first one will report.
% 0.49/0.77  % (14678)dis-1010_2:3_sil=16000:sp=reverse_frequency:random_seed=1568186365:i=119:av=off:ss=axioms_3000 on theBenchmark for (3000ds/119Mi)
% 0.49/0.77  % (14676)lrs+1010_1_anc=all:sfv=off:to=kbo:ncem=casc2026/models/loop7.pt:sil=128000:npcc=on:prc=on:sos=all:bsr=unit_only:sac=on:random_seed=900180864:i=141695:sd=1:nm=32:gsp=on:ss=included_3000 on theBenchmark for (3000ds/141695Mi)
% 0.49/0.77  % (14678)Also succeeded, but the first one will report.
% 0.49/0.77  % (14675)lrs+11_1_ncem=casc2026/models/loop8.pt:sil=128000:npcc=on:lma=off:spb=units:urr=ec_only:bce=on:s2agt=64:updr=off:random_seed=1075804491:i=134677:sd=20:aac=none:nm=16:ss=included:sgt=10_3000 on theBenchmark for (3000ds/134677Mi)
% 0.49/0.93  % (14679)Refutation found. Thanks to Tanya!
% 0.49/0.93  % SZS status Theorem for theBenchmark
% 0.49/0.93  % SZS output start Proof for theBenchmark
% See solution above
% 0.49/0.93  % (14679)------------------------------
% 0.49/0.93  % (14679)Version: Vampire 5.0.1 (Release build, commit 1b9f22200 on 2026-04-29 16:18:29 +0200)
% 0.49/0.93  % (14679)Linked with Z3 4.14.0.0 3c47fd96cf5645d0c42b2c819d9e9a84380aa721 z3-4.8.4-9178-g3c47fd96c
% 0.49/0.93  % (14679)CaDiCaL version: 2.1.3
% 0.49/0.93  % (14679)Termination reason: Refutation
% 0.49/0.93  % (14679)Time elapsed: 0.001 s
% 0.49/0.93  % (14679)Peak memory usage: 81 MB
% 0.49/0.93  % (14679)Instructions burned: 1 (million)
% 0.49/0.93  % (14679)------------------------------
% 0.49/0.93  % (14679)------------------------------
% 0.49/0.93  % (14659)Success in time 0.297 s
%------------------------------------------------------------------------------

