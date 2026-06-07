% Proof : Problems/SYN049+1.p
%------------------------------------------------------------------------------
% File     : Vampire---5.0.1
% Problem  : SYN049+1 : TPTP v9.2.1. Released v2.0.0.
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
% DateTime : Fri May  1 04:39:26 PM UTC 2026

% Result   : Theorem 1.66s 0.74s
% Output   : Refutation 1.66s
% Verified : 
% SZS Type : Refutation
%            Derivation depth      :    7
%            Number of leaves      :    2
% Syntax   : Number of formulae    :   11 (   4 unt;   0 def)
%            Number of atoms       :   34 (   0 equ)
%            Maximal formula atoms :    8 (   3 avg)
%            Number of connectives :   37 (  14   ~;   6   |;  10   &)
%                                         (   0 <=>;   7  =>;   0  <=;   0 <~>)
%            Maximal formula depth :    9 (   5 avg)
%            Maximal term depth    :    2 (   1 avg)
%            Number of predicates  :    3 (   2 usr;   1 prp; 0-1 aty)
%            Number of functors    :    2 (   2 usr;   0 con; 1-1 aty)
%            Number of variables   :   20 (  12   !;   8   ?)

% Comments : 
%------------------------------------------------------------------------------
fof(f1,conjecture,
    ? [X0] :
    ! [X1,X2] :
      ( ( big_p(X1)
       => big_q(X2) )
     => ( big_p(X0)
       => big_q(X0) ) ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',pel19) ).

fof(f2,negated_conjecture,
    ~ ? [X0] :
      ! [X1,X2] :
        ( ( big_p(X1)
         => big_q(X2) )
       => ( big_p(X0)
         => big_q(X0) ) ),
    inference(negated_conjecture,[status(cth)],[f1]) ).

fof(f3,plain,
    ! [X0] :
    ? [X1,X2] :
      ( ~ big_q(X0)
      & big_p(X0)
      & ( big_q(X2)
        | ~ big_p(X1) ) ),
    inference(ennf_transformation,[],[f2]) ).

fof(f4,plain,
    ! [X0] :
    ? [X1,X2] :
      ( ~ big_q(X0)
      & big_p(X0)
      & ( big_q(X2)
        | ~ big_p(X1) ) ),
    inference(flattening,[],[f3]) ).

fof(f5,plain,
    ! [X0] :
      ( ? [X1,X2] :
          ( ~ big_q(X0)
          & big_p(X0)
          & ( big_q(X2)
            | ~ big_p(X1) ) )
     => ( ~ big_q(X0)
        & big_p(X0)
        & ( big_q(sK1(X0))
          | ~ big_p(sK0(X0)) ) ) ),
    introduced(definition,[],[skolem_symbol_introduction]) ).

fof(f6,plain,
    ! [X0] :
      ( ~ big_q(X0)
      & big_p(X0)
      & ( big_q(sK1(X0))
        | ~ big_p(sK0(X0)) ) ),
    inference(skolemisation,[status(esa),new_symbols(skolem,[sK0,sK1])],[f4,f5]) ).

fof(f7,plain,
    ! [X0] :
      ( big_q(sK1(X0))
      | ~ big_p(sK0(X0)) ),
    inference(cnf_transformation,[],[f6]) ).

fof(f8,plain,
    ! [X0] : big_p(X0),
    inference(cnf_transformation,[],[f6]) ).

fof(f9,plain,
    ! [X0] : ~ big_q(X0),
    inference(cnf_transformation,[],[f6]) ).

fof(f10,plain,
    ! [X0] : ~ big_p(sK0(X0)),
    inference(resolution,[],[f7,f9]) ).

fof(f11,plain,
    $false,
    inference(resolution,[],[f10,f8]) ).

%------------------------------------------------------------------------------
%----ORIGINAL SYSTEM OUTPUT
% 0.00/0.09  % Problem    : SYN049+1 : TPTP v9.2.1. Released v2.0.0.
% 0.00/0.09  % Command    : run_vampire %s %d THM
% 0.09/0.28  % Computer   : n002.cluster.edu
% 0.09/0.28  % Model      : x86_64 x86_64
% 0.09/0.28  % CPU        : Intel(R) Xeon(R) CPU E5-2620 v4 @ 2.10GHz
% 0.09/0.28  % Memory     : 8042.1875MB
% 0.09/0.28  % OS         : Linux 3.10.0-693.el7.x86_64
% 0.09/0.28  % CPULimit   : 300
% 0.09/0.28  % WCLimit    : 300
% 0.09/0.28  % DateTime   : Fri May  1 05:43:46 EDT 2026
% 0.09/0.28  % CPUTime    : 
% 0.09/0.30  This is a FOF_THM_RFO_NEQ problem
% 0.09/0.30  Running first-order theorem proving
% 0.09/0.30  Running /export/starexec/sandbox2/solver/bin/vampire --input_syntax tptp --proof tptp --output_axiom_names on --mode casc --cores 7 -m 16384 -t 300 /export/starexec/sandbox2/benchmark/theBenchmark.p
% 0.27/0.49  % (8137)Detected formulas, will run a generic FOF schedule.
% 0.61/0.59  % (8145)dis-21_1_sil=8000:lcm=predicate:random_seed=169043249:st=5:avsq=on:i=129:avsqr=1,16:sd=3:aac=none:ep=RS:fsr=off:ss=included_3000 on theBenchmark for (3000ds/129Mi)
% 0.61/0.59  % (8145)First to succeed.
% 0.61/0.59  % (8145)Solution written to "/export/starexec/sandbox2/tmp/vampire-proof-8137"
% 0.61/0.61  % (8143)dis-1010_2:3_sil=16000:sp=reverse_frequency:random_seed=1657044262:i=119:av=off:ss=axioms_3000 on theBenchmark for (3000ds/119Mi)
% 0.61/0.61  % (8143)Also succeeded, but the first one will report.
% 0.61/0.61  % (8139)lrs+10_1_ncem=casc2026/models/loop8.pt:sil=128000:tgt=full:npcc=on:drc=off:sp=weighted_frequency:spb=goal:fd=preordered:foolp=on:random_seed=4118585989:i=141193_3000 on theBenchmark for (3000ds/141193Mi)
% 0.61/0.61  % (8141)lrs+1010_1_anc=all:sfv=off:to=kbo:ncem=casc2026/models/loop7.pt:sil=128000:npcc=on:prc=on:sos=all:bsr=unit_only:sac=on:random_seed=1032052799:i=141695:sd=1:nm=32:gsp=on:ss=included_3000 on theBenchmark for (3000ds/141695Mi)
% 0.61/0.62  % (8144)dis-1011_1_sil=16000:fde=unused:s2agt=70:random_seed=4113009523:s2a=on:i=139:gtg=position_3000 on theBenchmark for (3000ds/139Mi)
% 0.61/0.62  % (8140)lrs+11_1_ncem=casc2026/models/loop8.pt:sil=128000:npcc=on:lma=off:spb=units:urr=ec_only:bce=on:s2agt=64:updr=off:random_seed=2832656874:i=134677:sd=20:aac=none:nm=16:ss=included:sgt=10_3000 on theBenchmark for (3000ds/134677Mi)
% 0.61/0.62  % (8142)lrs+1010_1_to=lpo:sil=32000:sos=on:spb=goal_then_units:bce=on:random_seed=1936323478:i=109:sd=1:ins=1:gsp=on:ss=axioms_3000 on theBenchmark for (3000ds/109Mi)
% 0.61/0.62  % (8144)Also succeeded, but the first one will report.
% 0.61/0.62  % (8142)Also succeeded, but the first one will report.
% 1.66/0.74  % (8145)Refutation found. Thanks to Tanya!
% 1.66/0.74  % SZS status Theorem for theBenchmark
% 1.66/0.74  % SZS output start Proof for theBenchmark
% See solution above
% 1.66/0.74  % (8145)------------------------------
% 1.66/0.74  % (8145)Version: Vampire 5.0.1 (Release build, commit 1b9f22200 on 2026-04-29 16:18:29 +0200)
% 1.66/0.74  % (8145)Linked with Z3 4.14.0.0 3c47fd96cf5645d0c42b2c819d9e9a84380aa721 z3-4.8.4-9178-g3c47fd96c
% 1.66/0.74  % (8145)CaDiCaL version: 2.1.3
% 1.66/0.74  % (8145)Termination reason: Refutation
% 1.66/0.74  % (8145)Time elapsed: 0.001 s
% 1.66/0.74  % (8145)Peak memory usage: 80 MB
% 1.66/0.74  % (8145)------------------------------
% 1.66/0.74  % (8145)------------------------------
% 1.66/0.74  % (8137)Success in time 0.243 s
%------------------------------------------------------------------------------

