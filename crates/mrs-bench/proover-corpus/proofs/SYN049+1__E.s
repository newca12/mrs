% Proof : Problems/SYN049+1.p
%------------------------------------------------------------------------------
% File     : E---3.3.0
% Problem  : SYN049+1 : TPTP v9.2.0. Released v2.0.0.
% Transfm  : none
% Format   : tptp:raw
% Command  : run_E /export/starexec/sandbox2/benchmark/theBenchmark.p 300 THM

% Computer : n022.cluster.edu
% Model    : x86_64 x86_64
% CPU      : Intel(R) Xeon(R) CPU E5-2620 v4 2.10GHz
% Memory   : 8042.1875MB
% OS       : Linux 3.10.0-693.el7.x86_64
% CPULimit : 300s
% WCLimit  : 300s
% DateTime : Mon Sep 29 11:23:47 PM UTC 2025

% Result   : Theorem 0.18s 0.47s
% Output   : CNFRefutation 0.18s
% Verified : 
% SZS Type : Refutation
%            Derivation depth      :    5
%            Number of leaves      :    1
% Syntax   : Number of formulae    :    8 (   4 unt;   0 def)
%            Number of atoms       :   18 (   0 equ)
%            Maximal formula atoms :    4 (   2 avg)
%            Number of connectives :   15 (   5   ~;   2   |;   2   &)
%                                         (   0 <=>;   6  =>;   0  <=;   0 <~>)
%            Maximal formula depth :    7 (   3 avg)
%            Maximal term depth    :    1 (   1 avg)
%            Number of predicates  :    3 (   2 usr;   1 prp; 0-1 aty)
%            Number of functors    :    2 (   2 usr;   2 con; 0-0 aty)
%            Number of variables   :   10 (   2 sgn   6   !;   2   ?)

