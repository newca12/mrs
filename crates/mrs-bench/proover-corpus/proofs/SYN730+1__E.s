% Proof : Problems/SYN730+1.p
%------------------------------------------------------------------------------
% File     : E---3.3.0
% Problem  : SYN730+1 : TPTP v9.2.0. Released v2.5.0.
% Transfm  : none
% Format   : tptp:raw
% Command  : run_E /export/starexec/sandbox/benchmark/theBenchmark.p 300 THM

% Computer : n023.cluster.edu
% Model    : x86_64 x86_64
% CPU      : Intel(R) Xeon(R) CPU E5-2620 v4 2.10GHz
% Memory   : 8042.1875MB
% OS       : Linux 3.10.0-693.el7.x86_64
% CPULimit : 300s
% WCLimit  : 300s
% DateTime : Mon Sep 29 11:27:22 PM UTC 2025

% Result   : Theorem 0.19s 0.47s
% Output   : CNFRefutation 0.19s
% Verified : 
% SZS Type : Refutation
%            Derivation depth      :    5
%            Number of leaves      :    1
% Syntax   : Number of formulae    :    7 (   3 unt;   0 def)
%            Number of atoms       :   14 (   0 equ)
%            Maximal formula atoms :    3 (   2 avg)
%            Number of connectives :   10 (   3   ~;   4   |;   1   &)
%                                         (   0 <=>;   2  =>;   0  <=;   0 <~>)
%            Maximal formula depth :    7 (   3 avg)
%            Maximal term depth    :    3 (   1 avg)
%            Number of predicates  :    2 (   1 usr;   1 prp; 0-3 aty)
%            Number of functors    :    4 (   4 usr;   1 con; 0-1 aty)
%            Number of variables   :   12 (   1 sgn   4   !;   4   ?)

% Comments : 
%------------------------------------------------------------------------------
fof(thm75,conjecture,
    ? [X1] :
    ! [X2] :
    ? [X3] :
      ( ( p(a,X2,h(X2))
        | p(X1,X2,k(X2)) )
     => p(X1,X2,X3) ),
    file('/export/starexec/sandbox/benchmark/theBenchmark.p',thm75) ).

fof(c_0_1,negated_conjecture,
    ~ ? [X1] :
      ! [X2] :
      ? [X3] :
        ( ( p(a,X2,h(X2))
          | p(X1,X2,k(X2)) )
       => p(X1,X2,X3) ),
    inference(assume_negation,[status(cth)],[thm75]) ).

fof(c_0_2,negated_conjecture,
    ! [X4,X6] :
      ( ( p(a,esk1_1(X4),h(esk1_1(X4)))
        | p(X4,esk1_1(X4),k(esk1_1(X4))) )
      & ~ p(X4,esk1_1(X4),X6) ),
    inference(fof_nnf,[status(thm)],[inference(shift_quantors,[status(thm)],[inference(skolemize,[status(esa)],[inference(variable_rename,[status(thm)],[inference(shift_quantors,[status(thm)],[inference(fof_nnf,[status(thm)],[c_0_1])])])])])]) ).

fof(c_0_3,negated_conjecture,
    ( p(a,esk1_1(X1),h(esk1_1(X1)))
    | p(X1,esk1_1(X1),k(esk1_1(X1))) ),
    inference(split_conjunct,[status(thm)],[c_0_2]) ).

fof(c_0_4,negated_conjecture,
    ~ p(X1,esk1_1(X1),X2),
    inference(split_conjunct,[status(thm)],[c_0_2]) ).

fof(c_0_5,negated_conjecture,
    p(a,esk1_1(X1),h(esk1_1(X1))),
    inference(sr,[status(thm)],[c_0_3,c_0_4]) ).

fof(c_0_6,negated_conjecture,
    $false,
    inference(spm,[status(thm)],[c_0_4,c_0_5]),
    [proof] ).

