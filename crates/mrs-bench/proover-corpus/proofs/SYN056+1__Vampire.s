% Proof : Problems/SYN056+1.p
%------------------------------------------------------------------------------
% File     : Vampire---5.0.1
% Problem  : SYN056+1 : TPTP v9.2.1. Released v2.0.0.
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
% DateTime : Fri May  1 04:39:28 PM UTC 2026

% Result   : Theorem 0.49s 0.89s
% Output   : Refutation 0.49s
% Verified : 
% SZS Type : Refutation
%            Derivation depth      :   18
%            Number of leaves      :   19
% Syntax   : Number of formulae    :  109 (  12 unt;  12 def)
%            Number of atoms       :  288 (   0 equ)
%            Maximal formula atoms :    8 (   2 avg)
%            Number of connectives :  330 ( 151   ~; 133   |;  18   &)
%                                         (  18 <=>;   9  =>;   0  <=;   1 <~>)
%            Maximal formula depth :    7 (   4 avg)
%            Maximal term depth    :    1 (   1 avg)
%            Number of predicates  :   17 (  16 usr;  13 prp; 0-1 aty)
%            Number of functors    :    4 (   4 usr;   4 con; 0-0 aty)
%            Number of variables   :   64 (   0 sgn  50   !;  14   ?)

% Comments : 
%------------------------------------------------------------------------------
fof(f1,axiom,
    ( ? [X0] : big_p(X0)
  <=> ? [X1] : big_q(X1) ),
    file('/export/starexec/sandbox/benchmark/theBenchmark.p',pel26_1) ).

fof(f2,axiom,
    ! [X0,X1] :
      ( ( big_p(X0)
        & big_q(X1) )
     => ( big_r(X0)
      <=> big_s(X1) ) ),
    file('/export/starexec/sandbox/benchmark/theBenchmark.p',pel26_2) ).

fof(f3,conjecture,
    ( ! [X0] :
        ( big_p(X0)
       => big_r(X0) )
  <=> ! [X1] :
        ( big_q(X1)
       => big_s(X1) ) ),
    file('/export/starexec/sandbox/benchmark/theBenchmark.p',pel26) ).

fof(f4,negated_conjecture,
    ~ ( ! [X0] :
          ( big_p(X0)
         => big_r(X0) )
    <=> ! [X1] :
          ( big_q(X1)
         => big_s(X1) ) ),
    inference(negated_conjecture,[status(cth)],[f3]) ).

fof(f5,plain,
    ! [X0,X1] :
      ( ( big_r(X0)
      <=> big_s(X1) )
      | ~ big_p(X0)
      | ~ big_q(X1) ),
    inference(ennf_transformation,[],[f2]) ).

fof(f6,plain,
    ! [X0,X1] :
      ( ( big_r(X0)
      <=> big_s(X1) )
      | ~ big_p(X0)
      | ~ big_q(X1) ),
    inference(flattening,[],[f5]) ).

fof(f7,plain,
    ( ! [X0] :
        ( big_r(X0)
        | ~ big_p(X0) )
  <~> ! [X1] :
        ( big_s(X1)
        | ~ big_q(X1) ) ),
    inference(ennf_transformation,[],[f4]) ).

fof(f8,plain,
    ( ( ? [X0] : big_p(X0)
      | ! [X1] : ~ big_q(X1) )
    & ( ? [X1] : big_q(X1)
      | ! [X0] : ~ big_p(X0) ) ),
    inference(nnf_transformation,[],[f1]) ).

fof(f9,plain,
    ( ( ? [X0] : big_p(X0)
      | ! [X1] : ~ big_q(X1) )
    & ( ? [X2] : big_q(X2)
      | ! [X3] : ~ big_p(X3) ) ),
    inference(rectify,[],[f8]) ).

fof(f10,plain,
    ( ? [X0] : big_p(X0)
   => big_p(sK0) ),
    introduced(definition,[],[skolem_symbol_introduction]) ).

fof(f11,plain,
    ( ? [X2] : big_q(X2)
   => big_q(sK1) ),
    introduced(definition,[],[skolem_symbol_introduction]) ).

fof(f12,plain,
    ( ( big_p(sK0)
      | ! [X1] : ~ big_q(X1) )
    & ( big_q(sK1)
      | ! [X3] : ~ big_p(X3) ) ),
    inference(skolemisation,[status(esa),new_symbols(skolem,[sK0,sK1])],[f9,f11,f10]) ).

