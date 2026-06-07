% Proof : Problems/SYN054+1.p
%------------------------------------------------------------------------------
% File     : E---3.3.0
% Problem  : SYN054+1 : TPTP v9.2.0. Released v2.0.0.
% Transfm  : none
% Format   : tptp:raw
% Command  : run_E /export/starexec/sandbox2/benchmark/theBenchmark.p 300 THM

% Computer : n002.cluster.edu
% Model    : x86_64 x86_64
% CPU      : Intel(R) Xeon(R) CPU E5-2620 v4 2.10GHz
% Memory   : 8042.1875MB
% OS       : Linux 3.10.0-693.el7.x86_64
% CPULimit : 300s
% WCLimit  : 300s
% DateTime : Mon Sep 29 11:23:49 PM UTC 2025

% Result   : Theorem 0.10s 0.34s
% Output   : CNFRefutation 0.10s
% Verified : 
% SZS Type : Refutation
%            Derivation depth      :    5
%            Number of leaves      :    5
% Syntax   : Number of formulae    :   20 (   4 unt;   0 def)
%            Number of atoms       :   42 (   0 equ)
%            Maximal formula atoms :    4 (   2 avg)
%            Number of connectives :   40 (  18   ~;  15   |;   4   &)
%                                         (   0 <=>;   3  =>;   0  <=;   0 <~>)
%            Maximal formula depth :    5 (   3 avg)
%            Maximal term depth    :    1 (   1 avg)
%            Number of predicates  :    5 (   4 usr;   1 prp; 0-1 aty)
%            Number of functors    :    2 (   2 usr;   2 con; 0-0 aty)
%            Number of variables   :   17 (   2 sgn   6   !;   5   ?)

