% Proof : Problems/SYN051+1.p
%------------------------------------------------------------------------------
% File     : Vampire---5.0.1
% Problem  : SYN051+1 : TPTP v9.2.1. Released v2.0.0.
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

% Result   : Theorem 0.51s 0.94s
% Output   : Refutation 0.51s
% Verified : 
% SZS Type : Refutation
%            Derivation depth      :   10
%            Number of leaves      :   10
% Syntax   : Number of formulae    :   44 (   5 unt;   5 def)
%            Number of atoms       :   91 (   0 equ)
%            Maximal formula atoms :    4 (   2 avg)
%            Number of connectives :   83 (  36   ~;  34   |;   1   &)
%                                         (   7 <=>;   4  =>;   0  <=;   1 <~>)
%            Maximal formula depth :    5 (   3 avg)
%            Maximal term depth    :    1 (   1 avg)
%            Number of predicates  :    8 (   7 usr;   7 prp; 0-1 aty)
%            Number of functors    :    2 (   2 usr;   2 con; 0-0 aty)
%            Number of variables   :   16 (   0 sgn   8   !;   8   ?)

% Comments : 
%------------------------------------------------------------------------------
fof(f1,axiom,
    ? [X0] :
      ( p
     => big_f(X0) ),
    file('/export/starexec/sandbox/benchmark/theBenchmark.p',pel21_1) ).

fof(f2,axiom,
    ? [X0] :
      ( big_f(X0)
     => p ),
    file('/export/starexec/sandbox/benchmark/theBenchmark.p',pel21_2) ).

fof(f3,conjecture,
    ? [X0] :
      ( p
    <=> big_f(X0) ),
    file('/export/starexec/sandbox/benchmark/theBenchmark.p',pel21) ).

fof(f4,negated_conjecture,
    ~ ? [X0] :
        ( p
      <=> big_f(X0) ),
    inference(negated_conjecture,[status(cth)],[f3]) ).

fof(f5,plain,
    ? [X0] :
      ( big_f(X0)
      | ~ p ),
    inference(ennf_transformation,[],[f1]) ).

fof(f6,plain,
    ? [X0] :
      ( p
      | ~ big_f(X0) ),
    inference(ennf_transformation,[],[f2]) ).

fof(f7,plain,
    ! [X0] :
      ( p
    <~> big_f(X0) ),
    inference(ennf_transformation,[],[f4]) ).

fof(f8,plain,
    ( ? [X0] :
        ( big_f(X0)
        | ~ p )
   => ( big_f(sK0)
      | ~ p ) ),
    introduced(definition,[],[skolem_symbol_introduction]) ).

fof(f9,plain,
    ( big_f(sK0)
    | ~ p ),
    inference(skolemisation,[status(esa),new_symbols(skolem,[sK0])],[f5,f8]) ).

fof(f10,plain,
    ( ? [X0] :
        ( p
        | ~ big_f(X0) )
   => ( p
      | ~ big_f(sK1) ) ),
    introduced(definition,[],[skolem_symbol_introduction]) ).

fof(f11,plain,
    ( p
    | ~ big_f(sK1) ),
    inference(skolemisation,[status(esa),new_symbols(skolem,[sK1])],[f6,f10]) ).

fof(f12,plain,
    ! [X0] :
      ( ( ~ big_f(X0)
        | ~ p )
      & ( big_f(X0)
        | p ) ),
    inference(nnf_transformation,[],[f7]) ).

fof(f13,plain,
    ( big_f(sK0)
    | ~ p ),
    inference(cnf_transformation,[],[f9]) ).

fof(f14,plain,
    ( p
    | ~ big_f(sK1) ),
    inference(cnf_transformation,[],[f11]) ).

fof(f15,plain,
    ! [X0] :
      ( big_f(X0)
      | p ),
    inference(cnf_transformation,[],[f12]) ).

fof(f16,plain,
    ! [X0] :
      ( ~ big_f(X0)
      | ~ p ),
    inference(cnf_transformation,[],[f12]) ).

fof(f18,definition,
    ( spl2_1
  <=> p ),
    introduced(definition,[new_symbols(naming,[spl2_1])],[avatar_definition]) ).

fof(f21,definition,
    ( spl2_2
  <=> ! [X0] : big_f(X0) ),
    introduced(definition,[new_symbols(naming,[spl2_2])],[avatar_definition]) ).

