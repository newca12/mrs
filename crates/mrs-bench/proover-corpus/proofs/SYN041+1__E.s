% Proof : Problems/SYN041+1.p
%------------------------------------------------------------------------------
% File     : E---3.3.0
% Problem  : SYN041+1 : TPTP v9.2.0. Released v2.0.0.
% Transfm  : none
% Format   : tptp:raw
% Command  : run_E /export/starexec/sandbox/benchmark/theBenchmark.p 300 THM

% Computer : n024.cluster.edu
% Model    : x86_64 x86_64
% CPU      : Intel(R) Xeon(R) CPU E5-2620 v4 2.10GHz
% Memory   : 8042.1875MB
% OS       : Linux 3.10.0-693.el7.x86_64
% CPULimit : 300s
% WCLimit  : 300s
% DateTime : Mon Sep 29 11:23:24 PM UTC 2025

% Result   : Theorem 0.11s 0.34s
% Output   : CNFRefutation 0.11s
% Verified : 
% SZS Type : Refutation
%            Derivation depth      :    4
%            Number of leaves      :    1
% Syntax   : Number of formulae    :    6 (   3 unt;   0 def)
%            Number of atoms       :   15 (   0 equ)
%            Maximal formula atoms :    4 (   2 avg)
%            Number of connectives :   15 (   6   ~;   0   |;   3   &)
%                                         (   0 <=>;   6  =>;   0  <=;   0 <~>)
%            Maximal formula depth :    5 (   3 avg)
%            Maximal term depth    :    0 (   0 avg)
%            Number of predicates  :    3 (   2 usr;   3 prp; 0-0 aty)
%            Number of functors    :    0 (   0 usr;   0 con; --- aty)
%            Number of variables   :    0 (   0 sgn   0   !;   0   ?)

% Comments : 
%------------------------------------------------------------------------------
fof(pel3,conjecture,
    ( ~ ( p
       => q )
   => ( q
     => p ) ),
    file('/export/starexec/sandbox/benchmark/theBenchmark.p',pel3) ).

fof(c_0_1,negated_conjecture,
    ~ ( ~ ( p
         => q )
     => ( q
       => p ) ),
    inference(assume_negation,[status(cth)],[pel3]) ).

fof(c_0_2,negated_conjecture,
    ( p
    & ~ q
    & q
    & ~ p ),
    inference(fof_nnf,[status(thm)],[inference(fof_nnf,[status(thm)],[c_0_1])]) ).

fof(c_0_3,negated_conjecture,
    ~ p,
    inference(split_conjunct,[status(thm)],[c_0_2]) ).

fof(c_0_4,negated_conjecture,
    p,
    inference(split_conjunct,[status(thm)],[c_0_2]) ).

fof(c_0_5,negated_conjecture,
    $false,
    inference(cn,[status(thm)],[inference(rw,[status(thm)],[c_0_3,c_0_4])]),
    [proof] ).