fof(f13,plain,
    ! [X0,X1] :
      ( ( ( big_r(X0)
          | ~ big_s(X1) )
        & ( big_s(X1)
          | ~ big_r(X0) ) )
      | ~ big_p(X0)
      | ~ big_q(X1) ),
    inference(nnf_transformation,[],[f6]) ).

fof(f14,plain,
    ( ( ? [X1] :
          ( ~ big_s(X1)
          & big_q(X1) )
      | ? [X0] :
          ( ~ big_r(X0)
          & big_p(X0) ) )
    & ( ! [X1] :
          ( big_s(X1)
          | ~ big_q(X1) )
      | ! [X0] :
          ( big_r(X0)
          | ~ big_p(X0) ) ) ),
    inference(nnf_transformation,[],[f7]) ).

fof(f15,plain,
    ( ( ? [X0] :
          ( ~ big_s(X0)
          & big_q(X0) )
      | ? [X1] :
          ( ~ big_r(X1)
          & big_p(X1) ) )
    & ( ! [X2] :
          ( big_s(X2)
          | ~ big_q(X2) )
      | ! [X3] :
          ( big_r(X3)
          | ~ big_p(X3) ) ) ),
    inference(rectify,[],[f14]) ).

fof(f16,plain,
    ( ? [X0] :
        ( ~ big_s(X0)
        & big_q(X0) )
   => ( ~ big_s(sK2)
      & big_q(sK2) ) ),
    introduced(definition,[],[skolem_symbol_introduction]) ).

fof(f17,plain,
    ( ? [X1] :
        ( ~ big_r(X1)
        & big_p(X1) )
   => ( ~ big_r(sK3)
      & big_p(sK3) ) ),
    introduced(definition,[],[skolem_symbol_introduction]) ).

fof(f18,plain,
    ( ( ( ~ big_s(sK2)
        & big_q(sK2) )
      | ( ~ big_r(sK3)
        & big_p(sK3) ) )
    & ( ! [X2] :
          ( big_s(X2)
          | ~ big_q(X2) )
      | ! [X3] :
          ( big_r(X3)
          | ~ big_p(X3) ) ) ),
    inference(skolemisation,[status(esa),new_symbols(skolem,[sK2,sK3])],[f15,f17,f16]) ).

fof(f19,plain,
    ! [X3] :
      ( big_q(sK1)
      | ~ big_p(X3) ),
    inference(cnf_transformation,[],[f12]) ).

fof(f20,plain,
    ! [X1] :
      ( big_p(sK0)
      | ~ big_q(X1) ),
    inference(cnf_transformation,[],[f12]) ).

fof(f21,plain,
    ! [X0,X1] :
      ( big_s(X1)
      | ~ big_r(X0)
      | ~ big_p(X0)
      | ~ big_q(X1) ),
    inference(cnf_transformation,[],[f13]) ).

fof(f22,plain,
    ! [X0,X1] :
      ( big_r(X0)
      | ~ big_s(X1)
      | ~ big_p(X0)
      | ~ big_q(X1) ),
    inference(cnf_transformation,[],[f13]) ).

fof(f23,plain,
    ! [X2,X3] :
      ( big_s(X2)
      | ~ big_q(X2)
      | big_r(X3)
      | ~ big_p(X3) ),
    inference(cnf_transformation,[],[f18]) ).

fof(f24,plain,
    ( big_q(sK2)
    | big_p(sK3) ),
    inference(cnf_transformation,[],[f18]) ).

fof(f25,plain,
    ( big_q(sK2)
    | ~ big_r(sK3) ),
    inference(cnf_transformation,[],[f18]) ).

fof(f26,plain,
    ( ~ big_s(sK2)
    | big_p(sK3) ),
    inference(cnf_transformation,[],[f18]) ).

fof(f27,plain,
    ( ~ big_s(sK2)
    | ~ big_r(sK3) ),
    inference(cnf_transformation,[],[f18]) ).

fof(f29,definition,
    ( spl4_1
  <=> ! [X3] :
        ( big_r(X3)
        | ~ big_p(X3) ) ),
    introduced(definition,[new_symbols(naming,[spl4_1])],[avatar_definition]) ).

fof(f30,plain,
    ( ! [X3] :
        ( big_r(X3)
        | ~ big_p(X3) )
    | ~ spl4_1 ),
    inference(avatar_component_clause,[],[f29]) ).

