% Proof : Problems/SYN349+1.p
%------------------------------------------------------------------------------
% File     : Vampire---5.0.1
% Problem  : SYN349+1 : TPTP v9.2.1. Released v2.0.0.
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
% DateTime : Fri May  1 04:40:40 PM UTC 2026

% Result   : Theorem 2.32s 1.20s
% Output   : Refutation 2.32s
% Verified : 
% SZS Type : Refutation
%            Derivation depth      :   14
%            Number of leaves      :    3
% Syntax   : Number of formulae    :   27 (   4 unt;   0 def)
%            Number of atoms       :  246 (   0 equ)
%            Maximal formula atoms :   52 (   9 avg)
%            Number of connectives :  335 ( 116   ~; 139   |;  64   &)
%                                         (  11 <=>;   4  =>;   0  <=;   1 <~>)
%            Maximal formula depth :   14 (   6 avg)
%            Maximal term depth    :    3 (   1 avg)
%            Number of predicates  :    2 (   1 usr;   1 prp; 0-2 aty)
%            Number of functors    :    2 (   2 usr;   0 con; 1-2 aty)
%            Number of variables   :   55 (  41   !;  14   ?)

% Comments : 
%------------------------------------------------------------------------------
fof(f1,conjecture,
    ? [X0] :
    ! [X1] :
    ? [X2] :
    ! [X3] :
      ( ( big_f(X0,X3)
      <=> big_f(X1,X3) )
     => ( ( ( big_f(X0,X3)
          <=> big_f(X3,X2) )
        <=> big_f(X2,X3) )
      <=> big_f(X3,X1) ) ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',church_46_17_5) ).

fof(f2,negated_conjecture,
    ~ ? [X0] :
      ! [X1] :
      ? [X2] :
      ! [X3] :
        ( ( big_f(X0,X3)
        <=> big_f(X1,X3) )
       => ( ( ( big_f(X0,X3)
            <=> big_f(X3,X2) )
          <=> big_f(X2,X3) )
        <=> big_f(X3,X1) ) ),
    inference(negated_conjecture,[status(cth)],[f1]) ).

fof(f3,plain,
    ! [X0] :
    ? [X1] :
    ! [X2] :
    ? [X3] :
      ( ( ( ( big_f(X0,X3)
          <=> big_f(X3,X2) )
        <=> big_f(X2,X3) )
      <~> big_f(X3,X1) )
      & ( big_f(X0,X3)
      <=> big_f(X1,X3) ) ),
    inference(ennf_transformation,[],[f2]) ).

fof(f4,plain,
    ! [X0] :
    ? [X1] :
    ! [X2] :
    ? [X3] :
      ( ( ~ big_f(X3,X1)
        | ( ( ~ big_f(X2,X3)
            | ( ( ~ big_f(X3,X2)
                | ~ big_f(X0,X3) )
              & ( big_f(X3,X2)
                | big_f(X0,X3) ) ) )
          & ( big_f(X2,X3)
            | ( ( big_f(X0,X3)
                | ~ big_f(X3,X2) )
              & ( big_f(X3,X2)
                | ~ big_f(X0,X3) ) ) ) ) )
      & ( big_f(X3,X1)
        | ( ( ( ( big_f(X0,X3)
                | ~ big_f(X3,X2) )
              & ( big_f(X3,X2)
                | ~ big_f(X0,X3) ) )
            | ~ big_f(X2,X3) )
          & ( big_f(X2,X3)
            | ( ( ~ big_f(X3,X2)
                | ~ big_f(X0,X3) )
              & ( big_f(X3,X2)
                | big_f(X0,X3) ) ) ) ) )
      & ( big_f(X0,X3)
        | ~ big_f(X1,X3) )
      & ( big_f(X1,X3)
        | ~ big_f(X0,X3) ) ),
    inference(nnf_transformation,[],[f3]) ).

fof(f5,plain,
    ! [X0] :
    ? [X1] :
    ! [X2] :
    ? [X3] :
      ( ( ~ big_f(X3,X1)
        | ( ( ~ big_f(X2,X3)
            | ( ( ~ big_f(X3,X2)
                | ~ big_f(X0,X3) )
              & ( big_f(X3,X2)
                | big_f(X0,X3) ) ) )
          & ( big_f(X2,X3)
            | ( ( big_f(X0,X3)
                | ~ big_f(X3,X2) )
              & ( big_f(X3,X2)
                | ~ big_f(X0,X3) ) ) ) ) )
      & ( big_f(X3,X1)
        | ( ( ( ( big_f(X0,X3)
                | ~ big_f(X3,X2) )
              & ( big_f(X3,X2)
                | ~ big_f(X0,X3) ) )
            | ~ big_f(X2,X3) )
          & ( big_f(X2,X3)
            | ( ( ~ big_f(X3,X2)
                | ~ big_f(X0,X3) )
              & ( big_f(X3,X2)
                | big_f(X0,X3) ) ) ) ) )
      & ( big_f(X0,X3)
        | ~ big_f(X1,X3) )
      & ( big_f(X1,X3)
        | ~ big_f(X0,X3) ) ),
    inference(flattening,[],[f4]) ).

fof(f6,plain,
    ! [X0] :
      ( ? [X1] :
        ! [X2] :
        ? [X3] :
          ( ( ~ big_f(X3,X1)
            | ( ( ~ big_f(X2,X3)
                | ( ( ~ big_f(X3,X2)
                    | ~ big_f(X0,X3) )
                  & ( big_f(X3,X2)
                    | big_f(X0,X3) ) ) )
              & ( big_f(X2,X3)
                | ( ( big_f(X0,X3)
                    | ~ big_f(X3,X2) )
                  & ( big_f(X3,X2)
                    | ~ big_f(X0,X3) ) ) ) ) )
          & ( big_f(X3,X1)
            | ( ( ( ( big_f(X0,X3)
                    | ~ big_f(X3,X2) )
                  & ( big_f(X3,X2)
                    | ~ big_f(X0,X3) ) )
                | ~ big_f(X2,X3) )
              & ( big_f(X2,X3)
                | ( ( ~ big_f(X3,X2)
                    | ~ big_f(X0,X3) )
                  & ( big_f(X3,X2)
                    | big_f(X0,X3) ) ) ) ) )
          & ( big_f(X0,X3)
            | ~ big_f(X1,X3) )
          & ( big_f(X1,X3)
            | ~ big_f(X0,X3) ) )
     => ! [X2] :
        ? [X3] :
          ( ( ~ big_f(X3,sK0(X0))
            | ( ( ~ big_f(X2,X3)
                | ( ( ~ big_f(X3,X2)
                    | ~ big_f(X0,X3) )
                  & ( big_f(X3,X2)
                    | big_f(X0,X3) ) ) )
              & ( big_f(X2,X3)
                | ( ( big_f(X0,X3)
                    | ~ big_f(X3,X2) )
                  & ( big_f(X3,X2)
                    | ~ big_f(X0,X3) ) ) ) ) )
          & ( big_f(X3,sK0(X0))
            | ( ( ( ( big_f(X0,X3)
                    | ~ big_f(X3,X2) )
                  & ( big_f(X3,X2)
                    | ~ big_f(X0,X3) ) )
                | ~ big_f(X2,X3) )
              & ( big_f(X2,X3)
                | ( ( ~ big_f(X3,X2)
                    | ~ big_f(X0,X3) )
                  & ( big_f(X3,X2)
                    | big_f(X0,X3) ) ) ) ) )
          & ( big_f(X0,X3)
            | ~ big_f(sK0(X0),X3) )
          & ( big_f(sK0(X0),X3)
            | ~ big_f(X0,X3) ) ) ),
    introduced(definition,[],[skolem_symbol_introduction]) ).

fof(f7,plain,
    ! [X0,X2] :
      ( ? [X3] :
          ( ( ~ big_f(X3,sK0(X0))
            | ( ( ~ big_f(X2,X3)
                | ( ( ~ big_f(X3,X2)
                    | ~ big_f(X0,X3) )
                  & ( big_f(X3,X2)
                    | big_f(X0,X3) ) ) )
              & ( big_f(X2,X3)
                | ( ( big_f(X0,X3)
                    | ~ big_f(X3,X2) )
                  & ( big_f(X3,X2)
                    | ~ big_f(X0,X3) ) ) ) ) )
          & ( big_f(X3,sK0(X0))
            | ( ( ( ( big_f(X0,X3)
                    | ~ big_f(X3,X2) )
                  & ( big_f(X3,X2)
                    | ~ big_f(X0,X3) ) )
                | ~ big_f(X2,X3) )
              & ( big_f(X2,X3)
                | ( ( ~ big_f(X3,X2)
                    | ~ big_f(X0,X3) )
                  & ( big_f(X3,X2)
                    | big_f(X0,X3) ) ) ) ) )
          & ( big_f(X0,X3)
            | ~ big_f(sK0(X0),X3) )
          & ( big_f(sK0(X0),X3)
            | ~ big_f(X0,X3) ) )
     => ( ( ~ big_f(sK1(X0,X2),sK0(X0))
          | ( ( ~ big_f(X2,sK1(X0,X2))
              | ( ( ~ big_f(sK1(X0,X2),X2)
                  | ~ big_f(X0,sK1(X0,X2)) )
                & ( big_f(sK1(X0,X2),X2)
                  | big_f(X0,sK1(X0,X2)) ) ) )
            & ( big_f(X2,sK1(X0,X2))
              | ( ( big_f(X0,sK1(X0,X2))
                  | ~ big_f(sK1(X0,X2),X2) )
                & ( big_f(sK1(X0,X2),X2)
                  | ~ big_f(X0,sK1(X0,X2)) ) ) ) ) )
        & ( big_f(sK1(X0,X2),sK0(X0))
          | ( ( ( ( big_f(X0,sK1(X0,X2))
                  | ~ big_f(sK1(X0,X2),X2) )
                & ( big_f(sK1(X0,X2),X2)
                  | ~ big_f(X0,sK1(X0,X2)) ) )
              | ~ big_f(X2,sK1(X0,X2)) )
            & ( big_f(X2,sK1(X0,X2))
              | ( ( ~ big_f(sK1(X0,X2),X2)
                  | ~ big_f(X0,sK1(X0,X2)) )
                & ( big_f(sK1(X0,X2),X2)
                  | big_f(X0,sK1(X0,X2)) ) ) ) ) )
        & ( big_f(X0,sK1(X0,X2))
          | ~ big_f(sK0(X0),sK1(X0,X2)) )
        & ( big_f(sK0(X0),sK1(X0,X2))
          | ~ big_f(X0,sK1(X0,X2)) ) ) ),
    introduced(definition,[],[skolem_symbol_introduction]) ).

fof(f8,plain,
    ! [X0,X2] :
      ( ( ~ big_f(sK1(X0,X2),sK0(X0))
        | ( ( ~ big_f(X2,sK1(X0,X2))
            | ( ( ~ big_f(sK1(X0,X2),X2)
                | ~ big_f(X0,sK1(X0,X2)) )
              & ( big_f(sK1(X0,X2),X2)
                | big_f(X0,sK1(X0,X2)) ) ) )
          & ( big_f(X2,sK1(X0,X2))
            | ( ( big_f(X0,sK1(X0,X2))
                | ~ big_f(sK1(X0,X2),X2) )
              & ( big_f(sK1(X0,X2),X2)
                | ~ big_f(X0,sK1(X0,X2)) ) ) ) ) )
      & ( big_f(sK1(X0,X2),sK0(X0))
        | ( ( ( ( big_f(X0,sK1(X0,X2))
                | ~ big_f(sK1(X0,X2),X2) )
              & ( big_f(sK1(X0,X2),X2)
                | ~ big_f(X0,sK1(X0,X2)) ) )
            | ~ big_f(X2,sK1(X0,X2)) )
          & ( big_f(X2,sK1(X0,X2))
            | ( ( ~ big_f(sK1(X0,X2),X2)
                | ~ big_f(X0,sK1(X0,X2)) )
              & ( big_f(sK1(X0,X2),X2)
                | big_f(X0,sK1(X0,X2)) ) ) ) ) )
      & ( big_f(X0,sK1(X0,X2))
        | ~ big_f(sK0(X0),sK1(X0,X2)) )
      & ( big_f(sK0(X0),sK1(X0,X2))
        | ~ big_f(X0,sK1(X0,X2)) ) ),
    inference(skolemisation,[status(esa),new_symbols(skolem,[sK0,sK1])],[f5,f7,f6]) ).

fof(f9,plain,
    ! [X2,X0] :
      ( big_f(sK0(X0),sK1(X0,X2))
      | ~ big_f(X0,sK1(X0,X2)) ),
    inference(cnf_transformation,[],[f8]) ).

fof(f10,plain,
    ! [X2,X0] :
      ( ~ big_f(sK0(X0),sK1(X0,X2))
      | big_f(X0,sK1(X0,X2)) ),
    inference(cnf_transformation,[],[f8]) ).

fof(f11,plain,
    ! [X2,X0] :
      ( big_f(sK1(X0,X2),sK0(X0))
      | big_f(X2,sK1(X0,X2))
      | big_f(sK1(X0,X2),X2)
      | big_f(X0,sK1(X0,X2)) ),
    inference(cnf_transformation,[],[f8]) ).

fof(f13,plain,
    ! [X2,X0] :
      ( big_f(sK1(X0,X2),sK0(X0))
      | big_f(sK1(X0,X2),X2)
      | ~ big_f(X0,sK1(X0,X2))
      | ~ big_f(X2,sK1(X0,X2)) ),
    inference(cnf_transformation,[],[f8]) ).

fof(f16,plain,
    ! [X2,X0] :
      ( ~ big_f(sK1(X0,X2),sK0(X0))
      | big_f(X2,sK1(X0,X2))
      | big_f(X0,sK1(X0,X2))
      | ~ big_f(sK1(X0,X2),X2) ),
    inference(cnf_transformation,[],[f8]) ).

fof(f18,plain,
    ! [X2,X0] :
      ( ~ big_f(sK1(X0,X2),sK0(X0))
      | ~ big_f(X2,sK1(X0,X2))
      | ~ big_f(sK1(X0,X2),X2)
      | ~ big_f(X0,sK1(X0,X2)) ),
    inference(cnf_transformation,[],[f8]) ).

fof(f31,plain,
    ! [X0] :
      ( ~ big_f(sK1(X0,sK0(X0)),sK0(X0))
      | big_f(sK0(X0),sK1(X0,sK0(X0)))
      | big_f(X0,sK1(X0,sK0(X0))) ),
    inference(factoring,[],[f16]) ).

fof(f35,plain,
    ! [X0] :
      ( big_f(sK0(X0),sK1(X0,sK0(X0)))
      | ~ big_f(sK1(X0,sK0(X0)),sK0(X0)) ),
    inference(forward_subsumption_resolution,[],[f31,f9]) ).

fof(f43,plain,
    ! [X0] :
      ( ~ big_f(sK1(X0,sK0(X0)),sK0(X0))
      | big_f(X0,sK1(X0,sK0(X0))) ),
    inference(resolution,[],[f35,f10]) ).

fof(f56,plain,
    ! [X0] :
      ( ~ big_f(sK1(X0,sK0(X0)),sK0(X0))
      | ~ big_f(sK0(X0),sK1(X0,sK0(X0)))
      | ~ big_f(X0,sK1(X0,sK0(X0))) ),
    inference(factoring,[],[f18]) ).

fof(f60,plain,
    ! [X0] :
      ( ~ big_f(sK1(X0,sK0(X0)),sK0(X0))
      | ~ big_f(X0,sK1(X0,sK0(X0))) ),
    inference(forward_subsumption_resolution,[],[f56,f9]) ).

fof(f61,plain,
    ! [X0] : ~ big_f(sK1(X0,sK0(X0)),sK0(X0)),
    inference(forward_subsumption_resolution,[],[f60,f43]) ).

fof(f63,plain,
    ! [X0] :
      ( big_f(sK1(X0,sK0(X0)),sK0(X0))
      | ~ big_f(X0,sK1(X0,sK0(X0)))
      | ~ big_f(sK0(X0),sK1(X0,sK0(X0))) ),
    inference(resolution,[],[f61,f13]) ).

fof(f65,plain,
    ! [X0] :
      ( big_f(sK0(X0),sK1(X0,sK0(X0)))
      | big_f(sK1(X0,sK0(X0)),sK0(X0))
      | big_f(X0,sK1(X0,sK0(X0))) ),
    inference(resolution,[],[f61,f11]) ).

fof(f66,plain,
    ! [X0] :
      ( big_f(sK0(X0),sK1(X0,sK0(X0)))
      | big_f(sK1(X0,sK0(X0)),sK0(X0)) ),
    inference(forward_subsumption_resolution,[],[f65,f9]) ).

fof(f67,plain,
    ! [X0] :
      ( ~ big_f(X0,sK1(X0,sK0(X0)))
      | ~ big_f(sK0(X0),sK1(X0,sK0(X0))) ),
    inference(forward_subsumption_resolution,[],[f63,f61]) ).

fof(f68,plain,
    ! [X0] : big_f(sK0(X0),sK1(X0,sK0(X0))),
    inference(forward_subsumption_resolution,[],[f66,f35]) ).

fof(f69,plain,
    ! [X0] : ~ big_f(sK0(X0),sK1(X0,sK0(X0))),
    inference(forward_subsumption_resolution,[],[f67,f10]) ).

fof(f71,plain,
    $false,
    inference(forward_subsumption_resolution,[],[f69,f68]) ).

%------------------------------------------------------------------------------
%----ORIGINAL SYSTEM OUTPUT
% 0.09/0.14  % Problem    : SYN349+1 : TPTP v9.2.1. Released v2.0.0.
% 0.09/0.14  % Command    : run_vampire %s %d THM
% 0.16/0.34  % Computer   : n026.cluster.edu
% 0.16/0.34  % Model      : x86_64 x86_64
% 0.16/0.34  % CPU        : Intel(R) Xeon(R) CPU E5-2620 v4 @ 2.10GHz
% 0.16/0.34  % Memory     : 8042.1875MB
% 0.16/0.34  % OS         : Linux 3.10.0-693.el7.x86_64
% 0.16/0.34  % CPULimit   : 300
% 0.16/0.34  % WCLimit    : 300
% 0.16/0.34  % DateTime   : Fri May  1 06:02:24 EDT 2026
% 0.16/0.34  % CPUTime    : 
% 0.16/0.36  This is a FOF_THM_RFO_NEQ problem
% 0.16/0.36  Running first-order theorem proving
% 0.16/0.36  Running /export/starexec/sandbox2/solver/bin/vampire --input_syntax tptp --proof tptp --output_axiom_names on --mode casc --cores 7 -m 16384 -t 300 /export/starexec/sandbox2/benchmark/theBenchmark.p
% 0.54/0.76  % (751)Detected formulas, will run a generic FOF schedule.
% 0.60/0.97  % (753)lrs+10_1_ncem=casc2026/models/loop8.pt:sil=128000:tgt=full:npcc=on:drc=off:sp=weighted_frequency:spb=goal:fd=preordered:foolp=on:random_seed=1246979008:i=141193_3000 on theBenchmark for (3000ds/141193Mi)
% 0.60/0.97  % (760)dis-21_1_sil=8000:lcm=predicate:random_seed=877940231:st=5:avsq=on:i=129:avsqr=1,16:sd=3:aac=none:ep=RS:fsr=off:ss=included_3000 on theBenchmark for (3000ds/129Mi)
% 0.60/0.97  % (758)dis-1010_2:3_sil=16000:sp=reverse_frequency:random_seed=1868465109:i=119:av=off:ss=axioms_3000 on theBenchmark for (3000ds/119Mi)
% 0.60/0.97  % (759)dis-1011_1_sil=16000:fde=unused:s2agt=70:random_seed=930210944:s2a=on:i=139:gtg=position_3000 on theBenchmark for (3000ds/139Mi)
% 0.60/0.97  % (757)lrs+1010_1_to=lpo:sil=32000:sos=on:spb=goal_then_units:bce=on:random_seed=1323058114:i=109:sd=1:ins=1:gsp=on:ss=axioms_3000 on theBenchmark for (3000ds/109Mi)
% 0.60/0.97  % (754)lrs+11_1_ncem=casc2026/models/loop8.pt:sil=128000:npcc=on:lma=off:spb=units:urr=ec_only:bce=on:s2agt=64:updr=off:random_seed=2456451942:i=134677:sd=20:aac=none:nm=16:ss=included:sgt=10_3000 on theBenchmark for (3000ds/134677Mi)
% 0.60/0.97  % (756)lrs+1010_1_anc=all:sfv=off:to=kbo:ncem=casc2026/models/loop7.pt:sil=128000:npcc=on:prc=on:sos=all:bsr=unit_only:sac=on:random_seed=3021927149:i=141695:sd=1:nm=32:gsp=on:ss=included_3000 on theBenchmark for (3000ds/141695Mi)
% 0.60/0.97  % (758)First to succeed.
% 0.60/0.97  % (760)Also succeeded, but the first one will report.
% 0.60/0.97  % (759)Also succeeded, but the first one will report.
% 0.60/0.97  % (757)Also succeeded, but the first one will report.
% 0.60/0.97  % (758)Solution written to "/export/starexec/sandbox2/tmp/vampire-proof-751"
% 2.32/1.20  % (758)Refutation found. Thanks to Tanya!
% 2.32/1.20  % SZS status Theorem for theBenchmark
% 2.32/1.20  % SZS output start Proof for theBenchmark
% See solution above
% 2.32/1.20  % (758)------------------------------
% 2.32/1.20  % (758)Version: Vampire 5.0.1 (Release build, commit 1b9f22200 on 2026-04-29 16:18:29 +0200)
% 2.32/1.20  % (758)Linked with Z3 4.14.0.0 3c47fd96cf5645d0c42b2c819d9e9a84380aa721 z3-4.8.4-9178-g3c47fd96c
% 2.32/1.20  % (758)CaDiCaL version: 2.1.3
% 2.32/1.20  % (758)Termination reason: Refutation
% 2.32/1.20  % (758)Time elapsed: 0.002 s
% 2.32/1.20  % (758)Peak memory usage: 80 MB
% 2.32/1.20  % (758)Instructions burned: 3 (million)
% 2.32/1.20  % (758)------------------------------
% 2.32/1.20  % (758)------------------------------
% 2.32/1.20  % (751)Success in time 0.446 s
%------------------------------------------------------------------------------

