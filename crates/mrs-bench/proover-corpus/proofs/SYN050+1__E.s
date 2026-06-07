% Proof : Problems/SYN050+1.p
%------------------------------------------------------------------------------
% File     : E---3.3.0
% Problem  : SYN050+1 : TPTP v9.2.0. Released v2.0.0.
% Transfm  : none
% Format   : tptp:raw
% Command  : run_E /export/starexec/sandbox/benchmark/theBenchmark.p 300 THM

% Computer : n017.cluster.edu
% Model    : x86_64 x86_64
% CPU      : Intel(R) Xeon(R) CPU E5-2620 v4 2.10GHz
% Memory   : 8042.1875MB
% OS       : Linux 3.10.0-693.el7.x86_64
% CPULimit : 300s
% WCLimit  : 300s
% DateTime : Mon Sep 29 11:23:47 PM UTC 2025

% Result   : Theorem 0.15s 0.41s
% Output   : CNFRefutation 0.15s
% Verified : 
% SZS Type : Refutation
%            Derivation depth      :    4
%            Number of leaves      :    1
% Syntax   : Number of formulae    :    8 (   4 unt;   0 def)
%            Number of atoms       :   30 (   0 equ)
%            Maximal formula atoms :    9 (   3 avg)
%            Number of connectives :   31 (   9   ~;   6   |;  10   &)
%                                         (   0 <=>;   6  =>;   0  <=;   0 <~>)
%            Maximal formula depth :   12 (   5 avg)
%            Maximal term depth    :    1 (   1 avg)
%            Number of predicates  :    5 (   4 usr;   1 prp; 0-1 aty)
%            Number of functors    :    1 (   1 usr;   1 con; 0-0 aty)
%            Number of variables   :   25 (   5 sgn  12   !;   8   ?)

% Comments : 
%------------------------------------------------------------------------------
fof(pel20,conjecture,
    ( ! [X1,X2] :
      ? [X3] :
      ! [X4] :
        ( ( big_p(X1)
          & big_q(X2) )
       => ( big_r(X3)
          & big_s(X4) ) )
   => ? [X5,X6] :
        ( ( big_p(X5)
          & big_q(X6) )
       => ? [X7] : big_r(X7) ) ),
    file('/export/starexec/sandbox/benchmark/theBenchmark.p',pel20) ).

fof(c_0_1,negated_conjecture,
    ~ ( ! [X1,X2] :
        ? [X3] :
        ! [X4] :
          ( ( big_p(X1)
            & big_q(X2) )
         => ( big_r(X3)
            & big_s(X4) ) )
     => ? [X5,X6] :
          ( ( big_p(X5)
            & big_q(X6) )
         => ? [X7] : big_r(X7) ) ),
    inference(assume_negation,[status(cth)],[pel20]) ).

fof(c_0_2,negated_conjecture,
    ! [X8,X9,X11,X12,X13,X14] :
      ( ( big_r(esk1_0)
        | ~ big_p(X8)
        | ~ big_q(X9) )
      & ( big_s(X11)
        | ~ big_p(X8)
        | ~ big_q(X9) )
      & big_p(X12)
      & big_q(X13)
      & ~ big_r(X14) ),
    inference(distribute,[status(thm)],[inference(fof_nnf,[status(thm)],[inference(shift_quantors,[status(thm)],[inference(skolemize,[status(esa)],[inference(variable_rename,[status(thm)],[inference(shift_quantors,[status(thm)],[inference(fof_nnf,[status(thm)],[c_0_1])])])])])])]) ).

fof(c_0_3,negated_conjecture,
    ( big_r(esk1_0)
    | ~ big_p(X1)
    | ~ big_q(X2) ),
    inference(split_conjunct,[status(thm)],[c_0_2]) ).

fof(c_0_4,negated_conjecture,
    big_p(X1),
    inference(split_conjunct,[status(thm)],[c_0_2]) ).

fof(c_0_5,negated_conjecture,
    big_q(X1),
    inference(split_conjunct,[status(thm)],[c_0_2]) ).

fof(c_0_6,negated_conjecture,
    ~ big_r(X1),
    inference(split_conjunct,[status(thm)],[c_0_2]) ).

fof(c_0_7,negated_conjecture,
    $false,
    inference(sr,[status(thm)],[inference(cn,[status(thm)],[inference(rw,[status(thm)],[inference(rw,[status(thm)],[c_0_3,c_0_4]),c_0_5])]),c_0_6]),
    [proof] ).

