% Proof : Problems/SYN347+1.p
%------------------------------------------------------------------------------
% File     : E---3.3.0
% Problem  : SYN347+1 : TPTP v9.2.0. Released v2.0.0.
% Transfm  : none
% Format   : tptp:raw
% Command  : run_E /export/starexec/sandbox/benchmark/theBenchmark.p 300 THM

% Computer : n006.cluster.edu
% Model    : x86_64 x86_64
% CPU      : Intel(R) Xeon(R) CPU E5-2620 v4 2.10GHz
% Memory   : 8042.1875MB
% OS       : Linux 3.10.0-693.el7.x86_64
% CPULimit : 300s
% WCLimit  : 300s
% DateTime : Mon Sep 29 11:24:58 PM UTC 2025

% Result   : Theorem 0.22s 0.50s
% Output   : CNFRefutation 0.22s
% Verified : 
% SZS Type : Refutation
%            Derivation depth      :   11
%            Number of leaves      :    1
% Syntax   : Number of formulae    :   21 (   4 unt;   0 def)
%            Number of atoms       :   64 (   0 equ)
%            Maximal formula atoms :   16 (   3 avg)
%            Number of connectives :   66 (  23   ~;  32   |;   5   &)
%                                         (   6 <=>;   0  =>;   0  <=;   0 <~>)
%            Maximal formula depth :   10 (   4 avg)
%            Maximal term depth    :    2 (   1 avg)
%            Number of predicates  :    2 (   1 usr;   1 prp; 0-2 aty)
%            Number of functors    :    3 (   3 usr;   2 con; 0-2 aty)
%            Number of variables   :   31 (   0 sgn   8   !;   4   ?)

% Comments : 
%------------------------------------------------------------------------------
fof(church_46_17_3,conjecture,
    ! [X1,X2] :
    ? [X3,X4] :
    ! [X5] :
      ( ( ( big_f(X3,X5)
        <=> big_f(X4,X5) )
      <=> big_f(X1,X2) )
      | ( big_f(X1,X5)
      <=> big_f(X2,X5) ) ),
    file('/export/starexec/sandbox/benchmark/theBenchmark.p',church_46_17_3) ).

fof(c_0_1,negated_conjecture,
    ~ ! [X1,X2] :
      ? [X3,X4] :
      ! [X5] :
        ( ( ( big_f(X3,X5)
          <=> big_f(X4,X5) )
        <=> big_f(X1,X2) )
        | ( big_f(X1,X5)
        <=> big_f(X2,X5) ) ),
    inference(assume_negation,[status(cth)],[church_46_17_3]) ).

fof(c_0_2,negated_conjecture,
    ! [X8,X9] :
      ( ( ~ big_f(X8,esk3_2(X8,X9))
        | ~ big_f(X9,esk3_2(X8,X9))
        | ~ big_f(esk1_0,esk2_0) )
      & ( big_f(X8,esk3_2(X8,X9))
        | big_f(X9,esk3_2(X8,X9))
        | ~ big_f(esk1_0,esk2_0) )
      & ( ~ big_f(X8,esk3_2(X8,X9))
        | big_f(X9,esk3_2(X8,X9))
        | big_f(esk1_0,esk2_0) )
      & ( ~ big_f(X9,esk3_2(X8,X9))
        | big_f(X8,esk3_2(X8,X9))
        | big_f(esk1_0,esk2_0) )
      & ( ~ big_f(esk1_0,esk3_2(X8,X9))
        | ~ big_f(esk2_0,esk3_2(X8,X9)) )
      & ( big_f(esk1_0,esk3_2(X8,X9))
        | big_f(esk2_0,esk3_2(X8,X9)) ) ),
    inference(distribute,[status(thm)],[inference(fof_nnf,[status(thm)],[inference(skolemize,[status(esa)],[inference(variable_rename,[status(thm)],[inference(fof_nnf,[status(thm)],[c_0_1])])])])]) ).

fof(c_0_3,negated_conjecture,
    ( big_f(X2,esk3_2(X2,X1))
    | big_f(esk1_0,esk2_0)
    | ~ big_f(X1,esk3_2(X2,X1)) ),
    inference(split_conjunct,[status(thm)],[c_0_2]) ).

fof(c_0_4,negated_conjecture,
    ( big_f(esk1_0,esk3_2(X1,X2))
    | big_f(esk2_0,esk3_2(X1,X2)) ),
    inference(split_conjunct,[status(thm)],[c_0_2]) ).

