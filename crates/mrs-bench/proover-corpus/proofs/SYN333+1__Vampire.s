% Proof : Problems/SYN333+1.p
%------------------------------------------------------------------------------
% File     : Vampire---5.0.1
% Problem  : SYN333+1 : TPTP v9.2.1. Released v2.0.0.
% Transfm  : none
% Format   : tptp:raw
% Command  : run_vampire %s %d THM

% Computer : n027.cluster.edu
% Model    : x86_64 x86_64
% CPU      : Intel(R) Xeon(R) CPU E5-2620 v4 2.10GHz
% Memory   : 8042.1875MB
% OS       : Linux 3.10.0-693.el7.x86_64
% CPULimit : 300s
% WCLimit  : 300s
% DateTime : Fri May  1 04:40:35 PM UTC 2026

% Result   : Theorem 0.72s 0.90s
% Output   : Refutation 0.72s
% Verified : 
% SZS Type : Refutation
%            Derivation depth      :   13
%            Number of leaves      :    2
% Syntax   : Number of formulae    :   17 (   3 unt;   0 def)
%            Number of atoms       :   84 (   0 equ)
%            Maximal formula atoms :   14 (   4 avg)
%            Number of connectives :  120 (  53   ~;  39   |;  23   &)
%                                         (   0 <=>;   5  =>;   0  <=;   0 <~>)
%            Maximal formula depth :   11 (   7 avg)
%            Maximal term depth    :    3 (   1 avg)
%            Number of predicates  :    3 (   2 usr;   1 prp; 0-2 aty)
%            Number of functors    :    1 (   1 usr;   0 con; 2-2 aty)
%            Number of variables   :   37 (  30   !;   7   ?)

% Comments : 
%------------------------------------------------------------------------------
fof(f1,conjecture,
    ? [X0,X1] :
    ! [X2] :
      ( big_f(X0,X1)
     => ( big_f(X1,X2)
        & big_f(X2,X2)
        & ( ( big_f(X0,X1)
            & big_g(X0,X1) )
         => ( big_g(X0,X2)
            & big_g(X2,X2) ) ) ) ),
    file('/export/starexec/sandbox/benchmark/theBenchmark.p',church_46_14_5) ).

fof(f2,negated_conjecture,
    ~ ? [X0,X1] :
      ! [X2] :
        ( big_f(X0,X1)
       => ( big_f(X1,X2)
          & big_f(X2,X2)
          & ( ( big_f(X0,X1)
              & big_g(X0,X1) )
           => ( big_g(X0,X2)
              & big_g(X2,X2) ) ) ) ),
    inference(negated_conjecture,[status(cth)],[f1]) ).

fof(f3,plain,
    ! [X0,X1] :
    ? [X2] :
      ( ( ~ big_f(X1,X2)
        | ~ big_f(X2,X2)
        | ( ( ~ big_g(X0,X2)
            | ~ big_g(X2,X2) )
          & big_f(X0,X1)
          & big_g(X0,X1) ) )
      & big_f(X0,X1) ),
    inference(ennf_transformation,[],[f2]) ).

fof(f4,plain,
    ! [X0,X1] :
    ? [X2] :
      ( ( ~ big_f(X1,X2)
        | ~ big_f(X2,X2)
        | ( ( ~ big_g(X0,X2)
            | ~ big_g(X2,X2) )
          & big_f(X0,X1)
          & big_g(X0,X1) ) )
      & big_f(X0,X1) ),
    inference(flattening,[],[f3]) ).

fof(f5,plain,
    ! [X0,X1] :
      ( ? [X2] :
          ( ( ~ big_f(X1,X2)
            | ~ big_f(X2,X2)
            | ( ( ~ big_g(X0,X2)
                | ~ big_g(X2,X2) )
              & big_f(X0,X1)
              & big_g(X0,X1) ) )
          & big_f(X0,X1) )
     => ( ( ~ big_f(X1,sK0(X0,X1))
          | ~ big_f(sK0(X0,X1),sK0(X0,X1))
          | ( ( ~ big_g(X0,sK0(X0,X1))
              | ~ big_g(sK0(X0,X1),sK0(X0,X1)) )
            & big_f(X0,X1)
            & big_g(X0,X1) ) )
        & big_f(X0,X1) ) ),
    introduced(definition,[],[skolem_symbol_introduction]) ).

fof(f6,plain,
    ! [X0,X1] :
      ( ( ~ big_f(X1,sK0(X0,X1))
        | ~ big_f(sK0(X0,X1),sK0(X0,X1))
        | ( ( ~ big_g(X0,sK0(X0,X1))
            | ~ big_g(sK0(X0,X1),sK0(X0,X1)) )
          & big_f(X0,X1)
          & big_g(X0,X1) ) )
      & big_f(X0,X1) ),
    inference(skolemisation,[status(esa),new_symbols(skolem,[sK0])],[f4,f5]) ).