fof(f22,plain,
    ( ! [X0] : big_f(X0)
    | ~ spl2_2 ),
    inference(avatar_component_clause,[],[f21]) ).

fof(f23,plain,
    ( spl2_1
    | spl2_2 ),
    inference(avatar_split_clause,[],[f15,f21,f18]) ).

fof(f26,definition,
    ( spl2_3
  <=> ! [X0] : ~ big_f(X0) ),
    introduced(definition,[new_symbols(naming,[spl2_3])],[avatar_definition]) ).

fof(f27,plain,
    ( ! [X0] : ~ big_f(X0)
    | ~ spl2_3 ),
    inference(avatar_component_clause,[],[f26]) ).

fof(f28,plain,
    ( ~ spl2_1
    | spl2_3 ),
    inference(avatar_split_clause,[],[f16,f26,f18]) ).

fof(f30,definition,
    ( spl2_4
  <=> big_f(sK1) ),
    introduced(definition,[new_symbols(naming,[spl2_4])],[avatar_definition]) ).

fof(f31,plain,
    ( ~ big_f(sK1)
    | spl2_4 ),
    inference(avatar_component_clause,[],[f30]) ).

fof(f32,plain,
    ( ~ spl2_4
    | spl2_1 ),
    inference(avatar_split_clause,[],[f14,f18,f30]) ).

fof(f34,definition,
    ( spl2_5
  <=> big_f(sK0) ),
    introduced(definition,[new_symbols(naming,[spl2_5])],[avatar_definition]) ).

fof(f35,plain,
    ( big_f(sK0)
    | ~ spl2_5 ),
    inference(avatar_component_clause,[],[f34]) ).

fof(f36,plain,
    ( ~ spl2_1
    | spl2_5 ),
    inference(avatar_split_clause,[],[f13,f34,f18]) ).

fof(f37,plain,
    ( $false
    | ~ spl2_2
    | spl2_4 ),
    inference(resolution,[],[f31,f22]) ).

fof(f38,plain,
    ( ~ spl2_2
    | spl2_4 ),
    inference(avatar_contradiction_clause,[],[f37]) ).

fof(f41,plain,
    ( $false
    | ~ spl2_3
    | ~ spl2_5 ),
    inference(resolution,[],[f35,f27]) ).

fof(f42,plain,
    ( ~ spl2_3
    | ~ spl2_5 ),
    inference(avatar_contradiction_clause,[],[f41]) ).

fof(s1,plain,
    ( spl2_1
    | spl2_2 ),
    inference(sat_conversion,[],[f23]) ).

fof(s2,plain,
    ( ~ spl2_1
    | spl2_3 ),
    inference(sat_conversion,[],[f28]) ).

fof(s3,plain,
    ( spl2_1
    | ~ spl2_4 ),
    inference(sat_conversion,[],[f32]) ).

fof(s4,plain,
    ( ~ spl2_1
    | spl2_5 ),
    inference(sat_conversion,[],[f36]) ).

fof(s5,plain,
    ( ~ spl2_2
    | spl2_4 ),
    inference(sat_conversion,[],[f38]) ).

fof(s7,plain,
    ( ~ spl2_3
    | ~ spl2_5 ),
    inference(sat_conversion,[],[f42]) ).

fof(s8,plain,
    spl2_1,
    inference(rat,[],[s5,s1,s3]) ).

fof(s9,plain,
    spl2_5,
    inference(rat,[],[s4,s8]) ).

fof(s10,plain,
    spl2_3,
    inference(rat,[],[s2,s8]) ).

fof(s11,plain,
    $false,
    inference(rat,[],[s7,s9,s10]) ).

fof(f43,plain,
    $false,
    inference(avatar_sat_refutation,[],[s11]) ).

