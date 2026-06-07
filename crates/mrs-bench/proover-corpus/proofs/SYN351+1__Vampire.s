% Proof : Problems/SYN351+1.p
%------------------------------------------------------------------------------
% File     : Vampire---5.0.1
% Problem  : SYN351+1 : TPTP v9.2.1. Released v2.0.0.
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
% DateTime : Fri May  1 04:40:40 PM UTC 2026

% Result   : Theorem 0.48s 0.90s
% Output   : Refutation 0.48s
% Verified : 
% SZS Type : Refutation
%            Derivation depth      :   10
%            Number of leaves      :    3
% Syntax   : Number of formulae    :   14 (   3 unt;   0 def)
%            Number of atoms       :  135 (   0 equ)
%            Maximal formula atoms :   26 (   9 avg)
%            Number of connectives :  165 (  44   ~;  40   |;  61   &)
%                                         (   6 <=>;  12  =>;   0  <=;   2 <~>)
%            Maximal formula depth :   13 (   9 avg)
%            Maximal term depth    :    2 (   1 avg)
%            Number of predicates  :    2 (   1 usr;   1 prp; 0-4 aty)
%            Number of functors    :    3 (   3 usr;   2 con; 0-2 aty)
%            Number of variables   :   50 (  29   !;  21   ?)

