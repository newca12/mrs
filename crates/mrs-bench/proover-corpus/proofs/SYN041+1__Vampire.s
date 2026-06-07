% Proof : Problems/SYN041+1.p
%------------------------------------------------------------------------------
% File     : Vampire---5.0.1
% Problem  : SYN041+1 : TPTP v9.2.1. Released v2.0.0.
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

% Result   : Theorem 0.74s 0.90s
% Output   : Refutation 0.74s
% Verified : 
% SZS Type : Refutation
%            Derivation depth      :    5
%            Number of leaves      :    1
% Syntax   : Number of formulae    :    7 (   3 unt;   0 def)
%            Number of atoms       :   19 (   0 equ)
%            Maximal formula atoms :    4 (   2 avg)
%            Number of connectives :   20 (   8   ~;   0   |;   6   &)
%                                         (   0 <=>;   6  =>;   0  <=;   0 <~>)
%            Maximal formula depth :    5 (   3 avg)
%            Maximal term depth    :    0 (   0 avg)
%            Number of predicates  :    3 (   2 usr;   3 prp; 0-0 aty)
%            Number of functors    :    0 (   0 usr;   0 con; --- aty)
%            Number of variables   :    0 (   0   !;   0   ?)

% Comments : 
%------------------------------------------------------------------------------
fof(f1,conjecture,
    ( ~ ( p
       => q )
   => ( q
     => p ) ),
    file('/export/starexec/sandbox/benchmark/theBenchmark.p',pel3) ).

fof(f2,negated_conjecture,
    ~ ( ~ ( p
         => q )
     => ( q
       => p ) ),
    inference(negated_conjecture,[status(cth)],[f1]) ).

fof(f3,plain,
    ( ~ p
    & q
    & ~ q
    & p ),
    inference(ennf_transformation,[],[f2]) ).

fof(f4,plain,
    ( ~ p
    & q
    & ~ q
    & p ),
    inference(flattening,[],[f3]) ).

fof(f6,plain,
    ~ q,
    inference(cnf_transformation,[],[f4]) ).

fof(f7,plain,
    q,
    inference(cnf_transformation,[],[f4]) ).

fof(f9,plain,
    $false,
    inference(resolution,[],[f7,f6]) ).

%------------------------------------------------------------------------------
%----ORIGINAL SYSTEM OUTPUT
% 0.00/0.12  % Problem    : SYN041+1 : TPTP v9.2.1. Released v2.0.0.
% 0.00/0.12  % Command    : run_vampire %s %d THM
% 0.14/0.33  % Computer   : n009.cluster.edu
% 0.14/0.33  % Model      : x86_64 x86_64
% 0.14/0.33  % CPU        : Intel(R) Xeon(R) CPU E5-2620 v4 @ 2.10GHz
% 0.14/0.33  % Memory     : 8042.1875MB
% 0.14/0.33  % OS         : Linux 3.10.0-693.el7.x86_64
% 0.14/0.33  % CPULimit   : 300
% 0.14/0.33  % WCLimit    : 300
% 0.14/0.33  % DateTime   : Fri May  1 05:42:11 EDT 2026
% 0.14/0.33  % CPUTime    : 
% 0.14/0.35  This is a FOF_THM_PRP problem
% 0.14/0.35  Running first-order theorem proving
% 0.14/0.35  Running /export/starexec/sandbox/solver/bin/vampire --input_syntax tptp --proof tptp --output_axiom_names on --mode casc --cores 7 -m 16384 -t 300 /export/starexec/sandbox/benchmark/theBenchmark.p
% 0.48/0.64  % (32045)Detected formulas, will run a generic FOF schedule.
% 0.50/0.75  % (32053)dis-21_1_sil=8000:lcm=predicate:random_seed=421297774:st=5:avsq=on:i=129:avsqr=1,16:sd=3:aac=none:ep=RS:fsr=off:ss=included_3000 on theBenchmark for (3000ds/129Mi)
% 0.50/0.76  % (32053)First to succeed.
% 0.50/0.76  % (32053)Solution written to "/export/starexec/sandbox/tmp/vampire-proof-32045"
% 0.50/0.78  % (32051)dis-1010_2:3_sil=16000:sp=reverse_frequency:random_seed=769110222:i=119:av=off:ss=axioms_3000 on theBenchmark for (3000ds/119Mi)
% 0.50/0.78  % (32049)lrs+1010_1_anc=all:sfv=off:to=kbo:ncem=casc2026/models/loop7.pt:sil=128000:npcc=on:prc=on:sos=all:bsr=unit_only:sac=on:random_seed=903781166:i=141695:sd=1:nm=32:gsp=on:ss=included_3000 on theBenchmark for (3000ds/141695Mi)
% 0.50/0.78  % (32047)lrs+10_1_ncem=casc2026/models/loop8.pt:sil=128000:tgt=full:npcc=on:drc=off:sp=weighted_frequency:spb=goal:fd=preordered:foolp=on:random_seed=4052216251:i=141193_3000 on theBenchmark for (3000ds/141193Mi)
% 0.50/0.78  % (32050)lrs+1010_1_to=lpo:sil=32000:sos=on:spb=goal_then_units:bce=on:random_seed=3979209787:i=109:sd=1:ins=1:gsp=on:ss=axioms_3000 on theBenchmark for (3000ds/109Mi)
% 0.50/0.78  % (32052)dis-1011_1_sil=16000:fde=unused:s2agt=70:random_seed=1328988045:s2a=on:i=139:gtg=position_3000 on theBenchmark for (3000ds/139Mi)
% 0.50/0.78  % (32051)Also succeeded, but the first one will report.
% 0.50/0.78  % (32050)Also succeeded, but the first one will report.
% 0.50/0.78  % (32052)Also succeeded, but the first one will report.
% 0.50/0.80  % (32048)lrs+11_1_ncem=casc2026/models/loop8.pt:sil=128000:npcc=on:lma=off:spb=units:urr=ec_only:bce=on:s2agt=64:updr=off:random_seed=2602169399:i=134677:sd=20:aac=none:nm=16:ss=included:sgt=10_3000 on theBenchmark for (3000ds/134677Mi)
% 0.74/0.90  % (32053)Refutation found. Thanks to Tanya!
% 0.74/0.90  % SZS status Theorem for theBenchmark
% 0.74/0.90  % SZS output start Proof for theBenchmark
% See solution above
% 0.74/0.90  % (32053)------------------------------
% 0.74/0.90  % (32053)Version: Vampire 5.0.1 (Release build, commit 1b9f22200 on 2026-04-29 16:18:29 +0200)
% 0.74/0.90  % (32053)Linked with Z3 4.14.0.0 3c47fd96cf5645d0c42b2c819d9e9a84380aa721 z3-4.8.4-9178-g3c47fd96c
% 0.74/0.90  % (32053)CaDiCaL version: 2.1.3
% 0.74/0.90  % (32053)Termination reason: Refutation
% 0.74/0.90  % (32053)Time elapsed: 0.001 s
% 0.74/0.90  % (32053)Peak memory usage: 80 MB
% 0.74/0.90  % (32053)------------------------------
% 0.74/0.90  % (32053)------------------------------
% 0.74/0.90  % (32045)Success in time 0.261 s
%------------------------------------------------------------------------------

