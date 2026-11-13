  %------------------------------------------------------------------------------
% File     : Leo-III---1.8.0
% Problem  : SWV406+1 : TPTP v9.3.1. Released v3.3.0.
% Transfm  : none
% Format   : tptp:raw
% Command  : java -Xss128m -Xmx2g -Xms1g -jar /export/starexec/sandbox2/solver/bin/leo3.jar /export/starexec/sandbox2/benchmark/theBenchmark.p -t 300 -p  --atp eprover=/export/starexec/sandbox2/solver/bin/externals/eprover --instantiate 39

% Computer : n016.cluster.edu
% Model    : x86_64 x86_64
% CPU      : Intel(R) Xeon(R) CPU E5-2620 v4 2.10GHz
% Memory   : 8046.5625MB
% OS       : Linux 6.8.0-71-generic
% CPULimit : 300s
% WCLimit  : 300s
% DateTime : Sun Sep 27 08:59:17 AM UTC 2026

% Result   : Theorem 7.07s 18.88s
% Output   : Refutation 8.06s
% Verified :
% SZS Type : Refutation
%            Derivation depth      :    3
%            Number of leaves      :   42
% Syntax   : Number of formulae    :   86 (  34 unt;   0 typ;   0 def)
%            Number of atoms       :  201 (  61 equ;   0 cnn)
%            Maximal formula atoms :   12 (   2 avg)
%            Number of connectives :  925 (  31   ~;   8   |;  33   &; 779   @)
%                                         (  11 <=>;  63  =>;   0  <=;   0 <~>)
%            Maximal formula depth :   19 (   8 avg)
%            Number of types       :    2 (   0 usr)
%            Number of type conns  :    0 (   0   >;   0   *;   0   +;   0  <<)
%            Number of symbols     :   29 (  27 usr;   4 con; 0-3 aty)
%            Number of variables   :  267 (   0   ^; 267   !;   0   ?; 267   :)

% Comments :
%------------------------------------------------------------------------------
thf(check_cpq_decl,type,
    check_cpq: $i > $o ).

thf(triple_decl,type,
    triple: $i > $i > $i > $i ).

thf(pair_in_list_decl,type,
    pair_in_list: $i > $i > $i > $o ).

thf(less_than_decl,type,
    less_than: $i > $i > $o ).

thf(insert_slb_decl,type,
    insert_slb: $i > $i > $i ).

thf(pair_decl,type,
    pair: $i > $i > $i ).

thf(succ_cpq_decl,type,
    succ_cpq: $i > $i > $o ).

thf(removemin_cpq_eff_decl,type,
    removemin_cpq_eff: $i > $i ).

thf(contains_slb_decl,type,
    contains_slb: $i > $i > $o ).

thf(create_slb_decl,type,
    create_slb: $i ).

thf(insert_cpq_decl,type,
    insert_cpq: $i > $i > $i ).

thf(update_slb_decl,type,
    update_slb: $i > $i > $i ).

thf(remove_cpq_decl,type,
    remove_cpq: $i > $i > $i ).

thf(findmin_cpq_eff_decl,type,
    findmin_cpq_eff: $i > $i ).

thf(findmin_cpq_res_decl,type,
    findmin_cpq_res: $i > $i ).

thf(isnonempty_slb_decl,type,
    isnonempty_slb: $i > $o ).

thf(removemin_cpq_res_decl,type,
    removemin_cpq_res: $i > $i ).

thf(remove_slb_decl,type,
    remove_slb: $i > $i > $i ).

thf(bottom_decl,type,
    bottom: $i ).

thf(bad_decl,type,
    bad: $i ).

thf(lookup_slb_decl,type,
    lookup_slb: $i > $i > $i ).

thf(remove_pqp_decl,type,
    remove_pqp: $i > $i > $i ).

thf(findmin_pqp_res_decl,type,
    findmin_pqp_res: $i > $i ).

thf(strictly_less_than_decl,type,
    strictly_less_than: $i > $i > $o ).

thf(ok_decl,type,
    ok: $i > $o ).

thf(insert_pqp_decl,type,
    insert_pqp: $i > $i > $i ).

thf(contains_cpq_decl,type,
    contains_cpq: $i > $i > $o ).

