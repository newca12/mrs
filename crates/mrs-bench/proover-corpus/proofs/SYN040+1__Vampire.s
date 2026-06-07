% Proof : Problems/SYN040+1.p
%------------------------------------------------------------------------------
% File     : Vampire---5.0.1
% Problem  : SYN040+1 : TPTP v9.2.1. Released v2.0.0.
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
% DateTime : Fri May  1 04:39:23 PM UTC 2026

% Result   : Theorem 0.99s 0.90s
% Output   : Refutation 0.99s
% Verified : 
% SZS Type : Refutation
%            Derivation depth      :   10
%            Number of leaves      :    3
% Syntax   : Number of formulae    :   21 (   8 unt;   2 def)
%            Number of atoms       :   54 (   0 equ)
%            Maximal formula atoms :    8 (   2 avg)
%            Number of connectives :   58 (  25   ~;  18   |;   6   &)
%                                         (   4 <=>;   4  =>;   0  <=;   1 <~>)
%            Maximal formula depth :    6 (   3 avg)
%            Maximal term depth    :    0 (   0 avg)
%            Number of predicates  :    5 (   4 usr;   5 prp; 0-0 aty)
%            Number of functors    :    0 (   0 usr;   0 con; --- aty)
%            Number of variables   :    0 (   0 sgn   0   !;   0   ?)

% Comments : 
%------------------------------------------------------------------------------
fof(f1,conjecture,
    ( ( p
     => q )
  <=> ( ~ q
     => ~ p ) ),
    file('/export/starexec/sandbox/benchmark/theBenchmark.p',pel1) ).

fof(f2,negated_conjecture,
    ~ ( ( p
       => q )
    <=> ( ~ q
       => ~ p ) ),
    inference(negated_conjecture,[status(cth)],[f1]) ).

fof(f3,plain,
    ( ( q
      | ~ p )
  <~> ( ~ p
      | q ) ),
    inference(ennf_transformation,[],[f2]) ).

fof(f4,plain,
    ( ( ( p
        & ~ q )
      | ( ~ q
        & p ) )
    & ( ~ p
      | q
      | q
      | ~ p ) ),
    inference(nnf_transformation,[],[f3]) ).

fof(f5,plain,
    ( ( ( p
        & ~ q )
      | ( ~ q
        & p ) )
    & ( ~ p
      | q
      | q
      | ~ p ) ),
    inference(flattening,[],[f4]) ).

fof(f6,plain,
    ( ~ p
    | q
    | q
    | ~ p ),
    inference(cnf_transformation,[],[f5]) ).

fof(f8,plain,
    ( ~ q
    | ~ q ),
    inference(cnf_transformation,[],[f5]) ).

fof(f9,plain,
    ( p
    | p ),
    inference(cnf_transformation,[],[f5]) ).

fof(f11,plain,
    ( ~ p
    | q ),
    inference(duplicate_literal_removal,[],[f6]) ).

fof(f12,plain,
    ~ q,
    inference(duplicate_literal_removal,[],[f8]) ).

fof(f13,plain,
    p,
    inference(duplicate_literal_removal,[],[f9]) ).

fof(f15,definition,
    ( spl0_1
  <=> q ),
    introduced(definition,[new_symbols(naming,[spl0_1])],[avatar_definition]) ).

fof(f18,definition,
    ( spl0_2
  <=> p ),
    introduced(definition,[new_symbols(naming,[spl0_2])],[avatar_definition]) ).

fof(f20,plain,
    ( spl0_1
    | ~ spl0_2 ),
    inference(avatar_split_clause,[],[f11,f18,f15]) ).

fof(f24,plain,
    ~ spl0_1,
    inference(avatar_split_clause,[],[f12,f15]) ).

fof(f25,plain,
    spl0_2,
    inference(avatar_split_clause,[],[f13,f18]) ).

fof(s1,plain,
    ( spl0_1
    | ~ spl0_2 ),
    inference(sat_conversion,[],[f20]) ).

fof(s3,plain,
    ~ spl0_1,
    inference(sat_conversion,[],[f24]) ).

fof(s4,plain,
    spl0_2,
    inference(sat_conversion,[],[f25]) ).

fof(s6,plain,
    $false,
    inference(rat,[],[s1,s4,s3]) ).

fof(f27,plain,
    $false,
    inference(avatar_sat_refutation,[],[s6]) ).

