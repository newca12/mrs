% Proof : Problems/SYN317+1.p
%------------------------------------------------------------------------------
% File     : Vampire---5.0.1
% Problem  : SYN317+1 : TPTP v9.2.1. Released v2.0.0.
% Transfm  : none
% Format   : tptp:raw
% Command  : run_vampire %s %d THM

% Computer : n011.cluster.edu
% Model    : x86_64 x86_64
% CPU      : Intel(R) Xeon(R) CPU E5-2620 v4 2.10GHz
% Memory   : 8042.1875MB
% OS       : Linux 3.10.0-693.el7.x86_64
% CPULimit : 300s
% WCLimit  : 300s
% DateTime : Fri May  1 04:40:31 PM UTC 2026

% Result   : Theorem 0.50s 0.97s
% Output   : Refutation 0.50s
% Verified : 
% SZS Type : Refutation
%            Derivation depth      :   14
%            Number of leaves      :    3
% Syntax   : Number of formulae    :   19 (   4 unt;   0 def)
%            Number of atoms       :   67 (   0 equ)
%            Maximal formula atoms :    8 (   3 avg)
%            Number of connectives :   79 (  31   ~;  27   |;   9   &)
%                                         (   3 <=>;   8  =>;   0  <=;   1 <~>)
%            Maximal formula depth :    7 (   5 avg)
%            Maximal term depth    :    1 (   1 avg)
%            Number of predicates  :    3 (   2 usr;   1 prp; 0-1 aty)
%            Number of functors    :    3 (   3 usr;   3 con; 0-0 aty)
%            Number of variables   :   37 (  16   !;  21   ?)

