% Proof : Problems/SYN315+1.p
%------------------------------------------------------------------------------
% File     : E---3.3.0
% Problem  : SYN315+1 : TPTP v9.2.0. Released v2.0.0.
% Transfm  : none
% Format   : tptp:raw
% Command  : run_E /export/starexec/sandbox/benchmark/theBenchmark.p 300 THM

% Computer : n031.cluster.edu
% Model    : x86_64 x86_64
% CPU      : Intel(R) Xeon(R) CPU E5-2620 v4 2.10GHz
% Memory   : 8042.1875MB
% OS       : Linux 3.10.0-693.el7.x86_64
% CPULimit : 300s
% WCLimit  : 300s
% DateTime : Mon Sep 29 11:24:48 PM UTC 2025

% Result   : Theorem 0.17s 0.49s
% Output   : CNFRefutation 0.17s
% Verified : 
% SZS Type : Refutation
%            Derivation depth      :    6
%            Number of leaves      :    1
% Syntax   : Number of formulae    :   10 (   3 unt;   0 def)
%            Number of atoms       :   27 (   0 equ)
%            Maximal formula atoms :    8 (   2 avg)
%            Number of connectives :   27 (  10   ~;   8   |;   3   &)
%                                         (   4 <=>;   2  =>;   0  <=;   0 <~>)
%            Maximal formula depth :    8 (   3 avg)
%            Maximal term depth    :    1 (   1 avg)
%            Number of predicates  :    3 (   2 usr;   2 prp; 0-1 aty)
%            Number of functors    :    1 (   1 usr;   1 con; 0-0 aty)
%            Number of variables   :    8 (   2 sgn   4   !;   2   ?)

% Comments : 
%------------------------------------------------------------------------------
fof(church_46_2_1,conjecture,
    ? [X1] :
    ! [X2] :
      ( ( big_f(X1)
      <=> p )
     => ( big_f(X2)
      <=> p ) ),
    file('/export/starexec/sandbox/benchmark/theBenchmark.p',church_46_2_1) ).

fof(c_0_1,negated_conjecture,
    ~ ? [X1] :
      ! [X2] :
        ( ( big_f(X1)
        <=> p )
       => ( big_f(X2)
        <=> p ) ),
    inference(assume_negation,[status(cth)],[church_46_2_1]) ).

fof(c_0_2,negated_conjecture,
    ! [X3,X4] :
      ( ( ~ big_f(X3)
        | p )
      & ( ~ p
        | big_f(X4) )
      & ( ~ big_f(esk1_0)
        | ~ p )
      & ( big_f(esk1_0)
        | p ) ),
    inference(fof_nnf,[status(thm)],[inference(shift_quantors,[status(thm)],[inference(skolemize,[status(esa)],[inference(variable_rename,[status(thm)],[inference(shift_quantors,[status(thm)],[inference(fof_nnf,[status(thm)],[c_0_1])])])])])]) ).

fof(c_0_3,negated_conjecture,
    ( ~ big_f(esk1_0)
    | ~ p ),
    inference(split_conjunct,[status(thm)],[c_0_2]) ).

fof(c_0_4,negated_conjecture,
    ( big_f(X1)
    | ~ p ),
    inference(split_conjunct,[status(thm)],[c_0_2]) ).

fof(c_0_5,negated_conjecture,
    ( big_f(esk1_0)
    | p ),
    inference(split_conjunct,[status(thm)],[c_0_2]) ).

fof(c_0_6,negated_conjecture,
    ~ p,
    inference(spm,[status(thm)],[c_0_3,c_0_4]) ).

fof(c_0_7,negated_conjecture,
    ( p
    | ~ big_f(X1) ),
    inference(split_conjunct,[status(thm)],[c_0_2]) ).

fof(c_0_8,negated_conjecture,
    big_f(esk1_0),
    inference(sr,[status(thm)],[c_0_5,c_0_6]) ).

fof(c_0_9,negated_conjecture,
    $false,
    inference(sr,[status(thm)],[inference(spm,[status(thm)],[c_0_7,c_0_8]),c_0_6]),
    [proof] ).

