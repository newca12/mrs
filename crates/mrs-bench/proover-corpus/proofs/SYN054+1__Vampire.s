% Proof : Problems/SYN054+1.p
%------------------------------------------------------------------------------
% File     : Vampire---5.0.1
% Problem  : SYN054+1 : TPTP v9.2.1. Released v2.0.0.
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

% Result   : Theorem 0.49s 0.92s
% Output   : Refutation 0.49s
% Verified : 
% SZS Type : Refutation
%            Derivation depth      :   11
%            Number of leaves      :    9
% Syntax   : Number of formulae    :   40 (   7 unt;   2 def)
%            Number of atoms       :   80 (   0 equ)
%            Maximal formula atoms :    3 (   2 avg)
%            Number of connectives :   72 (  32   ~;  29   |;   4   &)
%                                         (   2 <=>;   5  =>;   0  <=;   0 <~>)
%            Maximal formula depth :    5 (   3 avg)
%            Maximal term depth    :    1 (   1 avg)
%            Number of predicates  :    7 (   6 usr;   3 prp; 0-1 aty)
%            Number of functors    :    2 (   2 usr;   2 con; 0-0 aty)
%            Number of variables   :   26 (   0 sgn  15   !;  11   ?)

% Comments : 
%------------------------------------------------------------------------------
fof(f1,axiom,
    ~ ? [X0] :
        ( big_s(X0)
        & big_q(X0) ),
    file('/export/starexec/sandbox/benchmark/theBenchmark.p',pel24_1) ).

fof(f2,axiom,
    ! [X0] :
      ( big_p(X0)
     => ( big_q(X0)
        | big_r(X0) ) ),
    file('/export/starexec/sandbox/benchmark/theBenchmark.p',pel24_2) ).

fof(f3,axiom,
    ( ~ ? [X0] : big_p(X0)
   => ? [X1] : big_q(X1) ),
    file('/export/starexec/sandbox/benchmark/theBenchmark.p',pel24_3) ).

fof(f4,axiom,
    ! [X0] :
      ( ( big_q(X0)
        | big_r(X0) )
     => big_s(X0) ),
    file('/export/starexec/sandbox/benchmark/theBenchmark.p',pel24_4) ).

fof(f5,conjecture,
    ? [X0] :
      ( big_p(X0)
      & big_r(X0) ),
    file('/export/starexec/sandbox/benchmark/theBenchmark.p',pel24) ).

fof(f6,negated_conjecture,
    ~ ? [X0] :
        ( big_p(X0)
        & big_r(X0) ),
    inference(negated_conjecture,[status(cth)],[f5]) ).

fof(f7,plain,
    ! [X0] :
      ( ~ big_s(X0)
      | ~ big_q(X0) ),
    inference(ennf_transformation,[],[f1]) ).

fof(f8,plain,
    ! [X0] :
      ( big_q(X0)
      | big_r(X0)
      | ~ big_p(X0) ),
    inference(ennf_transformation,[],[f2]) ).

fof(f9,plain,
    ! [X0] :
      ( big_q(X0)
      | big_r(X0)
      | ~ big_p(X0) ),
    inference(flattening,[],[f8]) ).

fof(f10,plain,
    ( ? [X1] : big_q(X1)
    | ? [X0] : big_p(X0) ),
    inference(ennf_transformation,[],[f3]) ).

fof(f11,plain,
    ! [X0] :
      ( big_s(X0)
      | ( ~ big_q(X0)
        & ~ big_r(X0) ) ),
    inference(ennf_transformation,[],[f4]) ).

fof(f12,plain,
    ! [X0] :
      ( ~ big_p(X0)
      | ~ big_r(X0) ),
    inference(ennf_transformation,[],[f6]) ).

fof(f13,plain,
    ( ? [X0] : big_q(X0)
    | ? [X1] : big_p(X1) ),
    inference(rectify,[],[f10]) ).

fof(f14,plain,
    ( ? [X0] : big_q(X0)
   => big_q(sK0) ),
    introduced(definition,[],[skolem_symbol_introduction]) ).

fof(f15,plain,
    ( ? [X1] : big_p(X1)
   => big_p(sK1) ),
    introduced(definition,[],[skolem_symbol_introduction]) ).

fof(f16,plain,
    ( big_q(sK0)
    | big_p(sK1) ),
    inference(skolemisation,[status(esa),new_symbols(skolem,[sK0,sK1])],[f13,f15,f14]) ).

fof(f17,plain,
    ! [X0] :
      ( ~ big_s(X0)
      | ~ big_q(X0) ),
    inference(cnf_transformation,[],[f7]) ).

fof(f18,plain,
    ! [X0] :
      ( big_r(X0)
      | big_q(X0)
      | ~ big_p(X0) ),
    inference(cnf_transformation,[],[f9]) ).