fof(f32,definition,
    ( spl4_2
  <=> ! [X2] :
        ( big_s(X2)
        | ~ big_q(X2) ) ),
    introduced(definition,[new_symbols(naming,[spl4_2])],[avatar_definition]) ).

fof(f33,plain,
    ( ! [X2] :
        ( big_s(X2)
        | ~ big_q(X2) )
    | ~ spl4_2 ),
    inference(avatar_component_clause,[],[f32]) ).

fof(f34,plain,
    ( spl4_1
    | spl4_2 ),
    inference(avatar_split_clause,[],[f23,f32,f29]) ).

fof(f36,definition,
    ( spl4_3
  <=> big_p(sK3) ),
    introduced(definition,[new_symbols(naming,[spl4_3])],[avatar_definition]) ).

fof(f37,plain,
    ( big_p(sK3)
    | ~ spl4_3 ),
    inference(avatar_component_clause,[],[f36]) ).

fof(f39,definition,
    ( spl4_4
  <=> big_q(sK2) ),
    introduced(definition,[new_symbols(naming,[spl4_4])],[avatar_definition]) ).

fof(f40,plain,
    ( big_q(sK2)
    | ~ spl4_4 ),
    inference(avatar_component_clause,[],[f39]) ).

fof(f41,plain,
    ( spl4_3
    | spl4_4 ),
    inference(avatar_split_clause,[],[f24,f39,f36]) ).

fof(f43,definition,
    ( spl4_5
  <=> big_r(sK3) ),
    introduced(definition,[new_symbols(naming,[spl4_5])],[avatar_definition]) ).

fof(f44,plain,
    ( ~ big_r(sK3)
    | spl4_5 ),
    inference(avatar_component_clause,[],[f43]) ).

fof(f45,plain,
    ( ~ spl4_5
    | spl4_4 ),
    inference(avatar_split_clause,[],[f25,f39,f43]) ).

fof(f47,definition,
    ( spl4_6
  <=> big_s(sK2) ),
    introduced(definition,[new_symbols(naming,[spl4_6])],[avatar_definition]) ).

fof(f48,plain,
    ( ~ big_s(sK2)
    | spl4_6 ),
    inference(avatar_component_clause,[],[f47]) ).

fof(f49,plain,
    ( spl4_3
    | ~ spl4_6 ),
    inference(avatar_split_clause,[],[f26,f47,f36]) ).

fof(f50,plain,
    ( ~ spl4_5
    | ~ spl4_6 ),
    inference(avatar_split_clause,[],[f27,f47,f43]) ).

fof(f52,definition,
    ( spl4_7
  <=> ! [X0] :
        ( ~ big_r(X0)
        | ~ big_p(X0) ) ),
    introduced(definition,[new_symbols(naming,[spl4_7])],[avatar_definition]) ).

fof(f53,plain,
    ( ! [X0] :
        ( ~ big_r(X0)
        | ~ big_p(X0) )
    | ~ spl4_7 ),
    inference(avatar_component_clause,[],[f52]) ).

fof(f54,plain,
    ( spl4_7
    | spl4_2 ),
    inference(avatar_split_clause,[],[f21,f32,f52]) ).

fof(f56,definition,
    ( spl4_8
  <=> ! [X1] :
        ( ~ big_s(X1)
        | ~ big_q(X1) ) ),
    introduced(definition,[new_symbols(naming,[spl4_8])],[avatar_definition]) ).

fof(f57,plain,
    ( ! [X1] :
        ( ~ big_s(X1)
        | ~ big_q(X1) )
    | ~ spl4_8 ),
    inference(avatar_component_clause,[],[f56]) ).

fof(f58,plain,
    ( spl4_8
    | spl4_1 ),
    inference(avatar_split_clause,[],[f22,f29,f56]) ).

fof(f60,definition,
    ( spl4_9
  <=> ! [X3] : ~ big_p(X3) ),
    introduced(definition,[new_symbols(naming,[spl4_9])],[avatar_definition]) ).

fof(f61,plain,
    ( ! [X3] : ~ big_p(X3)
    | ~ spl4_9 ),
    inference(avatar_component_clause,[],[f60]) ).

fof(f63,definition,
    ( spl4_10
  <=> big_q(sK1) ),
    introduced(definition,[new_symbols(naming,[spl4_10])],[avatar_definition]) ).

fof(f64,plain,
    ( big_q(sK1)
    | ~ spl4_10 ),
    inference(avatar_component_clause,[],[f63]) ).

