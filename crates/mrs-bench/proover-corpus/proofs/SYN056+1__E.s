% Proof : Problems/SYN056+1.p
%------------------------------------------------------------------------------
% File     : E---3.3.0
% Problem  : SYN056+1 : TPTP v9.2.0. Released v2.0.0.
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
% DateTime : Mon Sep 29 11:23:49 PM UTC 2025

% Result   : Theorem 0.19s 0.48s
% Output   : CNFRefutation 0.19s
% Verified : 
% SZS Type : Refutation
%            Derivation depth      :    8
%            Number of leaves      :    3
% Syntax   : Number of formulae    :   26 (   5 unt;   0 def)
%            Number of atoms       :   79 (   0 equ)
%            Maximal formula atoms :   12 (   3 avg)
%            Number of connectives :   89 (  36   ~;  37   |;   7   &)
%                                         (   4 <=>;   5  =>;   0  <=;   0 <~>)
%            Maximal formula depth :   11 (   4 avg)
%            Maximal term depth    :    1 (   1 avg)
%            Number of predicates  :    5 (   4 usr;   1 prp; 0-1 aty)
%            Number of functors    :    4 (   4 usr;   4 con; 0-0 aty)
%            Number of variables   :   28 (   4 sgn  12   !;   2   ?)

% Comments : 
%------------------------------------------------------------------------------
fof(pel26,conjecture,
    ( ! [X1] :
        ( big_p(X1)
       => big_r(X1) )
  <=> ! [X2] :
        ( big_q(X2)
       => big_s(X2) ) ),
    file('/export/starexec/sandbox/benchmark/theBenchmark.p',pel26) ).

fof(pel26_1,axiom,
    ( ? [X1] : big_p(X1)
  <=> ? [X2] : big_q(X2) ),
    file('/export/starexec/sandbox/benchmark/theBenchmark.p',pel26_1) ).

fof(pel26_2,axiom,
    ! [X1,X2] :
      ( ( big_p(X1)
        & big_q(X2) )
     => ( big_r(X1)
      <=> big_s(X2) ) ),
    file('/export/starexec/sandbox/benchmark/theBenchmark.p',pel26_2) ).

fof(c_0_3,negated_conjecture,
    ~ ( ! [X1] :
          ( big_p(X1)
         => big_r(X1) )
    <=> ! [X2] :
          ( big_q(X2)
         => big_s(X2) ) ),
    inference(assume_negation,[status(cth)],[pel26]) ).

fof(c_0_4,plain,
    ! [X9,X11] :
      ( ( ~ big_p(X9)
        | big_q(esk3_0) )
      & ( ~ big_q(X11)
        | big_p(esk4_0) ) ),
    inference(fof_nnf,[status(thm)],[inference(shift_quantors,[status(thm)],[inference(skolemize,[status(esa)],[inference(variable_rename,[status(thm)],[inference(fof_nnf,[status(thm)],[pel26_1])])])])]) ).

fof(c_0_5,negated_conjecture,
    ! [X5,X6] :
      ( ( big_q(esk2_0)
        | big_p(esk1_0) )
      & ( ~ big_s(esk2_0)
        | big_p(esk1_0) )
      & ( big_q(esk2_0)
        | ~ big_r(esk1_0) )
      & ( ~ big_s(esk2_0)
        | ~ big_r(esk1_0) )
      & ( ~ big_p(X5)
        | big_r(X5)
        | ~ big_q(X6)
        | big_s(X6) ) ),
    inference(distribute,[status(thm)],[inference(fof_nnf,[status(thm)],[inference(shift_quantors,[status(thm)],[inference(skolemize,[status(esa)],[inference(variable_rename,[status(thm)],[inference(fof_nnf,[status(thm)],[c_0_3])])])])])]) ).