%------------------------------------------------------------------------------
%----ORIGINAL SYSTEM OUTPUT
% 0.01/0.07  % Problem    : SYN041+1 : TPTP v9.2.0. Released v2.0.0.
% 0.01/0.07  % Command    : run_E /export/starexec/sandbox/benchmark/theBenchmark.p 300 THM
% 0.06/0.25  % Computer : n024.cluster.edu
% 0.06/0.25  % Model    : x86_64 x86_64
% 0.06/0.25  % CPU      : Intel(R) Xeon(R) CPU E5-2620 v4 @ 2.10GHz
% 0.06/0.25  % Memory   : 8042.1875MB
% 0.06/0.25  % OS       : Linux 3.10.0-693.el7.x86_64
% 0.06/0.25  % CPULimit   : 300
% 0.06/0.25  % WCLimit    : 300
% 0.06/0.25  % DateTime   : Fri Sep 26 15:04:08 EDT 2025
% 0.06/0.26  % CPUTime    : 
% 0.11/0.34  Running first-order theorem proving
% 0.11/0.34  Running: /export/starexec/sandbox/solver/bin/eprover --delete-bad-limit=2000000000 --definitional-cnf=24 -s --print-statistics -R --print-version --proof-object --auto-schedule=8 --cpu-limit=300 /export/starexec/sandbox/benchmark/theBenchmark.p
% 0.11/0.34  # Version: 3.0.0
% 0.11/0.34  # Preprocessing class: FSSSSLSSSSSNFFN.
% 0.11/0.34  # Scheduled 4 strats onto 8 cores with 300 seconds (2400 total)
% 0.11/0.34  # Starting SAT001_MinMin_p005000_rr_RG with 1500s (5) cores
% 0.11/0.34  # Starting new_bool_3 with 300s (1) cores
% 0.11/0.34  # Starting new_bool_1 with 300s (1) cores
% 0.11/0.34  # Starting sh5l with 300s (1) cores
% 0.11/0.34  # sh5l with pid 6205 completed with status 0
% 0.11/0.34  # Result found by sh5l
% 0.11/0.34  # Preprocessing class: FSSSSLSSSSSNFFN.
% 0.11/0.34  # Scheduled 4 strats onto 8 cores with 300 seconds (2400 total)
% 0.11/0.34  # Starting SAT001_MinMin_p005000_rr_RG with 1500s (5) cores
% 0.11/0.34  # Starting new_bool_3 with 300s (1) cores
% 0.11/0.34  # Starting new_bool_1 with 300s (1) cores
% 0.11/0.34  # Starting sh5l with 300s (1) cores
% 0.11/0.34  # SinE strategy is gf500_gu_R04_F100_L20000
% 0.11/0.34  # Search class: FUUNF-FFSF00-SFFFFFNN
% 0.11/0.34  # Scheduled 5 strats onto 1 cores with 300 seconds (300 total)
% 0.11/0.34  # Starting SAT001_MinMin_p005000_rr_RG with 181s (1) cores
% 0.11/0.34  # SAT001_MinMin_p005000_rr_RG with pid 6212 completed with status 0
% 0.11/0.34  # Result found by SAT001_MinMin_p005000_rr_RG
% 0.11/0.34  # Preprocessing class: FSSSSLSSSSSNFFN.
% 0.11/0.34  # Scheduled 4 strats onto 8 cores with 300 seconds (2400 total)
% 0.11/0.34  # Starting SAT001_MinMin_p005000_rr_RG with 1500s (5) cores
% 0.11/0.34  # Starting new_bool_3 with 300s (1) cores
% 0.11/0.34  # Starting new_bool_1 with 300s (1) cores
% 0.11/0.34  # Starting sh5l with 300s (1) cores
% 0.11/0.34  # SinE strategy is gf500_gu_R04_F100_L20000
% 0.11/0.34  # Search class: FUUNF-FFSF00-SFFFFFNN
% 0.11/0.34  # Scheduled 5 strats onto 1 cores with 300 seconds (300 total)
% 0.11/0.34  # Starting SAT001_MinMin_p005000_rr_RG with 181s (1) cores
% 0.11/0.34  # Preprocessing time       : 0.001 s
% 0.11/0.34  # Presaturation interreduction done
% 0.11/0.34  
% 0.11/0.34  # Proof found!
% 0.11/0.34  # SZS status Theorem
% 0.11/0.34  # SZS output start CNFRefutation
% See solution above
% 0.11/0.34  # Parsed axioms                        : 1
% 0.11/0.34  # Removed by relevancy pruning/SinE    : 0
% 0.11/0.34  # Initial clauses                      : 4
% 0.11/0.34  # Removed in clause preprocessing      : 0
% 0.11/0.34  # Initial clauses in saturation        : 4
% 0.11/0.34  # Processed clauses                    : 2
% 0.11/0.34  # ...of these trivial                  : 0
% 0.11/0.34  # ...subsumed                          : 0
% 0.11/0.34  # ...remaining for further processing  : 1
% 0.11/0.34  # Other redundant clauses eliminated   : 0
% 0.11/0.34  # Clauses deleted for lack of memory   : 0
% 0.11/0.34  # Backward-subsumed                    : 0
% 0.11/0.34  # Backward-rewritten                   : 0
% 0.11/0.34  # Generated clauses                    : 0
% 0.11/0.34  # ...of the previous two non-redundant : 0
% 0.11/0.34  # ...aggressively subsumed             : 0
% 0.11/0.34  # Contextual simplify-reflections      : 0
% 0.11/0.34  # Paramodulations                      : 0
% 0.11/0.34  # Factorizations                       : 0
% 0.11/0.34  # NegExts                              : 0
% 0.11/0.34  # Equation resolutions                 : 0
% 0.11/0.34  # Disequality decompositions           : 0
% 0.11/0.34  # Total rewrite steps                  : 1
% 0.11/0.34  # ...of those cached                   : 0
% 0.11/0.34  # Propositional unsat checks           : 0
% 0.11/0.34  #    Propositional check models        : 0
% 0.11/0.34  #    Propositional check unsatisfiable : 0
% 0.11/0.34  #    Propositional clauses             : 0
% 0.11/0.34  #    Propositional clauses after purity: 0
% 0.11/0.34  #    Propositional unsat core size     : 0
% 0.11/0.34  #    Propositional preprocessing time  : 0.000
% 0.11/0.34  #    Propositional encoding time       : 0.000
% 0.11/0.34  #    Propositional solver time         : 0.000
% 0.11/0.34  #    Success case prop preproc time    : 0.000
% 0.11/0.34  #    Success case prop encoding time   : 0.000
% 0.11/0.34  #    Success case prop solver time     : 0.000
% 0.11/0.34  # Current number of processed clauses  : 1
% 0.11/0.34  #    Positive orientable unit clauses  : 1
% 0.11/0.34  #    Positive unorientable unit clauses: 0
% 0.11/0.34  #    Negative unit clauses             : 0
% 0.11/0.34  #    Non-unit-clauses                  : 0
% 0.11/0.34  # Current number of unprocessed clauses: 2
% 0.11/0.34  # ...number of literals in the above   : 2
% 0.11/0.34  # Current number of archived formulas  : 0
% 0.11/0.34  # Current number of archived clauses   : 0
% 0.11/0.34  # Clause-clause subsumption calls (NU) : 0
% 0.11/0.34  # Rec. Clause-clause subsumption calls : 0
% 0.11/0.34  # Non-unit clause-clause subsumptions  : 0
% 0.11/0.34  # Unit Clause-clause subsumption calls : 0
% 0.11/0.34  # Rewrite failures with RHS unbound    : 0
% 0.11/0.34  # BW rewrite match attempts            : 0
% 0.11/0.34  # BW rewrite match successes           : 0
% 0.11/0.34  # Condensation attempts                : 0
% 0.11/0.34  # Condensation successes               : 0
% 0.11/0.34  # Termbank termtop insertions          : 100
% 0.11/0.34  # Search garbage collected termcells   : 20
% 0.11/0.34  
% 0.11/0.34  # -------------------------------------------------
% 0.11/0.34  # User time                : 0.002 s
% 0.11/0.34  # System time              : 0.000 s
% 0.11/0.34  # Total time               : 0.002 s
% 0.11/0.34  # Maximum resident set size: 1632 pages
% 0.11/0.34  
% 0.11/0.34  # -------------------------------------------------
% 0.11/0.34  # User time                : 0.002 s
% 0.11/0.34  # System time              : 0.002 s
% 0.11/0.34  # Total time               : 0.004 s
% 0.11/0.34  # Maximum resident set size: 1680 pages
% 0.11/0.34  % E exiting
% 0.11/0.34  % E exiting
%------------------------------------------------------------------------------

