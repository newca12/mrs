% Proof : Problems/SYN044+1.p
%------------------------------------------------------------------------------
% File     : E---3.3.0
% Problem  : SYN044+1 : TPTP v9.2.0. Released v2.0.0.
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
% DateTime : Mon Sep 29 11:23:24 PM UTC 2025

% Result   : Theorem 0.19s 0.49s
% Output   : CNFRefutation 0.19s
% Verified : 
% SZS Type : Refutation
%            Derivation depth      :    7
%            Number of leaves      :    4
% Syntax   : Number of formulae    :   19 (   4 unt;   0 def)
%            Number of atoms       :   42 (   0 equ)
%            Maximal formula atoms :    4 (   2 avg)
%            Number of connectives :   37 (  14   ~;  15   |;   3   &)
%                                         (   2 <=>;   3  =>;   0  <=;   0 <~>)
%            Maximal formula depth :    4 (   3 avg)
%            Maximal term depth    :    0 (   0 avg)
%            Number of predicates  :    4 (   3 usr;   4 prp; 0-0 aty)
%            Number of functors    :    0 (   0 usr;   0 con; --- aty)
%            Number of variables   :    0 (   0 sgn   0   !;   0   ?)

% Comments : 
%------------------------------------------------------------------------------
fof(pel10,conjecture,
    ( p
  <=> q ),
    file('/export/starexec/sandbox/benchmark/theBenchmark.p',pel10) ).

fof(pel10_3,axiom,
    ( p
   => ( q
      | r ) ),
    file('/export/starexec/sandbox/benchmark/theBenchmark.p',pel10_3) ).

fof(pel10_1,axiom,
    ( q
   => r ),
    file('/export/starexec/sandbox/benchmark/theBenchmark.p',pel10_1) ).

fof(pel10_2,axiom,
    ( r
   => ( p
      & q ) ),
    file('/export/starexec/sandbox/benchmark/theBenchmark.p',pel10_2) ).

fof(c_0_4,negated_conjecture,
    ~ ( p
    <=> q ),
    inference(assume_negation,[status(cth)],[pel10]) ).

fof(c_0_5,plain,
    ( ~ p
    | q
    | r ),
    inference(fof_nnf,[status(thm)],[inference(fof_nnf,[status(thm)],[pel10_3])]) ).

fof(c_0_6,negated_conjecture,
    ( ( ~ p
      | ~ q )
    & ( p
      | q ) ),
    inference(fof_nnf,[status(thm)],[inference(fof_nnf,[status(thm)],[c_0_4])]) ).

fof(c_0_7,plain,
    ( ~ q
    | r ),
    inference(fof_nnf,[status(thm)],[inference(fof_nnf,[status(thm)],[pel10_1])]) ).

fof(c_0_8,plain,
    ( ( p
      | ~ r )
    & ( q
      | ~ r ) ),
    inference(distribute,[status(thm)],[inference(fof_nnf,[status(thm)],[inference(fof_nnf,[status(thm)],[pel10_2])])]) ).

fof(c_0_9,plain,
    ( q
    | r
    | ~ p ),
    inference(split_conjunct,[status(thm)],[c_0_5]) ).

fof(c_0_10,negated_conjecture,
    ( p
    | q ),
    inference(split_conjunct,[status(thm)],[c_0_6]) ).

fof(c_0_11,plain,
    ( r
    | ~ q ),
    inference(split_conjunct,[status(thm)],[c_0_7]) ).

fof(c_0_12,plain,
    ( p
    | ~ r ),
    inference(split_conjunct,[status(thm)],[c_0_8]) ).

fof(c_0_13,plain,
    r,
    inference(csr,[status(thm)],[inference(csr,[status(thm)],[c_0_9,c_0_10]),c_0_11]) ).

fof(c_0_14,negated_conjecture,
    ( ~ p
    | ~ q ),
    inference(split_conjunct,[status(thm)],[c_0_6]) ).

fof(c_0_15,plain,
    p,
    inference(cn,[status(thm)],[inference(rw,[status(thm)],[c_0_12,c_0_13])]) ).

fof(c_0_16,plain,
    ( q
    | ~ r ),
    inference(split_conjunct,[status(thm)],[c_0_8]) ).

fof(c_0_17,negated_conjecture,
    ~ q,
    inference(cn,[status(thm)],[inference(rw,[status(thm)],[c_0_14,c_0_15])]) ).

