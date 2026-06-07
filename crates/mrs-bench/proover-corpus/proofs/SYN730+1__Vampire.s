% Proof : Problems/SYN730+1.p
%------------------------------------------------------------------------------
% File     : Vampire---5.0.1
% Problem  : SYN730+1 : TPTP v9.2.1. Released v2.5.0.
% Transfm  : none
% Format   : tptp:raw
% Command  : run_vampire %s %d THM

% Computer : n028.cluster.edu
% Model    : x86_64 x86_64
% CPU      : Intel(R) Xeon(R) CPU E5-2620 v4 2.10GHz
% Memory   : 8042.1875MB
% OS       : Linux 3.10.0-693.el7.x86_64
% CPULimit : 300s
% WCLimit  : 300s
% DateTime : Fri May  1 04:49:29 PM UTC 2026

% Result   : Theorem 0.49s 0.93s
% Output   : Refutation 0.49s
% Verified : 
% SZS Type : Refutation
%            Derivation depth      :    6
%            Number of leaves      :    2
% Syntax   : Number of formulae    :    9 (   3 unt;   0 def)
%            Number of atoms       :   23 (   0 equ)
%            Maximal formula atoms :    6 (   2 avg)
%            Number of connectives :   20 (   6   ~;   7   |;   4   &)
%                                         (   0 <=>;   3  =>;   0  <=;   0 <~>)
%            Maximal formula depth :    7 (   4 avg)
%            Maximal term depth    :    3 (   1 avg)
%            Number of predicates  :    2 (   1 usr;   1 prp; 0-3 aty)
%            Number of functors    :    4 (   4 usr;   1 con; 0-1 aty)
%            Number of variables   :   18 (  12   !;   6   ?)