thf(17,axiom,
    ! [A: $i] : ( A @ ( bottom @ less_than ) ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',bottom_smallest) ).

thf(101,plain,
    ! [A: $i] : ( A @ ( bottom @ less_than ) ),
    inference(defexp_and_simp_and_etaexpand,[status(thm)],[17]) ).

thf(36,axiom,
    ! [A: $i,B: $i,C: $i,D: $i] :
      ( ( D @ ( C @ ( B @ ( A @ triple ) ) @ insert_cpq ) )
      = ( C @ ( bottom @ ( D @ pair ) @ ( B @ insert_slb ) @ ( D @ ( A @ insert_pqp ) @ triple ) ) ) ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',ax42) ).

thf(174,plain,
    ! [A: $i,B: $i,C: $i,D: $i] :
      ( ( D @ ( C @ ( B @ ( A @ triple ) ) @ insert_cpq ) )
      = ( C @ ( bottom @ ( D @ pair ) @ ( B @ insert_slb ) @ ( D @ ( A @ insert_pqp ) @ triple ) ) ) ),
    inference(defexp_and_simp_and_etaexpand,[status(thm)],[36]) ).

thf(11,axiom,
    ! [A: $i,B: $i,C: $i] :
      ( ( B @ ( A @ succ_cpq ) )
     => ( C @ ( B @ remove_cpq ) @ ( A @ succ_cpq ) ) ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',ax33) ).

thf(78,plain,
    ! [A: $i,B: $i,C: $i] :
      ( ( B @ ( A @ succ_cpq ) )
     => ( C @ ( B @ remove_cpq ) @ ( A @ succ_cpq ) ) ),
    inference(defexp_and_simp_and_etaexpand,[status(thm)],[11]) ).

thf(4,axiom,
    ! [A: $i] : ( A @ ( A @ succ_cpq ) ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',ax31) ).

thf(60,plain,
    ! [A: $i] : ( A @ ( A @ succ_cpq ) ),
    inference(defexp_and_simp_and_etaexpand,[status(thm)],[4]) ).

thf(28,axiom,
    ! [A: $i,B: $i,C: $i,D: $i,E: $i] :
      ( ( E @ ( C @ ( D @ ( B @ pair ) @ ( A @ insert_slb ) @ pair_in_list ) ) )
    <=> ( ( ( D = E )
          & ( B = C ) )
        | ( E @ ( C @ ( A @ pair_in_list ) ) ) ) ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',ax23) ).

thf(137,plain,
    ! [A: $i,B: $i,C: $i,D: $i,E: $i] :
      ( ( ( ( ( D = E )
            & ( B = C ) )
          | ( E @ ( C @ ( A @ pair_in_list ) ) ) )
       => ( E @ ( C @ ( D @ ( B @ pair ) @ ( A @ insert_slb ) @ pair_in_list ) ) ) )
      & ( ( E @ ( C @ ( D @ ( B @ pair ) @ ( A @ insert_slb ) @ pair_in_list ) ) )
       => ( ( ( D = E )
            & ( B = C ) )
          | ( E @ ( C @ ( A @ pair_in_list ) ) ) ) ) ),
    inference(defexp_and_simp_and_etaexpand,[status(thm)],[28]) ).

thf(41,axiom,
    ! [A: $i,B: $i,C: $i,D: $i] :
      ( ( D @ ( C @ ( B @ ( A @ triple ) ) @ contains_cpq ) )
    <=> ( D @ ( B @ contains_slb ) ) ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',ax39) ).

thf(188,plain,
    ! [A: $i,B: $i,C: $i,D: $i] :
      ( ( ( D @ ( B @ contains_slb ) )
       => ( D @ ( C @ ( B @ ( A @ triple ) ) @ contains_cpq ) ) )
      & ( ( D @ ( C @ ( B @ ( A @ triple ) ) @ contains_cpq ) )
       => ( D @ ( B @ contains_slb ) ) ) ),
    inference(defexp_and_simp_and_etaexpand,[status(thm)],[41]) ).

thf(26,axiom,
    ! [A: $i,B: $i,C: $i,D: $i,E: $i] :
      ( ( D @ ( E @ less_than ) )
     => ( ( C @ ( E @ ( D @ pair ) @ ( B @ insert_slb ) @ ( A @ triple ) ) @ check_cpq )
      <=> ( C @ ( B @ ( A @ triple ) ) @ check_cpq ) ) ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',ax37) ).

thf(129,plain,
    ! [A: $i,B: $i,C: $i,D: $i,E: $i] :
      ( ( D @ ( E @ less_than ) )
     => ( ( ( C @ ( B @ ( A @ triple ) ) @ check_cpq )
         => ( C @ ( E @ ( D @ pair ) @ ( B @ insert_slb ) @ ( A @ triple ) ) @ check_cpq ) )
        & ( ( C @ ( E @ ( D @ pair ) @ ( B @ insert_slb ) @ ( A @ triple ) ) @ check_cpq )
         => ( C @ ( B @ ( A @ triple ) ) @ check_cpq ) ) ) ),
    inference(defexp_and_simp_and_etaexpand,[status(thm)],[26]) ).

thf(1,conjecture,
    ! [A: $i] :
      ( ! [B: $i,C: $i] :
          ( ( C @ ( A @ ( B @ triple ) ) @ check_cpq )
        <=> ! [D: $i,E: $i] :
              ( ( E @ ( D @ ( A @ pair_in_list ) ) )
             => ( D @ ( E @ less_than ) ) ) )
     => ! [B: $i,C: $i,D: $i,E: $i] :
          ( ( C @ ( E @ ( D @ pair ) @ ( A @ insert_slb ) @ ( B @ triple ) ) @ check_cpq )
        <=> ! [F: $i,G: $i] :
              ( ( G @ ( F @ ( E @ ( D @ pair ) @ ( A @ insert_slb ) @ pair_in_list ) ) )
             => ( F @ ( G @ less_than ) ) ) ) ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',l42_co) ).

thf(2,negated_conjecture,
    ~ ! [A: $i] :
        ( ! [B: $i,C: $i] :
            ( ( C @ ( A @ ( B @ triple ) ) @ check_cpq )
          <=> ! [D: $i,E: $i] :
                ( ( E @ ( D @ ( A @ pair_in_list ) ) )
               => ( D @ ( E @ less_than ) ) ) )
       => ! [B: $i,C: $i,D: $i,E: $i] :
            ( ( C @ ( E @ ( D @ pair ) @ ( A @ insert_slb ) @ ( B @ triple ) ) @ check_cpq )
          <=> ! [F: $i,G: $i] :
                ( ( G @ ( F @ ( E @ ( D @ pair ) @ ( A @ insert_slb ) @ pair_in_list ) ) )
               => ( F @ ( G @ less_than ) ) ) ) ),
    inference(neg_conjecture,[status(cth)],[1]) ).

thf(44,plain,
    ~ ! [A: $i] :
        ( ! [B: $i,C: $i] :
            ( ( ! [D: $i,E: $i] :
                  ( ( E @ ( D @ ( A @ pair_in_list ) ) )
                 => ( D @ ( E @ less_than ) ) )
             => ( C @ ( A @ ( B @ triple ) ) @ check_cpq ) )
            & ( ( C @ ( A @ ( B @ triple ) ) @ check_cpq )
             => ! [D: $i,E: $i] :
                  ( ( E @ ( D @ ( A @ pair_in_list ) ) )
                 => ( D @ ( E @ less_than ) ) ) ) )
       => ! [B: $i,C: $i,D: $i,E: $i] :
            ( ( ! [F: $i,G: $i] :
                  ( ( G @ ( F @ ( E @ ( D @ pair ) @ ( A @ insert_slb ) @ pair_in_list ) ) )
                 => ( F @ ( G @ less_than ) ) )
             => ( C @ ( E @ ( D @ pair ) @ ( A @ insert_slb ) @ ( B @ triple ) ) @ check_cpq ) )
            & ( ( C @ ( E @ ( D @ pair ) @ ( A @ insert_slb ) @ ( B @ triple ) ) @ check_cpq )
             => ! [F: $i,G: $i] :
                  ( ( G @ ( F @ ( E @ ( D @ pair ) @ ( A @ insert_slb ) @ pair_in_list ) ) )
                 => ( F @ ( G @ less_than ) ) ) ) ) ),
    inference(defexp_and_simp_and_etaexpand,[status(thm)],[2]) ).

thf(18,axiom,
    ! [A: $i,B: $i,C: $i,D: $i] :
      ( ~ ( D @ ( B @ contains_slb ) )
     => ( ( D @ ( C @ ( B @ ( A @ triple ) ) @ remove_cpq ) )
        = ( bad @ ( B @ ( A @ triple ) ) ) ) ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',ax43) ).

thf(103,plain,
    ! [A: $i,B: $i,C: $i,D: $i] :
      ( ~ ( D @ ( B @ contains_slb ) )
     => ( ( D @ ( C @ ( B @ ( A @ triple ) ) @ remove_cpq ) )
        = ( bad @ ( B @ ( A @ triple ) ) ) ) ),
    inference(defexp_and_simp_and_etaexpand,[status(thm)],[18]) ).

thf(31,axiom,
    ! [A: $i,B: $i,C: $i] :
      ( ~ ( C @ ( B @ ( A @ triple ) ) @ ok )
     => ( C = bad ) ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',ax41) ).

thf(154,plain,
    ! [A: $i,B: $i,C: $i] :
      ( ~ ( C @ ( B @ ( A @ triple ) ) @ ok )
     => ( C = bad ) ),
    inference(defexp_and_simp_and_etaexpand,[status(thm)],[31]) ).

thf(3,axiom,
    ! [A: $i,B: $i] :
      ( ( B @ ( A @ succ_cpq ) )
     => ( B @ removemin_cpq_eff @ ( A @ succ_cpq ) ) ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',ax35) ).

thf(58,plain,
    ! [A: $i,B: $i] :
      ( ( B @ ( A @ succ_cpq ) )
     => ( B @ removemin_cpq_eff @ ( A @ succ_cpq ) ) ),
    inference(defexp_and_simp_and_etaexpand,[status(thm)],[3]) ).

thf(30,axiom,
    ! [A: $i,B: $i] :
      ( ( B @ ( create_slb @ ( A @ triple ) ) @ findmin_cpq_res )
      = bottom ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',ax50) ).

thf(151,plain,
    ! [A: $i,B: $i] :
      ( ( B @ ( create_slb @ ( A @ triple ) ) @ findmin_cpq_res )
      = bottom ),
    inference(defexp_and_simp_and_etaexpand,[status(thm)],[30]) ).

thf(25,axiom,
    ! [A: $i,B: $i] :
      ~ ( B @ ( A @ ( create_slb @ pair_in_list ) ) ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',ax22) ).

thf(126,plain,
    ! [A: $i,B: $i] :
      ~ ( B @ ( A @ ( create_slb @ pair_in_list ) ) ),
    inference(defexp_and_simp_and_etaexpand,[status(thm)],[25]) ).

thf(35,axiom,
    ! [A: $i,B: $i,C: $i,D: $i] :
      ( ( ( A @ findmin_pqp_res @ ( A @ findmin_pqp_res @ ( B @ lookup_slb ) @ less_than ) )
        & ( A @ findmin_pqp_res @ ( B @ contains_slb ) )
        & ( B != create_slb ) )
     => ( ( C @ ( B @ ( A @ triple ) ) @ findmin_cpq_eff )
        = ( C @ ( A @ findmin_pqp_res @ ( B @ update_slb ) @ ( A @ triple ) ) ) ) ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',ax49) ).

thf(170,plain,
    ! [A: $i,B: $i,C: $i] :
      ( ( ( A @ findmin_pqp_res @ ( A @ findmin_pqp_res @ ( B @ lookup_slb ) @ less_than ) )
        & ( A @ findmin_pqp_res @ ( B @ contains_slb ) )
        & ( B != create_slb ) )
     => ( ( C @ ( B @ ( A @ triple ) ) @ findmin_cpq_eff )
        = ( C @ ( A @ findmin_pqp_res @ ( B @ update_slb ) @ ( A @ triple ) ) ) ) ),
    inference(defexp_and_simp_and_etaexpand,[status(thm)],[35]) ).

thf(23,axiom,
    ! [A: $i,B: $i] :
      ( ( B @ ( create_slb @ ( A @ triple ) ) @ findmin_cpq_eff )
      = ( bad @ ( create_slb @ ( A @ triple ) ) ) ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',ax46) ).

thf(120,plain,
    ! [A: $i,B: $i] :
      ( ( B @ ( create_slb @ ( A @ triple ) ) @ findmin_cpq_eff )
      = ( bad @ ( create_slb @ ( A @ triple ) ) ) ),
    inference(defexp_and_simp_and_etaexpand,[status(thm)],[23]) ).

thf(40,axiom,
    ! [A: $i,B: $i,C: $i,D: $i] :
      ( ( ( A @ findmin_pqp_res @ ( B @ lookup_slb ) @ ( A @ findmin_pqp_res @ strictly_less_than ) )
        & ( A @ findmin_pqp_res @ ( B @ contains_slb ) )
        & ( B != create_slb ) )
     => ( ( C @ ( B @ ( A @ triple ) ) @ findmin_cpq_eff )
        = ( bad @ ( A @ findmin_pqp_res @ ( B @ update_slb ) @ ( A @ triple ) ) ) ) ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',ax48) ).

thf(184,plain,
    ! [A: $i,B: $i,C: $i] :
      ( ( ( A @ findmin_pqp_res @ ( B @ lookup_slb ) @ ( A @ findmin_pqp_res @ strictly_less_than ) )
        & ( A @ findmin_pqp_res @ ( B @ contains_slb ) )
        & ( B != create_slb ) )
     => ( ( C @ ( B @ ( A @ triple ) ) @ findmin_cpq_eff )
        = ( bad @ ( A @ findmin_pqp_res @ ( B @ update_slb ) @ ( A @ triple ) ) ) ) ),
    inference(defexp_and_simp_and_etaexpand,[status(thm)],[40]) ).

thf(32,axiom,
    ! [A: $i,B: $i,C: $i,D: $i] :
      ( ( ( C @ ( A @ contains_slb ) )
        & ( B != C ) )
     => ( ( C @ ( D @ ( B @ pair ) @ ( A @ insert_slb ) @ lookup_slb ) )
        = ( C @ ( A @ lookup_slb ) ) ) ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',ax27) ).

thf(157,plain,
    ! [A: $i,B: $i,C: $i,D: $i] :
      ( ( ( C @ ( A @ contains_slb ) )
        & ( B != C ) )
     => ( ( C @ ( D @ ( B @ pair ) @ ( A @ insert_slb ) @ lookup_slb ) )
        = ( C @ ( A @ lookup_slb ) ) ) ),
    inference(defexp_and_simp_and_etaexpand,[status(thm)],[32]) ).

thf(38,axiom,
    ! [A: $i,B: $i,C: $i] :
      ( ( B @ ( C @ ( B @ pair ) @ ( A @ insert_slb ) @ lookup_slb ) )
      = C ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',ax26) ).

thf(179,plain,
    ! [A: $i,B: $i,C: $i] :
      ( ( B @ ( C @ ( B @ pair ) @ ( A @ insert_slb ) @ lookup_slb ) )
      = C ),
    inference(defexp_and_simp_and_etaexpand,[status(thm)],[38]) ).

thf(19,axiom,
    ! [A: $i,B: $i,C: $i,D: $i] :
      ( ( ( D @ ( D @ ( B @ lookup_slb ) @ less_than ) )
        & ( D @ ( B @ contains_slb ) ) )
     => ( ( D @ ( C @ ( B @ ( A @ triple ) ) @ remove_cpq ) )
        = ( C @ ( D @ ( B @ remove_slb ) @ ( D @ ( A @ remove_pqp ) @ triple ) ) ) ) ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',ax44) ).

thf(106,plain,
    ! [A: $i,B: $i,C: $i,D: $i] :
      ( ( ( D @ ( D @ ( B @ lookup_slb ) @ less_than ) )
        & ( D @ ( B @ contains_slb ) ) )
     => ( ( D @ ( C @ ( B @ ( A @ triple ) ) @ remove_cpq ) )
        = ( C @ ( D @ ( B @ remove_slb ) @ ( D @ ( A @ remove_pqp ) @ triple ) ) ) ) ),
    inference(defexp_and_simp_and_etaexpand,[status(thm)],[19]) ).

thf(13,axiom,
    ! [A: $i,B: $i,C: $i] : ( C @ ( B @ pair ) @ ( A @ insert_slb ) @ isnonempty_slb ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',ax19) ).

thf(84,plain,
    ! [A: $i,B: $i,C: $i] : ( C @ ( B @ pair ) @ ( A @ insert_slb ) @ isnonempty_slb ),
    inference(defexp_and_simp_and_etaexpand,[status(thm)],[13]) ).

thf(27,axiom,
    ! [A: $i,B: $i,C: $i,D: $i] :
      ( ( ~ ( A @ findmin_pqp_res @ ( B @ contains_slb ) )
        & ( B != create_slb ) )
     => ( ( C @ ( B @ ( A @ triple ) ) @ findmin_cpq_eff )
        = ( bad @ ( A @ findmin_pqp_res @ ( B @ update_slb ) @ ( A @ triple ) ) ) ) ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',ax47) ).

thf(133,plain,
    ! [A: $i,B: $i,C: $i] :
      ( ( ~ ( A @ findmin_pqp_res @ ( B @ contains_slb ) )
        & ( B != create_slb ) )
     => ( ( C @ ( B @ ( A @ triple ) ) @ findmin_cpq_eff )
        = ( bad @ ( A @ findmin_pqp_res @ ( B @ update_slb ) @ ( A @ triple ) ) ) ) ),
    inference(defexp_and_simp_and_etaexpand,[status(thm)],[27]) ).

thf(22,axiom,
    ! [A: $i,B: $i,C: $i,D: $i] :
      ( ( ( C @ ( A @ contains_slb ) )
        & ( B != C ) )
     => ( ( C @ ( D @ ( B @ pair ) @ ( A @ insert_slb ) @ remove_slb ) )
        = ( D @ ( B @ pair ) @ ( C @ ( A @ remove_slb ) @ insert_slb ) ) ) ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',ax25) ).

thf(116,plain,
    ! [A: $i,B: $i,C: $i,D: $i] :
      ( ( ( C @ ( A @ contains_slb ) )
        & ( B != C ) )
     => ( ( C @ ( D @ ( B @ pair ) @ ( A @ insert_slb ) @ remove_slb ) )
        = ( D @ ( B @ pair ) @ ( C @ ( A @ remove_slb ) @ insert_slb ) ) ) ),
    inference(defexp_and_simp_and_etaexpand,[status(thm)],[22]) ).

thf(9,axiom,
    ! [A: $i,B: $i] :
      ( ( B @ ( A @ succ_cpq ) )
     => ( B @ findmin_cpq_eff @ ( A @ succ_cpq ) ) ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',ax34) ).

thf(74,plain,
    ! [A: $i,B: $i] :
      ( ( B @ ( A @ succ_cpq ) )
     => ( B @ findmin_cpq_eff @ ( A @ succ_cpq ) ) ),
    inference(defexp_and_simp_and_etaexpand,[status(thm)],[9]) ).

thf(43,axiom,
    ! [A: $i,B: $i,C: $i,D: $i] :
      ( ( ( D @ ( B @ lookup_slb ) @ ( D @ strictly_less_than ) )
        & ( D @ ( B @ contains_slb ) ) )
     => ( ( D @ ( C @ ( B @ ( A @ triple ) ) @ remove_cpq ) )
        = ( bad @ ( D @ ( B @ remove_slb ) @ ( D @ ( A @ remove_pqp ) @ triple ) ) ) ) ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',ax45) ).

thf(197,plain,
    ! [A: $i,B: $i,C: $i,D: $i] :
      ( ( ( D @ ( B @ lookup_slb ) @ ( D @ strictly_less_than ) )
        & ( D @ ( B @ contains_slb ) ) )
     => ( ( D @ ( C @ ( B @ ( A @ triple ) ) @ remove_cpq ) )
        = ( bad @ ( D @ ( B @ remove_slb ) @ ( D @ ( A @ remove_pqp ) @ triple ) ) ) ) ),
    inference(defexp_and_simp_and_etaexpand,[status(thm)],[43]) ).

thf(6,axiom,
    ! [A: $i,B: $i,C: $i] :
      ( ( B @ ( A @ succ_cpq ) )
     => ( C @ ( B @ insert_cpq ) @ ( A @ succ_cpq ) ) ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',ax32) ).

thf(65,plain,
    ! [A: $i,B: $i,C: $i] :
      ( ( B @ ( A @ succ_cpq ) )
     => ( C @ ( B @ insert_cpq ) @ ( A @ succ_cpq ) ) ),
    inference(defexp_and_simp_and_etaexpand,[status(thm)],[6]) ).

thf(20,axiom,
    ! [A: $i,B: $i,C: $i,D: $i] :
      ( ( B != create_slb )
     => ( ( C @ ( B @ ( A @ triple ) ) @ findmin_cpq_res )
        = ( A @ findmin_pqp_res ) ) ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',ax51) ).

thf(109,plain,
    ! [A: $i,B: $i,C: $i] :
      ( ( B != create_slb )
     => ( ( C @ ( B @ ( A @ triple ) ) @ findmin_cpq_res )
        = ( A @ findmin_pqp_res ) ) ),
    inference(defexp_and_simp_and_etaexpand,[status(thm)],[20]) ).

thf(15,axiom,
    ! [A: $i] : ( A @ ( A @ less_than ) ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',reflexivity) ).

thf(96,plain,
    ! [A: $i] : ( A @ ( A @ less_than ) ),
    inference(defexp_and_simp_and_etaexpand,[status(thm)],[15]) ).

thf(33,axiom,
    ! [A: $i,B: $i,C: $i,D: $i,E: $i] :
      ( ( E @ ( D @ strictly_less_than ) )
     => ( ( C @ ( E @ ( D @ pair ) @ ( B @ insert_slb ) @ ( A @ triple ) ) @ check_cpq )
      <=> $false ) ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',ax38) ).

thf(161,plain,
    ! [A: $i,B: $i,C: $i,D: $i,E: $i] :
      ( ( E @ ( D @ strictly_less_than ) )
     => ~ ( C @ ( E @ ( D @ pair ) @ ( B @ insert_slb ) @ ( A @ triple ) ) @ check_cpq ) ),
    inference(defexp_and_simp_and_etaexpand,[status(thm)],[33]) ).

thf(29,axiom,
    ! [A: $i,B: $i] : ( B @ ( create_slb @ ( A @ triple ) ) @ check_cpq ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',ax36) ).

thf(149,plain,
    ! [A: $i,B: $i] : ( B @ ( create_slb @ ( A @ triple ) ) @ check_cpq ),
    inference(defexp_and_simp_and_etaexpand,[status(thm)],[29]) ).

thf(8,axiom,
    ! [A: $i] :
      ( ( A @ removemin_cpq_eff )
      = ( A @ findmin_cpq_res @ ( A @ findmin_cpq_eff @ remove_cpq ) ) ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',ax52) ).

thf(71,plain,
    ! [A: $i] :
      ( ( A @ removemin_cpq_eff )
      = ( A @ findmin_cpq_res @ ( A @ findmin_cpq_eff @ remove_cpq ) ) ),
    inference(defexp_and_simp_and_etaexpand,[status(thm)],[8]) ).

thf(14,axiom,
    ! [A: $i,B: $i,C: $i,D: $i] :
      ( ( C @ ( D @ ( B @ pair ) @ ( A @ insert_slb ) @ contains_slb ) )
    <=> ( ( B = C )
        | ( C @ ( A @ contains_slb ) ) ) ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',ax21) ).

thf(86,plain,
    ! [A: $i,B: $i,C: $i,D: $i] :
      ( ( ( ( B = C )
          | ( C @ ( A @ contains_slb ) ) )
       => ( C @ ( D @ ( B @ pair ) @ ( A @ insert_slb ) @ contains_slb ) ) )
      & ( ( C @ ( D @ ( B @ pair ) @ ( A @ insert_slb ) @ contains_slb ) )
       => ( ( B = C )
          | ( C @ ( A @ contains_slb ) ) ) ) ),
    inference(defexp_and_simp_and_etaexpand,[status(thm)],[14]) ).

thf(21,axiom,
    ! [A: $i,B: $i,C: $i,D: $i] :
      ( ( C @ ( D @ strictly_less_than ) )
     => ( ( C @ ( D @ ( B @ pair ) @ ( A @ insert_slb ) @ update_slb ) )
        = ( C @ ( B @ pair ) @ ( C @ ( A @ update_slb ) @ insert_slb ) ) ) ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',ax29) ).

thf(113,plain,
    ! [A: $i,B: $i,C: $i,D: $i] :
      ( ( C @ ( D @ strictly_less_than ) )
     => ( ( C @ ( D @ ( B @ pair ) @ ( A @ insert_slb ) @ update_slb ) )
        = ( C @ ( B @ pair ) @ ( C @ ( A @ update_slb ) @ insert_slb ) ) ) ),
    inference(defexp_and_simp_and_etaexpand,[status(thm)],[21]) ).

thf(12,axiom,
    ! [A: $i] :
      ( ( A @ removemin_cpq_res )
      = ( A @ findmin_cpq_res ) ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',ax53) ).

thf(81,plain,
    ! [A: $i] :
      ( ( A @ removemin_cpq_res )
      = ( A @ findmin_cpq_res ) ),
    inference(defexp_and_simp_and_etaexpand,[status(thm)],[12]) ).

thf(10,axiom,
    ~ ( create_slb @ isnonempty_slb ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',ax18) ).

thf(76,plain,
    ~ ( create_slb @ isnonempty_slb ),
    inference(defexp_and_simp_and_etaexpand,[status(thm)],[10]) ).

thf(16,axiom,
    ! [A: $i,B: $i,C: $i] :
      ( ( B @ ( C @ ( B @ pair ) @ ( A @ insert_slb ) @ remove_slb ) )
      = A ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',ax24) ).

thf(98,plain,
    ! [A: $i,B: $i,C: $i] :
      ( ( B @ ( C @ ( B @ pair ) @ ( A @ insert_slb ) @ remove_slb ) )
      = A ),
    inference(defexp_and_simp_and_etaexpand,[status(thm)],[16]) ).

thf(24,axiom,
    ! [A: $i,B: $i] :
      ( ( bad @ ( B @ ( A @ triple ) ) @ ok )
    <=> $false ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',ax40) ).

thf(123,plain,
    ! [A: $i,B: $i] :
      ~ ( bad @ ( B @ ( A @ triple ) ) @ ok ),
    inference(defexp_and_simp_and_etaexpand,[status(thm)],[24]) ).

thf(42,axiom,
    ! [A: $i,B: $i,C: $i,D: $i] :
      ( ( D @ ( C @ less_than ) )
     => ( ( C @ ( D @ ( B @ pair ) @ ( A @ insert_slb ) @ update_slb ) )
        = ( D @ ( B @ pair ) @ ( C @ ( A @ update_slb ) @ insert_slb ) ) ) ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',ax30) ).

thf(194,plain,
    ! [A: $i,B: $i,C: $i,D: $i] :
      ( ( D @ ( C @ less_than ) )
     => ( ( C @ ( D @ ( B @ pair ) @ ( A @ insert_slb ) @ update_slb ) )
        = ( D @ ( B @ pair ) @ ( C @ ( A @ update_slb ) @ insert_slb ) ) ) ),
    inference(defexp_and_simp_and_etaexpand,[status(thm)],[42]) ).

thf(37,axiom,
    ! [A: $i,B: $i,C: $i] :
      ( ( ( C @ ( B @ less_than ) )
        & ( B @ ( A @ less_than ) ) )
     => ( C @ ( A @ less_than ) ) ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',transitivity) ).

thf(177,plain,
    ! [A: $i,B: $i,C: $i] :
      ( ( ( C @ ( B @ less_than ) )
        & ( B @ ( A @ less_than ) ) )
     => ( C @ ( A @ less_than ) ) ),
    inference(defexp_and_simp_and_etaexpand,[status(thm)],[37]) ).

thf(39,axiom,
    ! [A: $i,B: $i] :
      ( ( A @ ( B @ less_than ) )
      | ( B @ ( A @ less_than ) ) ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',totality) ).

thf(182,plain,
    ! [A: $i,B: $i] :
      ( ( A @ ( B @ less_than ) )
      | ( B @ ( A @ less_than ) ) ),
    inference(defexp_and_simp_and_etaexpand,[status(thm)],[39]) ).

thf(34,axiom,
    ! [A: $i,B: $i] :
      ( ( B @ ( A @ strictly_less_than ) )
    <=> ( ~ ( A @ ( B @ less_than ) )
        & ( B @ ( A @ less_than ) ) ) ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',stricly_smaller_definition) ).

thf(163,plain,
    ! [A: $i,B: $i] :
      ( ( ( ~ ( A @ ( B @ less_than ) )
          & ( B @ ( A @ less_than ) ) )
       => ( B @ ( A @ strictly_less_than ) ) )
      & ( ( B @ ( A @ strictly_less_than ) )
       => ( ~ ( A @ ( B @ less_than ) )
          & ( B @ ( A @ less_than ) ) ) ) ),
    inference(defexp_and_simp_and_etaexpand,[status(thm)],[34]) ).

thf(7,axiom,
    ! [A: $i] :
      ( ( A @ ( create_slb @ update_slb ) )
      = create_slb ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',ax28) ).

thf(68,plain,
    ! [A: $i] :
      ( ( A @ ( create_slb @ update_slb ) )
      = create_slb ),
    inference(defexp_and_simp_and_etaexpand,[status(thm)],[7]) ).

thf(5,axiom,
    ! [A: $i] :
      ~ ( A @ ( create_slb @ contains_slb ) ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',ax20) ).

thf(62,plain,
    ! [A: $i] :
      ~ ( A @ ( create_slb @ contains_slb ) ),
    inference(defexp_and_simp_and_etaexpand,[status(thm)],[5]) ).

thf(335,plain,
    $false,
    inference(e,[status(thm)],[101,174,78,60,137,188,129,44,103,154,58,151,126,170,120,184,157,179,106,84,133,116,74,197,65,109,96,161,149,71,86,113,81,76,98,123,194,177,182,163,68,62]) ).

%------------------------------------------------------------------------------
%----ORIGINAL SYSTEM OUTPUT
% 0.00/0.05  % Problem  : SWV406+1 : TPTP v9.3.1. Released v3.3.0.
% 0.00/0.11  % Command  : java -Xss128m -Xmx2g -Xms1g -jar /export/starexec/sandbox2/solver/bin/leo3.jar /export/starexec/sandbox2/benchmark/theBenchmark.p -t 300 -p  --atp eprover=/export/starexec/sandbox2/solver/bin/externals/eprover --instantiate 39
% 0.09/5.38   % Computer : n016.cluster.edu
% 0.09/5.38   % Model    : x86_64 x86_64
% 0.09/5.38   % CPU      : Intel(R) Xeon(R) CPU E5-2620 v4 @ 2.10GHz
% 0.09/5.38   % Memory   : 8046.5625MB
% 0.09/5.38   % OS       : Linux 6.8.0-71-generic
% 0.19/15.73  % CPULimit : 300
% 0.19/15.73  % WCLimit  : 300
% 0.19/15.73  % DateTime : Sat Sep 26 13:42:42 UTC 2026
% 0.19/15.73  % CPUTime  :
% 0.19/15.73  Running java -Xss128m -Xmx2g -Xms1g -jar /export/starexec/sandbox2/solver/bin/leo3.jar /export/starexec/sandbox2/benchmark/theBenchmark.p -t 300 -p  --atp eprover=/export/starexec/sandbox2/solver/bin/externals/eprover --instantiate 39
% 1.24/16.32  % [INFO] 	 Parsing problem /export/starexec/sandbox2/benchmark/theBenchmark.p ...
% 1.57/16.56  % [INFO] 	 Parsing done (242ms).
% 1.57/16.58  % [INFO] 	 Running in sequential loop mode.
% 2.30/16.97  % [INFO] 	 eprover registered as external prover.
% 2.30/16.98  % [INFO] 	 Scanning for conjecture ...
% 2.54/17.10  % [INFO] 	 Found a conjecture (or negated_conjecture) and 41 axioms. Running axiom selection ...
% 2.85/17.20  % [INFO] 	 Axiom selection finished. Selected 41 axioms (removed 0 axioms).
% 2.85/17.27  % [INFO] 	 Problem is first-order (TPTP FOF).
% 2.85/17.29  % [INFO] 	 Type checking passed.
% 2.85/17.29  % [CONFIG] 	 Using configuration: timeout(300) with strategy<name(default),share(1.0),primSubst(3),sos(false),unifierCount(4),uniDepth(8),boolExt(true),choice(true),renaming(true),funcspec(false), domConstr(0),specialInstances(39),restrictUniAttempts(true),termOrdering(CPO)>.  Searching for refutation ...
% 7.07/18.87  % External prover 'e' found a proof!
% 7.07/18.88  % [INFO] 	 Killing All external provers ...
% 7.07/18.88  % Time passed: 3002ms (effective reasoning time: 2292ms)
% 7.07/18.88  % Solved by strategy<name(default),share(1.0),primSubst(3),sos(false),unifierCount(4),uniDepth(8),boolExt(true),choice(true),renaming(true),funcspec(false), domConstr(0),specialInstances(39),restrictUniAttempts(true),termOrdering(CPO)>
% 7.07/18.88  % Axioms used in derivation (41): ax28, ax39, ax27, ax32, ax22, ax43, ax19, ax49, ax25, ax53, ax20, ax52, ax24, ax47, ax41, stricly_smaller_definition, ax31, ax45, ax40, ax35, ax51, ax48, ax21, ax38, ax44, reflexivity, ax33, ax26, bottom_smallest, ax50, ax37, totality, ax46, ax34, ax23, transitivity, ax30, ax42, ax18, ax36, ax29
% 7.07/18.88  % No. of inferences in proof: 86
% 7.07/18.88  % SZS status Theorem for /export/starexec/sandbox2/benchmark/theBenchmark.p : 3002 ms resp. 2292 ms w/o parsing
% 8.06/18.96  % SZS output start Refutation for /export/starexec/sandbox2/benchmark/theBenchmark.p
% See solution above
% 8.06/18.96  % [INFO] 	 Killing All external provers ...
%------------------------------------------------------------------------------
