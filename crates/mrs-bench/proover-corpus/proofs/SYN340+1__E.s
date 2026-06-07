% Proof : Problems/SYN340+1.p
%------------------------------------------------------------------------------
% File     : E---3.3.0
% Problem  : SYN340+1 : TPTP v9.2.0. Released v2.0.0.
% Transfm  : none
% Format   : tptp:raw
% Command  : run_E /export/starexec/sandbox2/benchmark/theBenchmark.p 300 THM

% Computer : n028.cluster.edu
% Model    : x86_64 x86_64
% CPU      : Intel(R) Xeon(R) CPU E5-2620 v4 2.10GHz
% Memory   : 8042.1875MB
% OS       : Linux 3.10.0-693.el7.x86_64
% CPULimit : 300s
% WCLimit  : 300s
% DateTime : Mon Sep 29 11:24:56 PM UTC 2025

% Result   : Theorem 0.19s 0.48s
% Output   : CNFRefutation 0.19s
% Verified : 
% SZS Type : Refutation
%            Derivation depth      :    4
%            Number of leaves      :    1
% Syntax   : Number of formulae    :    6 (   3 unt;   0 def)
%            Number of atoms       :    9 (   0 equ)
%            Maximal formula atoms :    2 (   1 avg)
%            Number of connectives :    6 (   3   ~;   0   |;   1   &)
%                                         (   0 <=>;   2  =>;   0  <=;   0 <~>)
%            Maximal formula depth :    8 (   4 avg)
%            Maximal term depth    :    2 (   1 avg)
%            Number of predicates  :    2 (   1 usr;   1 prp; 0-5 aty)
%            Number of functors    :    1 (   1 usr;   0 con; 1-1 aty)
%            Number of variables   :   19 (   2 sgn   7   !;   6   ?)