fof(f65,plain,
    ( spl4_9
    | spl4_10 ),
    inference(avatar_split_clause,[],[f19,f63,f60]) ).

fof(f67,definition,
    ( spl4_11
  <=> ! [X1] : ~ big_q(X1) ),
    introduced(definition,[new_symbols(naming,[spl4_11])],[avatar_definition]) ).

fof(f68,plain,
    ( ! [X1] : ~ big_q(X1)
    | ~ spl4_11 ),
    inference(avatar_component_clause,[],[f67]) ).

fof(f70,definition,
    ( spl4_12
  <=> big_p(sK0) ),
    introduced(definition,[new_symbols(naming,[spl4_12])],[avatar_definition]) ).

fof(f71,plain,
    ( big_p(sK0)
    | ~ spl4_12 ),
    inference(avatar_component_clause,[],[f70]) ).

fof(f72,plain,
    ( spl4_11
    | spl4_12 ),
    inference(avatar_split_clause,[],[f20,f70,f67]) ).

fof(f73,plain,
    ( $false
    | ~ spl4_3
    | ~ spl4_9 ),
    inference(resolution,[],[f61,f37]) ).

fof(f74,plain,
    ( ~ spl4_3
    | ~ spl4_9 ),
    inference(avatar_contradiction_clause,[],[f73]) ).

fof(f75,plain,
    ( $false
    | ~ spl4_10
    | ~ spl4_11 ),
    inference(resolution,[],[f68,f64]) ).

fof(f76,plain,
    ( ~ spl4_10
    | ~ spl4_11 ),
    inference(avatar_contradiction_clause,[],[f75]) ).

fof(f77,plain,
    ( ! [X0] :
        ( ~ big_q(X0)
        | ~ big_q(X0) )
    | ~ spl4_2
    | ~ spl4_8 ),
    inference(resolution,[],[f57,f33]) ).

fof(f78,plain,
    ( ! [X0] : ~ big_q(X0)
    | ~ spl4_2
    | ~ spl4_8 ),
    inference(duplicate_literal_removal,[],[f77]) ).

fof(f79,plain,
    ( spl4_11
    | ~ spl4_2
    | ~ spl4_8 ),
    inference(avatar_split_clause,[],[f78,f56,f32,f67]) ).

fof(f80,plain,
    ( ~ big_p(sK3)
    | ~ spl4_1
    | spl4_5 ),
    inference(resolution,[],[f30,f44]) ).

fof(f81,plain,
    ( ! [X0] :
        ( ~ big_p(X0)
        | ~ big_p(X0) )
    | ~ spl4_1
    | ~ spl4_7 ),
    inference(resolution,[],[f53,f30]) ).

fof(f82,plain,
    ( ! [X0] : ~ big_p(X0)
    | ~ spl4_1
    | ~ spl4_7 ),
    inference(duplicate_literal_removal,[],[f81]) ).

fof(f83,plain,
    ( spl4_9
    | ~ spl4_1
    | ~ spl4_7 ),
    inference(avatar_split_clause,[],[f82,f52,f29,f60]) ).

fof(f84,plain,
    ( $false
    | ~ spl4_9
    | ~ spl4_12 ),
    inference(resolution,[],[f61,f71]) ).

fof(f85,plain,
    ( ~ spl4_9
    | ~ spl4_12 ),
    inference(avatar_contradiction_clause,[],[f84]) ).

fof(f86,plain,
    ( ~ big_q(sK2)
    | ~ spl4_2
    | spl4_6 ),
    inference(resolution,[],[f33,f48]) ).

fof(f87,plain,
    ( $false
    | ~ spl4_2
    | ~ spl4_4
    | spl4_6 ),
    inference(resolution,[],[f86,f40]) ).

fof(f88,plain,
    ( ~ spl4_2
    | ~ spl4_4
    | spl4_6 ),
    inference(avatar_contradiction_clause,[],[f87]) ).

fof(f89,plain,
    ( $false
    | ~ spl4_1
    | ~ spl4_3
    | spl4_5 ),
    inference(resolution,[],[f37,f80]) ).

fof(f90,plain,
    ( ~ spl4_1
    | ~ spl4_3
    | spl4_5 ),
    inference(avatar_contradiction_clause,[],[f89]) ).

fof(f92,plain,
    ( $false
    | ~ spl4_4
    | ~ spl4_11 ),
    inference(resolution,[],[f68,f40]) ).

fof(f93,plain,
    ( ~ spl4_4
    | ~ spl4_11 ),
    inference(avatar_contradiction_clause,[],[f92]) ).