fof(c_0_18,plain,
    $false,
    inference(sr,[status(thm)],[inference(cn,[status(thm)],[inference(rw,[status(thm)],[c_0_16,c_0_13])]),c_0_17]),
    [proof] ).

%------------------------------------------------------------------------------
%----ORIGINAL SYSTEM OUTPUT
% 0.07/0.12  % Problem    : SYN044+1 : TPTP v9.2.0. Released v2.0.0.
% 0.07/0.12  % Command    : run_E /export/starexec/sandbox/benchmark/theBenchmark.p 300 THM
% 0.11/0.33  % Computer : n017.cluster.edu
% 0.11/0.33  % Model    : x86_64 x86_64
% 0.11/0.33  % CPU      : Intel(R) Xeon(R) CPU E5-2620 v4 @ 2.10GHz
% 0.11/0.33  % Memory   : 8042.1875MB
% 0.11/0.33  % OS       : Linux 3.10.0-693.el7.x86_64
% 0.11/0.33  % CPULimit   : 300
% 0.11/0.33  % WCLimit    : 300
% 0.11/0.33  % DateTime   : Fri Sep 26 15:06:08 EDT 2025
% 0.11/0.33  % CPUTime    : 
% 0.19/0.48  Running first-order theorem proving
% 0.19/0.48  Running: /export/starexec/sandbox/solver/bin/eprover --delete-bad-limit=2000000000 --definitional-cnf=24 -s --print-statistics -R --print-version --proof-object --auto-schedule=8 --cpu-limit=300 /export/starexec/sandbox/benchmark/theBenchmark.p
% 0.19/0.49  # Version: 3.0.0
% 0.19/0.49  # Preprocessing class: FSSSSLSSSSSNFFN.
% 0.19/0.49  # Scheduled 4 strats onto 8 cores with 300 seconds (2400 total)
% 0.19/0.49  # Starting SAT001_MinMin_p005000_rr_RG with 1500s (5) cores
% 0.19/0.49  # Starting new_bool_3 with 300s (1) cores
% 0.19/0.49  # Starting new_bool_1 with 300s (1) cores
% 0.19/0.49  # Starting sh5l with 300s (1) cores
% 0.19/0.49  # SAT001_MinMin_p005000_rr_RG with pid 16320 completed with status 0
% 0.19/0.49  # Result found by SAT001_MinMin_p005000_rr_RG
% 0.19/0.49  # Preprocessing class: FSSSSLSSSSSNFFN.
% 0.19/0.49  # Scheduled 4 strats onto 8 cores with 300 seconds (2400 total)
% 0.19/0.49  # Starting SAT001_MinMin_p005000_rr_RG with 1500s (5) cores
% 0.19/0.49  # No SInE strategy applied
% 0.19/0.49  # Search class: FGHNF-FFSF00-SFFFFFNN
% 0.19/0.49  # Scheduled 5 strats onto 5 cores with 1500 seconds (1500 total)
% 0.19/0.49  # Starting SAT001_MinMin_p005000_rr_RG with 901s (1) cores
% 0.19/0.49  # Starting G-E--_208_C18_F1_SE_CS_SP_PS_S5PRR_S0Y with 151s (1) cores
% 0.19/0.49  # Starting new_bool_3 with 151s (1) cores
% 0.19/0.49  # Starting new_bool_1 with 151s (1) cores
% 0.19/0.49  # Starting sh5l with 146s (1) cores
% 0.19/0.49  # SAT001_MinMin_p005000_rr_RG with pid 16325 completed with status 0
% 0.19/0.49  # Result found by SAT001_MinMin_p005000_rr_RG
% 0.19/0.49  # Preprocessing class: FSSSSLSSSSSNFFN.
% 0.19/0.49  # Scheduled 4 strats onto 8 cores with 300 seconds (2400 total)
% 0.19/0.49  # Starting SAT001_MinMin_p005000_rr_RG with 1500s (5) cores
% 0.19/0.49  # No SInE strategy applied
% 0.19/0.49  # Search class: FGHNF-FFSF00-SFFFFFNN
% 0.19/0.49  # Scheduled 5 strats onto 5 cores with 1500 seconds (1500 total)
% 0.19/0.49  # Starting SAT001_MinMin_p005000_rr_RG with 901s (1) cores
% 0.19/0.49  # Preprocessing time       : 0.001 s
% 0.19/0.49  # Presaturation interreduction done
% 0.19/0.49  
% 0.19/0.49  # Proof found!
% 0.19/0.49  # SZS status Theorem
% 0.19/0.49  # SZS output start CNFRefutation
% See solution above
% 0.19/0.49  # Parsed axioms                        : 4
% 0.19/0.49  # Removed by relevancy pruning/SinE    : 0
% 0.19/0.49  # Initial clauses                      : 6
% 0.19/0.49  # Removed in clause preprocessing      : 0
% 0.19/0.49  # Initial clauses in saturation        : 6
% 0.19/0.49  # Processed clauses                    : 9
% 0.19/0.49  # ...of these trivial                  : 0
% 0.19/0.49  # ...subsumed                          : 0
% 0.19/0.49  # ...remaining for further processing  : 8
% 0.19/0.49  # Other redundant clauses eliminated   : 0
% 0.19/0.49  # Clauses deleted for lack of memory   : 0
% 0.19/0.49  # Backward-subsumed                    : 2
% 0.19/0.49  # Backward-rewritten                   : 3
% 0.19/0.49  # Generated clauses                    : 0
% 0.19/0.49  # ...of the previous two non-redundant : 3
% 0.19/0.49  # ...aggressively subsumed             : 0
% 0.19/0.49  # Contextual simplify-reflections      : 2
% 0.19/0.49  # Paramodulations                      : 0
% 0.19/0.49  # Factorizations                       : 0
% 0.19/0.49  # NegExts                              : 0
% 0.19/0.49  # Equation resolutions                 : 0
% 0.19/0.49  # Disequality decompositions           : 0
% 0.19/0.49  # Total rewrite steps                  : 3
% 0.19/0.49  # ...of those cached                   : 1
% 0.19/0.49  # Propositional unsat checks           : 0
% 0.19/0.49  #    Propositional check models        : 0
% 0.19/0.49  #    Propositional check unsatisfiable : 0
% 0.19/0.49  #    Propositional clauses             : 0
% 0.19/0.49  #    Propositional clauses after purity: 0
% 0.19/0.49  #    Propositional unsat core size     : 0
% 0.19/0.49  #    Propositional preprocessing time  : 0.000
% 0.19/0.49  #    Propositional encoding time       : 0.000
% 0.19/0.49  #    Propositional solver time         : 0.000
% 0.19/0.49  #    Success case prop preproc time    : 0.000
% 0.19/0.49  #    Success case prop encoding time   : 0.000
% 0.19/0.49  #    Success case prop solver time     : 0.000
% 0.19/0.49  # Current number of processed clauses  : 3
% 0.19/0.49  #    Positive orientable unit clauses  : 2
% 0.19/0.49  #    Positive unorientable unit clauses: 0
% 0.19/0.49  #    Negative unit clauses             : 1
% 0.19/0.49  #    Non-unit-clauses                  : 0
% 0.19/0.49  # Current number of unprocessed clauses: 0
% 0.19/0.49  # ...number of literals in the above   : 0
% 0.19/0.49  # Current number of archived formulas  : 0
% 0.19/0.49  # Current number of archived clauses   : 5
% 0.19/0.49  # Clause-clause subsumption calls (NU) : 2
% 0.19/0.49  # Rec. Clause-clause subsumption calls : 2
% 0.19/0.49  # Non-unit clause-clause subsumptions  : 2
% 0.19/0.49  # Unit Clause-clause subsumption calls : 2
% 0.19/0.49  # Rewrite failures with RHS unbound    : 0
% 0.19/0.49  # BW rewrite match attempts            : 2
% 0.19/0.49  # BW rewrite match successes           : 2
% 0.19/0.49  # Condensation attempts                : 0
% 0.19/0.49  # Condensation successes               : 0
% 0.19/0.49  # Termbank termtop insertions          : 215
% 0.19/0.49  # Search garbage collected termcells   : 21
% 0.19/0.49  
% 0.19/0.49  # -------------------------------------------------
% 0.19/0.49  # User time                : 0.003 s
% 0.19/0.49  # System time              : 0.001 s
% 0.19/0.49  # Total time               : 0.004 s
% 0.19/0.49  # Maximum resident set size: 1648 pages
% 0.19/0.49  
% 0.19/0.49  # -------------------------------------------------
% 0.19/0.49  # User time                : 0.009 s
% 0.19/0.49  # System time              : 0.005 s
% 0.19/0.49  # Total time               : 0.014 s
% 0.19/0.49  # Maximum resident set size: 1680 pages
% 0.19/0.49  % E exiting
% 0.19/0.49  % E exiting
%------------------------------------------------------------------------------