%------------------------------------------------------------------------------
%----ORIGINAL SYSTEM OUTPUT
% 0.00/0.09  % Problem    : SYN050+1 : TPTP v9.2.0. Released v2.0.0.
% 0.00/0.09  % Command    : run_E /export/starexec/sandbox/benchmark/theBenchmark.p 300 THM
% 0.08/0.29  % Computer : n017.cluster.edu
% 0.08/0.29  % Model    : x86_64 x86_64
% 0.08/0.29  % CPU      : Intel(R) Xeon(R) CPU E5-2620 v4 @ 2.10GHz
% 0.08/0.29  % Memory   : 8042.1875MB
% 0.08/0.29  % OS       : Linux 3.10.0-693.el7.x86_64
% 0.08/0.29  % CPULimit   : 300
% 0.08/0.29  % WCLimit    : 300
% 0.08/0.29  % DateTime   : Fri Sep 26 14:48:08 EDT 2025
% 0.08/0.29  % CPUTime    : 
% 0.15/0.41  Running first-order theorem proving
% 0.15/0.41  Running: /export/starexec/sandbox/solver/bin/eprover --delete-bad-limit=2000000000 --definitional-cnf=24 -s --print-statistics -R --print-version --proof-object --auto-schedule=8 --cpu-limit=300 /export/starexec/sandbox/benchmark/theBenchmark.p
% 0.15/0.41  # Version: 3.0.0
% 0.15/0.41  # Preprocessing class: FSSSSMSSSSSNFFN.
% 0.15/0.41  # Scheduled 4 strats onto 8 cores with 300 seconds (2400 total)
% 0.15/0.41  # Starting G-E--_302_C18_F1_URBAN_RG_S04BN with 1500s (5) cores
% 0.15/0.41  # Starting new_bool_3 with 300s (1) cores
% 0.15/0.41  # Starting new_bool_1 with 300s (1) cores
% 0.15/0.41  # Starting sh5l with 300s (1) cores
% 0.15/0.41  # new_bool_3 with pid 7322 completed with status 0
% 0.15/0.41  # Result found by new_bool_3
% 0.15/0.41  # Preprocessing class: FSSSSMSSSSSNFFN.
% 0.15/0.41  # Scheduled 4 strats onto 8 cores with 300 seconds (2400 total)
% 0.15/0.41  # Starting G-E--_302_C18_F1_URBAN_RG_S04BN with 1500s (5) cores
% 0.15/0.41  # Starting new_bool_3 with 300s (1) cores
% 0.15/0.41  # SinE strategy is GSinE(CountFormulas,hypos,1.5,,3,20000,1.0)
% 0.15/0.41  # Search class: FHUNS-FFSF00-SFFFFFNN
% 0.15/0.41  # Scheduled 5 strats onto 1 cores with 300 seconds (300 total)
% 0.15/0.41  # Starting SAT001_MinMin_p005000_rr_RG with 181s (1) cores
% 0.15/0.41  # SAT001_MinMin_p005000_rr_RG with pid 7326 completed with status 0
% 0.15/0.41  # Result found by SAT001_MinMin_p005000_rr_RG
% 0.15/0.41  # Preprocessing class: FSSSSMSSSSSNFFN.
% 0.15/0.41  # Scheduled 4 strats onto 8 cores with 300 seconds (2400 total)
% 0.15/0.41  # Starting G-E--_302_C18_F1_URBAN_RG_S04BN with 1500s (5) cores
% 0.15/0.41  # Starting new_bool_3 with 300s (1) cores
% 0.15/0.41  # SinE strategy is GSinE(CountFormulas,hypos,1.5,,3,20000,1.0)
% 0.15/0.41  # Search class: FHUNS-FFSF00-SFFFFFNN
% 0.15/0.41  # Scheduled 5 strats onto 1 cores with 300 seconds (300 total)
% 0.15/0.41  # Starting SAT001_MinMin_p005000_rr_RG with 181s (1) cores
% 0.15/0.41  # Preprocessing time       : 0.001 s
% 0.15/0.41  # Presaturation interreduction done
% 0.15/0.41  
% 0.15/0.41  # Proof found!
% 0.15/0.41  # SZS status Theorem
% 0.15/0.41  # SZS output start CNFRefutation
% See solution above
% 0.15/0.41  # Parsed axioms                        : 1
% 0.15/0.41  # Removed by relevancy pruning/SinE    : 0
% 0.15/0.41  # Initial clauses                      : 5
% 0.15/0.41  # Removed in clause preprocessing      : 0
% 0.15/0.41  # Initial clauses in saturation        : 5
% 0.15/0.41  # Processed clauses                    : 4
% 0.15/0.41  # ...of these trivial                  : 0
% 0.15/0.41  # ...subsumed                          : 0
% 0.15/0.41  # ...remaining for further processing  : 3
% 0.15/0.41  # Other redundant clauses eliminated   : 0
% 0.15/0.41  # Clauses deleted for lack of memory   : 0
% 0.15/0.41  # Backward-subsumed                    : 0
% 0.15/0.41  # Backward-rewritten                   : 0
% 0.15/0.41  # Generated clauses                    : 0
% 0.15/0.41  # ...of the previous two non-redundant : 0
% 0.15/0.41  # ...aggressively subsumed             : 0
% 0.15/0.41  # Contextual simplify-reflections      : 0
% 0.15/0.41  # Paramodulations                      : 0
% 0.15/0.41  # Factorizations                       : 0
% 0.15/0.41  # NegExts                              : 0
% 0.15/0.41  # Equation resolutions                 : 0
% 0.15/0.41  # Disequality decompositions           : 0
% 0.15/0.41  # Total rewrite steps                  : 2
% 0.15/0.41  # ...of those cached                   : 0
% 0.15/0.41  # Propositional unsat checks           : 0
% 0.15/0.41  #    Propositional check models        : 0
% 0.15/0.41  #    Propositional check unsatisfiable : 0
% 0.15/0.41  #    Propositional clauses             : 0
% 0.15/0.41  #    Propositional clauses after purity: 0
% 0.15/0.41  #    Propositional unsat core size     : 0
% 0.15/0.41  #    Propositional preprocessing time  : 0.000
% 0.15/0.41  #    Propositional encoding time       : 0.000
% 0.15/0.41  #    Propositional solver time         : 0.000
% 0.15/0.41  #    Success case prop preproc time    : 0.000
% 0.15/0.41  #    Success case prop encoding time   : 0.000
% 0.15/0.41  #    Success case prop solver time     : 0.000
% 0.15/0.41  # Current number of processed clauses  : 3
% 0.15/0.41  #    Positive orientable unit clauses  : 2
% 0.15/0.41  #    Positive unorientable unit clauses: 0
% 0.15/0.41  #    Negative unit clauses             : 1
% 0.15/0.41  #    Non-unit-clauses                  : 0
% 0.15/0.41  # Current number of unprocessed clauses: 1
% 0.15/0.41  # ...number of literals in the above   : 3
% 0.15/0.41  # Current number of archived formulas  : 0
% 0.15/0.41  # Current number of archived clauses   : 0
% 0.15/0.41  # Clause-clause subsumption calls (NU) : 0
% 0.15/0.41  # Rec. Clause-clause subsumption calls : 0
% 0.15/0.41  # Non-unit clause-clause subsumptions  : 0
% 0.15/0.41  # Unit Clause-clause subsumption calls : 0
% 0.15/0.41  # Rewrite failures with RHS unbound    : 0
% 0.15/0.41  # BW rewrite match attempts            : 0
% 0.15/0.41  # BW rewrite match successes           : 0
% 0.15/0.41  # Condensation attempts                : 0
% 0.15/0.41  # Condensation successes               : 0
% 0.15/0.41  # Termbank termtop insertions          : 357
% 0.15/0.41  # Search garbage collected termcells   : 154
% 0.15/0.41  
% 0.15/0.41  # -------------------------------------------------
% 0.15/0.41  # User time                : 0.001 s
% 0.15/0.41  # System time              : 0.001 s
% 0.15/0.41  # Total time               : 0.003 s
% 0.15/0.41  # Maximum resident set size: 1768 pages
% 0.15/0.41  
% 0.15/0.41  # -------------------------------------------------
% 0.15/0.41  # User time                : 0.003 s
% 0.15/0.41  # System time              : 0.002 s
% 0.15/0.41  # Total time               : 0.005 s
% 0.15/0.41  # Maximum resident set size: 1692 pages
% 0.15/0.41  % E exiting
% 0.15/0.42  % E exiting
%------------------------------------------------------------------------------