fof(s1,plain,
    ( spl4_1
    | spl4_2 ),
    inference(sat_conversion,[],[f34]) ).

fof(s2,plain,
    ( spl4_3
    | spl4_4 ),
    inference(sat_conversion,[],[f41]) ).

fof(s3,plain,
    ( spl4_4
    | ~ spl4_5 ),
    inference(sat_conversion,[],[f45]) ).

fof(s4,plain,
    ( spl4_3
    | ~ spl4_6 ),
    inference(sat_conversion,[],[f49]) ).

fof(s5,plain,
    ( ~ spl4_5
    | ~ spl4_6 ),
    inference(sat_conversion,[],[f50]) ).

fof(s6,plain,
    ( spl4_2
    | spl4_7 ),
    inference(sat_conversion,[],[f54]) ).

fof(s7,plain,
    ( spl4_1
    | spl4_8 ),
    inference(sat_conversion,[],[f58]) ).

fof(s8,plain,
    ( spl4_9
    | spl4_10 ),
    inference(sat_conversion,[],[f65]) ).

fof(s9,plain,
    ( spl4_11
    | spl4_12 ),
    inference(sat_conversion,[],[f72]) ).

fof(s10,plain,
    ( ~ spl4_3
    | ~ spl4_9 ),
    inference(sat_conversion,[],[f74]) ).

fof(s11,plain,
    ( ~ spl4_10
    | ~ spl4_11 ),
    inference(sat_conversion,[],[f76]) ).

fof(s12,plain,
    ( ~ spl4_2
    | ~ spl4_8
    | spl4_11 ),
    inference(sat_conversion,[],[f79]) ).

fof(s13,plain,
    ( ~ spl4_1
    | ~ spl4_7
    | spl4_9 ),
    inference(sat_conversion,[],[f83]) ).

fof(s14,plain,
    ( ~ spl4_9
    | ~ spl4_12 ),
    inference(sat_conversion,[],[f85]) ).

fof(s15,plain,
    ( ~ spl4_2
    | ~ spl4_4
    | spl4_6 ),
    inference(sat_conversion,[],[f88]) ).

fof(s16,plain,
    ( ~ spl4_1
    | ~ spl4_3
    | spl4_5 ),
    inference(sat_conversion,[],[f90]) ).

fof(s19,plain,
    ( ~ spl4_4
    | ~ spl4_11 ),
    inference(sat_conversion,[],[f93]) ).

fof(s20,plain,
    ~ spl4_11,
    inference(rat,[],[s10,s8,s2,s11,s19]) ).

fof(s21,plain,
    spl4_12,
    inference(rat,[],[s9,s20]) ).

fof(s22,plain,
    ~ spl4_9,
    inference(rat,[],[s14,s21]) ).

fof(s24,plain,
    spl4_1,
    inference(rat,[],[s12,s1,s7,s20]) ).

fof(s25,plain,
    ~ spl4_7,
    inference(rat,[],[s13,s22,s24]) ).

fof(s26,plain,
    spl4_2,
    inference(rat,[],[s6,s25]) ).

fof(s28,plain,
    ~ spl4_5,
    inference(rat,[],[s15,s3,s5,s26]) ).

fof(s29,plain,
    ~ spl4_3,
    inference(rat,[],[s16,s24,s28]) ).

fof(s30,plain,
    ~ spl4_6,
    inference(rat,[],[s4,s29]) ).

fof(s31,plain,
    spl4_4,
    inference(rat,[],[s2,s29]) ).

fof(s32,plain,
    $false,
    inference(rat,[],[s15,s26,s30,s31]) ).

fof(f94,plain,
    $false,
    inference(avatar_sat_refutation,[],[s32]) ).

