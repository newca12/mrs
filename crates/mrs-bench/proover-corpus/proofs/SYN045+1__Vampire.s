% Proof : Problems/SYN045+1.p
%------------------------------------------------------------------------------
% File     : Vampire---5.0.1
% Problem  : SYN045+1 : TPTP v9.2.1. Released v2.0.0.
% Transfm  : none
% Format   : tptp:raw
% Command  : run_vampire %s %d THM

% Computer : n002.cluster.edu
% Model    : x86_64 x86_64
% CPU      : Intel(R) Xeon(R) CPU E5-2620 v4 2.10GHz
% Memory   : 8042.1875MB
% OS       : Linux 3.10.0-693.el7.x86_64
% CPULimit : 300s
% WCLimit  : 300s
% DateTime : Fri May  1 04:39:24 PM UTC 2026

% Result   : Theorem 0.71s 0.90s
% Output   : Refutation 0.71s
% Verified : 
% SZS Type : Refutation
%            Derivation depth      :   14
%            Number of leaves      :    6
% Syntax   : Number of formulae    :   45 (   6 unt;   4 def)
%            Number of atoms       :  151 (   0 equ)
%            Maximal formula atoms :   10 (   3 avg)
%            Number of connectives :  158 (  52   ~;  75   |;  22   &)
%                                         (   7 <=>;   0  =>;   0  <=;   2 <~>)
%            Maximal formula depth :    6 (   3 avg)
%            Maximal term depth    :    0 (   0 avg)
%            Number of predicates  :    9 (   8 usr;   9 prp; 0-0 aty)
%            Number of functors    :    0 (   0 usr;   0 con; --- aty)
%            Number of variables   :    0 (   0 sgn   0   !;   0   ?)

% Comments : 
%------------------------------------------------------------------------------
fof(f1,conjecture,
    ( ( p
      | ( q
        & r ) )
  <=> ( ( p
        | q )
      & ( p
        | r ) ) ),
    file('/export/starexec/sandbox/benchmark/theBenchmark.p',pel13) ).

fof(f2,negated_conjecture,
    ~ ( ( p
        | ( q
          & r ) )
    <=> ( ( p
          | q )
        & ( p
          | r ) ) ),
    inference(negated_conjecture,[status(cth)],[f1]) ).

fof(f3,plain,
    ( ( p
      | ( q
        & r ) )
  <~> ( ( p
        | q )
      & ( p
        | r ) ) ),
    inference(ennf_transformation,[],[f2]) ).

fof(f4,plain,
    ( sP0
  <=> ( ( p
        | q )
      & ( p
        | r ) ) ),
    introduced(definition,[new_symbols(naming,[sP0])],[predicate_definition_introduction]) ).

fof(f5,plain,
    ( ( p
      | ( q
        & r ) )
  <~> sP0 ),
    inference(definition_folding,[],[f3,f4]) ).

fof(f6,plain,
    ( ( sP0
      | ( ~ p
        & ~ q )
      | ( ~ p
        & ~ r ) )
    & ( ( ( p
          | q )
        & ( p
          | r ) )
      | ~ sP0 ) ),
    inference(nnf_transformation,[],[f4]) ).

fof(f7,plain,
    ( ( sP0
      | ( ~ p
        & ~ q )
      | ( ~ p
        & ~ r ) )
    & ( ( ( p
          | q )
        & ( p
          | r ) )
      | ~ sP0 ) ),
    inference(flattening,[],[f6]) ).

fof(f8,plain,
    ( ( ~ sP0
      | ( ~ p
        & ( ~ q
          | ~ r ) ) )
    & ( sP0
      | p
      | ( q
        & r ) ) ),
    inference(nnf_transformation,[],[f5]) ).

fof(f9,plain,
    ( ( ~ sP0
      | ( ~ p
        & ( ~ q
          | ~ r ) ) )
    & ( sP0
      | p
      | ( q
        & r ) ) ),
    inference(flattening,[],[f8]) ).

fof(f10,plain,
    ( p
    | r
    | ~ sP0 ),
    inference(cnf_transformation,[],[f7]) ).

fof(f11,plain,
    ( p
    | q
    | ~ sP0 ),
    inference(cnf_transformation,[],[f7]) ).

fof(f12,plain,
    ( sP0
    | ~ q
    | ~ r ),
    inference(cnf_transformation,[],[f7]) ).

fof(f15,plain,
    ( sP0
    | ~ p
    | ~ p ),
    inference(cnf_transformation,[],[f7]) ).

fof(f16,plain,
    ( sP0
    | p
    | r ),
    inference(cnf_transformation,[],[f9]) ).

fof(f17,plain,
    ( sP0
    | p
    | q ),
    inference(cnf_transformation,[],[f9]) ).