%------------------------------------------------------------------------------
%----ORIGINAL SYSTEM OUTPUT
% 0.00/0.13  % Problem    : SYN051+1 : TPTP v9.2.1. Released v2.0.0.
% 0.00/0.13  % Command    : run_vampire %s %d THM
% 0.17/0.34  % Computer   : n007.cluster.edu
% 0.17/0.34  % Model      : x86_64 x86_64
% 0.17/0.34  % CPU        : Intel(R) Xeon(R) CPU E5-2620 v4 @ 2.10GHz
% 0.17/0.34  % Memory     : 8042.1875MB
% 0.17/0.34  % OS         : Linux 3.10.0-693.el7.x86_64
% 0.17/0.34  % CPULimit   : 300
% 0.17/0.34  % WCLimit    : 300
% 0.17/0.34  % DateTime   : Fri May  1 05:42:39 EDT 2026
% 0.17/0.34  % CPUTime    : 
% 0.20/0.36  This is a FOF_THM_RFO_NEQ problem
% 0.20/0.36  Running first-order theorem proving
% 0.20/0.36  Running /export/starexec/sandbox/solver/bin/vampire --input_syntax tptp --proof tptp --output_axiom_names on --mode casc --cores 7 -m 16384 -t 300 /export/starexec/sandbox/benchmark/theBenchmark.p
% 0.47/0.66  % (13360)Detected formulas, will run a generic FOF schedule.
% 0.51/0.79  % (13368)dis-21_1_sil=8000:lcm=predicate:random_seed=3241571738:st=5:avsq=on:i=129:avsqr=1,16:sd=3:aac=none:ep=RS:fsr=off:ss=included_3000 on theBenchmark for (3000ds/129Mi)
% 0.51/0.79  % (13368)First to succeed.
% 0.51/0.79  % (13368)Solution written to "/export/starexec/sandbox/tmp/vampire-proof-13360"
% 0.51/0.81  % (13366)dis-1010_2:3_sil=16000:sp=reverse_frequency:random_seed=2285080032:i=119:av=off:ss=axioms_3000 on theBenchmark for (3000ds/119Mi)
% 0.51/0.81  % (13363)lrs+11_1_ncem=casc2026/models/loop8.pt:sil=128000:npcc=on:lma=off:spb=units:urr=ec_only:bce=on:s2agt=64:updr=off:random_seed=1784471091:i=134677:sd=20:aac=none:nm=16:ss=included:sgt=10_3000 on theBenchmark for (3000ds/134677Mi)
% 0.51/0.81  % (13367)dis-1011_1_sil=16000:fde=unused:s2agt=70:random_seed=981404666:s2a=on:i=139:gtg=position_3000 on theBenchmark for (3000ds/139Mi)
% 0.51/0.81  % (13362)lrs+10_1_ncem=casc2026/models/loop8.pt:sil=128000:tgt=full:npcc=on:drc=off:sp=weighted_frequency:spb=goal:fd=preordered:foolp=on:random_seed=3448267165:i=141193_3000 on theBenchmark for (3000ds/141193Mi)
% 0.51/0.81  % (13365)lrs+1010_1_to=lpo:sil=32000:sos=on:spb=goal_then_units:bce=on:random_seed=539268869:i=109:sd=1:ins=1:gsp=on:ss=axioms_3000 on theBenchmark for (3000ds/109Mi)
% 0.51/0.81  % (13366)Also succeeded, but the first one will report.
% 0.51/0.81  % (13365)Also succeeded, but the first one will report.
% 0.51/0.81  % (13367)Also succeeded, but the first one will report.
% 0.51/0.83  % (13364)lrs+1010_1_anc=all:sfv=off:to=kbo:ncem=casc2026/models/loop7.pt:sil=128000:npcc=on:prc=on:sos=all:bsr=unit_only:sac=on:random_seed=3110522757:i=141695:sd=1:nm=32:gsp=on:ss=included_3000 on theBenchmark for (3000ds/141695Mi)
% 0.51/0.94  % (13368)Refutation found. Thanks to Tanya!
% 0.51/0.94  % SZS status Theorem for theBenchmark
% 0.51/0.94  % SZS output start Proof for theBenchmark
% See solution above
% 0.51/0.94  % (13368)------------------------------
% 0.51/0.94  % (13368)Version: Vampire 5.0.1 (Release build, commit 1b9f22200 on 2026-04-29 16:18:29 +0200)
% 0.51/0.94  % (13368)Linked with Z3 4.14.0.0 3c47fd96cf5645d0c42b2c819d9e9a84380aa721 z3-4.8.4-9178-g3c47fd96c
% 0.51/0.94  % (13368)CaDiCaL version: 2.1.3
% 0.51/0.94  % (13368)Termination reason: Refutation
% 0.51/0.94  % (13368)Time elapsed: 0.002 s
% 0.51/0.94  % (13368)Peak memory usage: 81 MB
% 0.51/0.94  % (13368)Instructions burned: 1 (million)
% 0.51/0.94  % (13368)------------------------------
% 0.51/0.94  % (13368)------------------------------
% 0.51/0.94  % (13360)Success in time 0.286 s
%------------------------------------------------------------------------------