fof(c_0_5,negated_conjecture,
    ( big_f(esk2_0,esk3_2(X1,esk1_0))
    | big_f(X1,esk3_2(X1,esk1_0))
    | big_f(esk1_0,esk2_0) ),
    inference(spm,[status(thm)],[c_0_3,c_0_4]) ).

fof(c_0_6,negated_conjecture,
    ( big_f(X2,esk3_2(X1,X2))
    | big_f(esk1_0,esk2_0)
    | ~ big_f(X1,esk3_2(X1,X2)) ),
    inference(split_conjunct,[status(thm)],[c_0_2]) ).

fof(c_0_7,negated_conjecture,
    ( big_f(esk2_0,esk3_2(esk2_0,esk1_0))
    | big_f(esk1_0,esk2_0) ),
    inference(ef,[status(thm)],[c_0_5]) ).

fof(c_0_8,negated_conjecture,
    ( ~ big_f(esk1_0,esk3_2(X1,X2))
    | ~ big_f(esk2_0,esk3_2(X1,X2)) ),
    inference(split_conjunct,[status(thm)],[c_0_2]) ).

fof(c_0_9,negated_conjecture,
    ( big_f(esk1_0,esk3_2(esk2_0,esk1_0))
    | big_f(esk1_0,esk2_0) ),
    inference(spm,[status(thm)],[c_0_6,c_0_7]) ).

fof(c_0_10,negated_conjecture,
    ( ~ big_f(X1,esk3_2(X1,X2))
    | ~ big_f(X2,esk3_2(X1,X2))
    | ~ big_f(esk1_0,esk2_0) ),
    inference(split_conjunct,[status(thm)],[c_0_2]) ).

fof(c_0_11,negated_conjecture,
    ( big_f(X1,esk3_2(X1,X2))
    | big_f(X2,esk3_2(X1,X2))
    | ~ big_f(esk1_0,esk2_0) ),
    inference(split_conjunct,[status(thm)],[c_0_2]) ).

fof(c_0_12,negated_conjecture,
    big_f(esk1_0,esk2_0),
    inference(csr,[status(thm)],[inference(spm,[status(thm)],[c_0_8,c_0_9]),c_0_7]) ).

fof(c_0_13,negated_conjecture,
    ( big_f(esk2_0,esk3_2(X1,esk1_0))
    | ~ big_f(X1,esk3_2(X1,esk1_0))
    | ~ big_f(esk1_0,esk2_0) ),
    inference(spm,[status(thm)],[c_0_10,c_0_4]) ).

fof(c_0_14,negated_conjecture,
    ( big_f(X1,esk3_2(X2,X1))
    | big_f(X2,esk3_2(X2,X1)) ),
    inference(cn,[status(thm)],[inference(rw,[status(thm)],[c_0_11,c_0_12])]) ).

fof(c_0_15,negated_conjecture,
    ( big_f(esk2_0,esk3_2(esk1_0,esk1_0))
    | ~ big_f(esk1_0,esk2_0) ),
    inference(spm,[status(thm)],[c_0_13,c_0_4]) ).

fof(c_0_16,negated_conjecture,
    ( big_f(X1,esk3_2(esk1_0,X1))
    | ~ big_f(esk2_0,esk3_2(esk1_0,X1)) ),
    inference(spm,[status(thm)],[c_0_8,c_0_14]) ).

fof(c_0_17,negated_conjecture,
    big_f(esk2_0,esk3_2(esk1_0,esk1_0)),
    inference(cn,[status(thm)],[inference(rw,[status(thm)],[c_0_15,c_0_12])]) ).

fof(c_0_18,negated_conjecture,
    ( ~ big_f(X1,esk3_2(X2,X1))
    | ~ big_f(X2,esk3_2(X2,X1)) ),
    inference(cn,[status(thm)],[inference(rw,[status(thm)],[c_0_10,c_0_12])]) ).

fof(c_0_19,negated_conjecture,
    big_f(esk1_0,esk3_2(esk1_0,esk1_0)),
    inference(spm,[status(thm)],[c_0_16,c_0_17]) ).

fof(c_0_20,negated_conjecture,
    $false,
    inference(cn,[status(thm)],[inference(rw,[status(thm)],[inference(spm,[status(thm)],[c_0_18,c_0_19]),c_0_19])]),
    [proof] ).