fof(f19,plain,
    ( big_q(sK0)
    | big_p(sK1) ),
    inference(cnf_transformation,[],[f16]) ).

fof(f21,plain,
    ! [X0] :
      ( big_s(X0)
      | ~ big_q(X0) ),
    inference(cnf_transformation,[],[f11]) ).

fof(f22,plain,
    ! [X0] :
      ( ~ big_r(X0)
      | ~ big_p(X0) ),
    inference(cnf_transformation,[],[f12]) ).

fof(f24,definition,
    ( spl2_1
  <=> big_p(sK1) ),
    introduced(definition,[new_symbols(naming,[spl2_1])],[avatar_definition]) ).

fof(f25,plain,
    ( big_p(sK1)
    | ~ spl2_1 ),
    inference(avatar_component_clause,[],[f24]) ).

fof(f27,definition,
    ( spl2_2
  <=> big_q(sK0) ),
    introduced(definition,[new_symbols(naming,[spl2_2])],[avatar_definition]) ).

fof(f28,plain,
    ( big_q(sK0)
    | ~ spl2_2 ),
    inference(avatar_component_clause,[],[f27]) ).

fof(f29,plain,
    ( spl2_1
    | spl2_2 ),
    inference(avatar_split_clause,[],[f19,f27,f24]) ).

fof(f30,plain,
    ! [X0] :
      ( ~ big_q(X0)
      | ~ big_q(X0) ),
    inference(resolution,[],[f21,f17]) ).

fof(f31,plain,
    ! [X0] : ~ big_q(X0),
    inference(duplicate_literal_removal,[],[f30]) ).

fof(f32,plain,
    ( $false
    | ~ spl2_2 ),
    inference(resolution,[],[f31,f28]) ).

fof(f33,plain,
    ~ spl2_2,
    inference(avatar_contradiction_clause,[],[f32]) ).

fof(f35,plain,
    ! [X0] :
      ( big_q(X0)
      | ~ big_p(X0)
      | ~ big_p(X0) ),
    inference(resolution,[],[f18,f22]) ).

fof(f36,plain,
    ! [X0] :
      ( ~ big_p(X0)
      | big_q(X0) ),
    inference(duplicate_literal_removal,[],[f35]) ).

fof(f37,plain,
    ( big_q(sK1)
    | ~ spl2_1 ),
    inference(resolution,[],[f36,f25]) ).

fof(f38,plain,
    ( $false
    | ~ spl2_1 ),
    inference(resolution,[],[f37,f31]) ).

fof(f39,plain,
    ~ spl2_1,
    inference(avatar_contradiction_clause,[],[f38]) ).

fof(s1,plain,
    ( spl2_1
    | spl2_2 ),
    inference(sat_conversion,[],[f29]) ).

fof(s2,plain,
    ~ spl2_2,
    inference(sat_conversion,[],[f33]) ).

fof(s3,plain,
    ~ spl2_1,
    inference(sat_conversion,[],[f39]) ).

fof(s4,plain,
    $false,
    inference(rat,[],[s1,s2,s3]) ).

fof(f40,plain,
    $false,
    inference(avatar_sat_refutation,[],[s4]) ).