%------------------------------------------------------------------------------
%----ORIGINAL SYSTEM OUTPUT
% 0.00/0.12  % Problem    : SYN040+1 : TPTP v9.2.1. Released v2.0.0.
% 0.00/0.12  % Command    : run_vampire %s %d THM
% 0.16/0.33  % Computer   : n009.cluster.edu
% 0.16/0.33  % Model      : x86_64 x86_64
% 0.16/0.33  % CPU        : Intel(R) Xeon(R) CPU E5-2620 v4 @ 2.10GHz
% 0.16/0.33  % Memory     : 8042.1875MB
% 0.16/0.33  % OS         : Linux 3.10.0-693.el7.x86_64
% 0.16/0.33  % CPULimit   : 300
% 0.16/0.33  % WCLimit    : 300
% 0.16/0.33  % DateTime   : Fri May  1 05:41:56 EDT 2026
% 0.16/0.33  % CPUTime    : 
% 0.16/0.35  This is a FOF_THM_PRP problem
% 0.16/0.35  Running first-order theorem proving
% 0.16/0.35  Running /export/starexec/sandbox/solver/bin/vampire --input_syntax tptp --proof tptp --output_axiom_names on --mode casc --cores 7 -m 16384 -t 300 /export/starexec/sandbox/benchmark/theBenchmark.p
% 0.48/0.64  % (31419)Detected formulas, will run a generic FOF schedule.
% 0.50/0.75  % (31427)dis-21_1_sil=8000:lcm=predicate:random_seed=3456140190:st=5:avsq=on:i=129:avsqr=1,16:sd=3:aac=none:ep=RS:fsr=off:ss=included_3000 on theBenchmark for (3000ds/129Mi)
% 0.50/0.75  % (31427)First to succeed.
% 0.50/0.75  % (31427)Solution written to "/export/starexec/sandbox/tmp/vampire-proof-31419"
% 0.50/0.78  % (31421)lrs+10_1_ncem=casc2026/models/loop8.pt:sil=128000:tgt=full:npcc=on:drc=off:sp=weighted_frequency:spb=goal:fd=preordered:foolp=on:random_seed=3488292662:i=141193_3000 on theBenchmark for (3000ds/141193Mi)
% 0.50/0.78  % (31425)dis-1010_2:3_sil=16000:sp=reverse_frequency:random_seed=1766896507:i=119:av=off:ss=axioms_3000 on theBenchmark for (3000ds/119Mi)
% 0.50/0.78  % (31422)lrs+11_1_ncem=casc2026/models/loop8.pt:sil=128000:npcc=on:lma=off:spb=units:urr=ec_only:bce=on:s2agt=64:updr=off:random_seed=3076156429:i=134677:sd=20:aac=none:nm=16:ss=included:sgt=10_3000 on theBenchmark for (3000ds/134677Mi)
% 0.50/0.78  % (31423)lrs+1010_1_anc=all:sfv=off:to=kbo:ncem=casc2026/models/loop7.pt:sil=128000:npcc=on:prc=on:sos=all:bsr=unit_only:sac=on:random_seed=1438303418:i=141695:sd=1:nm=32:gsp=on:ss=included_3000 on theBenchmark for (3000ds/141695Mi)
% 0.50/0.78  % (31424)lrs+1010_1_to=lpo:sil=32000:sos=on:spb=goal_then_units:bce=on:random_seed=3352521841:i=109:sd=1:ins=1:gsp=on:ss=axioms_3000 on theBenchmark for (3000ds/109Mi)
% 0.50/0.78  % (31426)dis-1011_1_sil=16000:fde=unused:s2agt=70:random_seed=431722772:s2a=on:i=139:gtg=position_3000 on theBenchmark for (3000ds/139Mi)
% 0.50/0.78  % (31425)Also succeeded, but the first one will report.
% 0.50/0.78  % (31424)Also succeeded, but the first one will report.
% 0.50/0.78  % (31426)Also succeeded, but the first one will report.
% 0.99/0.90  % (31427)Refutation found. Thanks to Tanya!
% 0.99/0.90  % SZS status Theorem for theBenchmark
% 0.99/0.90  % SZS output start Proof for theBenchmark
% See solution above
% 0.99/0.90  % (31427)------------------------------
% 0.99/0.90  % (31427)Version: Vampire 5.0.1 (Release build, commit 1b9f22200 on 2026-04-29 16:18:29 +0200)
% 0.99/0.90  % (31427)Linked with Z3 4.14.0.0 3c47fd96cf5645d0c42b2c819d9e9a84380aa721 z3-4.8.4-9178-g3c47fd96c
% 0.99/0.90  % (31427)CaDiCaL version: 2.1.3
% 0.99/0.90  % (31427)Termination reason: Refutation
% 0.99/0.90  % (31427)Time elapsed: 0.001 s
% 0.99/0.90  % (31427)Peak memory usage: 80 MB
% 0.99/0.90  % (31427)------------------------------
% 0.99/0.90  % (31427)------------------------------
% 0.99/0.90  % (31419)Success in time 0.261 s
%------------------------------------------------------------------------------