fof(f18,plain,
    ( ~ sP0
    | ~ q
    | ~ r ),
    inference(cnf_transformation,[],[f9]) ).

fof(f19,plain,
    ( ~ sP0
    | ~ p ),
    inference(cnf_transformation,[],[f9]) ).

fof(f20,plain,
    ( sP0
    | ~ p ),
    inference(duplicate_literal_removal,[],[f15]) ).

fof(f22,definition,
    ( spl1_1
  <=> r ),
    introduced(definition,[new_symbols(naming,[spl1_1])],[avatar_definition]) ).

fof(f25,definition,
    ( spl1_2
  <=> p ),
    introduced(definition,[new_symbols(naming,[spl1_2])],[avatar_definition]) ).

fof(f28,definition,
    ( spl1_3
  <=> sP0 ),
    introduced(definition,[new_symbols(naming,[spl1_3])],[avatar_definition]) ).

fof(f30,plain,
    ( spl1_1
    | spl1_2
    | spl1_3 ),
    inference(avatar_split_clause,[],[f16,f28,f25,f22]) ).

fof(f32,definition,
    ( spl1_4
  <=> q ),
    introduced(definition,[new_symbols(naming,[spl1_4])],[avatar_definition]) ).

fof(f34,plain,
    ( spl1_4
    | spl1_2
    | spl1_3 ),
    inference(avatar_split_clause,[],[f17,f28,f25,f32]) ).

fof(f38,plain,
    ( ~ spl1_1
    | ~ spl1_4
    | ~ spl1_3 ),
    inference(avatar_split_clause,[],[f18,f28,f32,f22]) ).

fof(f40,plain,
    ( ~ spl1_2
    | ~ spl1_3 ),
    inference(avatar_split_clause,[],[f19,f28,f25]) ).

fof(f41,plain,
    ( ~ spl1_3
    | spl1_1
    | spl1_2 ),
    inference(avatar_split_clause,[],[f10,f25,f22,f28]) ).

fof(f42,plain,
    ( ~ spl1_3
    | spl1_4
    | spl1_2 ),
    inference(avatar_split_clause,[],[f11,f25,f32,f28]) ).

fof(f43,plain,
    ( ~ spl1_1
    | ~ spl1_4
    | spl1_3 ),
    inference(avatar_split_clause,[],[f12,f28,f32,f22]) ).

fof(f46,plain,
    ( ~ spl1_2
    | spl1_3 ),
    inference(avatar_split_clause,[],[f20,f28,f25]) ).

fof(s1,plain,
    ( spl1_1
    | spl1_2
    | spl1_3 ),
    inference(sat_conversion,[],[f30]) ).

fof(s2,plain,
    ( spl1_2
    | spl1_3
    | spl1_4 ),
    inference(sat_conversion,[],[f34]) ).

fof(s3,plain,
    ( ~ spl1_1
    | ~ spl1_3
    | ~ spl1_4 ),
    inference(sat_conversion,[],[f38]) ).

fof(s4,plain,
    ( ~ spl1_2
    | ~ spl1_3 ),
    inference(sat_conversion,[],[f40]) ).

fof(s5,plain,
    ( spl1_1
    | spl1_2
    | ~ spl1_3 ),
    inference(sat_conversion,[],[f41]) ).

fof(s6,plain,
    ( spl1_2
    | ~ spl1_3
    | spl1_4 ),
    inference(sat_conversion,[],[f42]) ).

fof(s7,plain,
    ( ~ spl1_1
    | spl1_3
    | ~ spl1_4 ),
    inference(sat_conversion,[],[f43]) ).

fof(s10,plain,
    ( ~ spl1_2
    | spl1_3 ),
    inference(sat_conversion,[],[f46]) ).

fof(s11,plain,
    ( spl1_2
    | spl1_1 ),
    inference(rat,[],[s1,s5]) ).

fof(s12,plain,
    ~ spl1_2,
    inference(rat,[],[s4,s10]) ).

fof(s13,plain,
    spl1_1,
    inference(rat,[],[s11,s12]) ).

fof(s14,plain,
    spl1_3,
    inference(rat,[],[s2,s7,s12,s13]) ).

fof(s15,plain,
    spl1_4,
    inference(rat,[],[s6,s12,s14]) ).

fof(s16,plain,
    $false,
    inference(rat,[],[s3,s13,s14,s15]) ).

fof(f47,plain,
    $false,
    inference(avatar_sat_refutation,[],[s16]) ).

