% Proof : Problems/SYN044+1.p
%------------------------------------------------------------------------------
% File     : Vampire---5.0.1
% Problem  : SYN044+1 : TPTP v9.2.1. Released v2.0.0.
% Transfm  : none
% Format   : tptp:raw
% Command  : run_vampire %s %d THM

% Computer : n019.cluster.edu
% Model    : x86_64 x86_64
% CPU      : Intel(R) Xeon(R) CPU E5-2620 v4 2.10GHz
% Memory   : 8042.1875MB
% OS       : Linux 3.10.0-693.el7.x86_64
% CPULimit : 300s
% WCLimit  : 300s
% DateTime : Fri May  1 04:39:23 PM UTC 2026

% Result   : Theorem 1.95s 0.81s
% Output   : Refutation 1.95s
% Verified : 
% SZS Type : Refutation
%            Derivation depth      :    8
%            Number of leaves      :    4
% Syntax   : Number of formulae    :   24 (   5 unt;   0 def)
%            Number of atoms       :   51 (   0 equ)
%            Maximal formula atoms :    4 (   2 avg)
%            Number of connectives :   46 (  19   ~;  18   |;   3   &)
%                                         (   2 <=>;   3  =>;   0  <=;   1 <~>)
%            Maximal formula depth :    4 (   3 avg)
%            Maximal term depth    :    0 (   0 avg)
%            Number of predicates  :    4 (   3 usr;   4 prp; 0-0 aty)
%            Number of functors    :    0 (   0 usr;   0 con; --- aty)
%            Number of variables   :    0 (   0   !;   0   ?)