%------------------------------------------------------------------------------
%----ORIGINAL SYSTEM OUTPUT
% 0.03/0.12  % Problem    : SYN730+1 : TPTP v9.2.0. Released v2.5.0.
% 0.03/0.12  % Command    : run_E /export/starexec/sandbox/benchmark/theBenchmark.p 300 THM
% 0.12/0.33  % Computer : n023.cluster.edu
% 0.12/0.33  % Model    : x86_64 x86_64
% 0.12/0.33  % CPU      : Intel(R) Xeon(R) CPU E5-2620 v4 @ 2.10GHz
% 0.12/0.33  % Memory   : 8042.1875MB
% 0.12/0.33  % OS       : Linux 3.10.0-693.el7.x86_64
% 0.12/0.33  % CPULimit   : 300
% 0.12/0.33  % WCLimit    : 300
% 0.12/0.33  % DateTime   : Fri Sep 26 14:34:08 EDT 2025
% 0.12/0.33  % CPUTime    : 
% 0.19/0.46  Running first-order theorem proving
% 0.19/0.46  Running: /export/starexec/sandbox/solver/bin/eprover --delete-bad-limit=2000000000 --definitional-cnf=24 -s --print-statistics -R --print-version --proof-object --auto-schedule=8 --cpu-limit=300 /export/starexec/sandbox/benchmark/theBenchmark.p
% 0.19/0.47  # Version: 3.0.0
% 0.19/0.47  # Preprocessing class: FSSSSMSSSSSNFFN.
% 0.19/0.47  # Scheduled 4 strats onto 8 cores with 300 seconds (2400 total)
% 0.19/0.47  # Starting G-E--_302_C18_F1_URBAN_RG_S04BN with 1500s (5) cores
% 0.19/0.47  # Starting new_bool_3 with 300s (1) cores
% 0.19/0.47  # Starting new_bool_1 with 300s (1) cores
% 0.19/0.47  # Starting sh5l with 300s (1) cores
% 0.19/0.47  # new_bool_3 with pid 10769 completed with status 0
% 0.19/0.47  # Result found by new_bool_3
% 0.19/0.47  # Preprocessing class: FSSSSMSSSSSNFFN.
% 0.19/0.47  # Scheduled 4 strats onto 8 cores with 300 seconds (2400 total)
% 0.19/0.47  # Starting G-E--_302_C18_F1_URBAN_RG_S04BN with 1500s (5) cores
% 0.19/0.47  # Starting new_bool_3 with 300s (1) cores
% 0.19/0.47  # SinE strategy is GSinE(CountFormulas,hypos,1.5,,3,20000,1.0)
% 0.19/0.47  # Search class: FGUNF-FFSF11-SFFFFFNN
% 0.19/0.47  # Scheduled 5 strats onto 1 cores with 300 seconds (300 total)
% 0.19/0.47  # Starting SAT001_CO_MinMin_p005000_rr with 181s (1) cores
% 0.19/0.47  # SAT001_CO_MinMin_p005000_rr with pid 10774 completed with status 0
% 0.19/0.47  # Result found by SAT001_CO_MinMin_p005000_rr
% 0.19/0.47  # Preprocessing class: FSSSSMSSSSSNFFN.
% 0.19/0.47  # Scheduled 4 strats onto 8 cores with 300 seconds (2400 total)
% 0.19/0.47  # Starting G-E--_302_C18_F1_URBAN_RG_S04BN with 1500s (5) cores
% 0.19/0.47  # Starting new_bool_3 with 300s (1) cores
% 0.19/0.47  # SinE strategy is GSinE(CountFormulas,hypos,1.5,,3,20000,1.0)
% 0.19/0.47  # Search class: FGUNF-FFSF11-SFFFFFNN
% 0.19/0.47  # Scheduled 5 strats onto 1 cores with 300 seconds (300 total)
% 0.19/0.47  # Starting SAT001_CO_MinMin_p005000_rr with 181s (1) cores
% 0.19/0.47  # Preprocessing time       : 0.001 s
% 0.19/0.47  # Presaturation interreduction done
% 0.19/0.47  
% 0.19/0.47  # Proof found!
% 0.19/0.47  # SZS status Theorem
% 0.19/0.47  # SZS output start CNFRefutation
% See solution above
% 0.19/0.47  # Parsed axioms                        : 1
% 0.19/0.47  # Removed by relevancy pruning/SinE    : 0
% 0.19/0.47  # Initial clauses                      : 2
% 0.19/0.47  # Removed in clause preprocessing      : 0
% 0.19/0.47  # Initial clauses in saturation        : 2
% 0.19/0.47  # Processed clauses                    : 4
% 0.19/0.47  # ...of these trivial                  : 0
% 0.19/0.47  # ...subsumed                          : 0
% 0.19/0.47  # ...remaining for further processing  : 4
% 0.19/0.47  # Other redundant clauses eliminated   : 0
% 0.19/0.47  # Clauses deleted for lack of memory   : 0
% 0.19/0.47  # Backward-subsumed                    : 0
% 0.19/0.47  # Backward-rewritten                   : 0
% 0.19/0.47  # Generated clauses                    : 1
% 0.19/0.47  # ...of the previous two non-redundant : 0
% 0.19/0.47  # ...aggressively subsumed             : 0
% 0.19/0.47  # Contextual simplify-reflections      : 0
% 0.19/0.47  # Paramodulations                      : 1
% 0.19/0.47  # Factorizations                       : 0
% 0.19/0.47  # NegExts                              : 0
% 0.19/0.47  # Equation resolutions                 : 0
% 0.19/0.47  # Disequality decompositions           : 0
% 0.19/0.47  # Total rewrite steps                  : 0
% 0.19/0.47  # ...of those cached                   : 0
% 0.19/0.47  # Propositional unsat checks           : 0
% 0.19/0.47  #    Propositional check models        : 0
% 0.19/0.47  #    Propositional check unsatisfiable : 0
% 0.19/0.47  #    Propositional clauses             : 0
% 0.19/0.47  #    Propositional clauses after purity: 0
% 0.19/0.47  #    Propositional unsat core size     : 0
% 0.19/0.47  #    Propositional preprocessing time  : 0.000
% 0.19/0.47  #    Propositional encoding time       : 0.000
% 0.19/0.47  #    Propositional solver time         : 0.000
% 0.19/0.47  #    Success case prop preproc time    : 0.000
% 0.19/0.47  #    Success case prop encoding time   : 0.000
% 0.19/0.47  #    Success case prop solver time     : 0.000
% 0.19/0.47  # Current number of processed clauses  : 2
% 0.19/0.47  #    Positive orientable unit clauses  : 1
% 0.19/0.47  #    Positive unorientable unit clauses: 0
% 0.19/0.47  #    Negative unit clauses             : 1
% 0.19/0.47  #    Non-unit-clauses                  : 0
% 0.19/0.47  # Current number of unprocessed clauses: 0
% 0.19/0.47  # ...number of literals in the above   : 0
% 0.19/0.47  # Current number of archived formulas  : 0
% 0.19/0.47  # Current number of archived clauses   : 2
% 0.19/0.47  # Clause-clause subsumption calls (NU) : 0
% 0.19/0.47  # Rec. Clause-clause subsumption calls : 0
% 0.19/0.47  # Non-unit clause-clause subsumptions  : 0
% 0.19/0.47  # Unit Clause-clause subsumption calls : 0
% 0.19/0.47  # Rewrite failures with RHS unbound    : 0
% 0.19/0.47  # BW rewrite match attempts            : 0
% 0.19/0.47  # BW rewrite match successes           : 0
% 0.19/0.47  # Condensation attempts                : 4
% 0.19/0.47  # Condensation successes               : 0
% 0.19/0.47  # Termbank termtop insertions          : 177
% 0.19/0.47  # Search garbage collected termcells   : 52
% 0.19/0.47  
% 0.19/0.47  # -------------------------------------------------
% 0.19/0.47  # User time                : 0.003 s
% 0.19/0.47  # System time              : 0.001 s
% 0.19/0.47  # Total time               : 0.004 s
% 0.19/0.47  # Maximum resident set size: 1600 pages
% 0.19/0.47  
% 0.19/0.47  # -------------------------------------------------
% 0.19/0.47  # User time                : 0.006 s
% 0.19/0.47  # System time              : 0.001 s
% 0.19/0.47  # Total time               : 0.007 s
% 0.19/0.47  # Maximum resident set size: 1696 pages
% 0.19/0.47  % E exiting
% 0.19/0.47  % E exiting
%------------------------------------------------------------------------------