% Comments : 
%------------------------------------------------------------------------------
fof(f1,conjecture,
    ( ? [X0] :
        ( big_f(X0)
       => big_g(X0) )
  <=> ? [X0,X1] :
        ( big_f(X0)
       => big_g(X1) ) ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',church_46_2_3) ).

fof(f2,negated_conjecture,
    ~ ( ? [X0] :
          ( big_f(X0)
         => big_g(X0) )
    <=> ? [X0,X1] :
          ( big_f(X0)
         => big_g(X1) ) ),
    inference(negated_conjecture,[status(cth)],[f1]) ).

fof(f3,plain,
    ~ ( ? [X0] :
          ( big_f(X0)
         => big_g(X0) )
    <=> ? [X1,X2] :
          ( big_f(X1)
         => big_g(X2) ) ),
    inference(rectify,[],[f2]) ).

fof(f4,plain,
    ( ? [X0] :
        ( big_g(X0)
        | ~ big_f(X0) )
  <~> ? [X1,X2] :
        ( big_g(X2)
        | ~ big_f(X1) ) ),
    inference(ennf_transformation,[],[f3]) ).

fof(f5,plain,
    ( ( ! [X1,X2] :
          ( ~ big_g(X2)
          & big_f(X1) )
      | ! [X0] :
          ( ~ big_g(X0)
          & big_f(X0) ) )
    & ( ? [X1,X2] :
          ( big_g(X2)
          | ~ big_f(X1) )
      | ? [X0] :
          ( big_g(X0)
          | ~ big_f(X0) ) ) ),
    inference(nnf_transformation,[],[f4]) ).

fof(f6,plain,
    ( ( ! [X0,X1] :
          ( ~ big_g(X1)
          & big_f(X0) )
      | ! [X2] :
          ( ~ big_g(X2)
          & big_f(X2) ) )
    & ( ? [X3,X4] :
          ( big_g(X4)
          | ~ big_f(X3) )
      | ? [X5] :
          ( big_g(X5)
          | ~ big_f(X5) ) ) ),
    inference(rectify,[],[f5]) ).

fof(f7,plain,
    ( ? [X3,X4] :
        ( big_g(X4)
        | ~ big_f(X3) )
   => ( big_g(sK1)
      | ~ big_f(sK0) ) ),
    introduced(definition,[],[skolem_symbol_introduction]) ).

fof(f8,plain,
    ( ? [X5] :
        ( big_g(X5)
        | ~ big_f(X5) )
   => ( big_g(sK2)
      | ~ big_f(sK2) ) ),
    introduced(definition,[],[skolem_symbol_introduction]) ).

fof(f9,plain,
    ( ( ! [X0,X1] :
          ( ~ big_g(X1)
          & big_f(X0) )
      | ! [X2] :
          ( ~ big_g(X2)
          & big_f(X2) ) )
    & ( big_g(sK1)
      | ~ big_f(sK0)
      | big_g(sK2)
      | ~ big_f(sK2) ) ),
    inference(skolemisation,[status(esa),new_symbols(skolem,[sK0,sK1,sK2])],[f6,f8,f7]) ).

fof(f10,plain,
    ( big_g(sK1)
    | ~ big_f(sK0)
    | big_g(sK2)
    | ~ big_f(sK2) ),
    inference(cnf_transformation,[],[f9]) ).

fof(f11,plain,
    ! [X2,X0] :
      ( big_f(X2)
      | big_f(X0) ),
    inference(cnf_transformation,[],[f9]) ).

fof(f14,plain,
    ! [X2,X1] :
      ( ~ big_g(X2)
      | ~ big_g(X1) ),
    inference(cnf_transformation,[],[f9]) ).

fof(f15,plain,
    ! [X0] : ~ big_g(X0),
    inference(factoring,[],[f14]) ).

fof(f16,plain,
    ( ~ big_f(sK0)
    | big_g(sK2)
    | ~ big_f(sK2) ),
    inference(forward_subsumption_resolution,[],[f10,f15]) ).

fof(f17,plain,
    ( ~ big_f(sK2)
    | ~ big_f(sK0) ),
    inference(forward_subsumption_resolution,[],[f16,f15]) ).

fof(f19,plain,
    ! [X0] :
      ( ~ big_f(sK0)
      | big_f(X0) ),
    inference(resolution,[],[f17,f11]) ).

fof(f20,plain,
    ! [X0] : big_f(X0),
    inference(forward_subsumption_resolution,[],[f19,f11]) ).

fof(f21,plain,
    ~ big_f(sK0),
    inference(resolution,[],[f20,f17]) ).

fof(f22,plain,
    $false,
    inference(forward_subsumption_resolution,[],[f21,f20]) ).

%------------------------------------------------------------------------------
%----ORIGINAL SYSTEM OUTPUT
% 0.00/0.11  % Problem    : SYN317+1 : TPTP v9.2.1. Released v2.0.0.
% 0.00/0.11  % Command    : run_vampire %s %d THM
% 0.13/0.32  % Computer   : n011.cluster.edu
% 0.13/0.32  % Model      : x86_64 x86_64
% 0.13/0.32  % CPU        : Intel(R) Xeon(R) CPU E5-2620 v4 @ 2.10GHz
% 0.13/0.32  % Memory     : 8042.1875MB
% 0.13/0.32  % OS         : Linux 3.10.0-693.el7.x86_64
% 0.13/0.32  % CPULimit   : 300
% 0.13/0.32  % WCLimit    : 300
% 0.13/0.32  % DateTime   : Fri May  1 05:58:17 EDT 2026
% 0.13/0.33  % CPUTime    : 
% 0.13/0.35  This is a FOF_THM_RFO_NEQ problem
% 0.13/0.36  Running first-order theorem proving
% 0.13/0.36  Running /export/starexec/sandbox2/solver/bin/vampire --input_syntax tptp --proof tptp --output_axiom_names on --mode casc --cores 7 -m 16384 -t 300 /export/starexec/sandbox2/benchmark/theBenchmark.p
% 0.46/0.66  % (32608)Detected formulas, will run a generic FOF schedule.
% 0.50/0.76  % (32619)lrs+10_1_ncem=casc2026/models/loop8.pt:sil=128000:tgt=full:npcc=on:drc=off:sp=weighted_frequency:spb=goal:fd=preordered:foolp=on:random_seed=2277362783:i=141193_3000 on theBenchmark for (3000ds/141193Mi)
% 0.50/0.80  % (32624)dis-1011_1_sil=16000:fde=unused:s2agt=70:random_seed=3745570798:s2a=on:i=139:gtg=position_3000 on theBenchmark for (3000ds/139Mi)
% 0.50/0.80  % (32625)dis-21_1_sil=8000:lcm=predicate:random_seed=2189105286:st=5:avsq=on:i=129:avsqr=1,16:sd=3:aac=none:ep=RS:fsr=off:ss=included_3000 on theBenchmark for (3000ds/129Mi)
% 0.50/0.80  % (32623)dis-1010_2:3_sil=16000:sp=reverse_frequency:random_seed=3152531697:i=119:av=off:ss=axioms_3000 on theBenchmark for (3000ds/119Mi)
% 0.50/0.80  % (32622)lrs+1010_1_to=lpo:sil=32000:sos=on:spb=goal_then_units:bce=on:random_seed=2972588774:i=109:sd=1:ins=1:gsp=on:ss=axioms_3000 on theBenchmark for (3000ds/109Mi)
% 0.50/0.80  % (32621)lrs+1010_1_anc=all:sfv=off:to=kbo:ncem=casc2026/models/loop7.pt:sil=128000:npcc=on:prc=on:sos=all:bsr=unit_only:sac=on:random_seed=43598214:i=141695:sd=1:nm=32:gsp=on:ss=included_3000 on theBenchmark for (3000ds/141695Mi)
% 0.50/0.80  % (32620)lrs+11_1_ncem=casc2026/models/loop8.pt:sil=128000:npcc=on:lma=off:spb=units:urr=ec_only:bce=on:s2agt=64:updr=off:random_seed=1169285399:i=134677:sd=20:aac=none:nm=16:ss=included:sgt=10_3000 on theBenchmark for (3000ds/134677Mi)
% 0.50/0.81  % (32623)First to succeed.
% 0.50/0.81  % (32624)Also succeeded, but the first one will report.
% 0.50/0.81  % (32625)Also succeeded, but the first one will report.
% 0.50/0.81  % (32623)Solution written to "/export/starexec/sandbox2/tmp/vampire-proof-32608"
% 0.50/0.81  % (32622)Also succeeded, but the first one will report.
% 0.50/0.97  % (32623)Refutation found. Thanks to Tanya!
% 0.50/0.97  % SZS status Theorem for theBenchmark
% 0.50/0.97  % SZS output start Proof for theBenchmark
% See solution above
% 0.50/0.97  % (32623)------------------------------
% 0.50/0.97  % (32623)Version: Vampire 5.0.1 (Release build, commit 1b9f22200 on 2026-04-29 16:18:29 +0200)
% 0.50/0.97  % (32623)Linked with Z3 4.14.0.0 3c47fd96cf5645d0c42b2c819d9e9a84380aa721 z3-4.8.4-9178-g3c47fd96c
% 0.50/0.97  % (32623)CaDiCaL version: 2.1.3
% 0.50/0.97  % (32623)Termination reason: Refutation
% 0.50/0.97  % (32623)Time elapsed: 0.001 s
% 0.50/0.97  % (32623)Peak memory usage: 80 MB
% 0.50/0.97  % (32623)------------------------------
% 0.50/0.97  % (32623)------------------------------
% 0.50/0.97  % (32608)Success in time 0.312 s
%------------------------------------------------------------------------------