% Comments : 
%------------------------------------------------------------------------------
fof(f1,axiom,
    ( q
   => r ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',pel10_1) ).

fof(f2,axiom,
    ( r
   => ( p
      & q ) ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',pel10_2) ).

fof(f3,axiom,
    ( p
   => ( q
      | r ) ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',pel10_3) ).

fof(f4,conjecture,
    ( p
  <=> q ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',pel10) ).

fof(f5,negated_conjecture,
    ~ ( p
    <=> q ),
    inference(negated_conjecture,[status(cth)],[f4]) ).

fof(f6,plain,
    ( p
  <~> q ),
    inference(ennf_transformation,[],[f5]) ).

fof(f7,plain,
    ( q
    | r
    | ~ p ),
    inference(ennf_transformation,[],[f3]) ).

fof(f8,plain,
    ( q
    | r
    | ~ p ),
    inference(flattening,[],[f7]) ).

fof(f9,plain,
    ( ( p
      & q )
    | ~ r ),
    inference(ennf_transformation,[],[f2]) ).

fof(f10,plain,
    ( r
    | ~ q ),
    inference(ennf_transformation,[],[f1]) ).

fof(f11,plain,
    ( ( ~ q
      | ~ p )
    & ( q
      | p ) ),
    inference(nnf_transformation,[],[f6]) ).

fof(f12,plain,
    ( p
    | q ),
    inference(cnf_transformation,[],[f11]) ).

fof(f13,plain,
    ( ~ p
    | ~ q ),
    inference(cnf_transformation,[],[f11]) ).

fof(f14,plain,
    ( q
    | r
    | ~ p ),
    inference(cnf_transformation,[],[f8]) ).

fof(f15,plain,
    ( q
    | ~ r ),
    inference(cnf_transformation,[],[f9]) ).

fof(f16,plain,
    ( p
    | ~ r ),
    inference(cnf_transformation,[],[f9]) ).

fof(f17,plain,
    ( r
    | ~ q ),
    inference(cnf_transformation,[],[f10]) ).

fof(f19,plain,
    ( ~ r
    | ~ q ),
    inference(resolution,[],[f16,f13]) ).

fof(f20,plain,
    ~ r,
    inference(forward_subsumption_resolution,[],[f19,f15]) ).

fof(f21,plain,
    ~ q,
    inference(forward_subsumption_resolution,[],[f17,f20]) ).

fof(f23,plain,
    ( q
    | ~ p ),
    inference(forward_subsumption_resolution,[],[f14,f15]) ).

fof(f24,plain,
    ~ p,
    inference(forward_subsumption_resolution,[],[f23,f13]) ).

fof(f26,plain,
    q,
    inference(resolution,[],[f24,f12]) ).

fof(f27,plain,
    $false,
    inference(forward_subsumption_resolution,[],[f26,f21]) ).

%------------------------------------------------------------------------------
%----ORIGINAL SYSTEM OUTPUT
% 0.00/0.11  % Problem    : SYN044+1 : TPTP v9.2.1. Released v2.0.0.
% 0.00/0.11  % Command    : run_vampire %s %d THM
% 0.11/0.31  % Computer   : n019.cluster.edu
% 0.11/0.31  % Model      : x86_64 x86_64
% 0.11/0.31  % CPU        : Intel(R) Xeon(R) CPU E5-2620 v4 @ 2.10GHz
% 0.11/0.31  % Memory     : 8042.1875MB
% 0.11/0.31  % OS         : Linux 3.10.0-693.el7.x86_64
% 0.11/0.31  % CPULimit   : 300
% 0.11/0.31  % WCLimit    : 300
% 0.11/0.31  % DateTime   : Fri May  1 05:42:44 EDT 2026
% 0.11/0.32  % CPUTime    : 
% 0.11/0.33  This is a FOF_THM_PRP problem
% 0.11/0.33  Running first-order theorem proving
% 0.11/0.33  Running /export/starexec/sandbox2/solver/bin/vampire --input_syntax tptp --proof tptp --output_axiom_names on --mode casc --cores 7 -m 16384 -t 300 /export/starexec/sandbox2/benchmark/theBenchmark.p
% 0.27/0.52  % (29720)Detected formulas, will run a generic FOF schedule.
% 0.93/0.64  % (29796)lrs+10_1_ncem=casc2026/models/loop8.pt:sil=128000:tgt=full:npcc=on:drc=off:sp=weighted_frequency:spb=goal:fd=preordered:foolp=on:random_seed=1177993844:i=141193_3000 on theBenchmark for (3000ds/141193Mi)
% 0.93/0.66  % (29800)lrs+1010_1_to=lpo:sil=32000:sos=on:spb=goal_then_units:bce=on:random_seed=2952134579:i=109:sd=1:ins=1:gsp=on:ss=axioms_3000 on theBenchmark for (3000ds/109Mi)
% 0.93/0.66  % (29797)lrs+11_1_ncem=casc2026/models/loop8.pt:sil=128000:npcc=on:lma=off:spb=units:urr=ec_only:bce=on:s2agt=64:updr=off:random_seed=3841985117:i=134677:sd=20:aac=none:nm=16:ss=included:sgt=10_3000 on theBenchmark for (3000ds/134677Mi)
% 0.93/0.66  % (29803)dis-1010_2:3_sil=16000:sp=reverse_frequency:random_seed=978908935:i=119:av=off:ss=axioms_3000 on theBenchmark for (3000ds/119Mi)
% 0.93/0.66  % (29799)lrs+1010_1_anc=all:sfv=off:to=kbo:ncem=casc2026/models/loop7.pt:sil=128000:npcc=on:prc=on:sos=all:bsr=unit_only:sac=on:random_seed=2901586462:i=141695:sd=1:nm=32:gsp=on:ss=included_3000 on theBenchmark for (3000ds/141695Mi)
% 0.93/0.66  % (29805)dis-1011_1_sil=16000:fde=unused:s2agt=70:random_seed=2318838742:s2a=on:i=139:gtg=position_3000 on theBenchmark for (3000ds/139Mi)
% 0.93/0.67  % (29800)Refutation not found, incomplete strategy
% 0.93/0.67  % (29800)------------------------------
% 0.93/0.67  % (29800)Version: Vampire 5.0.1 (Release build, commit 1b9f22200 on 2026-04-29 16:18:29 +0200)
% 0.93/0.67  % (29800)Linked with Z3 4.14.0.0 3c47fd96cf5645d0c42b2c819d9e9a84380aa721 z3-4.8.4-9178-g3c47fd96c
% 0.93/0.67  % (29800)CaDiCaL version: 2.1.3
% 0.93/0.67  % (29800)Termination reason: Refutation not found, incomplete strategy
% 0.93/0.67  % (29800)Time elapsed: 0.0000 s
% 0.93/0.67  % (29800)Peak memory usage: 80 MB
% 0.93/0.67  % (29803)First to succeed.
% 0.93/0.67  % (29805)Also succeeded, but the first one will report.
% 0.93/0.67  % (29803)Solution written to "/export/starexec/sandbox2/tmp/vampire-proof-29720"
% 0.93/0.68  % (29806)dis-21_1_sil=8000:lcm=predicate:random_seed=1210757386:st=5:avsq=on:i=129:avsqr=1,16:sd=3:aac=none:ep=RS:fsr=off:ss=included_3000 on theBenchmark for (3000ds/129Mi)
% 0.93/0.68  % (29806)Also succeeded, but the first one will report.
% 1.95/0.81  % (29803)Refutation found. Thanks to Tanya!
% 1.95/0.81  % SZS status Theorem for theBenchmark
% 1.95/0.81  % SZS output start Proof for theBenchmark
% See solution above
% 1.95/0.81  % (29803)------------------------------
% 1.95/0.81  % (29803)Version: Vampire 5.0.1 (Release build, commit 1b9f22200 on 2026-04-29 16:18:29 +0200)
% 1.95/0.81  % (29803)Linked with Z3 4.14.0.0 3c47fd96cf5645d0c42b2c819d9e9a84380aa721 z3-4.8.4-9178-g3c47fd96c
% 1.95/0.81  % (29803)CaDiCaL version: 2.1.3
% 1.95/0.81  % (29803)Termination reason: Refutation
% 1.95/0.81  % (29803)Time elapsed: 0.001 s
% 1.95/0.81  % (29803)Peak memory usage: 80 MB
% 1.95/0.81  % (29803)------------------------------
% 1.95/0.81  % (29803)------------------------------
% 1.95/0.81  % (29720)Success in time 0.29 s
%------------------------------------------------------------------------------