% Comments : 
%------------------------------------------------------------------------------
fof(f1,conjecture,
    ? [X0] :
    ! [X1] :
    ? [X2] :
      ( ( p(a,X1,h(X1))
        | p(X0,X1,k(X1)) )
     => p(X0,X1,X2) ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',thm75) ).

fof(f2,negated_conjecture,
    ~ ? [X0] :
      ! [X1] :
      ? [X2] :
        ( ( p(a,X1,h(X1))
          | p(X0,X1,k(X1)) )
       => p(X0,X1,X2) ),
    inference(negated_conjecture,[status(cth)],[f1]) ).

fof(f3,plain,
    ! [X0] :
    ? [X1] :
    ! [X2] :
      ( ~ p(X0,X1,X2)
      & ( p(a,X1,h(X1))
        | p(X0,X1,k(X1)) ) ),
    inference(ennf_transformation,[],[f2]) ).

fof(f4,plain,
    ! [X0] :
      ( ? [X1] :
        ! [X2] :
          ( ~ p(X0,X1,X2)
          & ( p(a,X1,h(X1))
            | p(X0,X1,k(X1)) ) )
     => ! [X2] :
          ( ~ p(X0,sK0(X0),X2)
          & ( p(a,sK0(X0),h(sK0(X0)))
            | p(X0,sK0(X0),k(sK0(X0))) ) ) ),
    introduced(definition,[],[skolem_symbol_introduction]) ).

fof(f5,plain,
    ! [X0,X2] :
      ( ~ p(X0,sK0(X0),X2)
      & ( p(a,sK0(X0),h(sK0(X0)))
        | p(X0,sK0(X0),k(sK0(X0))) ) ),
    inference(skolemisation,[status(esa),new_symbols(skolem,[sK0])],[f3,f4]) ).

fof(f6,plain,
    ! [X0] :
      ( p(a,sK0(X0),h(sK0(X0)))
      | p(X0,sK0(X0),k(sK0(X0))) ),
    inference(cnf_transformation,[],[f5]) ).

fof(f7,plain,
    ! [X2,X0] : ~ p(X0,sK0(X0),X2),
    inference(cnf_transformation,[],[f5]) ).

fof(f8,plain,
    p(a,sK0(a),k(sK0(a))),
    inference(resolution,[],[f6,f7]) ).

fof(f9,plain,
    $false,
    inference(forward_subsumption_resolution,[],[f8,f7]) ).

%------------------------------------------------------------------------------
%----ORIGINAL SYSTEM OUTPUT
% 0.00/0.12  % Problem    : SYN730+1 : TPTP v9.2.1. Released v2.5.0.
% 0.00/0.12  % Command    : run_vampire %s %d THM
% 0.15/0.32  % Computer   : n028.cluster.edu
% 0.15/0.32  % Model      : x86_64 x86_64
% 0.15/0.32  % CPU        : Intel(R) Xeon(R) CPU E5-2620 v4 @ 2.10GHz
% 0.15/0.32  % Memory     : 8042.1875MB
% 0.15/0.32  % OS         : Linux 3.10.0-693.el7.x86_64
% 0.15/0.32  % CPULimit   : 300
% 0.15/0.32  % WCLimit    : 300
% 0.15/0.32  % DateTime   : Fri May  1 06:31:45 EDT 2026
% 0.15/0.33  % CPUTime    : 
% 0.15/0.34  This is a FOF_THM_RFO_NEQ problem
% 0.15/0.35  Running first-order theorem proving
% 0.15/0.35  Running /export/starexec/sandbox2/solver/bin/vampire --input_syntax tptp --proof tptp --output_axiom_names on --mode casc --cores 7 -m 16384 -t 300 /export/starexec/sandbox2/benchmark/theBenchmark.p
% 0.42/0.64  % (31315)Detected formulas, will run a generic FOF schedule.
% 0.49/0.77  % (31321)dis-1010_2:3_sil=16000:sp=reverse_frequency:random_seed=3492119706:i=119:av=off:ss=axioms_3000 on theBenchmark for (3000ds/119Mi)
% 0.49/0.77  % (31323)dis-21_1_sil=8000:lcm=predicate:random_seed=721817133:st=5:avsq=on:i=129:avsqr=1,16:sd=3:aac=none:ep=RS:fsr=off:ss=included_3000 on theBenchmark for (3000ds/129Mi)
% 0.49/0.77  % (31319)lrs+1010_1_anc=all:sfv=off:to=kbo:ncem=casc2026/models/loop7.pt:sil=128000:npcc=on:prc=on:sos=all:bsr=unit_only:sac=on:random_seed=1745529513:i=141695:sd=1:nm=32:gsp=on:ss=included_3000 on theBenchmark for (3000ds/141695Mi)
% 0.49/0.77  % (31322)dis-1011_1_sil=16000:fde=unused:s2agt=70:random_seed=1855081419:s2a=on:i=139:gtg=position_3000 on theBenchmark for (3000ds/139Mi)
% 0.49/0.77  % (31317)lrs+10_1_ncem=casc2026/models/loop8.pt:sil=128000:tgt=full:npcc=on:drc=off:sp=weighted_frequency:spb=goal:fd=preordered:foolp=on:random_seed=2524815434:i=141193_3000 on theBenchmark for (3000ds/141193Mi)
% 0.49/0.77  % (31320)lrs+1010_1_to=lpo:sil=32000:sos=on:spb=goal_then_units:bce=on:random_seed=1719498762:i=109:sd=1:ins=1:gsp=on:ss=axioms_3000 on theBenchmark for (3000ds/109Mi)
% 0.49/0.77  % (31318)lrs+11_1_ncem=casc2026/models/loop8.pt:sil=128000:npcc=on:lma=off:spb=units:urr=ec_only:bce=on:s2agt=64:updr=off:random_seed=2946149651:i=134677:sd=20:aac=none:nm=16:ss=included:sgt=10_3000 on theBenchmark for (3000ds/134677Mi)
% 0.49/0.77  % (31322)Also succeeded, but the first one will report.
% 0.49/0.77  % (31321)Also succeeded, but the first one will report.
% 0.49/0.77  % (31323)Also succeeded, but the first one will report.
% 0.49/0.77  % (31320)First to succeed.
% 0.49/0.77  % (31320)Solution written to "/export/starexec/sandbox2/tmp/vampire-proof-31315"
% 0.49/0.93  % (31320)Refutation found. Thanks to Tanya!
% 0.49/0.93  % SZS status Theorem for theBenchmark
% 0.49/0.93  % SZS output start Proof for theBenchmark
% See solution above
% 0.49/0.93  % (31320)------------------------------
% 0.49/0.93  % (31320)Version: Vampire 5.0.1 (Release build, commit 1b9f22200 on 2026-04-29 16:18:29 +0200)
% 0.49/0.93  % (31320)Linked with Z3 4.14.0.0 3c47fd96cf5645d0c42b2c819d9e9a84380aa721 z3-4.8.4-9178-g3c47fd96c
% 0.49/0.93  % (31320)CaDiCaL version: 2.1.3
% 0.49/0.93  % (31320)Termination reason: Refutation
% 0.49/0.93  % (31320)Time elapsed: 0.001 s
% 0.49/0.93  % (31320)Peak memory usage: 80 MB
% 0.49/0.93  % (31320)------------------------------
% 0.49/0.93  % (31320)------------------------------
% 0.49/0.93  % (31315)Success in time 0.296 s
%------------------------------------------------------------------------------

