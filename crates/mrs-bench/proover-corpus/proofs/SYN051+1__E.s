% Proof : Problems/SYN051+1.p
%------------------------------------------------------------------------------
% File     : E---3.3.0
% Problem  : SYN051+1 : TPTP v9.2.0. Released v2.0.0.
% Transfm  : none
% Format   : tptp:raw
% Command  : run_E /export/starexec/sandbox/benchmark/theBenchmark.p 300 THM

% Computer : n026.cluster.edu
% Model    : x86_64 x86_64
% CPU      : Intel(R) Xeon(R) CPU E5-2620 v4 2.10GHz
% Memory   : 8042.1875MB
% OS       : Linux 3.10.0-693.el7.x86_64
% CPULimit : 300s
% WCLimit  : 300s
% DateTime : Mon Sep 29 11:23:47 PM UTC 2025

% Result   : Theorem 0.21s 0.50s
% Output   : CNFRefutation 0.21s
% Verified : 
% SZS Type : Refutation
%            Derivation depth      :    7
%            Number of leaves      :    3
% Syntax   : Number of formulae    :   15 (   4 unt;   0 def)
%            Number of atoms       :   28 (   0 equ)
%            Maximal formula atoms :    4 (   1 avg)
%            Number of connectives :   24 (  11   ~;   8   |;   1   &)
%                                         (   2 <=>;   2  =>;   0  <=;   0 <~>)
%            Maximal formula depth :    5 (   3 avg)
%            Maximal term depth    :    1 (   1 avg)
%            Number of predicates  :    3 (   2 usr;   2 prp; 0-1 aty)
%            Number of functors    :    2 (   2 usr;   2 con; 0-0 aty)
%            Number of variables   :    8 (   3 sgn   1   !;   4   ?)

% Comments : 
%------------------------------------------------------------------------------
fof(pel21,conjecture,
    ? [X1] :
      ( p
    <=> big_f(X1) ),
    file('/export/starexec/sandbox/benchmark/theBenchmark.p',pel21) ).

fof(pel21_2,axiom,
    ? [X1] :
      ( big_f(X1)
     => p ),
    file('/export/starexec/sandbox/benchmark/theBenchmark.p',pel21_2) ).

fof(pel21_1,axiom,
    ? [X1] :
      ( p
     => big_f(X1) ),
    file('/export/starexec/sandbox/benchmark/theBenchmark.p',pel21_1) ).

fof(c_0_3,negated_conjecture,
    ~ ? [X1] :
        ( p
      <=> big_f(X1) ),
    inference(assume_negation,[status(cth)],[pel21]) ).

fof(c_0_4,plain,
    ( ~ big_f(esk2_0)
    | p ),
    inference(fof_nnf,[status(thm)],[inference(skolemize,[status(esa)],[inference(variable_rename,[status(thm)],[inference(fof_nnf,[status(thm)],[pel21_2])])])]) ).

fof(c_0_5,negated_conjecture,
    ! [X2] :
      ( ( ~ p
        | ~ big_f(X2) )
      & ( p
        | big_f(X2) ) ),
    inference(fof_nnf,[status(thm)],[inference(variable_rename,[status(thm)],[inference(fof_nnf,[status(thm)],[c_0_3])])]) ).

fof(c_0_6,plain,
    ( p
    | ~ big_f(esk2_0) ),
    inference(split_conjunct,[status(thm)],[c_0_4]) ).

fof(c_0_7,negated_conjecture,
    ( ~ p
    | ~ big_f(X1) ),
    inference(split_conjunct,[status(thm)],[c_0_5]) ).

fof(c_0_8,plain,
    ~ big_f(esk2_0),
    inference(csr,[status(thm)],[c_0_6,c_0_7]) ).

fof(c_0_9,negated_conjecture,
    ( p
    | big_f(X1) ),
    inference(split_conjunct,[status(thm)],[c_0_5]) ).

fof(c_0_10,plain,
    ( ~ p
    | big_f(esk1_0) ),
    inference(fof_nnf,[status(thm)],[inference(skolemize,[status(esa)],[inference(variable_rename,[status(thm)],[inference(fof_nnf,[status(thm)],[pel21_1])])])]) ).

fof(c_0_11,negated_conjecture,
    p,
    inference(spm,[status(thm)],[c_0_8,c_0_9]) ).

fof(c_0_12,plain,
    ( big_f(esk1_0)
    | ~ p ),
    inference(split_conjunct,[status(thm)],[c_0_10]) ).