fof(c_0_6,plain,
    ! [X7,X8] :
      ( ( ~ big_r(X7)
        | big_s(X8)
        | ~ big_p(X7)
        | ~ big_q(X8) )
      & ( ~ big_s(X8)
        | big_r(X7)
        | ~ big_p(X7)
        | ~ big_q(X8) ) ),
    inference(distribute,[status(thm)],[inference(fof_nnf,[status(thm)],[inference(variable_rename,[status(thm)],[inference(fof_nnf,[status(thm)],[pel26_2])])])]) ).

fof(c_0_7,plain,
    ( big_p(esk4_0)
    | ~ big_q(X1) ),
    inference(split_conjunct,[status(thm)],[c_0_4]) ).

fof(c_0_8,negated_conjecture,
    ( big_q(esk2_0)
    | big_p(esk1_0) ),
    inference(split_conjunct,[status(thm)],[c_0_5]) ).

fof(c_0_9,plain,
    ( big_r(X2)
    | ~ big_s(X1)
    | ~ big_p(X2)
    | ~ big_q(X1) ),
    inference(split_conjunct,[status(thm)],[c_0_6]) ).

fof(c_0_10,negated_conjecture,
    ( big_r(X1)
    | big_s(X2)
    | ~ big_p(X1)
    | ~ big_q(X2) ),
    inference(split_conjunct,[status(thm)],[c_0_5]) ).

fof(c_0_11,plain,
    ( big_q(esk3_0)
    | ~ big_p(X1) ),
    inference(split_conjunct,[status(thm)],[c_0_4]) ).

fof(c_0_12,negated_conjecture,
    ( big_p(esk1_0)
    | big_p(esk4_0) ),
    inference(spm,[status(thm)],[c_0_7,c_0_8]) ).

fof(c_0_13,plain,
    ( big_r(X1)
    | ~ big_q(X2)
    | ~ big_p(X1) ),
    inference(csr,[status(thm)],[c_0_9,c_0_10]) ).

fof(c_0_14,negated_conjecture,
    big_q(esk3_0),
    inference(csr,[status(thm)],[inference(spm,[status(thm)],[c_0_11,c_0_12]),c_0_11]) ).

fof(c_0_15,plain,
    ( big_s(X2)
    | ~ big_r(X1)
    | ~ big_p(X1)
    | ~ big_q(X2) ),
    inference(split_conjunct,[status(thm)],[c_0_6]) ).

fof(c_0_16,negated_conjecture,
    ( ~ big_s(esk2_0)
    | ~ big_r(esk1_0) ),
    inference(split_conjunct,[status(thm)],[c_0_5]) ).

fof(c_0_17,negated_conjecture,
    ( big_r(X1)
    | ~ big_p(X1) ),
    inference(spm,[status(thm)],[c_0_13,c_0_14]) ).

fof(c_0_18,negated_conjecture,
    ( big_p(esk1_0)
    | ~ big_s(esk2_0) ),
    inference(split_conjunct,[status(thm)],[c_0_5]) ).

fof(c_0_19,plain,
    ( big_s(X1)
    | ~ big_q(X1)
    | ~ big_p(X2) ),
    inference(csr,[status(thm)],[c_0_15,c_0_13]) ).

fof(c_0_20,negated_conjecture,
    big_p(esk4_0),
    inference(spm,[status(thm)],[c_0_7,c_0_14]) ).

fof(c_0_21,negated_conjecture,
    ( big_q(esk2_0)
    | ~ big_r(esk1_0) ),
    inference(split_conjunct,[status(thm)],[c_0_5]) ).

fof(c_0_22,negated_conjecture,
    ~ big_s(esk2_0),
    inference(csr,[status(thm)],[inference(spm,[status(thm)],[c_0_16,c_0_17]),c_0_18]) ).

fof(c_0_23,negated_conjecture,
    ( big_s(X1)
    | ~ big_q(X1) ),
    inference(spm,[status(thm)],[c_0_19,c_0_20]) ).

fof(c_0_24,negated_conjecture,
    big_q(esk2_0),
    inference(csr,[status(thm)],[inference(spm,[status(thm)],[c_0_21,c_0_17]),c_0_8]) ).

