% Proof : Problems/SYN057+1.p
%------------------------------------------------------------------------------
% File     : Vampire---5.0.1
% Problem  : SYN057+1 : TPTP v9.2.1. Released v2.0.0.
% Transfm  : none
% Format   : tptp:raw
% Command  : run_vampire %s %d THM

% Computer : n009.cluster.edu
% Model    : x86_64 x86_64
% CPU      : Intel(R) Xeon(R) CPU E5-2620 v4 2.10GHz
% Memory   : 8042.1875MB
% OS       : Linux 3.10.0-693.el7.x86_64
% CPULimit : 300s
% WCLimit  : 300s
% DateTime : Fri May  1 04:39:28 PM UTC 2026

% Result   : Theorem 0.50s 0.91s
% Output   : Refutation 0.50s
% Verified : 
% SZS Type : Refutation
%            Derivation depth      :   12
%            Number of leaves      :   11
% Syntax   : Number of formulae    :   53 (  12 unt;   4 def)
%            Number of atoms       :  115 (   0 equ)
%            Maximal formula atoms :    4 (   2 avg)
%            Number of connectives :  120 (  58   ~;  40   |;  10   &)
%                                         (   4 <=>;   8  =>;   0  <=;   0 <~>)
%            Maximal formula depth :    7 (   3 avg)
%            Maximal term depth    :    1 (   1 avg)
%            Number of predicates  :   10 (   9 usr;   5 prp; 0-1 aty)
%            Number of functors    :    2 (   2 usr;   2 con; 0-0 aty)
%            Number of variables   :   25 (   0 sgn  20   !;   5   ?)