%------------------------------------------------------------------------------
%----ORIGINAL SYSTEM OUTPUT
% 0.03/0.11  % Problem    : SYN315+1 : TPTP v9.2.0. Released v2.0.0.
% 0.03/0.11  % Command    : run_E /export/starexec/sandbox/benchmark/theBenchmark.p 300 THM
% 0.11/0.32  % Computer : n031.cluster.edu
% 0.11/0.32  % Model    : x86_64 x86_64
% 0.11/0.32  % CPU      : Intel(R) Xeon(R) CPU E5-2620 v4 @ 2.10GHz
% 0.11/0.32  % Memory   : 8042.1875MB
% 0.11/0.32  % OS       : Linux 3.10.0-693.el7.x86_64
% 0.11/0.32  % CPULimit   : 300
% 0.11/0.32  % WCLimit    : 300
% 0.11/0.32  % DateTime   : Fri Sep 26 14:44:23 EDT 2025
% 0.11/0.32  % CPUTime    : 
% 0.17/0.46  Running first-order theorem proving
% 0.17/0.46  Running: /export/starexec/sandbox/solver/bin/eprover --delete-bad-limit=2000000000 --definitional-cnf=24 -s --print-statistics -R --print-version --proof-object --auto-schedule=8 --cpu-limit=300 /export/starexec/sandbox/benchmark/theBenchmark.p
% 0.17/0.49  # Version: 3.0.0
% 0.17/0.49  # Preprocessing class: FSSSSMSSSSSNFFN.
% 0.17/0.49  # Scheduled 4 strats onto 8 cores with 300 seconds (2400 total)
% 0.17/0.49  # Starting G-E--_302_C18_F1_URBAN_RG_S04BN with 1500s (5) cores
% 0.17/0.49  # Starting new_bool_3 with 300s (1) cores
% 0.17/0.49  # Starting new_bool_1 with 300s (1) cores
% 0.17/0.49  # Starting sh5l with 300s (1) cores
% 0.17/0.49  # sh5l with pid 16109 completed with status 0
% 0.17/0.49  # Result found by sh5l
% 0.17/0.49  # Preprocessing class: FSSSSMSSSSSNFFN.
% 0.17/0.49  # Scheduled 4 strats onto 8 cores with 300 seconds (2400 total)
% 0.17/0.49  # Starting G-E--_302_C18_F1_URBAN_RG_S04BN with 1500s (5) cores
% 0.17/0.49  # Starting new_bool_3 with 300s (1) cores
% 0.17/0.49  # Starting new_bool_1 with 300s (1) cores
% 0.17/0.49  # Starting sh5l with 300s (1) cores
% 0.17/0.49  # SinE strategy is gf500_gu_R04_F100_L20000
% 0.17/0.49  # Search class: FGHNF-FFSF00-SFFFFFNN
% 0.17/0.49  # Scheduled 5 strats onto 1 cores with 300 seconds (300 total)
% 0.17/0.49  # Starting SAT001_MinMin_p005000_rr_RG with 181s (1) cores
% 0.17/0.49  # SAT001_MinMin_p005000_rr_RG with pid 16111 completed with status 0
% 0.17/0.49  # Result found by SAT001_MinMin_p005000_rr_RG
% 0.17/0.49  # Preprocessing class: FSSSSMSSSSSNFFN.
% 0.17/0.49  # Scheduled 4 strats onto 8 cores with 300 seconds (2400 total)
% 0.17/0.49  # Starting G-E--_302_C18_F1_URBAN_RG_S04BN with 1500s (5) cores
% 0.17/0.49  # Starting new_bool_3 with 300s (1) cores
% 0.17/0.49  # Starting new_bool_1 with 300s (1) cores
% 0.17/0.49  # Starting sh5l with 300s (1) cores
% 0.17/0.49  # SinE strategy is gf500_gu_R04_F100_L20000
% 0.17/0.49  # Search class: FGHNF-FFSF00-SFFFFFNN
% 0.17/0.49  # Scheduled 5 strats onto 1 cores with 300 seconds (300 total)
% 0.17/0.49  # Starting SAT001_MinMin_p005000_rr_RG with 181s (1) cores
% 0.17/0.49  # Preprocessing time       : 0.001 s
% 0.17/0.49  # Presaturation interreduction done
% 0.17/0.49  
% 0.17/0.49  # Proof found!
% 0.17/0.49  # SZS status Theorem
% 0.17/0.49  # SZS output start CNFRefutation
% See solution above
% 0.17/0.49  # Parsed axioms                        : 1
% 0.17/0.49  # Removed by relevancy pruning/SinE    : 0
% 0.17/0.49  # Initial clauses                      : 4
% 0.17/0.49  # Removed in clause preprocessing      : 0
% 0.17/0.49  # Initial clauses in saturation        : 4
% 0.17/0.49  # Processed clauses                    : 10
% 0.17/0.49  # ...of these trivial                  : 0
% 0.17/0.49  # ...subsumed                          : 0
% 0.17/0.49  # ...remaining for further processing  : 10
% 0.17/0.49  # Other redundant clauses eliminated   : 0
% 0.17/0.49  # Clauses deleted for lack of memory   : 0
% 0.17/0.49  # Backward-subsumed                    : 2
% 0.17/0.49  # Backward-rewritten                   : 0
% 0.17/0.49  # Generated clauses                    : 6
% 0.17/0.49  # ...of the previous two non-redundant : 3
% 0.17/0.49  # ...aggressively subsumed             : 0
% 0.17/0.49  # Contextual simplify-reflections      : 0
% 0.17/0.49  # Paramodulations                      : 5
% 0.17/0.49  # Factorizations                       : 0
% 0.17/0.49  # NegExts                              : 0
% 0.17/0.49  # Equation resolutions                 : 0
% 0.17/0.49  # Disequality decompositions           : 0
% 0.17/0.49  # Total rewrite steps                  : 0
% 0.17/0.49  # ...of those cached                   : 0
% 0.17/0.49  # Propositional unsat checks           : 0
% 0.17/0.49  #    Propositional check models        : 0
% 0.17/0.49  #    Propositional check unsatisfiable : 0
% 0.17/0.49  #    Propositional clauses             : 0
% 0.17/0.49  #    Propositional clauses after purity: 0
% 0.17/0.49  #    Propositional unsat core size     : 0
% 0.17/0.49  #    Propositional preprocessing time  : 0.000
% 0.17/0.49  #    Propositional encoding time       : 0.000
% 0.17/0.49  #    Propositional solver time         : 0.000
% 0.17/0.49  #    Success case prop preproc time    : 0.000
% 0.17/0.49  #    Success case prop encoding time   : 0.000
% 0.17/0.49  #    Success case prop solver time     : 0.000
% 0.17/0.49  # Current number of processed clauses  : 3
% 0.17/0.49  #    Positive orientable unit clauses  : 1
% 0.17/0.49  #    Positive unorientable unit clauses: 0
% 0.17/0.49  #    Negative unit clauses             : 1
% 0.17/0.49  #    Non-unit-clauses                  : 1
% 0.17/0.49  # Current number of unprocessed clauses: 1
% 0.17/0.49  # ...number of literals in the above   : 1
% 0.17/0.49  # Current number of archived formulas  : 0
% 0.17/0.49  # Current number of archived clauses   : 7
% 0.17/0.49  # Clause-clause subsumption calls (NU) : 0
% 0.17/0.49  # Rec. Clause-clause subsumption calls : 0
% 0.17/0.49  # Non-unit clause-clause subsumptions  : 0
% 0.17/0.49  # Unit Clause-clause subsumption calls : 2
% 0.17/0.49  # Rewrite failures with RHS unbound    : 0
% 0.17/0.49  # BW rewrite match attempts            : 0
% 0.17/0.49  # BW rewrite match successes           : 0
% 0.17/0.49  # Condensation attempts                : 0
% 0.17/0.49  # Condensation successes               : 0
% 0.17/0.49  # Termbank termtop insertions          : 295
% 0.17/0.49  # Search garbage collected termcells   : 96
% 0.17/0.49  
% 0.17/0.49  # -------------------------------------------------
% 0.17/0.49  # User time                : 0.003 s
% 0.17/0.49  # System time              : 0.001 s
% 0.17/0.49  # Total time               : 0.004 s
% 0.17/0.49  # Maximum resident set size: 1728 pages
% 0.17/0.49  
% 0.17/0.49  # -------------------------------------------------
% 0.17/0.49  # User time                : 0.003 s
% 0.17/0.49  # System time              : 0.003 s
% 0.17/0.49  # Total time               : 0.006 s
% 0.17/0.49  # Maximum resident set size: 1688 pages
% 0.17/0.49  % E exiting
% 0.17/0.49  % E exiting
%------------------------------------------------------------------------------