fof(f7,plain,
    ! [X0,X1] : big_f(X0,X1),
    inference(cnf_transformation,[],[f6]) ).

fof(f8,plain,
    ! [X0,X1] :
      ( big_g(X0,X1)
      | ~ big_f(sK0(X0,X1),sK0(X0,X1))
      | ~ big_f(X1,sK0(X0,X1)) ),
    inference(cnf_transformation,[],[f6]) ).

fof(f10,plain,
    ! [X0,X1] :
      ( ~ big_g(sK0(X0,X1),sK0(X0,X1))
      | ~ big_f(sK0(X0,X1),sK0(X0,X1))
      | ~ big_g(X0,sK0(X0,X1))
      | ~ big_f(X1,sK0(X0,X1)) ),
    inference(cnf_transformation,[],[f6]) ).

fof(f11,plain,
    ! [X0,X1] :
      ( ~ big_g(X0,sK0(X0,X1))
      | ~ big_f(sK0(X0,X1),sK0(X0,X1))
      | ~ big_f(X1,sK0(X0,X1))
      | ~ big_f(sK0(sK0(X0,X1),sK0(X0,X1)),sK0(sK0(X0,X1),sK0(X0,X1)))
      | ~ big_f(sK0(X0,X1),sK0(sK0(X0,X1),sK0(X0,X1))) ),
    inference(resolution,[],[f10,f8]) ).

fof(f12,plain,
    ! [X0,X1] :
      ( ~ big_f(sK0(sK0(X0,X1),sK0(X0,X1)),sK0(sK0(X0,X1),sK0(X0,X1)))
      | ~ big_f(X1,sK0(X0,X1))
      | ~ big_f(sK0(X0,X1),sK0(X0,X1))
      | ~ big_f(sK0(X0,X1),sK0(sK0(X0,X1),sK0(X0,X1)))
      | ~ big_f(sK0(X0,sK0(X0,X1)),sK0(X0,sK0(X0,X1)))
      | ~ big_f(sK0(X0,X1),sK0(X0,sK0(X0,X1))) ),
    inference(resolution,[],[f11,f8]) ).

fof(f13,plain,
    ! [X0,X1] :
      ( ~ big_f(sK0(X1,X0),sK0(sK0(X1,X0),sK0(X1,X0)))
      | ~ big_f(sK0(X1,sK0(X1,X0)),sK0(X1,sK0(X1,X0)))
      | ~ big_f(X0,sK0(X1,X0))
      | ~ big_f(sK0(X1,X0),sK0(X1,X0))
      | ~ big_f(sK0(X1,X0),sK0(X1,sK0(X1,X0))) ),
    inference(resolution,[],[f12,f7]) ).

fof(f14,plain,
    ! [X0,X1] :
      ( ~ big_f(sK0(X0,sK0(X0,X1)),sK0(X0,sK0(X0,X1)))
      | ~ big_f(X1,sK0(X0,X1))
      | ~ big_f(sK0(X0,X1),sK0(X0,X1))
      | ~ big_f(sK0(X0,X1),sK0(X0,sK0(X0,X1))) ),
    inference(resolution,[],[f13,f7]) ).

fof(f16,plain,
    ! [X0,X1] :
      ( ~ big_f(sK0(X1,X0),sK0(X1,sK0(X1,X0)))
      | ~ big_f(sK0(X1,X0),sK0(X1,X0))
      | ~ big_f(X0,sK0(X1,X0)) ),
    inference(resolution,[],[f14,f7]) ).

fof(f17,plain,
    ! [X0,X1] :
      ( ~ big_f(sK0(X0,X1),sK0(X0,X1))
      | ~ big_f(X1,sK0(X0,X1)) ),
    inference(resolution,[],[f16,f7]) ).

fof(f18,plain,
    ! [X0,X1] : ~ big_f(X0,sK0(X1,X0)),
    inference(resolution,[],[f17,f7]) ).

fof(f19,plain,
    $false,
    inference(resolution,[],[f18,f7]) ).