% Comments : 
%------------------------------------------------------------------------------
fof(pel24_4,axiom,
    ! [X1] :
      ( ( big_q(X1)
        | big_r(X1) )
     => big_s(X1) ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',pel24_4) ).

fof(pel24_1,axiom,
    ~ ? [X1] :
        ( big_s(X1)
        & big_q(X1) ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',pel24_1) ).

fof(pel24,conjecture,
    ? [X1] :
      ( big_p(X1)
      & big_r(X1) ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',pel24) ).

fof(pel24_3,axiom,
    ( ~ ? [X1] : big_p(X1)
   => ? [X2] : big_q(X2) ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',pel24_3) ).

fof(pel24_2,axiom,
    ! [X1] :
      ( big_p(X1)
     => ( big_q(X1)
        | big_r(X1) ) ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',pel24_2) ).

fof(c_0_5,plain,
    ! [X5] :
      ( ( ~ big_q(X5)
        | big_s(X5) )
      & ( ~ big_r(X5)
        | big_s(X5) ) ),
    inference(distribute,[status(thm)],[inference(fof_nnf,[status(thm)],[inference(variable_rename,[status(thm)],[inference(fof_nnf,[status(thm)],[pel24_4])])])]) ).

fof(c_0_6,plain,
    ! [X8] :
      ( ~ big_s(X8)
      | ~ big_q(X8) ),
    inference(fof_nnf,[status(thm)],[inference(variable_rename,[status(thm)],[inference(fof_nnf,[status(thm)],[pel24_1])])]) ).

fof(c_0_7,negated_conjecture,
    ~ ? [X1] :
        ( big_p(X1)
        & big_r(X1) ),
    inference(assume_negation,[status(cth)],[pel24]) ).

fof(c_0_8,plain,
    ( big_p(esk1_0)
    | big_q(esk2_0) ),
    inference(skolemize,[status(esa)],[inference(variable_rename,[status(thm)],[inference(fof_nnf,[status(thm)],[pel24_3])])]) ).

fof(c_0_9,plain,
    ( big_s(X1)
    | ~ big_q(X1) ),
    inference(split_conjunct,[status(thm)],[c_0_5]) ).

fof(c_0_10,plain,
    ( ~ big_s(X1)
    | ~ big_q(X1) ),
    inference(split_conjunct,[status(thm)],[c_0_6]) ).

fof(c_0_11,plain,
    ! [X4] :
      ( ~ big_p(X4)
      | big_q(X4)
      | big_r(X4) ),
    inference(fof_nnf,[status(thm)],[inference(variable_rename,[status(thm)],[inference(fof_nnf,[status(thm)],[pel24_2])])]) ).

fof(c_0_12,negated_conjecture,
    ! [X3] :
      ( ~ big_p(X3)
      | ~ big_r(X3) ),
    inference(fof_nnf,[status(thm)],[inference(variable_rename,[status(thm)],[inference(fof_nnf,[status(thm)],[c_0_7])])]) ).

fof(c_0_13,plain,
    ( big_p(esk1_0)
    | big_q(esk2_0) ),
    inference(split_conjunct,[status(thm)],[c_0_8]) ).

fof(c_0_14,plain,
    ~ big_q(X1),
    inference(csr,[status(thm)],[c_0_9,c_0_10]) ).

fof(c_0_15,plain,
    ( big_q(X1)
    | big_r(X1)
    | ~ big_p(X1) ),
    inference(split_conjunct,[status(thm)],[c_0_11]) ).

fof(c_0_16,negated_conjecture,
    ( ~ big_p(X1)
    | ~ big_r(X1) ),
    inference(split_conjunct,[status(thm)],[c_0_12]) ).

fof(c_0_17,plain,
    big_p(esk1_0),
    inference(sr,[status(thm)],[c_0_13,c_0_14]) ).

fof(c_0_18,plain,
    ~ big_p(X1),
    inference(csr,[status(thm)],[inference(sr,[status(thm)],[c_0_15,c_0_14]),c_0_16]) ).

fof(c_0_19,plain,
    $false,
    inference(sr,[status(thm)],[c_0_17,c_0_18]),
    [proof] ).

%------------------------------------------------------------------------------
%----ORIGINAL SYSTEM OUTPUT
% 0.00/0.06  % Problem    : SYN054+1 : TPTP v9.2.0. Released v2.0.0.
% 0.00/0.06  % Command    : run_E /export/starexec/sandbox2/benchmark/theBenchmark.p 300 THM
% 0.06/0.24  % Computer : n002.cluster.edu
% 0.06/0.24  % Model    : x86_64 x86_64
% 0.06/0.24  % CPU      : Intel(R) Xeon(R) CPU E5-2620 v4 @ 2.10GHz
% 0.06/0.24  % Memory   : 8042.1875MB
% 0.06/0.24  % OS       : Linux 3.10.0-693.el7.x86_64
% 0.06/0.24  % CPULimit   : 300
% 0.06/0.24  % WCLimit    : 300
% 0.06/0.24  % DateTime   : Fri Sep 26 15:01:23 EDT 2025
% 0.06/0.24  % CPUTime    : 
% 0.10/0.33  Running first-order theorem proving
% 0.10/0.33  Running: /export/starexec/sandbox2/solver/bin/eprover --delete-bad-limit=2000000000 --definitional-cnf=24 -s --print-statistics -R --print-version --proof-object --auto-schedule=8 --cpu-limit=300 /export/starexec/sandbox2/benchmark/theBenchmark.p
% 0.10/0.34  # Version: 3.0.0
% 0.10/0.34  # Preprocessing class: FSSSSMSSSSSNFFN.
% 0.10/0.34  # Scheduled 4 strats onto 8 cores with 300 seconds (2400 total)
% 0.10/0.34  # Starting G-E--_302_C18_F1_URBAN_RG_S04BN with 1500s (5) cores
% 0.10/0.34  # Starting new_bool_3 with 300s (1) cores
% 0.10/0.34  # Starting new_bool_1 with 300s (1) cores
% 0.10/0.34  # Starting sh5l with 300s (1) cores
% 0.10/0.34  # new_bool_3 with pid 5973 completed with status 0
% 0.10/0.34  # Result found by new_bool_3
% 0.10/0.34  # Preprocessing class: FSSSSMSSSSSNFFN.
% 0.10/0.34  # Scheduled 4 strats onto 8 cores with 300 seconds (2400 total)
% 0.10/0.34  # Starting G-E--_302_C18_F1_URBAN_RG_S04BN with 1500s (5) cores
% 0.10/0.34  # Starting new_bool_3 with 300s (1) cores
% 0.10/0.34  # SinE strategy is GSinE(CountFormulas,hypos,1.5,,3,20000,1.0)
% 0.10/0.34  # Search class: FGHNF-FFSF00-SFFFFFNN
% 0.10/0.34  # Scheduled 5 strats onto 1 cores with 300 seconds (300 total)
% 0.10/0.34  # Starting SAT001_MinMin_p005000_rr_RG with 181s (1) cores
% 0.10/0.34  # SAT001_MinMin_p005000_rr_RG with pid 5978 completed with status 0
% 0.10/0.34  # Result found by SAT001_MinMin_p005000_rr_RG
% 0.10/0.34  # Preprocessing class: FSSSSMSSSSSNFFN.
% 0.10/0.34  # Scheduled 4 strats onto 8 cores with 300 seconds (2400 total)
% 0.10/0.34  # Starting G-E--_302_C18_F1_URBAN_RG_S04BN with 1500s (5) cores
% 0.10/0.34  # Starting new_bool_3 with 300s (1) cores
% 0.10/0.34  # SinE strategy is GSinE(CountFormulas,hypos,1.5,,3,20000,1.0)
% 0.10/0.34  # Search class: FGHNF-FFSF00-SFFFFFNN
% 0.10/0.34  # Scheduled 5 strats onto 1 cores with 300 seconds (300 total)
% 0.10/0.34  # Starting SAT001_MinMin_p005000_rr_RG with 181s (1) cores
% 0.10/0.34  # Preprocessing time       : 0.001 s
% 0.10/0.34  # Presaturation interreduction done
% 0.10/0.34  
% 0.10/0.34  # Proof found!
% 0.10/0.34  # SZS status Theorem
% 0.10/0.34  # SZS output start CNFRefutation
% See solution above
% 0.10/0.34  # Parsed axioms                        : 5
% 0.10/0.34  # Removed by relevancy pruning/SinE    : 0
% 0.10/0.34  # Initial clauses                      : 6
% 0.10/0.34  # Removed in clause preprocessing      : 0
% 0.10/0.34  # Initial clauses in saturation        : 6
% 0.10/0.34  # Processed clauses                    : 7
% 0.10/0.34  # ...of these trivial                  : 0
% 0.10/0.34  # ...subsumed                          : 0
% 0.10/0.34  # ...remaining for further processing  : 7
% 0.10/0.34  # Other redundant clauses eliminated   : 0
% 0.10/0.34  # Clauses deleted for lack of memory   : 0
% 0.10/0.34  # Backward-subsumed                    : 2
% 0.10/0.34  # Backward-rewritten                   : 0
% 0.10/0.34  # Generated clauses                    : 2
% 0.10/0.34  # ...of the previous two non-redundant : 1
% 0.10/0.34  # ...aggressively subsumed             : 0
% 0.10/0.34  # Contextual simplify-reflections      : 2
% 0.10/0.34  # Paramodulations                      : 0
% 0.10/0.34  # Factorizations                       : 0
% 0.10/0.34  # NegExts                              : 0
% 0.10/0.34  # Equation resolutions                 : 0
% 0.10/0.34  # Disequality decompositions           : 0
% 0.10/0.34  # Total rewrite steps                  : 0
% 0.10/0.34  # ...of those cached                   : 0
% 0.10/0.34  # Propositional unsat checks           : 0
% 0.10/0.34  #    Propositional check models        : 0
% 0.10/0.34  #    Propositional check unsatisfiable : 0
% 0.10/0.34  #    Propositional clauses             : 0
% 0.10/0.34  #    Propositional clauses after purity: 0
% 0.10/0.34  #    Propositional unsat core size     : 0
% 0.10/0.34  #    Propositional preprocessing time  : 0.000
% 0.10/0.34  #    Propositional encoding time       : 0.000
% 0.10/0.34  #    Propositional solver time         : 0.000
% 0.10/0.34  #    Success case prop preproc time    : 0.000
% 0.10/0.34  #    Success case prop encoding time   : 0.000
% 0.10/0.34  #    Success case prop solver time     : 0.000
% 0.10/0.34  # Current number of processed clauses  : 3
% 0.10/0.34  #    Positive orientable unit clauses  : 0
% 0.10/0.34  #    Positive unorientable unit clauses: 0
% 0.10/0.34  #    Negative unit clauses             : 2
% 0.10/0.34  #    Non-unit-clauses                  : 1
% 0.10/0.34  # Current number of unprocessed clauses: 0
% 0.10/0.34  # ...number of literals in the above   : 0
% 0.10/0.34  # Current number of archived formulas  : 0
% 0.10/0.34  # Current number of archived clauses   : 4
% 0.10/0.34  # Clause-clause subsumption calls (NU) : 2
% 0.10/0.34  # Rec. Clause-clause subsumption calls : 2
% 0.10/0.34  # Non-unit clause-clause subsumptions  : 2
% 0.10/0.34  # Unit Clause-clause subsumption calls : 2
% 0.10/0.34  # Rewrite failures with RHS unbound    : 0
% 0.10/0.34  # BW rewrite match attempts            : 0
% 0.10/0.34  # BW rewrite match successes           : 0
% 0.10/0.34  # Condensation attempts                : 0
% 0.10/0.34  # Condensation successes               : 0
% 0.10/0.34  # Termbank termtop insertions          : 343
% 0.10/0.34  # Search garbage collected termcells   : 69
% 0.10/0.34  
% 0.10/0.34  # -------------------------------------------------
% 0.10/0.34  # User time                : 0.003 s
% 0.10/0.34  # System time              : 0.000 s
% 0.10/0.34  # Total time               : 0.003 s
% 0.10/0.34  # Maximum resident set size: 1600 pages
% 0.10/0.34  
% 0.10/0.34  # -------------------------------------------------
% 0.10/0.34  # User time                : 0.005 s
% 0.10/0.34  # System time              : 0.000 s
% 0.10/0.34  # Total time               : 0.005 s
% 0.10/0.34  # Maximum resident set size: 1696 pages
% 0.10/0.34  % E exiting
% 0.10/0.34  % E exiting
%------------------------------------------------------------------------------