%------------------------------------------------------------------------------
%----ORIGINAL SYSTEM OUTPUT
% 0.00/0.12  % Problem    : SYN056+1 : TPTP v9.2.1. Released v2.0.0.
% 0.00/0.12  % Command    : run_vampire %s %d THM
% 0.15/0.33  % Computer   : n002.cluster.edu
% 0.15/0.33  % Model      : x86_64 x86_64
% 0.15/0.33  % CPU        : Intel(R) Xeon(R) CPU E5-2620 v4 @ 2.10GHz
% 0.15/0.33  % Memory     : 8042.1875MB
% 0.15/0.33  % OS         : Linux 3.10.0-693.el7.x86_64
% 0.15/0.33  % CPULimit   : 300
% 0.15/0.33  % WCLimit    : 300
% 0.15/0.33  % DateTime   : Fri May  1 05:44:01 EDT 2026
% 0.15/0.33  % CPUTime    : 
% 0.15/0.35  This is a FOF_THM_RFO_NEQ problem
% 0.15/0.35  Running first-order theorem proving
% 0.15/0.36  Running /export/starexec/sandbox/solver/bin/vampire --input_syntax tptp --proof tptp --output_axiom_names on --mode casc --cores 7 -m 16384 -t 300 /export/starexec/sandbox/benchmark/theBenchmark.p
% 0.47/0.64  % (9366)Detected formulas, will run a generic FOF schedule.
% 0.49/0.73  % (9387)dis-21_1_sil=8000:lcm=predicate:random_seed=1439959868:st=5:avsq=on:i=129:avsqr=1,16:sd=3:aac=none:ep=RS:fsr=off:ss=included_3000 on theBenchmark for (3000ds/129Mi)
% 0.49/0.74  % (9387)First to succeed.
% 0.49/0.74  % (9387)Solution written to "/export/starexec/sandbox/tmp/vampire-proof-9366"
% 0.49/0.77  % (9382)lrs+11_1_ncem=casc2026/models/loop8.pt:sil=128000:npcc=on:lma=off:spb=units:urr=ec_only:bce=on:s2agt=64:updr=off:random_seed=3500501941:i=134677:sd=20:aac=none:nm=16:ss=included:sgt=10_3000 on theBenchmark for (3000ds/134677Mi)
% 0.49/0.78  % (9381)lrs+10_1_ncem=casc2026/models/loop8.pt:sil=128000:tgt=full:npcc=on:drc=off:sp=weighted_frequency:spb=goal:fd=preordered:foolp=on:random_seed=498452895:i=141193_3000 on theBenchmark for (3000ds/141193Mi)
% 0.49/0.78  % (9385)dis-1010_2:3_sil=16000:sp=reverse_frequency:random_seed=1663129338:i=119:av=off:ss=axioms_3000 on theBenchmark for (3000ds/119Mi)
% 0.49/0.78  % (9383)lrs+1010_1_anc=all:sfv=off:to=kbo:ncem=casc2026/models/loop7.pt:sil=128000:npcc=on:prc=on:sos=all:bsr=unit_only:sac=on:random_seed=696541239:i=141695:sd=1:nm=32:gsp=on:ss=included_3000 on theBenchmark for (3000ds/141695Mi)
% 0.49/0.79  % (9385)Also succeeded, but the first one will report.
% 0.49/0.80  % (9384)lrs+1010_1_to=lpo:sil=32000:sos=on:spb=goal_then_units:bce=on:random_seed=1900409879:i=109:sd=1:ins=1:gsp=on:ss=axioms_3000 on theBenchmark for (3000ds/109Mi)
% 0.49/0.80  % (9384)Also succeeded, but the first one will report.
% 0.49/0.81  % (9386)dis-1011_1_sil=16000:fde=unused:s2agt=70:random_seed=1668658516:s2a=on:i=139:gtg=position_3000 on theBenchmark for (3000ds/139Mi)
% 0.49/0.81  % (9386)Also succeeded, but the first one will report.
% 0.49/0.89  % (9387)Refutation found. Thanks to Tanya!
% 0.49/0.89  % SZS status Theorem for theBenchmark
% 0.49/0.89  % SZS output start Proof for theBenchmark
% See solution above
% 0.49/0.89  % (9387)------------------------------
% 0.49/0.89  % (9387)Version: Vampire 5.0.1 (Release build, commit 1b9f22200 on 2026-04-29 16:18:29 +0200)
% 0.49/0.89  % (9387)Linked with Z3 4.14.0.0 3c47fd96cf5645d0c42b2c819d9e9a84380aa721 z3-4.8.4-9178-g3c47fd96c
% 0.49/0.89  % (9387)CaDiCaL version: 2.1.3
% 0.49/0.89  % (9387)Termination reason: Refutation
% 0.49/0.89  % (9387)Time elapsed: 0.002 s
% 0.49/0.89  % (9387)Peak memory usage: 81 MB
% 0.49/0.89  % (9387)Instructions burned: 2 (million)
% 0.49/0.89  % (9387)------------------------------
% 0.49/0.89  % (9387)------------------------------
% 0.49/0.89  % (9366)Success in time 0.246 s
%------------------------------------------------------------------------------