% Comments : 
%------------------------------------------------------------------------------
fof(pel19,conjecture,
    ? [X1] :
    ! [X2,X3] :
      ( ( big_p(X2)
       => big_q(X3) )
     => ( big_p(X1)
       => big_q(X1) ) ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',pel19) ).

fof(c_0_1,negated_conjecture,
    ~ ? [X1] :
      ! [X2,X3] :
        ( ( big_p(X2)
         => big_q(X3) )
       => ( big_p(X1)
         => big_q(X1) ) ),
    inference(assume_negation,[status(cth)],[pel19]) ).

fof(c_0_2,negated_conjecture,
    ! [X6,X7] :
      ( ( ~ big_p(esk1_0)
        | big_q(esk2_0) )
      & big_p(X6)
      & ~ big_q(X7) ),
    inference(fof_nnf,[status(thm)],[inference(shift_quantors,[status(thm)],[inference(skolemize,[status(esa)],[inference(variable_rename,[status(thm)],[inference(shift_quantors,[status(thm)],[inference(fof_nnf,[status(thm)],[c_0_1])])])])])]) ).

fof(c_0_3,negated_conjecture,
    ( big_q(esk2_0)
    | ~ big_p(esk1_0) ),
    inference(split_conjunct,[status(thm)],[c_0_2]) ).

fof(c_0_4,negated_conjecture,
    big_p(X1),
    inference(split_conjunct,[status(thm)],[c_0_2]) ).

fof(c_0_5,negated_conjecture,
    big_q(esk2_0),
    inference(cn,[status(thm)],[inference(rw,[status(thm)],[c_0_3,c_0_4])]) ).

fof(c_0_6,negated_conjecture,
    ~ big_q(X1),
    inference(split_conjunct,[status(thm)],[c_0_2]) ).

fof(c_0_7,negated_conjecture,
    $false,
    inference(sr,[status(thm)],[c_0_5,c_0_6]),
    [proof] ).

%------------------------------------------------------------------------------
%----ORIGINAL SYSTEM OUTPUT
% 0.11/0.12  % Problem    : SYN049+1 : TPTP v9.2.0. Released v2.0.0.
% 0.11/0.12  % Command    : run_E /export/starexec/sandbox2/benchmark/theBenchmark.p 300 THM
% 0.12/0.32  % Computer : n022.cluster.edu
% 0.12/0.32  % Model    : x86_64 x86_64
% 0.12/0.32  % CPU      : Intel(R) Xeon(R) CPU E5-2620 v4 @ 2.10GHz
% 0.12/0.32  % Memory   : 8042.1875MB
% 0.12/0.32  % OS       : Linux 3.10.0-693.el7.x86_64
% 0.12/0.32  % CPULimit   : 300
% 0.12/0.32  % WCLimit    : 300
% 0.12/0.32  % DateTime   : Fri Sep 26 14:32:53 EDT 2025
% 0.12/0.32  % CPUTime    : 
% 0.18/0.46  Running first-order theorem proving
% 0.18/0.46  Running: /export/starexec/sandbox2/solver/bin/eprover --delete-bad-limit=2000000000 --definitional-cnf=24 -s --print-statistics -R --print-version --proof-object --auto-schedule=8 --cpu-limit=300 /export/starexec/sandbox2/benchmark/theBenchmark.p
% 0.18/0.47  # Version: 3.0.0
% 0.18/0.47  # Preprocessing class: FSSSSMSSSSSNFFN.
% 0.18/0.47  # Scheduled 4 strats onto 8 cores with 300 seconds (2400 total)
% 0.18/0.47  # Starting G-E--_302_C18_F1_URBAN_RG_S04BN with 1500s (5) cores
% 0.18/0.47  # Starting new_bool_3 with 300s (1) cores
% 0.18/0.47  # Starting new_bool_1 with 300s (1) cores
% 0.18/0.47  # Starting sh5l with 300s (1) cores
% 0.18/0.47  # G-E--_302_C18_F1_URBAN_RG_S04BN with pid 6035 completed with status 0
% 0.18/0.47  # Result found by G-E--_302_C18_F1_URBAN_RG_S04BN
% 0.18/0.47  # Preprocessing class: FSSSSMSSSSSNFFN.
% 0.18/0.47  # Scheduled 4 strats onto 8 cores with 300 seconds (2400 total)
% 0.18/0.47  # Starting G-E--_302_C18_F1_URBAN_RG_S04BN with 1500s (5) cores
% 0.18/0.47  # No SInE strategy applied
% 0.18/0.47  # Search class: FUUNF-FFSF00-SFFFFFNN
% 0.18/0.47  # Scheduled 6 strats onto 5 cores with 1500 seconds (1500 total)
% 0.18/0.47  # Starting SAT001_MinMin_p005000_rr_RG with 811s (1) cores
% 0.18/0.47  # Starting G-E--_302_C18_F1_URBAN_RG_S04BN with 151s (1) cores
% 0.18/0.47  # Starting new_bool_3 with 136s (1) cores
% 0.18/0.47  # Starting new_bool_1 with 136s (1) cores
% 0.18/0.47  # Starting sh5l with 136s (1) cores
% 0.18/0.47  # sh5l with pid 6043 completed with status 0
% 0.18/0.47  # Result found by sh5l
% 0.18/0.47  # Preprocessing class: FSSSSMSSSSSNFFN.
% 0.18/0.47  # Scheduled 4 strats onto 8 cores with 300 seconds (2400 total)
% 0.18/0.47  # Starting G-E--_302_C18_F1_URBAN_RG_S04BN with 1500s (5) cores
% 0.18/0.47  # No SInE strategy applied
% 0.18/0.47  # Search class: FUUNF-FFSF00-SFFFFFNN
% 0.18/0.47  # Scheduled 6 strats onto 5 cores with 1500 seconds (1500 total)
% 0.18/0.47  # Starting SAT001_MinMin_p005000_rr_RG with 811s (1) cores
% 0.18/0.47  # Starting G-E--_302_C18_F1_URBAN_RG_S04BN with 151s (1) cores
% 0.18/0.47  # Starting new_bool_3 with 136s (1) cores
% 0.18/0.47  # Starting new_bool_1 with 136s (1) cores
% 0.18/0.47  # Starting sh5l with 136s (1) cores
% 0.18/0.47  # Preprocessing time       : 0.001 s
% 0.18/0.47  # Presaturation interreduction done
% 0.18/0.47  
% 0.18/0.47  # Proof found!
% 0.18/0.47  # SZS status Theorem
% 0.18/0.47  # SZS output start CNFRefutation
% See solution above
% 0.18/0.47  # Parsed axioms                        : 1
% 0.18/0.47  # Removed by relevancy pruning/SinE    : 0
% 0.18/0.47  # Initial clauses                      : 3
% 0.18/0.47  # Removed in clause preprocessing      : 1
% 0.18/0.47  # Initial clauses in saturation        : 2
% 0.18/0.47  # Processed clauses                    : 2
% 0.18/0.47  # ...of these trivial                  : 0
% 0.18/0.47  # ...subsumed                          : 0
% 0.18/0.47  # ...remaining for further processing  : 2
% 0.18/0.47  # Other redundant clauses eliminated   : 0
% 0.18/0.47  # Clauses deleted for lack of memory   : 0
% 0.18/0.47  # Backward-subsumed                    : 0
% 0.18/0.47  # Backward-rewritten                   : 0
% 0.18/0.47  # Generated clauses                    : 1
% 0.18/0.47  # ...of the previous two non-redundant : 0
% 0.18/0.47  # ...aggressively subsumed             : 0
% 0.18/0.47  # Contextual simplify-reflections      : 0
% 0.18/0.47  # Paramodulations                      : 0
% 0.18/0.47  # Factorizations                       : 0
% 0.18/0.47  # NegExts                              : 0
% 0.18/0.47  # Equation resolutions                 : 0
% 0.18/0.47  # Disequality decompositions           : 0
% 0.18/0.47  # Total rewrite steps                  : 0
% 0.18/0.47  # ...of those cached                   : 0
% 0.18/0.47  # Propositional unsat checks           : 0
% 0.18/0.47  #    Propositional check models        : 0
% 0.18/0.47  #    Propositional check unsatisfiable : 0
% 0.18/0.47  #    Propositional clauses             : 0
% 0.18/0.47  #    Propositional clauses after purity: 0
% 0.18/0.47  #    Propositional unsat core size     : 0
% 0.18/0.47  #    Propositional preprocessing time  : 0.000
% 0.18/0.47  #    Propositional encoding time       : 0.000
% 0.18/0.47  #    Propositional solver time         : 0.000
% 0.18/0.47  #    Success case prop preproc time    : 0.000
% 0.18/0.47  #    Success case prop encoding time   : 0.000
% 0.18/0.47  #    Success case prop solver time     : 0.000
% 0.18/0.47  # Current number of processed clauses  : 1
% 0.18/0.47  #    Positive orientable unit clauses  : 0
% 0.18/0.47  #    Positive unorientable unit clauses: 0
% 0.18/0.47  #    Negative unit clauses             : 1
% 0.18/0.47  #    Non-unit-clauses                  : 0
% 0.18/0.47  # Current number of unprocessed clauses: 0
% 0.18/0.47  # ...number of literals in the above   : 0
% 0.18/0.47  # Current number of archived formulas  : 0
% 0.18/0.47  # Current number of archived clauses   : 2
% 0.18/0.47  # Clause-clause subsumption calls (NU) : 0
% 0.18/0.47  # Rec. Clause-clause subsumption calls : 0
% 0.18/0.47  # Non-unit clause-clause subsumptions  : 0
% 0.18/0.47  # Unit Clause-clause subsumption calls : 0
% 0.18/0.47  # Rewrite failures with RHS unbound    : 0
% 0.18/0.47  # BW rewrite match attempts            : 0
% 0.18/0.47  # BW rewrite match successes           : 0
% 0.18/0.47  # Condensation attempts                : 2
% 0.18/0.47  # Condensation successes               : 0
% 0.18/0.47  # Termbank termtop insertions          : 174
% 0.18/0.47  # Search garbage collected termcells   : 77
% 0.18/0.47  
% 0.18/0.47  # -------------------------------------------------
% 0.18/0.47  # User time                : 0.002 s
% 0.18/0.47  # System time              : 0.000 s
% 0.18/0.47  # Total time               : 0.002 s
% 0.18/0.47  # Maximum resident set size: 1684 pages
% 0.18/0.47  
% 0.18/0.47  # -------------------------------------------------
% 0.18/0.47  # User time                : 0.007 s
% 0.18/0.47  # System time              : 0.005 s
% 0.18/0.47  # Total time               : 0.012 s
% 0.18/0.47  # Maximum resident set size: 1692 pages
% 0.18/0.47  % E exiting
% 0.18/0.48  % E exiting
%------------------------------------------------------------------------------