%------------------------------------------------------------------------------
%----ORIGINAL SYSTEM OUTPUT
% 0.00/0.12  % Problem    : SYN054+1 : TPTP v9.2.1. Released v2.0.0.
% 0.00/0.12  % Command    : run_vampire %s %d THM
% 0.15/0.33  % Computer   : n007.cluster.edu
% 0.15/0.33  % Model      : x86_64 x86_64
% 0.15/0.33  % CPU        : Intel(R) Xeon(R) CPU E5-2620 v4 @ 2.10GHz
% 0.15/0.33  % Memory     : 8042.1875MB
% 0.15/0.33  % OS         : Linux 3.10.0-693.el7.x86_64
% 0.15/0.33  % CPULimit   : 300
% 0.15/0.33  % WCLimit    : 300
% 0.15/0.33  % DateTime   : Fri May  1 05:42:24 EDT 2026
% 0.15/0.33  % CPUTime    : 
% 0.15/0.35  This is a FOF_THM_RFO_NEQ problem
% 0.15/0.35  Running first-order theorem proving
% 0.15/0.35  Running /export/starexec/sandbox/solver/bin/vampire --input_syntax tptp --proof tptp --output_axiom_names on --mode casc --cores 7 -m 16384 -t 300 /export/starexec/sandbox/benchmark/theBenchmark.p
% 0.47/0.64  % (12733)Detected formulas, will run a generic FOF schedule.
% 0.49/0.77  % (12741)dis-21_1_sil=8000:lcm=predicate:random_seed=3180037688:st=5:avsq=on:i=129:avsqr=1,16:sd=3:aac=none:ep=RS:fsr=off:ss=included_3000 on theBenchmark for (3000ds/129Mi)
% 0.49/0.77  % (12741)First to succeed.
% 0.49/0.77  % (12741)Solution written to "/export/starexec/sandbox/tmp/vampire-proof-12733"
% 0.49/0.79  % (12738)lrs+1010_1_to=lpo:sil=32000:sos=on:spb=goal_then_units:bce=on:random_seed=1273241826:i=109:sd=1:ins=1:gsp=on:ss=axioms_3000 on theBenchmark for (3000ds/109Mi)
% 0.49/0.79  % (12739)dis-1010_2:3_sil=16000:sp=reverse_frequency:random_seed=1797727122:i=119:av=off:ss=axioms_3000 on theBenchmark for (3000ds/119Mi)
% 0.49/0.79  % (12736)lrs+11_1_ncem=casc2026/models/loop8.pt:sil=128000:npcc=on:lma=off:spb=units:urr=ec_only:bce=on:s2agt=64:updr=off:random_seed=1331297413:i=134677:sd=20:aac=none:nm=16:ss=included:sgt=10_3000 on theBenchmark for (3000ds/134677Mi)
% 0.49/0.79  % (12737)lrs+1010_1_anc=all:sfv=off:to=kbo:ncem=casc2026/models/loop7.pt:sil=128000:npcc=on:prc=on:sos=all:bsr=unit_only:sac=on:random_seed=2916717389:i=141695:sd=1:nm=32:gsp=on:ss=included_3000 on theBenchmark for (3000ds/141695Mi)
% 0.49/0.79  % (12735)lrs+10_1_ncem=casc2026/models/loop8.pt:sil=128000:tgt=full:npcc=on:drc=off:sp=weighted_frequency:spb=goal:fd=preordered:foolp=on:random_seed=1110554469:i=141193_3000 on theBenchmark for (3000ds/141193Mi)
% 0.49/0.79  % (12740)dis-1011_1_sil=16000:fde=unused:s2agt=70:random_seed=2589724608:s2a=on:i=139:gtg=position_3000 on theBenchmark for (3000ds/139Mi)
% 0.49/0.79  % (12739)Refutation not found, incomplete strategy
% 0.49/0.79  % (12739)------------------------------
% 0.49/0.79  % (12739)Version: Vampire 5.0.1 (Release build, commit 1b9f22200 on 2026-04-29 16:18:29 +0200)
% 0.49/0.79  % (12739)Linked with Z3 4.14.0.0 3c47fd96cf5645d0c42b2c819d9e9a84380aa721 z3-4.8.4-9178-g3c47fd96c
% 0.49/0.79  % (12739)CaDiCaL version: 2.1.3
% 0.49/0.79  % (12739)Termination reason: Refutation not found, incomplete strategy
% 0.49/0.79  % (12739)Time elapsed: 0.0000 s
% 0.49/0.79  % (12739)Peak memory usage: 80 MB
% 0.49/0.79  % (12738)Refutation not found, incomplete strategy
% 0.49/0.79  % (12738)------------------------------
% 0.49/0.79  % (12738)Version: Vampire 5.0.1 (Release build, commit 1b9f22200 on 2026-04-29 16:18:29 +0200)
% 0.49/0.79  % (12738)Linked with Z3 4.14.0.0 3c47fd96cf5645d0c42b2c819d9e9a84380aa721 z3-4.8.4-9178-g3c47fd96c
% 0.49/0.79  % (12738)CaDiCaL version: 2.1.3
% 0.49/0.79  % (12738)Termination reason: Refutation not found, incomplete strategy
% 0.49/0.79  % (12738)Time elapsed: 0.0000 s
% 0.49/0.79  % (12738)Peak memory usage: 80 MB
% 0.49/0.79  % (12740)Also succeeded, but the first one will report.
% 0.49/0.92  % (12741)Refutation found. Thanks to Tanya!
% 0.49/0.92  % SZS status Theorem for theBenchmark
% 0.49/0.92  % SZS output start Proof for theBenchmark
% See solution above
% 0.49/0.92  % (12741)------------------------------
% 0.49/0.92  % (12741)Version: Vampire 5.0.1 (Release build, commit 1b9f22200 on 2026-04-29 16:18:29 +0200)
% 0.49/0.92  % (12741)Linked with Z3 4.14.0.0 3c47fd96cf5645d0c42b2c819d9e9a84380aa721 z3-4.8.4-9178-g3c47fd96c
% 0.49/0.92  % (12741)CaDiCaL version: 2.1.3
% 0.49/0.92  % (12741)Termination reason: Refutation
% 0.49/0.92  % (12741)Time elapsed: 0.001 s
% 0.49/0.92  % (12741)Peak memory usage: 81 MB
% 0.49/0.92  % (12741)Instructions burned: 1 (million)
% 0.49/0.92  % (12741)------------------------------
% 0.49/0.92  % (12741)------------------------------
% 0.49/0.92  % (12733)Success in time 0.283 s
%------------------------------------------------------------------------------