%------------------------------------------------------------------------------
%----ORIGINAL SYSTEM OUTPUT
% 0.07/0.13  % Problem    : SYN347+1 : TPTP v9.2.0. Released v2.0.0.
% 0.07/0.13  % Command    : run_E /export/starexec/sandbox/benchmark/theBenchmark.p 300 THM
% 0.13/0.34  % Computer : n006.cluster.edu
% 0.13/0.34  % Model    : x86_64 x86_64
% 0.13/0.34  % CPU      : Intel(R) Xeon(R) CPU E5-2620 v4 @ 2.10GHz
% 0.13/0.34  % Memory   : 8042.1875MB
% 0.13/0.34  % OS       : Linux 3.10.0-693.el7.x86_64
% 0.13/0.34  % CPULimit   : 300
% 0.13/0.34  % WCLimit    : 300
% 0.13/0.34  % DateTime   : Fri Sep 26 14:41:23 EDT 2025
% 0.13/0.34  % CPUTime    : 
% 0.22/0.49  Running first-order theorem proving
% 0.22/0.49  Running: /export/starexec/sandbox/solver/bin/eprover --delete-bad-limit=2000000000 --definitional-cnf=24 -s --print-statistics -R --print-version --proof-object --auto-schedule=8 --cpu-limit=300 /export/starexec/sandbox/benchmark/theBenchmark.p
% 0.22/0.50  # Version: 3.0.0
% 0.22/0.50  # Preprocessing class: FSSSSMSSSSSNFFN.
% 0.22/0.50  # Scheduled 4 strats onto 8 cores with 300 seconds (2400 total)
% 0.22/0.50  # Starting G-E--_302_C18_F1_URBAN_RG_S04BN with 1500s (5) cores
% 0.22/0.50  # Starting new_bool_3 with 300s (1) cores
% 0.22/0.50  # Starting new_bool_1 with 300s (1) cores
% 0.22/0.50  # Starting sh5l with 300s (1) cores
% 0.22/0.50  # new_bool_3 with pid 11405 completed with status 0
% 0.22/0.50  # Result found by new_bool_3
% 0.22/0.50  # Preprocessing class: FSSSSMSSSSSNFFN.
% 0.22/0.50  # Scheduled 4 strats onto 8 cores with 300 seconds (2400 total)
% 0.22/0.50  # Starting G-E--_302_C18_F1_URBAN_RG_S04BN with 1500s (5) cores
% 0.22/0.50  # Starting new_bool_3 with 300s (1) cores
% 0.22/0.50  # SinE strategy is GSinE(CountFormulas,hypos,1.5,,3,20000,1.0)
% 0.22/0.50  # Search class: FGHNF-FFSF22-SFFFFFNN
% 0.22/0.50  # Scheduled 5 strats onto 1 cores with 300 seconds (300 total)
% 0.22/0.50  # Starting SAT001_MinMin_p005000_rr_RG with 181s (1) cores
% 0.22/0.50  # SAT001_MinMin_p005000_rr_RG with pid 11411 completed with status 0
% 0.22/0.50  # Result found by SAT001_MinMin_p005000_rr_RG
% 0.22/0.50  # Preprocessing class: FSSSSMSSSSSNFFN.
% 0.22/0.50  # Scheduled 4 strats onto 8 cores with 300 seconds (2400 total)
% 0.22/0.50  # Starting G-E--_302_C18_F1_URBAN_RG_S04BN with 1500s (5) cores
% 0.22/0.50  # Starting new_bool_3 with 300s (1) cores
% 0.22/0.50  # SinE strategy is GSinE(CountFormulas,hypos,1.5,,3,20000,1.0)
% 0.22/0.50  # Search class: FGHNF-FFSF22-SFFFFFNN
% 0.22/0.50  # Scheduled 5 strats onto 1 cores with 300 seconds (300 total)
% 0.22/0.50  # Starting SAT001_MinMin_p005000_rr_RG with 181s (1) cores
% 0.22/0.50  # Preprocessing time       : 0.001 s
% 0.22/0.50  # Presaturation interreduction done
% 0.22/0.50  
% 0.22/0.50  # Proof found!
% 0.22/0.50  # SZS status Theorem
% 0.22/0.50  # SZS output start CNFRefutation
% See solution above
% 0.22/0.50  # Parsed axioms                        : 1
% 0.22/0.50  # Removed by relevancy pruning/SinE    : 0
% 0.22/0.50  # Initial clauses                      : 6
% 0.22/0.50  # Removed in clause preprocessing      : 0
% 0.22/0.50  # Initial clauses in saturation        : 6
% 0.22/0.50  # Processed clauses                    : 27
% 0.22/0.50  # ...of these trivial                  : 0
% 0.22/0.50  # ...subsumed                          : 4
% 0.22/0.50  # ...remaining for further processing  : 23
% 0.22/0.50  # Other redundant clauses eliminated   : 0
% 0.22/0.50  # Clauses deleted for lack of memory   : 0
% 0.22/0.50  # Backward-subsumed                    : 0
% 0.22/0.50  # Backward-rewritten                   : 8
% 0.22/0.50  # Generated clauses                    : 29
% 0.22/0.50  # ...of the previous two non-redundant : 21
% 0.22/0.50  # ...aggressively subsumed             : 0
% 0.22/0.50  # Contextual simplify-reflections      : 1
% 0.22/0.50  # Paramodulations                      : 25
% 0.22/0.50  # Factorizations                       : 4
% 0.22/0.50  # NegExts                              : 0
% 0.22/0.50  # Equation resolutions                 : 0
% 0.22/0.50  # Disequality decompositions           : 0
% 0.22/0.50  # Total rewrite steps                  : 11
% 0.22/0.50  # ...of those cached                   : 8
% 0.22/0.50  # Propositional unsat checks           : 0
% 0.22/0.50  #    Propositional check models        : 0
% 0.22/0.50  #    Propositional check unsatisfiable : 0
% 0.22/0.50  #    Propositional clauses             : 0
% 0.22/0.50  #    Propositional clauses after purity: 0
% 0.22/0.50  #    Propositional unsat core size     : 0
% 0.22/0.50  #    Propositional preprocessing time  : 0.000
% 0.22/0.50  #    Propositional encoding time       : 0.000
% 0.22/0.50  #    Propositional solver time         : 0.000
% 0.22/0.50  #    Success case prop preproc time    : 0.000
% 0.22/0.50  #    Success case prop encoding time   : 0.000
% 0.22/0.50  #    Success case prop solver time     : 0.000
% 0.22/0.50  # Current number of processed clauses  : 9
% 0.22/0.50  #    Positive orientable unit clauses  : 3
% 0.22/0.50  #    Positive unorientable unit clauses: 0
% 0.22/0.50  #    Negative unit clauses             : 0
% 0.22/0.50  #    Non-unit-clauses                  : 6
% 0.22/0.50  # Current number of unprocessed clauses: 3
% 0.22/0.50  # ...number of literals in the above   : 4
% 0.22/0.50  # Current number of archived formulas  : 0
% 0.22/0.50  # Current number of archived clauses   : 14
% 0.22/0.50  # Clause-clause subsumption calls (NU) : 37
% 0.22/0.50  # Rec. Clause-clause subsumption calls : 26
% 0.22/0.50  # Non-unit clause-clause subsumptions  : 5
% 0.22/0.50  # Unit Clause-clause subsumption calls : 1
% 0.22/0.50  # Rewrite failures with RHS unbound    : 0
% 0.22/0.50  # BW rewrite match attempts            : 1
% 0.22/0.50  # BW rewrite match successes           : 1
% 0.22/0.50  # Condensation attempts                : 0
% 0.22/0.50  # Condensation successes               : 0
% 0.22/0.50  # Termbank termtop insertions          : 1001
% 0.22/0.50  # Search garbage collected termcells   : 160
% 0.22/0.50  
% 0.22/0.50  # -------------------------------------------------
% 0.22/0.50  # User time                : 0.003 s
% 0.22/0.50  # System time              : 0.002 s
% 0.22/0.50  # Total time               : 0.005 s
% 0.22/0.50  # Maximum resident set size: 1756 pages
% 0.22/0.50  
% 0.22/0.50  # -------------------------------------------------
% 0.22/0.50  # User time                : 0.004 s
% 0.22/0.50  # System time              : 0.004 s
% 0.22/0.50  # Total time               : 0.008 s
% 0.22/0.50  # Maximum resident set size: 1692 pages
% 0.22/0.50  % E exiting
% 0.22/0.50  % E exiting
%------------------------------------------------------------------------------