fof(c_0_13,negated_conjecture,
    ~ big_f(X1),
    inference(cn,[status(thm)],[inference(rw,[status(thm)],[c_0_7,c_0_11])]) ).

fof(c_0_14,plain,
    $false,
    inference(sr,[status(thm)],[inference(cn,[status(thm)],[inference(rw,[status(thm)],[c_0_12,c_0_11])]),c_0_13]),
    [proof] ).

%------------------------------------------------------------------------------
%----ORIGINAL SYSTEM OUTPUT
% 0.10/0.12  % Problem    : SYN051+1 : TPTP v9.2.0. Released v2.0.0.
% 0.10/0.12  % Command    : run_E /export/starexec/sandbox/benchmark/theBenchmark.p 300 THM
% 0.12/0.34  % Computer : n026.cluster.edu
% 0.12/0.34  % Model    : x86_64 x86_64
% 0.12/0.34  % CPU      : Intel(R) Xeon(R) CPU E5-2620 v4 @ 2.10GHz
% 0.12/0.34  % Memory   : 8042.1875MB
% 0.12/0.34  % OS       : Linux 3.10.0-693.el7.x86_64
% 0.12/0.34  % CPULimit   : 300
% 0.12/0.34  % WCLimit    : 300
% 0.12/0.34  % DateTime   : Fri Sep 26 14:51:23 EDT 2025
% 0.12/0.34  % CPUTime    : 
% 0.21/0.49  Running first-order theorem proving
% 0.21/0.49  Running: /export/starexec/sandbox/solver/bin/eprover --delete-bad-limit=2000000000 --definitional-cnf=24 -s --print-statistics -R --print-version --proof-object --auto-schedule=8 --cpu-limit=300 /export/starexec/sandbox/benchmark/theBenchmark.p
% 0.21/0.50  # Version: 3.0.0
% 0.21/0.50  # Preprocessing class: FSSSSMSSSSSNFFN.
% 0.21/0.50  # Scheduled 4 strats onto 8 cores with 300 seconds (2400 total)
% 0.21/0.50  # Starting G-E--_302_C18_F1_URBAN_RG_S04BN with 1500s (5) cores
% 0.21/0.50  # Starting new_bool_3 with 300s (1) cores
% 0.21/0.50  # Starting new_bool_1 with 300s (1) cores
% 0.21/0.50  # Starting sh5l with 300s (1) cores
% 0.21/0.50  # new_bool_1 with pid 28363 completed with status 0
% 0.21/0.50  # Result found by new_bool_1
% 0.21/0.50  # Preprocessing class: FSSSSMSSSSSNFFN.
% 0.21/0.50  # Scheduled 4 strats onto 8 cores with 300 seconds (2400 total)
% 0.21/0.50  # Starting G-E--_302_C18_F1_URBAN_RG_S04BN with 1500s (5) cores
% 0.21/0.50  # Starting new_bool_3 with 300s (1) cores
% 0.21/0.50  # Starting new_bool_1 with 300s (1) cores
% 0.21/0.50  # SinE strategy is GSinE(CountFormulas,hypos,1.5,,3,20000,1.0)
% 0.21/0.50  # Search class: FGHNF-FFSF00-SFFFFFNN
% 0.21/0.50  # Scheduled 5 strats onto 1 cores with 300 seconds (300 total)
% 0.21/0.50  # Starting SAT001_MinMin_p005000_rr_RG with 181s (1) cores
% 0.21/0.50  # SAT001_MinMin_p005000_rr_RG with pid 28366 completed with status 0
% 0.21/0.50  # Result found by SAT001_MinMin_p005000_rr_RG
% 0.21/0.50  # Preprocessing class: FSSSSMSSSSSNFFN.
% 0.21/0.50  # Scheduled 4 strats onto 8 cores with 300 seconds (2400 total)
% 0.21/0.50  # Starting G-E--_302_C18_F1_URBAN_RG_S04BN with 1500s (5) cores
% 0.21/0.50  # Starting new_bool_3 with 300s (1) cores
% 0.21/0.50  # Starting new_bool_1 with 300s (1) cores
% 0.21/0.50  # SinE strategy is GSinE(CountFormulas,hypos,1.5,,3,20000,1.0)
% 0.21/0.50  # Search class: FGHNF-FFSF00-SFFFFFNN
% 0.21/0.50  # Scheduled 5 strats onto 1 cores with 300 seconds (300 total)
% 0.21/0.50  # Starting SAT001_MinMin_p005000_rr_RG with 181s (1) cores
% 0.21/0.50  # Preprocessing time       : 0.001 s
% 0.21/0.50  # Presaturation interreduction done
% 0.21/0.50  
% 0.21/0.50  # Proof found!
% 0.21/0.50  # SZS status Theorem
% 0.21/0.50  # SZS output start CNFRefutation
% See solution above
% 0.21/0.50  # Parsed axioms                        : 3
% 0.21/0.50  # Removed by relevancy pruning/SinE    : 0
% 0.21/0.50  # Initial clauses                      : 4
% 0.21/0.50  # Removed in clause preprocessing      : 0
% 0.21/0.50  # Initial clauses in saturation        : 4
% 0.21/0.50  # Processed clauses                    : 10
% 0.21/0.50  # ...of these trivial                  : 0
% 0.21/0.50  # ...subsumed                          : 0
% 0.21/0.50  # ...remaining for further processing  : 9
% 0.21/0.50  # Other redundant clauses eliminated   : 0
% 0.21/0.50  # Clauses deleted for lack of memory   : 0
% 0.21/0.50  # Backward-subsumed                    : 1
% 0.21/0.50  # Backward-rewritten                   : 2
% 0.21/0.50  # Generated clauses                    : 1
% 0.21/0.50  # ...of the previous two non-redundant : 2
% 0.21/0.50  # ...aggressively subsumed             : 0
% 0.21/0.50  # Contextual simplify-reflections      : 1
% 0.21/0.50  # Paramodulations                      : 1
% 0.21/0.50  # Factorizations                       : 0
% 0.21/0.50  # NegExts                              : 0
% 0.21/0.50  # Equation resolutions                 : 0
% 0.21/0.50  # Disequality decompositions           : 0
% 0.21/0.50  # Total rewrite steps                  : 3
% 0.21/0.50  # ...of those cached                   : 2
% 0.21/0.50  # Propositional unsat checks           : 0
% 0.21/0.50  #    Propositional check models        : 0
% 0.21/0.50  #    Propositional check unsatisfiable : 0
% 0.21/0.50  #    Propositional clauses             : 0
% 0.21/0.50  #    Propositional clauses after purity: 0
% 0.21/0.50  #    Propositional unsat core size     : 0
% 0.21/0.50  #    Propositional preprocessing time  : 0.000
% 0.21/0.50  #    Propositional encoding time       : 0.000
% 0.21/0.50  #    Propositional solver time         : 0.000
% 0.21/0.50  #    Success case prop preproc time    : 0.000
% 0.21/0.50  #    Success case prop encoding time   : 0.000
% 0.21/0.50  #    Success case prop solver time     : 0.000
% 0.21/0.50  # Current number of processed clauses  : 2
% 0.21/0.50  #    Positive orientable unit clauses  : 1
% 0.21/0.50  #    Positive unorientable unit clauses: 0
% 0.21/0.50  #    Negative unit clauses             : 1
% 0.21/0.50  #    Non-unit-clauses                  : 0
% 0.21/0.50  # Current number of unprocessed clauses: 0
% 0.21/0.50  # ...number of literals in the above   : 0
% 0.21/0.50  # Current number of archived formulas  : 0
% 0.21/0.50  # Current number of archived clauses   : 7
% 0.21/0.50  # Clause-clause subsumption calls (NU) : 1
% 0.21/0.50  # Rec. Clause-clause subsumption calls : 1
% 0.21/0.50  # Non-unit clause-clause subsumptions  : 1
% 0.21/0.50  # Unit Clause-clause subsumption calls : 1
% 0.21/0.50  # Rewrite failures with RHS unbound    : 0
% 0.21/0.50  # BW rewrite match attempts            : 1
% 0.21/0.50  # BW rewrite match successes           : 1
% 0.21/0.50  # Condensation attempts                : 0
% 0.21/0.50  # Condensation successes               : 0
% 0.21/0.50  # Termbank termtop insertions          : 210
% 0.21/0.50  # Search garbage collected termcells   : 47
% 0.21/0.50  
% 0.21/0.50  # -------------------------------------------------
% 0.21/0.50  # User time                : 0.002 s
% 0.21/0.50  # System time              : 0.002 s
% 0.21/0.50  # Total time               : 0.004 s
% 0.21/0.50  # Maximum resident set size: 1588 pages
% 0.21/0.50  
% 0.21/0.50  # -------------------------------------------------
% 0.21/0.50  # User time                : 0.005 s
% 0.21/0.50  # System time              : 0.002 s
% 0.21/0.50  # Total time               : 0.007 s
% 0.21/0.50  # Maximum resident set size: 1692 pages
% 0.21/0.50  % E exiting
% 0.21/0.50  % E exiting
%------------------------------------------------------------------------------

