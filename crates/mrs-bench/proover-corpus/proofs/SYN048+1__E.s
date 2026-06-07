% Proof : Problems/SYN048+1.p
%------------------------------------------------------------------------------
% File     : E---3.3.0
% Problem  : SYN048+1 : TPTP v9.2.0. Released v2.0.0.
% Transfm  : none
% Format   : tptp:raw
% Command  : run_E /export/starexec/sandbox2/benchmark/theBenchmark.p 300 THM

% Computer : n026.cluster.edu
% Model    : x86_64 x86_64
% CPU      : Intel(R) Xeon(R) CPU E5-2620 v4 2.10GHz
% Memory   : 8042.1875MB
% OS       : Linux 3.10.0-693.el7.x86_64
% CPULimit : 300s
% WCLimit  : 300s
% DateTime : Mon Sep 29 11:23:46 PM UTC 2025

% Result   : Theorem 0.19s 0.46s
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
%            Maximal formula depth :    5 (   3 avg)
%            Maximal term depth    :    1 (   1 avg)
%            Number of predicates  :    2 (   1 usr;   1 prp; 0-1 aty)
%            Number of functors    :    1 (   1 usr;   1 con; 0-0 aty)
%            Number of variables   :    6 (   1 sgn   3   !;   2   ?)

% Comments : 
%------------------------------------------------------------------------------
fof(pel18,conjecture,
    ? [X1] :
    ! [X2] :
      ( big_f(X1)
     => big_f(X2) ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',pel18) ).

fof(c_0_1,negated_conjecture,
    ~ ? [X1] :
      ! [X2] :
        ( big_f(X1)
       => big_f(X2) ),
    inference(assume_negation,[status(cth)],[pel18]) ).

fof(c_0_2,negated_conjecture,
    ! [X3] :
      ( big_f(X3)
      & ~ big_f(esk1_0) ),
    inference(fof_nnf,[status(thm)],[inference(shift_quantors,[status(thm)],[inference(skolemize,[status(esa)],[inference(variable_rename,[status(thm)],[inference(shift_quantors,[status(thm)],[inference(fof_nnf,[status(thm)],[c_0_1])])])])])]) ).

fof(c_0_3,negated_conjecture,
    ~ big_f(esk1_0),
    inference(split_conjunct,[status(thm)],[c_0_2]) ).

fof(c_0_4,negated_conjecture,
    big_f(X1),
    inference(split_conjunct,[status(thm)],[c_0_2]) ).

fof(c_0_5,negated_conjecture,
    $false,
    inference(cn,[status(thm)],[inference(rw,[status(thm)],[c_0_3,c_0_4])]),
    [proof] ).

%------------------------------------------------------------------------------
%----ORIGINAL SYSTEM OUTPUT
% 0.06/0.12  % Problem    : SYN048+1 : TPTP v9.2.0. Released v2.0.0.
% 0.06/0.12  % Command    : run_E /export/starexec/sandbox2/benchmark/theBenchmark.p 300 THM
% 0.12/0.31  % Computer : n026.cluster.edu
% 0.12/0.31  % Model    : x86_64 x86_64
% 0.12/0.31  % CPU      : Intel(R) Xeon(R) CPU E5-2620 v4 @ 2.10GHz
% 0.12/0.31  % Memory   : 8042.1875MB
% 0.12/0.31  % OS       : Linux 3.10.0-693.el7.x86_64
% 0.12/0.31  % CPULimit   : 300
% 0.12/0.31  % WCLimit    : 300
% 0.12/0.31  % DateTime   : Fri Sep 26 15:16:23 EDT 2025
% 0.12/0.31  % CPUTime    : 
% 0.19/0.45  Running first-order theorem proving
% 0.19/0.45  Running: /export/starexec/sandbox2/solver/bin/eprover --delete-bad-limit=2000000000 --definitional-cnf=24 -s --print-statistics -R --print-version --proof-object --auto-schedule=8 --cpu-limit=300 /export/starexec/sandbox2/benchmark/theBenchmark.p
% 0.19/0.46  # Version: 3.0.0
% 0.19/0.46  # Preprocessing class: FSSSSMSSSSSNFFN.
% 0.19/0.46  # Scheduled 4 strats onto 8 cores with 300 seconds (2400 total)
% 0.19/0.46  # Starting G-E--_302_C18_F1_URBAN_RG_S04BN with 1500s (5) cores
% 0.19/0.46  # Starting new_bool_3 with 300s (1) cores
% 0.19/0.46  # Starting new_bool_1 with 300s (1) cores
% 0.19/0.46  # Starting sh5l with 300s (1) cores
% 0.19/0.46  # G-E--_302_C18_F1_URBAN_RG_S04BN with pid 24302 completed with status 0
% 0.19/0.46  # Result found by G-E--_302_C18_F1_URBAN_RG_S04BN
% 0.19/0.46  # Preprocessing class: FSSSSMSSSSSNFFN.
% 0.19/0.46  # Scheduled 4 strats onto 8 cores with 300 seconds (2400 total)
% 0.19/0.46  # Starting G-E--_302_C18_F1_URBAN_RG_S04BN with 1500s (5) cores
% 0.19/0.46  # No SInE strategy applied
% 0.19/0.46  # Search class: FUHPF-FFSF00-SFFFFFNN
% 0.19/0.46  # Scheduled 6 strats onto 5 cores with 1500 seconds (1500 total)
% 0.19/0.46  # Starting SAT001_MinMin_p005000_rr_RG with 811s (1) cores
% 0.19/0.46  # Starting G-E--_302_C18_F1_URBAN_RG_S04BN with 151s (1) cores
% 0.19/0.46  # Starting new_bool_3 with 136s (1) cores
% 0.19/0.46  # Starting new_bool_1 with 136s (1) cores
% 0.19/0.46  # Starting sh5l with 136s (1) cores
% 0.19/0.46  # sh5l with pid 24311 completed with status 0
% 0.19/0.46  # Result found by sh5l
% 0.19/0.46  # Preprocessing class: FSSSSMSSSSSNFFN.
% 0.19/0.46  # Scheduled 4 strats onto 8 cores with 300 seconds (2400 total)
% 0.19/0.46  # Starting G-E--_302_C18_F1_URBAN_RG_S04BN with 1500s (5) cores
% 0.19/0.46  # No SInE strategy applied
% 0.19/0.46  # Search class: FUHPF-FFSF00-SFFFFFNN
% 0.19/0.46  # Scheduled 6 strats onto 5 cores with 1500 seconds (1500 total)
% 0.19/0.46  # Starting SAT001_MinMin_p005000_rr_RG with 811s (1) cores
% 0.19/0.46  # Starting G-E--_302_C18_F1_URBAN_RG_S04BN with 151s (1) cores
% 0.19/0.46  # Starting new_bool_3 with 136s (1) cores
% 0.19/0.46  # Starting new_bool_1 with 136s (1) cores
% 0.19/0.46  # Starting sh5l with 136s (1) cores
% 0.19/0.46  # Preprocessing time       : 0.001 s
% 0.19/0.46  # Presaturation interreduction done
% 0.19/0.46  
% 0.19/0.46  # Proof found!
% 0.19/0.46  # SZS status Theorem
% 0.19/0.46  # SZS output start CNFRefutation
% See solution above
% 0.19/0.46  # Parsed axioms                        : 1
% 0.19/0.46  # Removed by relevancy pruning/SinE    : 0
% 0.19/0.46  # Initial clauses                      : 2
% 0.19/0.46  # Removed in clause preprocessing      : 1
% 0.19/0.46  # Initial clauses in saturation        : 1
% 0.19/0.46  # Processed clauses                    : 1
% 0.19/0.46  # ...of these trivial                  : 0
% 0.19/0.46  # ...subsumed                          : 0
% 0.19/0.46  # ...remaining for further processing  : 0
% 0.19/0.46  # Other redundant clauses eliminated   : 0
% 0.19/0.46  # Clauses deleted for lack of memory   : 0
% 0.19/0.46  # Backward-subsumed                    : 0
% 0.19/0.46  # Backward-rewritten                   : 0
% 0.19/0.46  # Generated clauses                    : 0
% 0.19/0.46  # ...of the previous two non-redundant : 0
% 0.19/0.46  # ...aggressively subsumed             : 0
% 0.19/0.46  # Contextual simplify-reflections      : 0
% 0.19/0.46  # Paramodulations                      : 0
% 0.19/0.46  # Factorizations                       : 0
% 0.19/0.46  # NegExts                              : 0
% 0.19/0.46  # Equation resolutions                 : 0
% 0.19/0.46  # Disequality decompositions           : 0
% 0.19/0.46  # Total rewrite steps                  : 0
% 0.19/0.46  # ...of those cached                   : 0
% 0.19/0.46  # Propositional unsat checks           : 0
% 0.19/0.46  #    Propositional check models        : 0
% 0.19/0.46  #    Propositional check unsatisfiable : 0
% 0.19/0.46  #    Propositional clauses             : 0
% 0.19/0.46  #    Propositional clauses after purity: 0
% 0.19/0.46  #    Propositional unsat core size     : 0
% 0.19/0.46  #    Propositional preprocessing time  : 0.000
% 0.19/0.46  #    Propositional encoding time       : 0.000
% 0.19/0.46  #    Propositional solver time         : 0.000
% 0.19/0.46  #    Success case prop preproc time    : 0.000
% 0.19/0.46  #    Success case prop encoding time   : 0.000
% 0.19/0.46  #    Success case prop solver time     : 0.000
% 0.19/0.46  # Current number of processed clauses  : 0
% 0.19/0.46  #    Positive orientable unit clauses  : 0
% 0.19/0.46  #    Positive unorientable unit clauses: 0
% 0.19/0.46  #    Negative unit clauses             : 0
% 0.19/0.46  #    Non-unit-clauses                  : 0
% 0.19/0.46  # Current number of unprocessed clauses: 0
% 0.19/0.46  # ...number of literals in the above   : 0
% 0.19/0.46  # Current number of archived formulas  : 0
% 0.19/0.46  # Current number of archived clauses   : 1
% 0.19/0.46  # Clause-clause subsumption calls (NU) : 0
% 0.19/0.46  # Rec. Clause-clause subsumption calls : 0
% 0.19/0.46  # Non-unit clause-clause subsumptions  : 0
% 0.19/0.46  # Unit Clause-clause subsumption calls : 0
% 0.19/0.46  # Rewrite failures with RHS unbound    : 0
% 0.19/0.46  # BW rewrite match attempts            : 0
% 0.19/0.46  # BW rewrite match successes           : 0
% 0.19/0.46  # Condensation attempts                : 1
% 0.19/0.46  # Condensation successes               : 0
% 0.19/0.46  # Termbank termtop insertions          : 75
% 0.19/0.46  # Search garbage collected termcells   : 31
% 0.19/0.46  
% 0.19/0.46  # -------------------------------------------------
% 0.19/0.46  # User time                : 0.002 s
% 0.19/0.46  # System time              : 0.000 s
% 0.19/0.46  # Total time               : 0.002 s
% 0.19/0.46  # Maximum resident set size: 1668 pages
% 0.19/0.46  
% 0.19/0.46  # -------------------------------------------------
% 0.19/0.46  # User time                : 0.008 s
% 0.19/0.46  # System time              : 0.003 s
% 0.19/0.46  # Total time               : 0.011 s
% 0.19/0.46  # Maximum resident set size: 1688 pages
% 0.19/0.46  % E exiting
% 0.19/0.47  % E exiting
%------------------------------------------------------------------------------