%------------------------------------------------------------------------------
%----ORIGINAL SYSTEM OUTPUT
% 0.00/0.12  % Problem    : SYN333+1 : TPTP v9.2.1. Released v2.0.0.
% 0.00/0.12  % Command    : run_vampire %s %d THM
% 0.13/0.33  % Computer   : n027.cluster.edu
% 0.13/0.33  % Model      : x86_64 x86_64
% 0.13/0.33  % CPU        : Intel(R) Xeon(R) CPU E5-2620 v4 @ 2.10GHz
% 0.13/0.33  % Memory     : 8042.1875MB
% 0.13/0.33  % OS         : Linux 3.10.0-693.el7.x86_64
% 0.13/0.33  % CPULimit   : 300
% 0.13/0.33  % WCLimit    : 300
% 0.13/0.33  % DateTime   : Fri May  1 06:00:50 EDT 2026
% 0.13/0.33  % CPUTime    : 
% 0.13/0.35  This is a FOF_THM_RFO_NEQ problem
% 0.13/0.35  Running first-order theorem proving
% 0.13/0.35  Running /export/starexec/sandbox/solver/bin/vampire --input_syntax tptp --proof tptp --output_axiom_names on --mode casc --cores 7 -m 16384 -t 300 /export/starexec/sandbox/benchmark/theBenchmark.p
% 0.46/0.64  % (28087)Detected formulas, will run a generic FOF schedule.
% 0.48/0.75  % (28095)dis-21_1_sil=8000:lcm=predicate:random_seed=2764188564:st=5:avsq=on:i=129:avsqr=1,16:sd=3:aac=none:ep=RS:fsr=off:ss=included_3000 on theBenchmark for (3000ds/129Mi)
% 0.48/0.75  % (28095)First to succeed.
% 0.48/0.76  % (28095)Solution written to "/export/starexec/sandbox/tmp/vampire-proof-28087"
% 0.48/0.78  % (28093)dis-1010_2:3_sil=16000:sp=reverse_frequency:random_seed=1007041908:i=119:av=off:ss=axioms_3000 on theBenchmark for (3000ds/119Mi)
% 0.48/0.78  % (28091)lrs+1010_1_anc=all:sfv=off:to=kbo:ncem=casc2026/models/loop7.pt:sil=128000:npcc=on:prc=on:sos=all:bsr=unit_only:sac=on:random_seed=1452071414:i=141695:sd=1:nm=32:gsp=on:ss=included_3000 on theBenchmark for (3000ds/141695Mi)
% 0.48/0.78  % (28089)lrs+10_1_ncem=casc2026/models/loop8.pt:sil=128000:tgt=full:npcc=on:drc=off:sp=weighted_frequency:spb=goal:fd=preordered:foolp=on:random_seed=1046351789:i=141193_3000 on theBenchmark for (3000ds/141193Mi)
% 0.48/0.78  % (28094)dis-1011_1_sil=16000:fde=unused:s2agt=70:random_seed=910562532:s2a=on:i=139:gtg=position_3000 on theBenchmark for (3000ds/139Mi)
% 0.48/0.78  % (28092)lrs+1010_1_to=lpo:sil=32000:sos=on:spb=goal_then_units:bce=on:random_seed=2393680326:i=109:sd=1:ins=1:gsp=on:ss=axioms_3000 on theBenchmark for (3000ds/109Mi)
% 0.48/0.78  % (28093)Also succeeded, but the first one will report.
% 0.48/0.78  % (28094)Also succeeded, but the first one will report.
% 0.48/0.78  % (28092)Also succeeded, but the first one will report.
% 0.48/0.79  % (28090)lrs+11_1_ncem=casc2026/models/loop8.pt:sil=128000:npcc=on:lma=off:spb=units:urr=ec_only:bce=on:s2agt=64:updr=off:random_seed=1564194329:i=134677:sd=20:aac=none:nm=16:ss=included:sgt=10_3000 on theBenchmark for (3000ds/134677Mi)
% 0.72/0.90  % (28095)Refutation found. Thanks to Tanya!
% 0.72/0.90  % SZS status Theorem for theBenchmark
% 0.72/0.90  % SZS output start Proof for theBenchmark
% See solution above
% 0.72/0.90  % (28095)------------------------------
% 0.72/0.90  % (28095)Version: Vampire 5.0.1 (Release build, commit 1b9f22200 on 2026-04-29 16:18:29 +0200)
% 0.72/0.90  % (28095)Linked with Z3 4.14.0.0 3c47fd96cf5645d0c42b2c819d9e9a84380aa721 z3-4.8.4-9178-g3c47fd96c
% 0.72/0.90  % (28095)CaDiCaL version: 2.1.3
% 0.72/0.90  % (28095)Termination reason: Refutation
% 0.72/0.90  % (28095)Time elapsed: 0.001 s
% 0.72/0.90  % (28095)Peak memory usage: 80 MB
% 0.72/0.90  % (28095)Instructions burned: 1 (million)
% 0.72/0.90  % (28095)------------------------------
% 0.72/0.90  % (28095)------------------------------
% 0.72/0.90  % (28087)Success in time 0.263 s
%------------------------------------------------------------------------------