fof(c_0_25,negated_conjecture,
    $false,
    inference(cn,[status(thm)],[inference(rw,[status(thm)],[inference(spm,[status(thm)],[c_0_22,c_0_23]),c_0_24])]),
    [proof] ).

%------------------------------------------------------------------------------
%----ORIGINAL SYSTEM OUTPUT
% 0.11/0.11  % Problem    : SYN056+1 : TPTP v9.2.0. Released v2.0.0.
% 0.11/0.11  % Command    : run_E /export/starexec/sandbox/benchmark/theBenchmark.p 300 THM
% 0.11/0.32  % Computer : n026.cluster.edu
% 0.11/0.32  % Model    : x86_64 x86_64
% 0.11/0.32  % CPU      : Intel(R) Xeon(R) CPU E5-2620 v4 @ 2.10GHz
% 0.11/0.32  % Memory   : 8042.1875MB
% 0.11/0.32  % OS       : Linux 3.10.0-693.el7.x86_64
% 0.11/0.32  % CPULimit   : 300
% 0.11/0.32  % WCLimit    : 300
% 0.11/0.32  % DateTime   : Fri Sep 26 15:00:53 EDT 2025
% 0.11/0.32  % CPUTime    : 
% 0.19/0.46  Running first-order theorem proving
% 0.19/0.46  Running: /export/starexec/sandbox/solver/bin/eprover --delete-bad-limit=2000000000 --definitional-cnf=24 -s --print-statistics -R --print-version --proof-object --auto-schedule=8 --cpu-limit=300 /export/starexec/sandbox/benchmark/theBenchmark.p
% 0.19/0.48  # Version: 3.0.0
% 0.19/0.48  # Preprocessing class: FSSSSMSSSSSNFFN.
% 0.19/0.48  # Scheduled 4 strats onto 8 cores with 300 seconds (2400 total)
% 0.19/0.48  # Starting G-E--_302_C18_F1_URBAN_RG_S04BN with 1500s (5) cores
% 0.19/0.48  # Starting new_bool_3 with 300s (1) cores
% 0.19/0.48  # Starting new_bool_1 with 300s (1) cores
% 0.19/0.48  # Starting sh5l with 300s (1) cores
% 0.19/0.48  # new_bool_3 with pid 30105 completed with status 0
% 0.19/0.48  # Result found by new_bool_3
% 0.19/0.48  # Preprocessing class: FSSSSMSSSSSNFFN.
% 0.19/0.48  # Scheduled 4 strats onto 8 cores with 300 seconds (2400 total)
% 0.19/0.48  # Starting G-E--_302_C18_F1_URBAN_RG_S04BN with 1500s (5) cores
% 0.19/0.48  # Starting new_bool_3 with 300s (1) cores
% 0.19/0.48  # SinE strategy is GSinE(CountFormulas,hypos,1.5,,3,20000,1.0)
% 0.19/0.48  # Search class: FGHNF-FFSF00-SFFFFFNN
% 0.19/0.48  # Scheduled 5 strats onto 1 cores with 300 seconds (300 total)
% 0.19/0.48  # Starting SAT001_MinMin_p005000_rr_RG with 181s (1) cores
% 0.19/0.48  # SAT001_MinMin_p005000_rr_RG with pid 30108 completed with status 0
% 0.19/0.48  # Result found by SAT001_MinMin_p005000_rr_RG
% 0.19/0.48  # Preprocessing class: FSSSSMSSSSSNFFN.
% 0.19/0.48  # Scheduled 4 strats onto 8 cores with 300 seconds (2400 total)
% 0.19/0.48  # Starting G-E--_302_C18_F1_URBAN_RG_S04BN with 1500s (5) cores
% 0.19/0.48  # Starting new_bool_3 with 300s (1) cores
% 0.19/0.48  # SinE strategy is GSinE(CountFormulas,hypos,1.5,,3,20000,1.0)
% 0.19/0.48  # Search class: FGHNF-FFSF00-SFFFFFNN
% 0.19/0.48  # Scheduled 5 strats onto 1 cores with 300 seconds (300 total)
% 0.19/0.48  # Starting SAT001_MinMin_p005000_rr_RG with 181s (1) cores
% 0.19/0.48  # Preprocessing time       : 0.001 s
% 0.19/0.48  # Presaturation interreduction done
% 0.19/0.48  
% 0.19/0.48  # Proof found!
% 0.19/0.48  # SZS status Theorem
% 0.19/0.48  # SZS output start CNFRefutation
% See solution above
% 0.19/0.48  # Parsed axioms                        : 3
% 0.19/0.48  # Removed by relevancy pruning/SinE    : 0
% 0.19/0.48  # Initial clauses                      : 9
% 0.19/0.48  # Removed in clause preprocessing      : 0
% 0.19/0.48  # Initial clauses in saturation        : 9
% 0.19/0.48  # Processed clauses                    : 24
% 0.19/0.48  # ...of these trivial                  : 0
% 0.19/0.48  # ...subsumed                          : 0
% 0.19/0.48  # ...remaining for further processing  : 24
% 0.19/0.48  # Other redundant clauses eliminated   : 0
% 0.19/0.48  # Clauses deleted for lack of memory   : 0
% 0.19/0.48  # Backward-subsumed                    : 7
% 0.19/0.48  # Backward-rewritten                   : 3
% 0.19/0.48  # Generated clauses                    : 9
% 0.19/0.48  # ...of the previous two non-redundant : 8
% 0.19/0.48  # ...aggressively subsumed             : 0
% 0.19/0.48  # Contextual simplify-reflections      : 5
% 0.19/0.48  # Paramodulations                      : 9
% 0.19/0.48  # Factorizations                       : 0
% 0.19/0.48  # NegExts                              : 0
% 0.19/0.48  # Equation resolutions                 : 0
% 0.19/0.48  # Disequality decompositions           : 0
% 0.19/0.48  # Total rewrite steps                  : 4
% 0.19/0.48  # ...of those cached                   : 1
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
% 0.19/0.48  # Current number of processed clauses  : 6
% 0.19/0.48  #    Positive orientable unit clauses  : 3
% 0.19/0.48  #    Positive unorientable unit clauses: 0
% 0.19/0.48  #    Negative unit clauses             : 1
% 0.19/0.48  #    Non-unit-clauses                  : 2
% 0.19/0.48  # Current number of unprocessed clauses: 0
% 0.19/0.48  # ...number of literals in the above   : 0
% 0.19/0.48  # Current number of archived formulas  : 0
% 0.19/0.48  # Current number of archived clauses   : 18
% 0.19/0.48  # Clause-clause subsumption calls (NU) : 13
% 0.19/0.48  # Rec. Clause-clause subsumption calls : 12
% 0.19/0.48  # Non-unit clause-clause subsumptions  : 8
% 0.19/0.48  # Unit Clause-clause subsumption calls : 6
% 0.19/0.48  # Rewrite failures with RHS unbound    : 0
% 0.19/0.48  # BW rewrite match attempts            : 3
% 0.19/0.48  # BW rewrite match successes           : 3
% 0.19/0.48  # Condensation attempts                : 0
% 0.19/0.48  # Condensation successes               : 0
% 0.19/0.48  # Termbank termtop insertions          : 715
% 0.19/0.48  # Search garbage collected termcells   : 184
% 0.19/0.48  
% 0.19/0.48  # -------------------------------------------------
% 0.19/0.48  # User time                : 0.002 s
% 0.19/0.48  # System time              : 0.003 s
% 0.19/0.48  # Total time               : 0.005 s
% 0.19/0.48  # Maximum resident set size: 1768 pages
% 0.19/0.48  
% 0.19/0.48  # -------------------------------------------------
% 0.19/0.48  # User time                : 0.003 s
% 0.19/0.48  # System time              : 0.005 s
% 0.19/0.48  # Total time               : 0.008 s
% 0.19/0.48  # Maximum resident set size: 1696 pages
% 0.19/0.48  % E exiting
% 0.19/0.48  % E exiting
%------------------------------------------------------------------------------

