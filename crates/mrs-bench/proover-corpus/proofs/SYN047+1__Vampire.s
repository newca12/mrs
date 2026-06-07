% Proof : Problems/SYN047+1.p
%------------------------------------------------------------------------------
% File     : Vampire---5.0.1
% Problem  : SYN047+1 : TPTP v9.2.1. Released v2.0.0.
% Transfm  : none
% Format   : tptp:raw
% Command  : run_vampire %s %d THM

% Computer : n026.cluster.edu
% Model    : x86_64 x86_64
% CPU      : Intel(R) Xeon(R) CPU E5-2620 v4 2.10GHz
% Memory   : 8042.1875MB
% OS       : Linux 3.10.0-693.el7.x86_64
% CPULimit : 300s
% WCLimit  : 300s
% DateTime : Fri May  1 04:39:25 PM UTC 2026

% Result   : Theorem 0.72s 0.90s
% Output   : Refutation 0.72s
% Verified : 
% SZS Type : Refutation
%            Derivation depth      :   14
%            Number of leaves      :    9
% Syntax   : Number of formulae    :   68 (   7 unt;   6 def)
%            Number of atoms       :  249 (   0 equ)
%            Maximal formula atoms :   10 (   3 avg)
%            Number of connectives :  293 ( 112   ~; 132   |;  32   &)
%                                         (  10 <=>;   4  =>;   0  <=;   3 <~>)
%            Maximal formula depth :    7 (   4 avg)
%            Maximal term depth    :    0 (   0 avg)
%            Number of predicates  :   13 (  12 usr;  13 prp; 0-0 aty)
%            Number of functors    :    0 (   0 usr;   0 con; --- aty)
%            Number of variables   :    0 (   0 sgn   0   !;   0   ?)

