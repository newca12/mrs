% Proof : Problems/SYN057+1.p
%------------------------------------------------------------------------------
% File     : E---3.3.0
% Problem  : SYN057+1 : TPTP v9.2.0. Released v2.0.0.
% Transfm  : none
% Format   : tptp:raw
% Command  : run_E /export/starexec/sandbox2/benchmark/theBenchmark.p 300 THM

% Computer : n013.cluster.edu
% Model    : x86_64 x86_64
% CPU      : Intel(R) Xeon(R) CPU E5-2620 v4 2.10GHz
% Memory   : 8042.1875MB
% OS       : Linux 3.10.0-693.el7.x86_64
% CPULimit : 300s
% WCLimit  : 300s
% DateTime : Mon Sep 29 11:23:50 PM UTC 2025

% Result   : Theorem 0.20s 0.49s
% Output   : CNFRefutation 0.20s
% Verified : 
% SZS Type : Refutation
%            Derivation depth      :    7
%            Number of leaves      :    5
% Syntax   : Number of formulae    :   25 (   7 unt;   0 def)
%            Number of atoms       :   56 (   0 equ)
%            Maximal formula atoms :    4 (   2 avg)
%            Number of connectives :   59 (  28   ~;  16   |;   7   &)
%                                         (   0 <=>;   8  =>;   0  <=;   0 <~>)
%            Maximal formula depth :    7 (   3 avg)
%            Maximal term depth    :    1 (   1 avg)
%            Number of predicates  :    6 (   5 usr;   1 prp; 0-1 aty)
%            Number of functors    :    2 (   2 usr;   2 con; 0-0 aty)
%            Number of variables   :   21 (   0 sgn  10   !;   4   ?)