% Comments : 
%------------------------------------------------------------------------------
fof(f1,conjecture,
    ! [X0,X1] :
    ? [X2,X3] :
    ! [X4] :
      ( big_f(X0,X3,X0,X4)
     => ( ( big_f(X0,X2,X0,X3)
        <=> big_f(X2,X1,X2,X3) )
       => ( big_f(X0,X2,X0,X3)
         => ( ( big_f(X0,X3,X2,X3)
             => big_f(X0,X4,X2,X4) )
            & ( big_f(X0,X4,X2,X4)
             => ( big_f(X0,X2,X0,X3)
              <=> big_f(X0,X3,X2,X3) ) ) ) ) ) ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',church_46_18_3) ).

fof(f2,negated_conjecture,
    ~ ! [X0,X1] :
      ? [X2,X3] :
      ! [X4] :
        ( big_f(X0,X3,X0,X4)
       => ( ( big_f(X0,X2,X0,X3)
          <=> big_f(X2,X1,X2,X3) )
         => ( big_f(X0,X2,X0,X3)
           => ( ( big_f(X0,X3,X2,X3)
               => big_f(X0,X4,X2,X4) )
              & ( big_f(X0,X4,X2,X4)
               => ( big_f(X0,X2,X0,X3)
                <=> big_f(X0,X3,X2,X3) ) ) ) ) ) ),
    inference(negated_conjecture,[status(cth)],[f1]) ).

fof(f3,plain,
    ? [X0,X1] :
    ! [X2,X3] :
    ? [X4] :
      ( ( ( ~ big_f(X0,X4,X2,X4)
          & big_f(X0,X3,X2,X3) )
        | ( ( big_f(X0,X2,X0,X3)
          <~> big_f(X0,X3,X2,X3) )
          & big_f(X0,X4,X2,X4) ) )
      & big_f(X0,X2,X0,X3)
      & ( big_f(X0,X2,X0,X3)
      <=> big_f(X2,X1,X2,X3) )
      & big_f(X0,X3,X0,X4) ),
    inference(ennf_transformation,[],[f2]) ).

fof(f4,plain,
    ? [X0,X1] :
    ! [X2,X3] :
    ? [X4] :
      ( ( ( ~ big_f(X0,X4,X2,X4)
          & big_f(X0,X3,X2,X3) )
        | ( ( big_f(X0,X2,X0,X3)
          <~> big_f(X0,X3,X2,X3) )
          & big_f(X0,X4,X2,X4) ) )
      & big_f(X0,X2,X0,X3)
      & ( big_f(X0,X2,X0,X3)
      <=> big_f(X2,X1,X2,X3) )
      & big_f(X0,X3,X0,X4) ),
    inference(flattening,[],[f3]) ).

fof(f5,plain,
    ? [X0,X1] :
    ! [X2,X3] :
    ? [X4] :
      ( ( ( ~ big_f(X0,X4,X2,X4)
          & big_f(X0,X3,X2,X3) )
        | ( ( ~ big_f(X0,X3,X2,X3)
            | ~ big_f(X0,X2,X0,X3) )
          & ( big_f(X0,X3,X2,X3)
            | big_f(X0,X2,X0,X3) )
          & big_f(X0,X4,X2,X4) ) )
      & big_f(X0,X2,X0,X3)
      & ( big_f(X0,X2,X0,X3)
        | ~ big_f(X2,X1,X2,X3) )
      & ( big_f(X2,X1,X2,X3)
        | ~ big_f(X0,X2,X0,X3) )
      & big_f(X0,X3,X0,X4) ),
    inference(nnf_transformation,[],[f4]) ).

fof(f6,plain,
    ? [X0,X1] :
    ! [X2,X3] :
    ? [X4] :
      ( ( ( ~ big_f(X0,X4,X2,X4)
          & big_f(X0,X3,X2,X3) )
        | ( ( ~ big_f(X0,X3,X2,X3)
            | ~ big_f(X0,X2,X0,X3) )
          & ( big_f(X0,X3,X2,X3)
            | big_f(X0,X2,X0,X3) )
          & big_f(X0,X4,X2,X4) ) )
      & big_f(X0,X2,X0,X3)
      & ( big_f(X0,X2,X0,X3)
        | ~ big_f(X2,X1,X2,X3) )
      & ( big_f(X2,X1,X2,X3)
        | ~ big_f(X0,X2,X0,X3) )
      & big_f(X0,X3,X0,X4) ),
    inference(flattening,[],[f5]) ).

fof(f7,plain,
    ( ? [X0,X1] :
      ! [X2,X3] :
      ? [X4] :
        ( ( ( ~ big_f(X0,X4,X2,X4)
            & big_f(X0,X3,X2,X3) )
          | ( ( ~ big_f(X0,X3,X2,X3)
              | ~ big_f(X0,X2,X0,X3) )
            & ( big_f(X0,X3,X2,X3)
              | big_f(X0,X2,X0,X3) )
            & big_f(X0,X4,X2,X4) ) )
        & big_f(X0,X2,X0,X3)
        & ( big_f(X0,X2,X0,X3)
          | ~ big_f(X2,X1,X2,X3) )
        & ( big_f(X2,X1,X2,X3)
          | ~ big_f(X0,X2,X0,X3) )
        & big_f(X0,X3,X0,X4) )
   => ! [X3,X2] :
      ? [X4] :
        ( ( ( ~ big_f(sK0,X4,X2,X4)
            & big_f(sK0,X3,X2,X3) )
          | ( ( ~ big_f(sK0,X3,X2,X3)
              | ~ big_f(sK0,X2,sK0,X3) )
            & ( big_f(sK0,X3,X2,X3)
              | big_f(sK0,X2,sK0,X3) )
            & big_f(sK0,X4,X2,X4) ) )
        & big_f(sK0,X2,sK0,X3)
        & ( big_f(sK0,X2,sK0,X3)
          | ~ big_f(X2,sK1,X2,X3) )
        & ( big_f(X2,sK1,X2,X3)
          | ~ big_f(sK0,X2,sK0,X3) )
        & big_f(sK0,X3,sK0,X4) ) ),
    introduced(definition,[],[skolem_symbol_introduction]) ).

fof(f8,plain,
    ! [X2,X3] :
      ( ? [X4] :
          ( ( ( ~ big_f(sK0,X4,X2,X4)
              & big_f(sK0,X3,X2,X3) )
            | ( ( ~ big_f(sK0,X3,X2,X3)
                | ~ big_f(sK0,X2,sK0,X3) )
              & ( big_f(sK0,X3,X2,X3)
                | big_f(sK0,X2,sK0,X3) )
              & big_f(sK0,X4,X2,X4) ) )
          & big_f(sK0,X2,sK0,X3)
          & ( big_f(sK0,X2,sK0,X3)
            | ~ big_f(X2,sK1,X2,X3) )
          & ( big_f(X2,sK1,X2,X3)
            | ~ big_f(sK0,X2,sK0,X3) )
          & big_f(sK0,X3,sK0,X4) )
     => ( ( ( ~ big_f(sK0,sK2(X2,X3),X2,sK2(X2,X3))
            & big_f(sK0,X3,X2,X3) )
          | ( ( ~ big_f(sK0,X3,X2,X3)
              | ~ big_f(sK0,X2,sK0,X3) )
            & ( big_f(sK0,X3,X2,X3)
              | big_f(sK0,X2,sK0,X3) )
            & big_f(sK0,sK2(X2,X3),X2,sK2(X2,X3)) ) )
        & big_f(sK0,X2,sK0,X3)
        & ( big_f(sK0,X2,sK0,X3)
          | ~ big_f(X2,sK1,X2,X3) )
        & ( big_f(X2,sK1,X2,X3)
          | ~ big_f(sK0,X2,sK0,X3) )
        & big_f(sK0,X3,sK0,sK2(X2,X3)) ) ),
    introduced(definition,[],[skolem_symbol_introduction]) ).

fof(f9,plain,
    ! [X2,X3] :
      ( ( ( ~ big_f(sK0,sK2(X2,X3),X2,sK2(X2,X3))
          & big_f(sK0,X3,X2,X3) )
        | ( ( ~ big_f(sK0,X3,X2,X3)
            | ~ big_f(sK0,X2,sK0,X3) )
          & ( big_f(sK0,X3,X2,X3)
            | big_f(sK0,X2,sK0,X3) )
          & big_f(sK0,sK2(X2,X3),X2,sK2(X2,X3)) ) )
      & big_f(sK0,X2,sK0,X3)
      & ( big_f(sK0,X2,sK0,X3)
        | ~ big_f(X2,sK1,X2,X3) )
      & ( big_f(X2,sK1,X2,X3)
        | ~ big_f(sK0,X2,sK0,X3) )
      & big_f(sK0,X3,sK0,sK2(X2,X3)) ),
    inference(skolemisation,[status(esa),new_symbols(skolem,[sK0,sK1,sK2])],[f6,f8,f7]) ).

fof(f13,plain,
    ! [X2,X3] : big_f(sK0,X2,sK0,X3),
    inference(cnf_transformation,[],[f9]) ).

fof(f19,plain,
    ! [X2,X3] :
      ( ~ big_f(sK0,sK2(X2,X3),X2,sK2(X2,X3))
      | ~ big_f(sK0,X3,X2,X3)
      | ~ big_f(sK0,X2,sK0,X3) ),
    inference(cnf_transformation,[],[f9]) ).

fof(f23,plain,
    ! [X2,X3] :
      ( ~ big_f(sK0,sK2(X2,X3),X2,sK2(X2,X3))
      | ~ big_f(sK0,X3,X2,X3) ),
    inference(forward_subsumption_resolution,[],[f19,f13]) ).

fof(f25,plain,
    ! [X0] : ~ big_f(sK0,X0,sK0,X0),
    inference(resolution,[],[f23,f13]) ).

fof(f26,plain,
    $false,
    inference(forward_subsumption_resolution,[],[f25,f13]) ).

%------------------------------------------------------------------------------
%----ORIGINAL SYSTEM OUTPUT
% 0.00/0.12  % Problem    : SYN351+1 : TPTP v9.2.1. Released v2.0.0.
% 0.00/0.12  % Command    : run_vampire %s %d THM
% 0.16/0.33  % Computer   : n019.cluster.edu
% 0.16/0.33  % Model      : x86_64 x86_64
% 0.16/0.33  % CPU        : Intel(R) Xeon(R) CPU E5-2620 v4 @ 2.10GHz
% 0.16/0.33  % Memory     : 8042.1875MB
% 0.16/0.33  % OS         : Linux 3.10.0-693.el7.x86_64
% 0.16/0.33  % CPULimit   : 300
% 0.16/0.33  % WCLimit    : 300
% 0.16/0.33  % DateTime   : Fri May  1 06:02:29 EDT 2026
% 0.16/0.33  % CPUTime    : 
% 0.21/0.35  This is a FOF_THM_RFO_NEQ problem
% 0.21/0.35  Running first-order theorem proving
% 0.21/0.35  Running /export/starexec/sandbox2/solver/bin/vampire --input_syntax tptp --proof tptp --output_axiom_names on --mode casc --cores 7 -m 16384 -t 300 /export/starexec/sandbox2/benchmark/theBenchmark.p
% 0.47/0.63  % (20279)Detected formulas, will run a generic FOF schedule.
% 0.48/0.75  % (20347)lrs+1010_1_anc=all:sfv=off:to=kbo:ncem=casc2026/models/loop7.pt:sil=128000:npcc=on:prc=on:sos=all:bsr=unit_only:sac=on:random_seed=1725638378:i=141695:sd=1:nm=32:gsp=on:ss=included_3000 on theBenchmark for (3000ds/141695Mi)
% 0.48/0.75  % (20346)lrs+11_1_ncem=casc2026/models/loop8.pt:sil=128000:npcc=on:lma=off:spb=units:urr=ec_only:bce=on:s2agt=64:updr=off:random_seed=2783998000:i=134677:sd=20:aac=none:nm=16:ss=included:sgt=10_3000 on theBenchmark for (3000ds/134677Mi)
% 0.48/0.75  % (20345)lrs+10_1_ncem=casc2026/models/loop8.pt:sil=128000:tgt=full:npcc=on:drc=off:sp=weighted_frequency:spb=goal:fd=preordered:foolp=on:random_seed=1542955575:i=141193_3000 on theBenchmark for (3000ds/141193Mi)
% 0.48/0.75  % (20349)dis-1010_2:3_sil=16000:sp=reverse_frequency:random_seed=1507366782:i=119:av=off:ss=axioms_3000 on theBenchmark for (3000ds/119Mi)
% 0.48/0.75  % (20351)dis-21_1_sil=8000:lcm=predicate:random_seed=1308997030:st=5:avsq=on:i=129:avsqr=1,16:sd=3:aac=none:ep=RS:fsr=off:ss=included_3000 on theBenchmark for (3000ds/129Mi)
% 0.48/0.75  % (20350)dis-1011_1_sil=16000:fde=unused:s2agt=70:random_seed=2135252305:s2a=on:i=139:gtg=position_3000 on theBenchmark for (3000ds/139Mi)
% 0.48/0.75  % (20348)lrs+1010_1_to=lpo:sil=32000:sos=on:spb=goal_then_units:bce=on:random_seed=579825046:i=109:sd=1:ins=1:gsp=on:ss=axioms_3000 on theBenchmark for (3000ds/109Mi)
% 0.48/0.75  % (20349)First to succeed.
% 0.48/0.75  % (20351)Also succeeded, but the first one will report.
% 0.48/0.75  % (20350)Also succeeded, but the first one will report.
% 0.48/0.75  % (20349)Solution written to "/export/starexec/sandbox2/tmp/vampire-proof-20279"
% 0.48/0.75  % (20348)Also succeeded, but the first one will report.
% 0.48/0.90  % (20349)Refutation found. Thanks to Tanya!
% 0.48/0.90  % SZS status Theorem for theBenchmark
% 0.48/0.90  % SZS output start Proof for theBenchmark
% See solution above
% 0.48/0.90  % (20349)------------------------------
% 0.48/0.90  % (20349)Version: Vampire 5.0.1 (Release build, commit 1b9f22200 on 2026-04-29 16:18:29 +0200)
% 0.48/0.90  % (20349)Linked with Z3 4.14.0.0 3c47fd96cf5645d0c42b2c819d9e9a84380aa721 z3-4.8.4-9178-g3c47fd96c
% 0.48/0.90  % (20349)CaDiCaL version: 2.1.3
% 0.48/0.90  % (20349)Termination reason: Refutation
% 0.48/0.90  % (20349)Time elapsed: 0.001 s
% 0.48/0.90  % (20349)Peak memory usage: 80 MB
% 0.48/0.90  % (20349)Instructions burned: 1 (million)
% 0.48/0.90  % (20349)------------------------------
% 0.48/0.90  % (20349)------------------------------
% 0.48/0.90  % (20279)Success in time 0.276 s
%------------------------------------------------------------------------------