%------------------------------------------------------------------------------
%----ORIGINAL SYSTEM OUTPUT
% 0.00/0.12  % Problem    : SYN045+1 : TPTP v9.2.1. Released v2.0.0.
% 0.00/0.12  % Command    : run_vampire %s %d THM
% 0.14/0.33  % Computer   : n002.cluster.edu
% 0.14/0.33  % Model      : x86_64 x86_64
% 0.14/0.33  % CPU        : Intel(R) Xeon(R) CPU E5-2620 v4 @ 2.10GHz
% 0.14/0.33  % Memory     : 8042.1875MB
% 0.14/0.33  % OS         : Linux 3.10.0-693.el7.x86_64
% 0.14/0.33  % CPULimit   : 300
% 0.14/0.33  % WCLimit    : 300
% 0.14/0.33  % DateTime   : Fri May  1 05:42:46 EDT 2026
% 0.14/0.33  % CPUTime    : 
% 0.14/0.35  This is a FOF_THM_PRP problem
% 0.14/0.35  Running first-order theorem proving
% 0.14/0.35  Running /export/starexec/sandbox/solver/bin/vampire --input_syntax tptp --proof tptp --output_axiom_names on --mode casc --cores 7 -m 16384 -t 300 /export/starexec/sandbox/benchmark/theBenchmark.p
% 0.45/0.64  % (5040)Detected formulas, will run a generic FOF schedule.
% 0.47/0.75  % (5048)dis-21_1_sil=8000:lcm=predicate:random_seed=415695429:st=5:avsq=on:i=129:avsqr=1,16:sd=3:aac=none:ep=RS:fsr=off:ss=included_3000 on theBenchmark for (3000ds/129Mi)
% 0.47/0.75  % (5048)First to succeed.
% 0.47/0.75  % (5048)Solution written to "/export/starexec/sandbox/tmp/vampire-proof-5040"
% 0.47/0.78  % (5042)lrs+10_1_ncem=casc2026/models/loop8.pt:sil=128000:tgt=full:npcc=on:drc=off:sp=weighted_frequency:spb=goal:fd=preordered:foolp=on:random_seed=2762319704:i=141193_3000 on theBenchmark for (3000ds/141193Mi)
% 0.47/0.78  % (5043)lrs+11_1_ncem=casc2026/models/loop8.pt:sil=128000:npcc=on:lma=off:spb=units:urr=ec_only:bce=on:s2agt=64:updr=off:random_seed=1239119122:i=134677:sd=20:aac=none:nm=16:ss=included:sgt=10_3000 on theBenchmark for (3000ds/134677Mi)
% 0.47/0.78  % (5045)lrs+1010_1_to=lpo:sil=32000:sos=on:spb=goal_then_units:bce=on:random_seed=1331207739:i=109:sd=1:ins=1:gsp=on:ss=axioms_3000 on theBenchmark for (3000ds/109Mi)
% 0.47/0.78  % (5047)dis-1011_1_sil=16000:fde=unused:s2agt=70:random_seed=339019892:s2a=on:i=139:gtg=position_3000 on theBenchmark for (3000ds/139Mi)
% 0.47/0.78  % (5046)dis-1010_2:3_sil=16000:sp=reverse_frequency:random_seed=2011126885:i=119:av=off:ss=axioms_3000 on theBenchmark for (3000ds/119Mi)
% 0.47/0.78  % (5044)lrs+1010_1_anc=all:sfv=off:to=kbo:ncem=casc2026/models/loop7.pt:sil=128000:npcc=on:prc=on:sos=all:bsr=unit_only:sac=on:random_seed=3602899190:i=141695:sd=1:nm=32:gsp=on:ss=included_3000 on theBenchmark for (3000ds/141695Mi)
% 0.47/0.78  % (5046)Also succeeded, but the first one will report.
% 0.47/0.78  % (5047)Also succeeded, but the first one will report.
% 0.47/0.78  % (5045)Also succeeded, but the first one will report.
% 0.71/0.90  % (5048)Refutation found. Thanks to Tanya!
% 0.71/0.90  % SZS status Theorem for theBenchmark
% 0.71/0.90  % SZS output start Proof for theBenchmark
% See solution above
% 0.71/0.90  % (5048)------------------------------
% 0.71/0.90  % (5048)Version: Vampire 5.0.1 (Release build, commit 1b9f22200 on 2026-04-29 16:18:29 +0200)
% 0.71/0.90  % (5048)Linked with Z3 4.14.0.0 3c47fd96cf5645d0c42b2c819d9e9a84380aa721 z3-4.8.4-9178-g3c47fd96c
% 0.71/0.90  % (5048)CaDiCaL version: 2.1.3
% 0.71/0.90  % (5048)Termination reason: Refutation
% 0.71/0.90  % (5048)Time elapsed: 0.001 s
% 0.71/0.90  % (5048)Peak memory usage: 81 MB
% 0.71/0.90  % (5048)Instructions burned: 1 (million)
% 0.71/0.90  % (5048)------------------------------
% 0.71/0.90  % (5048)------------------------------
% 0.71/0.90  % (5040)Success in time 0.268 s
%------------------------------------------------------------------------------