% Comments : 
%------------------------------------------------------------------------------
fof(church_46_15_5,conjecture,
    ? [X1] :
    ! [X2] :
    ? [X3,X4] :
      ( big_f(X1,X2,X3,X4,X3)
     => big_f(X3,X1,X2,X3,X4) ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',church_46_15_5) ).

fof(c_0_1,negated_conjecture,
    ~ ? [X1] :
      ! [X2] :
      ? [X3,X4] :
        ( big_f(X1,X2,X3,X4,X3)
       => big_f(X3,X1,X2,X3,X4) ),
    inference(assume_negation,[status(cth)],[church_46_15_5]) ).

fof(c_0_2,negated_conjecture,
    ! [X5,X7,X8,X9,X10] :
      ( big_f(X5,esk1_1(X5),X7,X8,X7)
      & ~ big_f(X9,X5,esk1_1(X5),X9,X10) ),
    inference(fof_nnf,[status(thm)],[inference(shift_quantors,[status(thm)],[inference(skolemize,[status(esa)],[inference(variable_rename,[status(thm)],[inference(shift_quantors,[status(thm)],[inference(fof_nnf,[status(thm)],[c_0_1])])])])])]) ).

fof(c_0_3,negated_conjecture,
    ~ big_f(X1,X2,esk1_1(X2),X1,X3),
    inference(split_conjunct,[status(thm)],[c_0_2]) ).

fof(c_0_4,negated_conjecture,
    big_f(X1,esk1_1(X1),X2,X3,X2),
    inference(split_conjunct,[status(thm)],[c_0_2]) ).

fof(c_0_5,negated_conjecture,
    $false,
    inference(spm,[status(thm)],[c_0_3,c_0_4]),
    [proof] ).

%------------------------------------------------------------------------------
%----ORIGINAL SYSTEM OUTPUT
% 0.06/0.12  % Problem    : SYN340+1 : TPTP v9.2.0. Released v2.0.0.
% 0.06/0.12  % Command    : run_E /export/starexec/sandbox2/benchmark/theBenchmark.p 300 THM
% 0.11/0.32  % Computer : n028.cluster.edu
% 0.11/0.32  % Model    : x86_64 x86_64
% 0.11/0.32  % CPU      : Intel(R) Xeon(R) CPU E5-2620 v4 @ 2.10GHz
% 0.11/0.32  % Memory   : 8042.1875MB
% 0.11/0.32  % OS       : Linux 3.10.0-693.el7.x86_64
% 0.11/0.32  % CPULimit   : 300
% 0.11/0.32  % WCLimit    : 300
% 0.11/0.32  % DateTime   : Fri Sep 26 15:15:38 EDT 2025
% 0.11/0.33  % CPUTime    : 
% 0.19/0.47  Running first-order theorem proving
% 0.19/0.47  Running: /export/starexec/sandbox2/solver/bin/eprover --delete-bad-limit=2000000000 --definitional-cnf=24 -s --print-statistics -R --print-version --proof-object --auto-schedule=8 --cpu-limit=300 /export/starexec/sandbox2/benchmark/theBenchmark.p
% 0.19/0.48  # Version: 3.0.0
% 0.19/0.48  # Preprocessing class: FSSSSMSSSSSNFFN.
% 0.19/0.48  # Scheduled 4 strats onto 8 cores with 300 seconds (2400 total)
% 0.19/0.48  # Starting G-E--_302_C18_F1_URBAN_RG_S04BN with 1500s (5) cores
% 0.19/0.48  # Starting new_bool_3 with 300s (1) cores
% 0.19/0.48  # Starting new_bool_1 with 300s (1) cores
% 0.19/0.48  # Starting sh5l with 300s (1) cores
% 0.19/0.48  # new_bool_3 with pid 5228 completed with status 0
% 0.19/0.48  # Result found by new_bool_3
% 0.19/0.48  # Preprocessing class: FSSSSMSSSSSNFFN.
% 0.19/0.48  # Scheduled 4 strats onto 8 cores with 300 seconds (2400 total)
% 0.19/0.48  # Starting G-E--_302_C18_F1_URBAN_RG_S04BN with 1500s (5) cores
% 0.19/0.48  # Starting new_bool_3 with 300s (1) cores
% 0.19/0.48  # SinE strategy is GSinE(CountFormulas,hypos,1.5,,3,20000,1.0)
% 0.19/0.48  # Search class: FUUNF-FFSF11-SFFFFFNN
% 0.19/0.48  # Scheduled 5 strats onto 1 cores with 300 seconds (300 total)
% 0.19/0.48  # Starting SAT001_MinMin_p005000_rr_RG with 181s (1) cores
% 0.19/0.48  # SAT001_MinMin_p005000_rr_RG with pid 5232 completed with status 0
% 0.19/0.48  # Result found by SAT001_MinMin_p005000_rr_RG
% 0.19/0.48  # Preprocessing class: FSSSSMSSSSSNFFN.
% 0.19/0.48  # Scheduled 4 strats onto 8 cores with 300 seconds (2400 total)
% 0.19/0.48  # Starting G-E--_302_C18_F1_URBAN_RG_S04BN with 1500s (5) cores
% 0.19/0.48  # Starting new_bool_3 with 300s (1) cores
% 0.19/0.48  # SinE strategy is GSinE(CountFormulas,hypos,1.5,,3,20000,1.0)
% 0.19/0.48  # Search class: FUUNF-FFSF11-SFFFFFNN
% 0.19/0.48  # Scheduled 5 strats onto 1 cores with 300 seconds (300 total)
% 0.19/0.48  # Starting SAT001_MinMin_p005000_rr_RG with 181s (1) cores
% 0.19/0.48  # Preprocessing time       : 0.001 s
% 0.19/0.48  # Presaturation interreduction done
% 0.19/0.48  
% 0.19/0.48  # Proof found!
% 0.19/0.48  # SZS status Theorem
% 0.19/0.48  # SZS output start CNFRefutation
% See solution above
% 0.19/0.48  # Parsed axioms                        : 1
% 0.19/0.48  # Removed by relevancy pruning/SinE    : 0
% 0.19/0.48  # Initial clauses                      : 2
% 0.19/0.48  # Removed in clause preprocessing      : 0
% 0.19/0.48  # Initial clauses in saturation        : 2
% 0.19/0.48  # Processed clauses                    : 4
% 0.19/0.48  # ...of these trivial                  : 0
% 0.19/0.48  # ...subsumed                          : 0
% 0.19/0.48  # ...remaining for further processing  : 4
% 0.19/0.48  # Other redundant clauses eliminated   : 0
% 0.19/0.48  # Clauses deleted for lack of memory   : 0
% 0.19/0.48  # Backward-subsumed                    : 0
% 0.19/0.48  # Backward-rewritten                   : 0
% 0.19/0.48  # Generated clauses                    : 1
% 0.19/0.48  # ...of the previous two non-redundant : 0
% 0.19/0.48  # ...aggressively subsumed             : 0
% 0.19/0.48  # Contextual simplify-reflections      : 0
% 0.19/0.48  # Paramodulations                      : 1
% 0.19/0.48  # Factorizations                       : 0
% 0.19/0.48  # NegExts                              : 0
% 0.19/0.48  # Equation resolutions                 : 0
% 0.19/0.48  # Disequality decompositions           : 0
% 0.19/0.48  # Total rewrite steps                  : 0
% 0.19/0.48  # ...of those cached                   : 0
% 0.19/0.48  # Propositional unsat checks           : 0
% 0.19/0.48  #    Propositional check models        : 0
% 0.19/0.48  #    Propositional check unsatisfiable : 0
% 0.19/0.48  #    Propositional clauses             : 0
% 0.19/0.48  #    Propositional clauses after purity: 0
% 0.19/0.48  #    Propositional unsat core size     : 0
% 0.19/0.48  #    Propositional preprocessing time  : 0.000
% 0.19/0.48  #    Propositional encoding time       : 0.000
% 0.19/0.48  #    Propositional solver time         : 0.000
% 0.19/0.48  #    Success case prop preproc time    : 0.000
% 0.19/0.48  #    Success case prop encoding time   : 0.000
% 0.19/0.48  #    Success case prop solver time     : 0.000
% 0.19/0.48  # Current number of processed clauses  : 2
% 0.19/0.48  #    Positive orientable unit clauses  : 1
% 0.19/0.48  #    Positive unorientable unit clauses: 0
% 0.19/0.48  #    Negative unit clauses             : 1
% 0.19/0.48  #    Non-unit-clauses                  : 0
% 0.19/0.48  # Current number of unprocessed clauses: 0
% 0.19/0.48  # ...number of literals in the above   : 0
% 0.19/0.48  # Current number of archived formulas  : 0
% 0.19/0.48  # Current number of archived clauses   : 2
% 0.19/0.48  # Clause-clause subsumption calls (NU) : 0
% 0.19/0.48  # Rec. Clause-clause subsumption calls : 0
% 0.19/0.48  # Non-unit clause-clause subsumptions  : 0
% 0.19/0.48  # Unit Clause-clause subsumption calls : 0
% 0.19/0.48  # Rewrite failures with RHS unbound    : 0
% 0.19/0.48  # BW rewrite match attempts            : 0
% 0.19/0.48  # BW rewrite match successes           : 0
% 0.19/0.48  # Condensation attempts                : 0
% 0.19/0.48  # Condensation successes               : 0
% 0.19/0.48  # Termbank termtop insertions          : 141
% 0.19/0.48  # Search garbage collected termcells   : 56
% 0.19/0.48  
% 0.19/0.48  # -------------------------------------------------
% 0.19/0.48  # User time                : 0.002 s
% 0.19/0.48  # System time              : 0.002 s
% 0.19/0.48  # Total time               : 0.004 s
% 0.19/0.48  # Maximum resident set size: 1600 pages
% 0.19/0.48  
% 0.19/0.48  # -------------------------------------------------
% 0.19/0.48  # User time                : 0.002 s
% 0.19/0.48  # System time              : 0.004 s
% 0.19/0.48  # Total time               : 0.007 s
% 0.19/0.48  # Maximum resident set size: 1688 pages
% 0.19/0.48  % E exiting
% 0.19/0.48  % E exiting
%------------------------------------------------------------------------------