% Comments : 
%------------------------------------------------------------------------------
fof(pel27_4,axiom,
    ( ? [X1] :
        ( big_h(X1)
        & ~ big_g(X1) )
   => ! [X2] :
        ( big_i(X2)
       => ~ big_h(X2) ) ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',pel27_4) ).

fof(pel27,conjecture,
    ! [X1] :
      ( big_j(X1)
     => ~ big_i(X1) ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',pel27) ).

fof(pel27_2,axiom,
    ! [X1] :
      ( big_f(X1)
     => big_h(X1) ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',pel27_2) ).

fof(pel27_3,axiom,
    ! [X1] :
      ( ( big_j(X1)
        & big_i(X1) )
     => big_f(X1) ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',pel27_3) ).

fof(pel27_1,axiom,
    ? [X1] :
      ( big_f(X1)
      & ~ big_g(X1) ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',pel27_1) ).

fof(c_0_5,plain,
    ( ? [X1] :
        ( big_h(X1)
        & ~ big_g(X1) )
   => ! [X2] :
        ( big_i(X2)
       => ~ big_h(X2) ) ),
    inference(fof_simplification,[status(thm)],[pel27_4]) ).

fof(c_0_6,negated_conjecture,
    ~ ! [X1] :
        ( big_j(X1)
       => ~ big_i(X1) ),
    inference(fof_simplification,[status(thm)],[inference(assume_negation,[status(cth)],[pel27])]) ).

fof(c_0_7,plain,
    ! [X5,X6] :
      ( ~ big_h(X5)
      | big_g(X5)
      | ~ big_i(X6)
      | ~ big_h(X6) ),
    inference(fof_nnf,[status(thm)],[inference(shift_quantors,[status(thm)],[inference(variable_rename,[status(thm)],[inference(fof_nnf,[status(thm)],[c_0_5])])])]) ).

fof(c_0_8,plain,
    ! [X8] :
      ( ~ big_f(X8)
      | big_h(X8) ),
    inference(fof_nnf,[status(thm)],[inference(variable_rename,[status(thm)],[inference(fof_nnf,[status(thm)],[pel27_2])])]) ).

fof(c_0_9,plain,
    ! [X4] :
      ( ~ big_j(X4)
      | ~ big_i(X4)
      | big_f(X4) ),
    inference(fof_nnf,[status(thm)],[inference(variable_rename,[status(thm)],[inference(fof_nnf,[status(thm)],[pel27_3])])]) ).

fof(c_0_10,negated_conjecture,
    ( big_j(esk1_0)
    & big_i(esk1_0) ),
    inference(skolemize,[status(esa)],[inference(variable_rename,[status(thm)],[inference(fof_nnf,[status(thm)],[c_0_6])])]) ).

fof(c_0_11,plain,
    ? [X1] :
      ( big_f(X1)
      & ~ big_g(X1) ),
    inference(fof_simplification,[status(thm)],[pel27_1]) ).

fof(c_0_12,plain,
    ( big_g(X1)
    | ~ big_h(X1)
    | ~ big_i(X2)
    | ~ big_h(X2) ),
    inference(split_conjunct,[status(thm)],[c_0_7]) ).

fof(c_0_13,plain,
    ( big_h(X1)
    | ~ big_f(X1) ),
    inference(split_conjunct,[status(thm)],[c_0_8]) ).

fof(c_0_14,plain,
    ( big_f(X1)
    | ~ big_j(X1)
    | ~ big_i(X1) ),
    inference(split_conjunct,[status(thm)],[c_0_9]) ).

fof(c_0_15,negated_conjecture,
    big_j(esk1_0),
    inference(split_conjunct,[status(thm)],[c_0_10]) ).

fof(c_0_16,negated_conjecture,
    big_i(esk1_0),
    inference(split_conjunct,[status(thm)],[c_0_10]) ).

fof(c_0_17,plain,
    ( big_f(esk2_0)
    & ~ big_g(esk2_0) ),
    inference(fof_nnf,[status(thm)],[inference(skolemize,[status(esa)],[inference(variable_rename,[status(thm)],[c_0_11])])]) ).

fof(c_0_18,plain,
    ( big_g(X1)
    | ~ big_i(X2)
    | ~ big_h(X1)
    | ~ big_f(X2) ),
    inference(spm,[status(thm)],[c_0_12,c_0_13]) ).

fof(c_0_19,negated_conjecture,
    big_f(esk1_0),
    inference(cn,[status(thm)],[inference(rw,[status(thm)],[inference(spm,[status(thm)],[c_0_14,c_0_15]),c_0_16])]) ).

fof(c_0_20,plain,
    ~ big_g(esk2_0),
    inference(split_conjunct,[status(thm)],[c_0_17]) ).

fof(c_0_21,negated_conjecture,
    ( big_g(X1)
    | ~ big_h(X1) ),
    inference(cn,[status(thm)],[inference(rw,[status(thm)],[inference(spm,[status(thm)],[c_0_18,c_0_16]),c_0_19])]) ).

fof(c_0_22,negated_conjecture,
    ~ big_h(esk2_0),
    inference(spm,[status(thm)],[c_0_20,c_0_21]) ).

fof(c_0_23,plain,
    big_f(esk2_0),
    inference(split_conjunct,[status(thm)],[c_0_17]) ).

fof(c_0_24,negated_conjecture,
    $false,
    inference(cn,[status(thm)],[inference(rw,[status(thm)],[inference(spm,[status(thm)],[c_0_22,c_0_13]),c_0_23])]),
    [proof] ).

%------------------------------------------------------------------------------
%----ORIGINAL SYSTEM OUTPUT
% 0.07/0.12  % Problem    : SYN057+1 : TPTP v9.2.0. Released v2.0.0.
% 0.07/0.12  % Command    : run_E /export/starexec/sandbox2/benchmark/theBenchmark.p 300 THM
% 0.12/0.33  % Computer : n013.cluster.edu
% 0.12/0.33  % Model    : x86_64 x86_64
% 0.12/0.33  % CPU      : Intel(R) Xeon(R) CPU E5-2620 v4 @ 2.10GHz
% 0.12/0.33  % Memory   : 8042.1875MB
% 0.12/0.33  % OS       : Linux 3.10.0-693.el7.x86_64
% 0.12/0.33  % CPULimit   : 300
% 0.12/0.33  % WCLimit    : 300
% 0.12/0.33  % DateTime   : Fri Sep 26 15:00:08 EDT 2025
% 0.12/0.34  % CPUTime    : 
% 0.20/0.48  Running first-order theorem proving
% 0.20/0.48  Running: /export/starexec/sandbox2/solver/bin/eprover --delete-bad-limit=2000000000 --definitional-cnf=24 -s --print-statistics -R --print-version --proof-object --auto-schedule=8 --cpu-limit=300 /export/starexec/sandbox2/benchmark/theBenchmark.p
% 0.20/0.49  # Version: 3.0.0
% 0.20/0.49  # Preprocessing class: FSSSSMSSSSSNFFN.
% 0.20/0.49  # Scheduled 4 strats onto 8 cores with 300 seconds (2400 total)
% 0.20/0.49  # Starting G-E--_302_C18_F1_URBAN_RG_S04BN with 1500s (5) cores
% 0.20/0.49  # Starting new_bool_3 with 300s (1) cores
% 0.20/0.49  # Starting new_bool_1 with 300s (1) cores
% 0.20/0.49  # Starting sh5l with 300s (1) cores
% 0.20/0.49  # new_bool_3 with pid 2664 completed with status 0
% 0.20/0.49  # Result found by new_bool_3
% 0.20/0.49  # Preprocessing class: FSSSSMSSSSSNFFN.
% 0.20/0.49  # Scheduled 4 strats onto 8 cores with 300 seconds (2400 total)
% 0.20/0.49  # Starting G-E--_302_C18_F1_URBAN_RG_S04BN with 1500s (5) cores
% 0.20/0.49  # Starting new_bool_3 with 300s (1) cores
% 0.20/0.49  # SinE strategy is GSinE(CountFormulas,hypos,1.5,,3,20000,1.0)
% 0.20/0.49  # Search class: FHUNF-FFSS00-SFFFFFNN
% 0.20/0.49  # Scheduled 5 strats onto 1 cores with 300 seconds (300 total)
% 0.20/0.49  # Starting SAT001_MinMin_p005000_rr_RG with 181s (1) cores
% 0.20/0.49  # SAT001_MinMin_p005000_rr_RG with pid 2667 completed with status 0
% 0.20/0.49  # Result found by SAT001_MinMin_p005000_rr_RG
% 0.20/0.49  # Preprocessing class: FSSSSMSSSSSNFFN.
% 0.20/0.49  # Scheduled 4 strats onto 8 cores with 300 seconds (2400 total)
% 0.20/0.49  # Starting G-E--_302_C18_F1_URBAN_RG_S04BN with 1500s (5) cores
% 0.20/0.49  # Starting new_bool_3 with 300s (1) cores
% 0.20/0.49  # SinE strategy is GSinE(CountFormulas,hypos,1.5,,3,20000,1.0)
% 0.20/0.49  # Search class: FHUNF-FFSS00-SFFFFFNN
% 0.20/0.49  # Scheduled 5 strats onto 1 cores with 300 seconds (300 total)
% 0.20/0.49  # Starting SAT001_MinMin_p005000_rr_RG with 181s (1) cores
% 0.20/0.49  # Preprocessing time       : 0.001 s
% 0.20/0.49  # Presaturation interreduction done
% 0.20/0.49  
% 0.20/0.49  # Proof found!
% 0.20/0.49  # SZS status Theorem
% 0.20/0.49  # SZS output start CNFRefutation
% See solution above
% 0.20/0.49  # Parsed axioms                        : 5
% 0.20/0.49  # Removed by relevancy pruning/SinE    : 0
% 0.20/0.49  # Initial clauses                      : 7
% 0.20/0.49  # Removed in clause preprocessing      : 0
% 0.20/0.49  # Initial clauses in saturation        : 7
% 0.20/0.49  # Processed clauses                    : 18
% 0.20/0.49  # ...of these trivial                  : 0
% 0.20/0.49  # ...subsumed                          : 0
% 0.20/0.49  # ...remaining for further processing  : 18
% 0.20/0.49  # Other redundant clauses eliminated   : 0
% 0.20/0.49  # Clauses deleted for lack of memory   : 0
% 0.20/0.49  # Backward-subsumed                    : 2
% 0.20/0.49  # Backward-rewritten                   : 0
% 0.20/0.49  # Generated clauses                    : 5
% 0.20/0.49  # ...of the previous two non-redundant : 4
% 0.20/0.49  # ...aggressively subsumed             : 0
% 0.20/0.49  # Contextual simplify-reflections      : 0
% 0.20/0.49  # Paramodulations                      : 5
% 0.20/0.49  # Factorizations                       : 0
% 0.20/0.49  # NegExts                              : 0
% 0.20/0.49  # Equation resolutions                 : 0
% 0.20/0.49  # Disequality decompositions           : 0
% 0.20/0.49  # Total rewrite steps                  : 3
% 0.20/0.49  # ...of those cached                   : 0
% 0.20/0.49  # Propositional unsat checks           : 0
% 0.20/0.49  #    Propositional check models        : 0
% 0.20/0.49  #    Propositional check unsatisfiable : 0
% 0.20/0.50  #    Propositional clauses             : 0
% 0.20/0.50  #    Propositional clauses after purity: 0
% 0.20/0.50  #    Propositional unsat core size     : 0
% 0.20/0.50  #    Propositional preprocessing time  : 0.000
% 0.20/0.50  #    Propositional encoding time       : 0.000
% 0.20/0.50  #    Propositional solver time         : 0.000
% 0.20/0.50  #    Success case prop preproc time    : 0.000
% 0.20/0.50  #    Success case prop encoding time   : 0.000
% 0.20/0.50  #    Success case prop solver time     : 0.000
% 0.20/0.50  # Current number of processed clauses  : 9
% 0.20/0.50  #    Positive orientable unit clauses  : 4
% 0.20/0.50  #    Positive unorientable unit clauses: 0
% 0.20/0.50  #    Negative unit clauses             : 2
% 0.20/0.50  #    Non-unit-clauses                  : 3
% 0.20/0.50  # Current number of unprocessed clauses: 0
% 0.20/0.50  # ...number of literals in the above   : 0
% 0.20/0.50  # Current number of archived formulas  : 0
% 0.20/0.50  # Current number of archived clauses   : 9
% 0.20/0.50  # Clause-clause subsumption calls (NU) : 3
% 0.20/0.50  # Rec. Clause-clause subsumption calls : 3
% 0.20/0.50  # Non-unit clause-clause subsumptions  : 2
% 0.20/0.50  # Unit Clause-clause subsumption calls : 0
% 0.20/0.50  # Rewrite failures with RHS unbound    : 0
% 0.20/0.50  # BW rewrite match attempts            : 0
% 0.20/0.50  # BW rewrite match successes           : 0
% 0.20/0.50  # Condensation attempts                : 0
% 0.20/0.50  # Condensation successes               : 0
% 0.20/0.50  # Termbank termtop insertions          : 470
% 0.20/0.50  # Search garbage collected termcells   : 81
% 0.20/0.50  
% 0.20/0.50  # -------------------------------------------------
% 0.20/0.50  # User time                : 0.005 s
% 0.20/0.50  # System time              : 0.001 s
% 0.20/0.50  # Total time               : 0.006 s
% 0.20/0.50  # Maximum resident set size: 1764 pages
% 0.20/0.50  
% 0.20/0.50  # -------------------------------------------------
% 0.20/0.50  # User time                : 0.006 s
% 0.20/0.50  # System time              : 0.003 s
% 0.20/0.50  # Total time               : 0.009 s
% 0.20/0.50  # Maximum resident set size: 1692 pages
% 0.20/0.50  % E exiting
% 0.20/0.50  % E exiting
%------------------------------------------------------------------------------