% Comments : 
%------------------------------------------------------------------------------
fof(f1,axiom,
    ? [X0] :
      ( big_f(X0)
      & ~ big_g(X0) ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',pel27_1) ).

fof(f2,axiom,
    ! [X0] :
      ( big_f(X0)
     => big_h(X0) ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',pel27_2) ).

fof(f3,axiom,
    ! [X0] :
      ( ( big_j(X0)
        & big_i(X0) )
     => big_f(X0) ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',pel27_3) ).

fof(f4,axiom,
    ( ? [X0] :
        ( big_h(X0)
        & ~ big_g(X0) )
   => ! [X1] :
        ( big_i(X1)
       => ~ big_h(X1) ) ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',pel27_4) ).

fof(f5,conjecture,
    ! [X0] :
      ( big_j(X0)
     => ~ big_i(X0) ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',pel27) ).

fof(f6,negated_conjecture,
    ~ ! [X0] :
        ( big_j(X0)
       => ~ big_i(X0) ),
    inference(negated_conjecture,[status(cth)],[f5]) ).

fof(f7,plain,
    ! [X0] :
      ( big_h(X0)
      | ~ big_f(X0) ),
    inference(ennf_transformation,[],[f2]) ).

fof(f8,plain,
    ! [X0] :
      ( big_f(X0)
      | ~ big_j(X0)
      | ~ big_i(X0) ),
    inference(ennf_transformation,[],[f3]) ).

fof(f9,plain,
    ! [X0] :
      ( big_f(X0)
      | ~ big_j(X0)
      | ~ big_i(X0) ),
    inference(flattening,[],[f8]) ).

fof(f10,plain,
    ( ! [X1] :
        ( ~ big_h(X1)
        | ~ big_i(X1) )
    | ! [X0] :
        ( ~ big_h(X0)
        | big_g(X0) ) ),
    inference(ennf_transformation,[],[f4]) ).

fof(f11,plain,
    ? [X0] :
      ( big_i(X0)
      & big_j(X0) ),
    inference(ennf_transformation,[],[f6]) ).

fof(f12,plain,
    ( ? [X0] :
        ( big_f(X0)
        & ~ big_g(X0) )
   => ( big_f(sK0)
      & ~ big_g(sK0) ) ),
    introduced(definition,[],[skolem_symbol_introduction]) ).

fof(f13,plain,
    ( big_f(sK0)
    & ~ big_g(sK0) ),
    inference(skolemisation,[status(esa),new_symbols(skolem,[sK0])],[f1,f12]) ).

fof(f14,plain,
    ( ! [X0] :
        ( ~ big_h(X0)
        | ~ big_i(X0) )
    | ! [X1] :
        ( ~ big_h(X1)
        | big_g(X1) ) ),
    inference(rectify,[],[f10]) ).

fof(f15,plain,
    ( ? [X0] :
        ( big_i(X0)
        & big_j(X0) )
   => ( big_i(sK1)
      & big_j(sK1) ) ),
    introduced(definition,[],[skolem_symbol_introduction]) ).

fof(f16,plain,
    ( big_i(sK1)
    & big_j(sK1) ),
    inference(skolemisation,[status(esa),new_symbols(skolem,[sK1])],[f11,f15]) ).

fof(f17,plain,
    ~ big_g(sK0),
    inference(cnf_transformation,[],[f13]) ).

fof(f18,plain,
    big_f(sK0),
    inference(cnf_transformation,[],[f13]) ).

fof(f19,plain,
    ! [X0] :
      ( big_h(X0)
      | ~ big_f(X0) ),
    inference(cnf_transformation,[],[f7]) ).

fof(f20,plain,
    ! [X0] :
      ( ~ big_j(X0)
      | big_f(X0)
      | ~ big_i(X0) ),
    inference(cnf_transformation,[],[f9]) ).

fof(f21,plain,
    ! [X0,X1] :
      ( ~ big_h(X0)
      | ~ big_i(X0)
      | ~ big_h(X1)
      | big_g(X1) ),
    inference(cnf_transformation,[],[f14]) ).

fof(f22,plain,
    big_j(sK1),
    inference(cnf_transformation,[],[f16]) ).

fof(f23,plain,
    big_i(sK1),
    inference(cnf_transformation,[],[f16]) ).

fof(f25,definition,
    ( spl2_1
  <=> ! [X1] :
        ( ~ big_h(X1)
        | big_g(X1) ) ),
    introduced(definition,[new_symbols(naming,[spl2_1])],[avatar_definition]) ).

fof(f26,plain,
    ( ! [X1] :
        ( big_g(X1)
        | ~ big_h(X1) )
    | ~ spl2_1 ),
    inference(avatar_component_clause,[],[f25]) ).

fof(f28,definition,
    ( spl2_2
  <=> ! [X0] :
        ( ~ big_h(X0)
        | ~ big_i(X0) ) ),
    introduced(definition,[new_symbols(naming,[spl2_2])],[avatar_definition]) ).

fof(f29,plain,
    ( ! [X0] :
        ( ~ big_i(X0)
        | ~ big_h(X0) )
    | ~ spl2_2 ),
    inference(avatar_component_clause,[],[f28]) ).

fof(f30,plain,
    ( spl2_1
    | spl2_2 ),
    inference(avatar_split_clause,[],[f21,f28,f25]) ).

fof(f31,plain,
    ( ~ big_h(sK1)
    | ~ spl2_2 ),
    inference(resolution,[],[f23,f29]) ).

fof(f32,plain,
    ( ~ big_f(sK1)
    | ~ spl2_2 ),
    inference(resolution,[],[f19,f31]) ).

fof(f33,plain,
    ( big_f(sK1)
    | ~ big_i(sK1) ),
    inference(resolution,[],[f20,f22]) ).

fof(f35,definition,
    ( spl2_3
  <=> big_i(sK1) ),
    introduced(definition,[new_symbols(naming,[spl2_3])],[avatar_definition]) ).

fof(f36,plain,
    ( ~ big_i(sK1)
    | spl2_3 ),
    inference(avatar_component_clause,[],[f35]) ).

fof(f38,definition,
    ( spl2_4
  <=> big_f(sK1) ),
    introduced(definition,[new_symbols(naming,[spl2_4])],[avatar_definition]) ).

fof(f39,plain,
    ( big_f(sK1)
    | ~ spl2_4 ),
    inference(avatar_component_clause,[],[f38]) ).

fof(f40,plain,
    ( ~ spl2_3
    | spl2_4 ),
    inference(avatar_split_clause,[],[f33,f38,f35]) ).

fof(f41,plain,
    ( $false
    | spl2_3 ),
    inference(resolution,[],[f36,f23]) ).

fof(f42,plain,
    spl2_3,
    inference(avatar_contradiction_clause,[],[f41]) ).

fof(f43,plain,
    ( $false
    | ~ spl2_2
    | ~ spl2_4 ),
    inference(resolution,[],[f39,f32]) ).

fof(f44,plain,
    ( ~ spl2_2
    | ~ spl2_4 ),
    inference(avatar_contradiction_clause,[],[f43]) ).

fof(f45,plain,
    ( ~ big_h(sK0)
    | ~ spl2_1 ),
    inference(resolution,[],[f26,f17]) ).

fof(f46,plain,
    ( ~ big_f(sK0)
    | ~ spl2_1 ),
    inference(resolution,[],[f45,f19]) ).

fof(f47,plain,
    ( $false
    | ~ spl2_1 ),
    inference(resolution,[],[f46,f18]) ).

fof(f48,plain,
    ~ spl2_1,
    inference(avatar_contradiction_clause,[],[f47]) ).

fof(s1,plain,
    ( spl2_1
    | spl2_2 ),
    inference(sat_conversion,[],[f30]) ).

fof(s2,plain,
    ( ~ spl2_3
    | spl2_4 ),
    inference(sat_conversion,[],[f40]) ).

fof(s3,plain,
    spl2_3,
    inference(sat_conversion,[],[f42]) ).

fof(s4,plain,
    ( ~ spl2_2
    | ~ spl2_4 ),
    inference(sat_conversion,[],[f44]) ).

fof(s5,plain,
    ~ spl2_1,
    inference(sat_conversion,[],[f48]) ).

fof(s6,plain,
    spl2_4,
    inference(rat,[],[s2,s3]) ).

fof(s7,plain,
    ~ spl2_2,
    inference(rat,[],[s4,s6]) ).

fof(s8,plain,
    $false,
    inference(rat,[],[s1,s7,s5]) ).

fof(f49,plain,
    $false,
    inference(avatar_sat_refutation,[],[s8]) ).

%------------------------------------------------------------------------------
%----ORIGINAL SYSTEM OUTPUT
% 0.00/0.12  % Problem    : SYN057+1 : TPTP v9.2.1. Released v2.0.0.
% 0.00/0.12  % Command    : run_vampire %s %d THM
% 0.15/0.33  % Computer   : n009.cluster.edu
% 0.15/0.33  % Model      : x86_64 x86_64
% 0.15/0.33  % CPU        : Intel(R) Xeon(R) CPU E5-2620 v4 @ 2.10GHz
% 0.15/0.33  % Memory     : 8042.1875MB
% 0.15/0.33  % OS         : Linux 3.10.0-693.el7.x86_64
% 0.15/0.33  % CPULimit   : 300
% 0.15/0.33  % WCLimit    : 300
% 0.15/0.33  % DateTime   : Fri May  1 05:44:11 EDT 2026
% 0.15/0.33  % CPUTime    : 
% 0.15/0.35  This is a FOF_THM_RFO_NEQ problem
% 0.15/0.35  Running first-order theorem proving
% 0.15/0.35  Running /export/starexec/sandbox2/solver/bin/vampire --input_syntax tptp --proof tptp --output_axiom_names on --mode casc --cores 7 -m 16384 -t 300 /export/starexec/sandbox2/benchmark/theBenchmark.p
% 0.47/0.64  % (936)Detected formulas, will run a generic FOF schedule.
% 0.50/0.76  % (945)dis-21_1_sil=8000:lcm=predicate:random_seed=730133810:st=5:avsq=on:i=129:avsqr=1,16:sd=3:aac=none:ep=RS:fsr=off:ss=included_3000 on theBenchmark for (3000ds/129Mi)
% 0.50/0.76  % (945)First to succeed.
% 0.50/0.76  % (945)Solution written to "/export/starexec/sandbox2/tmp/vampire-proof-936"
% 0.50/0.79  % (942)lrs+1010_1_to=lpo:sil=32000:sos=on:spb=goal_then_units:bce=on:random_seed=2724432409:i=109:sd=1:ins=1:gsp=on:ss=axioms_3000 on theBenchmark for (3000ds/109Mi)
% 0.50/0.79  % (938)lrs+10_1_ncem=casc2026/models/loop8.pt:sil=128000:tgt=full:npcc=on:drc=off:sp=weighted_frequency:spb=goal:fd=preordered:foolp=on:random_seed=2842299072:i=141193_3000 on theBenchmark for (3000ds/141193Mi)
% 0.50/0.79  % (940)lrs+11_1_ncem=casc2026/models/loop8.pt:sil=128000:npcc=on:lma=off:spb=units:urr=ec_only:bce=on:s2agt=64:updr=off:random_seed=2772480188:i=134677:sd=20:aac=none:nm=16:ss=included:sgt=10_3000 on theBenchmark for (3000ds/134677Mi)
% 0.50/0.79  % (941)lrs+1010_1_anc=all:sfv=off:to=kbo:ncem=casc2026/models/loop7.pt:sil=128000:npcc=on:prc=on:sos=all:bsr=unit_only:sac=on:random_seed=3845479119:i=141695:sd=1:nm=32:gsp=on:ss=included_3000 on theBenchmark for (3000ds/141695Mi)
% 0.50/0.79  % (943)dis-1010_2:3_sil=16000:sp=reverse_frequency:random_seed=4019982605:i=119:av=off:ss=axioms_3000 on theBenchmark for (3000ds/119Mi)
% 0.50/0.79  % (942)Refutation not found, incomplete strategy
% 0.50/0.79  % (942)------------------------------
% 0.50/0.79  % (942)Version: Vampire 5.0.1 (Release build, commit 1b9f22200 on 2026-04-29 16:18:29 +0200)
% 0.50/0.79  % (944)dis-1011_1_sil=16000:fde=unused:s2agt=70:random_seed=2107342473:s2a=on:i=139:gtg=position_3000 on theBenchmark for (3000ds/139Mi)
% 0.50/0.79  % (942)Linked with Z3 4.14.0.0 3c47fd96cf5645d0c42b2c819d9e9a84380aa721 z3-4.8.4-9178-g3c47fd96c
% 0.50/0.79  % (942)CaDiCaL version: 2.1.3
% 0.50/0.79  % (942)Termination reason: Refutation not found, incomplete strategy
% 0.50/0.79  % (942)Time elapsed: 0.0000 s
% 0.50/0.79  % (942)Peak memory usage: 80 MB
% 0.50/0.79  % (943)Refutation not found, incomplete strategy
% 0.50/0.79  % (943)------------------------------
% 0.50/0.79  % (943)Version: Vampire 5.0.1 (Release build, commit 1b9f22200 on 2026-04-29 16:18:29 +0200)
% 0.50/0.79  % (943)Linked with Z3 4.14.0.0 3c47fd96cf5645d0c42b2c819d9e9a84380aa721 z3-4.8.4-9178-g3c47fd96c
% 0.50/0.79  % (943)CaDiCaL version: 2.1.3
% 0.50/0.79  % (943)Termination reason: Refutation not found, incomplete strategy
% 0.50/0.79  % (943)Time elapsed: 0.0000 s
% 0.50/0.79  % (943)Peak memory usage: 80 MB
% 0.50/0.79  % (944)Also succeeded, but the first one will report.
% 0.50/0.91  % (945)Refutation found. Thanks to Tanya!
% 0.50/0.91  % SZS status Theorem for theBenchmark
% 0.50/0.91  % SZS output start Proof for theBenchmark
% See solution above
% 0.50/0.91  % (945)------------------------------
% 0.50/0.91  % (945)Version: Vampire 5.0.1 (Release build, commit 1b9f22200 on 2026-04-29 16:18:29 +0200)
% 0.50/0.91  % (945)Linked with Z3 4.14.0.0 3c47fd96cf5645d0c42b2c819d9e9a84380aa721 z3-4.8.4-9178-g3c47fd96c
% 0.50/0.91  % (945)CaDiCaL version: 2.1.3
% 0.50/0.91  % (945)Termination reason: Refutation
% 0.50/0.91  % (945)Time elapsed: 0.001 s
% 0.50/0.91  % (945)Peak memory usage: 81 MB
% 0.50/0.91  % (945)Instructions burned: 1 (million)
% 0.50/0.91  % (945)------------------------------
% 0.50/0.91  % (945)------------------------------
% 0.50/0.91  % (936)Success in time 0.264 s
%------------------------------------------------------------------------------

