% Proof : Problems/SYN048+1.p
%------------------------------------------------------------------------------
% File     : Vampire---5.0.1
% Problem  : SYN048+1 : TPTP v9.2.1. Released v2.0.0.
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
% DateTime : Fri May  1 04:39:25 PM UTC 2026

% Result   : Theorem 0.51s 0.95s
% Output   : Refutation 0.51s
% Verified : 
% SZS Type : Refutation
%            Derivation depth      :    5
%            Number of leaves      :    2
% Syntax   : Number of formulae    :    8 (   3 unt;   0 def)
%            Number of atoms       :   15 (   0 equ)
%            Maximal formula atoms :    4 (   1 avg)
%            Number of connectives :   13 (   6   ~;   0   |;   4   &)
%                                         (   0 <=>;   3  =>;   0  <=;   0 <~>)
%            Maximal formula depth :    6 (   4 avg)
%            Maximal term depth    :    2 (   1 avg)
%            Number of predicates  :    2 (   1 usr;   1 prp; 0-1 aty)
%            Number of functors    :    1 (   1 usr;   0 con; 1-1 aty)
%            Number of variables   :   11 (   7   !;   4   ?)

% Comments : 
%------------------------------------------------------------------------------
fof(f1,conjecture,
    ? [X0] :
    ! [X1] :
      ( big_f(X0)
     => big_f(X1) ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',pel18) ).

fof(f2,negated_conjecture,
    ~ ? [X0] :
      ! [X1] :
        ( big_f(X0)
       => big_f(X1) ),
    inference(negated_conjecture,[status(cth)],[f1]) ).

fof(f3,plain,
    ! [X0] :
    ? [X1] :
      ( ~ big_f(X1)
      & big_f(X0) ),
    inference(ennf_transformation,[],[f2]) ).

fof(f4,plain,
    ! [X0] :
      ( ? [X1] :
          ( ~ big_f(X1)
          & big_f(X0) )
     => ( ~ big_f(sK0(X0))
        & big_f(X0) ) ),
    introduced(definition,[],[skolem_symbol_introduction]) ).

fof(f5,plain,
    ! [X0] :
      ( ~ big_f(sK0(X0))
      & big_f(X0) ),
    inference(skolemisation,[status(esa),new_symbols(skolem,[sK0])],[f3,f4]) ).

fof(f6,plain,
    ! [X0] : big_f(X0),
    inference(cnf_transformation,[],[f5]) ).

fof(f7,plain,
    ! [X0] : ~ big_f(sK0(X0)),
    inference(cnf_transformation,[],[f5]) ).

fof(f8,plain,
    $false,
    inference(forward_subsumption_resolution,[],[f7,f6]) ).

%------------------------------------------------------------------------------
%----ORIGINAL SYSTEM OUTPUT
% 0.00/0.12  % Problem    : SYN048+1 : TPTP v9.2.1. Released v2.0.0.
% 0.00/0.12  % Command    : run_vampire %s %d THM
% 0.15/0.33  % Computer   : n007.cluster.edu
% 0.15/0.33  % Model      : x86_64 x86_64
% 0.15/0.33  % CPU        : Intel(R) Xeon(R) CPU E5-2620 v4 @ 2.10GHz
% 0.15/0.33  % Memory     : 8042.1875MB
% 0.15/0.33  % OS         : Linux 3.10.0-693.el7.x86_64
% 0.15/0.33  % CPULimit   : 300
% 0.15/0.33  % WCLimit    : 300
% 0.15/0.33  % DateTime   : Fri May  1 05:42:54 EDT 2026
% 0.15/0.33  % CPUTime    : 
% 0.15/0.35  This is a FOF_THM_RFO_NEQ problem
% 0.15/0.35  Running first-order theorem proving
% 0.15/0.35  Running /export/starexec/sandbox2/solver/bin/vampire --input_syntax tptp --proof tptp --output_axiom_names on --mode casc --cores 7 -m 16384 -t 300 /export/starexec/sandbox2/benchmark/theBenchmark.p
% 0.47/0.66  % (14672)Detected formulas, will run a generic FOF schedule.
% 0.51/0.79  % (14687)dis-1010_2:3_sil=16000:sp=reverse_frequency:random_seed=2206318834:i=119:av=off:ss=axioms_3000 on theBenchmark for (3000ds/119Mi)
% 0.51/0.79  % (14687)First to succeed.
% 0.51/0.79  % (14687)Solution written to "/export/starexec/sandbox2/tmp/vampire-proof-14672"
% 0.51/0.85  % (14686)lrs+1010_1_to=lpo:sil=32000:sos=on:spb=goal_then_units:bce=on:random_seed=526194669:i=109:sd=1:ins=1:gsp=on:ss=axioms_3000 on theBenchmark for (3000ds/109Mi)
% 0.51/0.85  % (14683)lrs+10_1_ncem=casc2026/models/loop8.pt:sil=128000:tgt=full:npcc=on:drc=off:sp=weighted_frequency:spb=goal:fd=preordered:foolp=on:random_seed=1752064853:i=141193_3000 on theBenchmark for (3000ds/141193Mi)
% 0.51/0.85  % (14686)Also succeeded, but the first one will report.
% 0.51/0.86  % (14684)lrs+11_1_ncem=casc2026/models/loop8.pt:sil=128000:npcc=on:lma=off:spb=units:urr=ec_only:bce=on:s2agt=64:updr=off:random_seed=1624170975:i=134677:sd=20:aac=none:nm=16:ss=included:sgt=10_3000 on theBenchmark for (3000ds/134677Mi)
% 0.51/0.86  % (14688)dis-1011_1_sil=16000:fde=unused:s2agt=70:random_seed=1645854741:s2a=on:i=139:gtg=position_3000 on theBenchmark for (3000ds/139Mi)
% 0.51/0.86  % (14685)lrs+1010_1_anc=all:sfv=off:to=kbo:ncem=casc2026/models/loop7.pt:sil=128000:npcc=on:prc=on:sos=all:bsr=unit_only:sac=on:random_seed=4203421139:i=141695:sd=1:nm=32:gsp=on:ss=included_3000 on theBenchmark for (3000ds/141695Mi)
% 0.51/0.86  % (14689)dis-21_1_sil=8000:lcm=predicate:random_seed=4183822031:st=5:avsq=on:i=129:avsqr=1,16:sd=3:aac=none:ep=RS:fsr=off:ss=included_3000 on theBenchmark for (3000ds/129Mi)
% 0.51/0.86  % (14688)Also succeeded, but the first one will report.
% 0.51/0.86  % (14689)Also succeeded, but the first one will report.
% 0.51/0.95  % (14687)Refutation found. Thanks to Tanya!
% 0.51/0.95  % SZS status Theorem for theBenchmark
% 0.51/0.95  % SZS output start Proof for theBenchmark
% See solution above
% 0.51/0.95  % (14687)------------------------------
% 0.51/0.95  % (14687)Version: Vampire 5.0.1 (Release build, commit 1b9f22200 on 2026-04-29 16:18:29 +0200)
% 0.51/0.95  % (14687)Linked with Z3 4.14.0.0 3c47fd96cf5645d0c42b2c819d9e9a84380aa721 z3-4.8.4-9178-g3c47fd96c
% 0.51/0.95  % (14687)CaDiCaL version: 2.1.3
% 0.51/0.95  % (14687)Termination reason: Refutation
% 0.51/0.95  % (14687)Time elapsed: 0.001 s
% 0.51/0.95  % (14687)Peak memory usage: 80 MB
% 0.51/0.95  % (14687)------------------------------
% 0.51/0.95  % (14687)------------------------------
% 0.51/0.95  % (14672)Success in time 0.291 s
%------------------------------------------------------------------------------