% Comments : 
%------------------------------------------------------------------------------
fof(f1,conjecture,
    ( ( ( p
        & ( q
         => r ) )
     => s )
  <=> ( ( ~ p
        | q
        | s )
      & ( ~ p
        | ~ r
        | s ) ) ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',pel17) ).

fof(f2,negated_conjecture,
    ~ ( ( ( p
          & ( q
           => r ) )
       => s )
    <=> ( ( ~ p
          | q
          | s )
        & ( ~ p
          | ~ r
          | s ) ) ),
    inference(negated_conjecture,[status(cth)],[f1]) ).

fof(f3,plain,
    ( ( s
      | ~ p
      | ( ~ r
        & q ) )
  <~> ( ( ~ p
        | q
        | s )
      & ( ~ p
        | ~ r
        | s ) ) ),
    inference(ennf_transformation,[],[f2]) ).

fof(f4,plain,
    ( ( s
      | ~ p
      | ( ~ r
        & q ) )
  <~> ( ( ~ p
        | q
        | s )
      & ( ~ p
        | ~ r
        | s ) ) ),
    inference(flattening,[],[f3]) ).

fof(f5,plain,
    ( sP0
  <=> ( ~ p
      | ~ r
      | s ) ),
    introduced(definition,[new_symbols(naming,[sP0])],[predicate_definition_introduction]) ).

fof(f6,plain,
    ( sP1
  <=> ( s
      | ~ p
      | ( ~ r
        & q ) ) ),
    introduced(definition,[new_symbols(naming,[sP1])],[predicate_definition_introduction]) ).

fof(f7,plain,
    ( sP1
  <~> ( ( ~ p
        | q
        | s )
      & sP0 ) ),
    inference(definition_folding,[],[f4,f6,f5]) ).

fof(f8,plain,
    ( ( sP1
      | ( ~ s
        & p
        & ( r
          | ~ q ) ) )
    & ( s
      | ~ p
      | ( ~ r
        & q )
      | ~ sP1 ) ),
    inference(nnf_transformation,[],[f6]) ).

fof(f9,plain,
    ( ( sP1
      | ( ~ s
        & p
        & ( r
          | ~ q ) ) )
    & ( s
      | ~ p
      | ( ~ r
        & q )
      | ~ sP1 ) ),
    inference(flattening,[],[f8]) ).

fof(f10,plain,
    ( ( sP0
      | ( p
        & r
        & ~ s ) )
    & ( ~ p
      | ~ r
      | s
      | ~ sP0 ) ),
    inference(nnf_transformation,[],[f5]) ).

fof(f11,plain,
    ( ( sP0
      | ( p
        & r
        & ~ s ) )
    & ( ~ p
      | ~ r
      | s
      | ~ sP0 ) ),
    inference(flattening,[],[f10]) ).

fof(f12,plain,
    ( ( ( p
        & ~ q
        & ~ s )
      | ~ sP0
      | ~ sP1 )
    & ( ( ( ~ p
          | q
          | s )
        & sP0 )
      | sP1 ) ),
    inference(nnf_transformation,[],[f7]) ).

fof(f13,plain,
    ( ( ( p
        & ~ q
        & ~ s )
      | ~ sP0
      | ~ sP1 )
    & ( ( ( ~ p
          | q
          | s )
        & sP0 )
      | sP1 ) ),
    inference(flattening,[],[f12]) ).

fof(f14,plain,
    ( s
    | ~ p
    | q
    | ~ sP1 ),
    inference(cnf_transformation,[],[f9]) ).

fof(f15,plain,
    ( s
    | ~ p
    | ~ r
    | ~ sP1 ),
    inference(cnf_transformation,[],[f9]) ).

fof(f16,plain,
    ( sP1
    | r
    | ~ q ),
    inference(cnf_transformation,[],[f9]) ).

fof(f17,plain,
    ( sP1
    | p ),
    inference(cnf_transformation,[],[f9]) ).

fof(f18,plain,
    ( sP1
    | ~ s ),
    inference(cnf_transformation,[],[f9]) ).

fof(f19,plain,
    ( ~ p
    | ~ r
    | s
    | ~ sP0 ),
    inference(cnf_transformation,[],[f11]) ).

fof(f20,plain,
    ( sP0
    | ~ s ),
    inference(cnf_transformation,[],[f11]) ).

fof(f21,plain,
    ( sP0
    | r ),
    inference(cnf_transformation,[],[f11]) ).

fof(f22,plain,
    ( sP0
    | p ),
    inference(cnf_transformation,[],[f11]) ).

fof(f23,plain,
    ( sP0
    | sP1 ),
    inference(cnf_transformation,[],[f13]) ).

fof(f24,plain,
    ( ~ p
    | q
    | s
    | sP1 ),
    inference(cnf_transformation,[],[f13]) ).

fof(f25,plain,
    ( ~ s
    | ~ sP0
    | ~ sP1 ),
    inference(cnf_transformation,[],[f13]) ).

fof(f26,plain,
    ( ~ q
    | ~ sP0
    | ~ sP1 ),
    inference(cnf_transformation,[],[f13]) ).

fof(f27,plain,
    ( p
    | ~ sP0
    | ~ sP1 ),
    inference(cnf_transformation,[],[f13]) ).

fof(f29,definition,
    ( spl2_1
  <=> sP1 ),
    introduced(definition,[new_symbols(naming,[spl2_1])],[avatar_definition]) ).

fof(f32,definition,
    ( spl2_2
  <=> sP0 ),
    introduced(definition,[new_symbols(naming,[spl2_2])],[avatar_definition]) ).

fof(f34,plain,
    ( spl2_1
    | spl2_2 ),
    inference(avatar_split_clause,[],[f23,f32,f29]) ).

fof(f36,definition,
    ( spl2_3
  <=> s ),
    introduced(definition,[new_symbols(naming,[spl2_3])],[avatar_definition]) ).

fof(f39,definition,
    ( spl2_4
  <=> q ),
    introduced(definition,[new_symbols(naming,[spl2_4])],[avatar_definition]) ).

fof(f42,definition,
    ( spl2_5
  <=> p ),
    introduced(definition,[new_symbols(naming,[spl2_5])],[avatar_definition]) ).

fof(f44,plain,
    ( spl2_1
    | spl2_3
    | spl2_4
    | ~ spl2_5 ),
    inference(avatar_split_clause,[],[f24,f42,f39,f36,f29]) ).

fof(f48,plain,
    ( ~ spl2_1
    | ~ spl2_2
    | ~ spl2_3 ),
    inference(avatar_split_clause,[],[f25,f36,f32,f29]) ).

fof(f50,plain,
    ( ~ spl2_1
    | ~ spl2_2
    | ~ spl2_4 ),
    inference(avatar_split_clause,[],[f26,f39,f32,f29]) ).

fof(f52,plain,
    ( ~ spl2_1
    | ~ spl2_2
    | spl2_5 ),
    inference(avatar_split_clause,[],[f27,f42,f32,f29]) ).

fof(f54,definition,
    ( spl2_6
  <=> r ),
    introduced(definition,[new_symbols(naming,[spl2_6])],[avatar_definition]) ).

fof(f56,plain,
    ( ~ spl2_2
    | spl2_3
    | ~ spl2_6
    | ~ spl2_5 ),
    inference(avatar_split_clause,[],[f19,f42,f54,f36,f32]) ).

fof(f57,plain,
    ( ~ spl2_3
    | spl2_2 ),
    inference(avatar_split_clause,[],[f20,f32,f36]) ).

fof(f59,plain,
    ( spl2_6
    | spl2_2 ),
    inference(avatar_split_clause,[],[f21,f32,f54]) ).

fof(f60,plain,
    ( spl2_5
    | spl2_2 ),
    inference(avatar_split_clause,[],[f22,f32,f42]) ).

fof(f61,plain,
    ( ~ spl2_1
    | spl2_4
    | ~ spl2_5
    | spl2_3 ),
    inference(avatar_split_clause,[],[f14,f36,f42,f39,f29]) ).

fof(f62,plain,
    ( ~ spl2_1
    | ~ spl2_6
    | ~ spl2_5
    | spl2_3 ),
    inference(avatar_split_clause,[],[f15,f36,f42,f54,f29]) ).

fof(f63,plain,
    ( ~ spl2_4
    | spl2_6
    | spl2_1 ),
    inference(avatar_split_clause,[],[f16,f29,f54,f39]) ).

fof(f64,plain,
    ( spl2_5
    | spl2_1 ),
    inference(avatar_split_clause,[],[f17,f29,f42]) ).

fof(f65,plain,
    ( ~ spl2_3
    | spl2_1 ),
    inference(avatar_split_clause,[],[f18,f29,f36]) ).

fof(s1,plain,
    ( spl2_1
    | spl2_2 ),
    inference(sat_conversion,[],[f34]) ).

fof(s2,plain,
    ( spl2_1
    | spl2_3
    | spl2_4
    | ~ spl2_5 ),
    inference(sat_conversion,[],[f44]) ).

fof(s3,plain,
    ( ~ spl2_1
    | ~ spl2_2
    | ~ spl2_3 ),
    inference(sat_conversion,[],[f48]) ).

fof(s4,plain,
    ( ~ spl2_1
    | ~ spl2_2
    | ~ spl2_4 ),
    inference(sat_conversion,[],[f50]) ).

fof(s5,plain,
    ( ~ spl2_1
    | ~ spl2_2
    | spl2_5 ),
    inference(sat_conversion,[],[f52]) ).

fof(s6,plain,
    ( ~ spl2_2
    | spl2_3
    | ~ spl2_5
    | ~ spl2_6 ),
    inference(sat_conversion,[],[f56]) ).

fof(s7,plain,
    ( spl2_2
    | ~ spl2_3 ),
    inference(sat_conversion,[],[f57]) ).

fof(s8,plain,
    ( spl2_2
    | spl2_6 ),
    inference(sat_conversion,[],[f59]) ).

fof(s9,plain,
    ( spl2_2
    | spl2_5 ),
    inference(sat_conversion,[],[f60]) ).

fof(s10,plain,
    ( ~ spl2_1
    | spl2_3
    | spl2_4
    | ~ spl2_5 ),
    inference(sat_conversion,[],[f61]) ).

fof(s11,plain,
    ( ~ spl2_1
    | spl2_3
    | ~ spl2_5
    | ~ spl2_6 ),
    inference(sat_conversion,[],[f62]) ).

fof(s12,plain,
    ( spl2_1
    | ~ spl2_4
    | spl2_6 ),
    inference(sat_conversion,[],[f63]) ).

fof(s13,plain,
    ( spl2_1
    | spl2_5 ),
    inference(sat_conversion,[],[f64]) ).

fof(s14,plain,
    ( spl2_1
    | ~ spl2_3 ),
    inference(sat_conversion,[],[f65]) ).

fof(s15,plain,
    spl2_1,
    inference(rat,[],[s12,s6,s2,s1,s13,s14]) ).

fof(s16,plain,
    ~ spl2_2,
    inference(rat,[],[s10,s3,s5,s4,s15]) ).

fof(s17,plain,
    spl2_5,
    inference(rat,[],[s9,s16]) ).

fof(s18,plain,
    spl2_6,
    inference(rat,[],[s8,s16]) ).

fof(s19,plain,
    ~ spl2_3,
    inference(rat,[],[s7,s16]) ).

fof(s20,plain,
    $false,
    inference(rat,[],[s11,s18,s15,s19,s17]) ).

fof(f66,plain,
    $false,
    inference(avatar_sat_refutation,[],[s20]) ).

%------------------------------------------------------------------------------
%----ORIGINAL SYSTEM OUTPUT
% 0.00/0.12  % Problem    : SYN047+1 : TPTP v9.2.1. Released v2.0.0.
% 0.00/0.12  % Command    : run_vampire %s %d THM
% 0.17/0.33  % Computer   : n026.cluster.edu
% 0.17/0.33  % Model      : x86_64 x86_64
% 0.17/0.33  % CPU        : Intel(R) Xeon(R) CPU E5-2620 v4 @ 2.10GHz
% 0.17/0.33  % Memory     : 8042.1875MB
% 0.17/0.33  % OS         : Linux 3.10.0-693.el7.x86_64
% 0.17/0.33  % CPULimit   : 300
% 0.17/0.33  % WCLimit    : 300
% 0.17/0.33  % DateTime   : Fri May  1 05:45:08 EDT 2026
% 0.17/0.33  % CPUTime    : 
% 0.17/0.35  This is a FOF_THM_PRP problem
% 0.17/0.35  Running first-order theorem proving
% 0.17/0.35  Running /export/starexec/sandbox2/solver/bin/vampire --input_syntax tptp --proof tptp --output_axiom_names on --mode casc --cores 7 -m 16384 -t 300 /export/starexec/sandbox2/benchmark/theBenchmark.p
% 0.48/0.64  % (21701)Detected formulas, will run a generic FOF schedule.
% 0.48/0.75  % (21709)dis-21_1_sil=8000:lcm=predicate:random_seed=2778414578:st=5:avsq=on:i=129:avsqr=1,16:sd=3:aac=none:ep=RS:fsr=off:ss=included_3000 on theBenchmark for (3000ds/129Mi)
% 0.48/0.75  % (21709)First to succeed.
% 0.48/0.75  % (21709)Solution written to "/export/starexec/sandbox2/tmp/vampire-proof-21701"
% 0.48/0.78  % (21706)lrs+1010_1_to=lpo:sil=32000:sos=on:spb=goal_then_units:bce=on:random_seed=3435735097:i=109:sd=1:ins=1:gsp=on:ss=axioms_3000 on theBenchmark for (3000ds/109Mi)
% 0.48/0.78  % (21703)lrs+10_1_ncem=casc2026/models/loop8.pt:sil=128000:tgt=full:npcc=on:drc=off:sp=weighted_frequency:spb=goal:fd=preordered:foolp=on:random_seed=2235632100:i=141193_3000 on theBenchmark for (3000ds/141193Mi)
% 0.48/0.78  % (21704)lrs+11_1_ncem=casc2026/models/loop8.pt:sil=128000:npcc=on:lma=off:spb=units:urr=ec_only:bce=on:s2agt=64:updr=off:random_seed=4290703832:i=134677:sd=20:aac=none:nm=16:ss=included:sgt=10_3000 on theBenchmark for (3000ds/134677Mi)
% 0.48/0.78  % (21707)dis-1010_2:3_sil=16000:sp=reverse_frequency:random_seed=1168981606:i=119:av=off:ss=axioms_3000 on theBenchmark for (3000ds/119Mi)
% 0.48/0.78  % (21708)dis-1011_1_sil=16000:fde=unused:s2agt=70:random_seed=2724249075:s2a=on:i=139:gtg=position_3000 on theBenchmark for (3000ds/139Mi)
% 0.48/0.78  % (21705)lrs+1010_1_anc=all:sfv=off:to=kbo:ncem=casc2026/models/loop7.pt:sil=128000:npcc=on:prc=on:sos=all:bsr=unit_only:sac=on:random_seed=1839446276:i=141695:sd=1:nm=32:gsp=on:ss=included_3000 on theBenchmark for (3000ds/141695Mi)
% 0.48/0.78  % (21706)Also succeeded, but the first one will report.
% 0.48/0.78  % (21707)Also succeeded, but the first one will report.
% 0.48/0.78  % (21708)Also succeeded, but the first one will report.
% 0.72/0.90  % (21709)Refutation found. Thanks to Tanya!
% 0.72/0.90  % SZS status Theorem for theBenchmark
% 0.72/0.90  % SZS output start Proof for theBenchmark
% See solution above
% 0.72/0.90  % (21709)------------------------------
% 0.72/0.90  % (21709)Version: Vampire 5.0.1 (Release build, commit 1b9f22200 on 2026-04-29 16:18:29 +0200)
% 0.72/0.90  % (21709)Linked with Z3 4.14.0.0 3c47fd96cf5645d0c42b2c819d9e9a84380aa721 z3-4.8.4-9178-g3c47fd96c
% 0.72/0.90  % (21709)CaDiCaL version: 2.1.3
% 0.72/0.90  % (21709)Termination reason: Refutation
% 0.72/0.90  % (21709)Time elapsed: 0.001 s
% 0.72/0.90  % (21709)Peak memory usage: 81 MB
% 0.72/0.90  % (21709)Instructions burned: 1 (million)
% 0.72/0.90  % (21709)------------------------------
% 0.72/0.90  % (21709)------------------------------
% 0.72/0.90  % (21701)Success in time 0.265 s
%------------------------------------------------------------------------------

