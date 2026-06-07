% Proof : Problems/SYN047+1.p
%------------------------------------------------------------------------------
% File     : E---3.3.0
% Problem  : SYN047+1 : TPTP v9.2.0. Released v2.0.0.
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
% DateTime : Mon Sep 29 11:23:46 PM UTC 2025

% Result   : Theorem 0.21s 0.50s
% Output   : CNFRefutation 0.21s
% Verified : 
% SZS Type : Refutation
%            Derivation depth      :    6
%            Number of leaves      :    1
% Syntax   : Number of formulae    :   16 (   5 unt;   0 def)
%            Number of atoms       :  169 (   0 equ)
%            Maximal formula atoms :  114 (  10 avg)
%            Number of connectives :  234 (  81   ~; 113   |;  34   &)
%                                         (   2 <=>;   4  =>;   0  <=;   0 <~>)
%            Maximal formula depth :   37 (   6 avg)
%            Maximal term depth    :    0 (   0 avg)
%            Number of predicates  :    5 (   4 usr;   5 prp; 0-0 aty)
%            Number of functors    :    0 (   0 usr;   0 con; --- aty)
%            Number of variables   :    0 (   0 sgn   0   !;   0   ?)

% Comments : 
%------------------------------------------------------------------------------
fof(pel17,conjecture,
    ( ( ( p
        & ( q
         => r ) )
     => s )
  <=> ( ( ~ p
        | q
        | s )
      & ( ~ p
        | ~ r
        | s ) ) ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',pel17) ).

fof(c_0_1,negated_conjecture,
    ~ ( ( ( p
          & ( q
           => r ) )
       => s )
    <=> ( ( ~ p
          | q
          | s )
        & ( ~ p
          | ~ r
          | s ) ) ),
    inference(fof_simplification,[status(thm)],[inference(assume_negation,[status(cth)],[pel17])]) ).

fof(c_0_2,negated_conjecture,
    ( ( p
      | p
      | p )
    & ( r
      | p
      | p )
    & ( ~ s
      | p
      | p )
    & ( p
      | ~ q
      | p )
    & ( r
      | ~ q
      | p )
    & ( ~ s
      | ~ q
      | p )
    & ( p
      | ~ s
      | p )
    & ( r
      | ~ s
      | p )
    & ( ~ s
      | ~ s
      | p )
    & ( p
      | p
      | ~ q
      | r )
    & ( r
      | p
      | ~ q
      | r )
    & ( ~ s
      | p
      | ~ q
      | r )
    & ( p
      | ~ q
      | ~ q
      | r )
    & ( r
      | ~ q
      | ~ q
      | r )
    & ( ~ s
      | ~ q
      | ~ q
      | r )
    & ( p
      | ~ s
      | ~ q
      | r )
    & ( r
      | ~ s
      | ~ q
      | r )
    & ( ~ s
      | ~ s
      | ~ q
      | r )
    & ( p
      | p
      | ~ s )
    & ( r
      | p
      | ~ s )
    & ( ~ s
      | p
      | ~ s )
    & ( p
      | ~ q
      | ~ s )
    & ( r
      | ~ q
      | ~ s )
    & ( ~ s
      | ~ q
      | ~ s )
    & ( p
      | ~ s
      | ~ s )
    & ( r
      | ~ s
      | ~ s )
    & ( ~ s
      | ~ s
      | ~ s )
    & ( ~ p
      | q
      | s
      | q
      | ~ p
      | s )
    & ( ~ p
      | ~ r
      | s
      | q
      | ~ p
      | s )
    & ( ~ p
      | q
      | s
      | ~ r
      | ~ p
      | s )
    & ( ~ p
      | ~ r
      | s
      | ~ r
      | ~ p
      | s ) ),
    inference(distribute,[status(thm)],[inference(fof_nnf,[status(thm)],[inference(fof_nnf,[status(thm)],[c_0_1])])]) ).

fof(c_0_3,negated_conjecture,
    ( q
    | s
    | q
    | s
    | ~ p
    | ~ p ),
    inference(split_conjunct,[status(thm)],[c_0_2]) ).

fof(c_0_4,negated_conjecture,
    ( p
    | p
    | p ),
    inference(split_conjunct,[status(thm)],[c_0_2]) ).

fof(c_0_5,negated_conjecture,
    ( ~ s
    | ~ s
    | ~ s ),
    inference(split_conjunct,[status(thm)],[c_0_2]) ).

fof(c_0_6,negated_conjecture,
    ( s
    | s
    | ~ p
    | ~ r
    | ~ r
    | ~ p ),
    inference(split_conjunct,[status(thm)],[c_0_2]) ).

fof(c_0_7,negated_conjecture,
    ( r
    | r
    | ~ q
    | ~ q ),
    inference(split_conjunct,[status(thm)],[c_0_2]) ).

fof(c_0_8,negated_conjecture,
    ( q
    | s
    | ~ p ),
    inference(cn,[status(thm)],[c_0_3]) ).

fof(c_0_9,negated_conjecture,
    p,
    inference(cn,[status(thm)],[c_0_4]) ).

fof(c_0_10,negated_conjecture,
    ~ s,
    inference(cn,[status(thm)],[c_0_5]) ).

fof(c_0_11,negated_conjecture,
    ( s
    | ~ p
    | ~ r ),
    inference(cn,[status(thm)],[c_0_6]) ).

fof(c_0_12,negated_conjecture,
    ( r
    | ~ q ),
    inference(cn,[status(thm)],[c_0_7]) ).

fof(c_0_13,negated_conjecture,
    q,
    inference(sr,[status(thm)],[inference(cn,[status(thm)],[inference(rw,[status(thm)],[c_0_8,c_0_9])]),c_0_10]) ).

fof(c_0_14,negated_conjecture,
    ~ r,
    inference(sr,[status(thm)],[inference(cn,[status(thm)],[inference(rw,[status(thm)],[c_0_11,c_0_9])]),c_0_10]) ).

fof(c_0_15,negated_conjecture,
    $false,
    inference(sr,[status(thm)],[inference(cn,[status(thm)],[inference(rw,[status(thm)],[c_0_12,c_0_13])]),c_0_14]),
    [proof] ).

%------------------------------------------------------------------------------
%----ORIGINAL SYSTEM OUTPUT
% 0.03/0.12  % Problem    : SYN047+1 : TPTP v9.2.0. Released v2.0.0.
% 0.03/0.12  % Command    : run_E /export/starexec/sandbox2/benchmark/theBenchmark.p 300 THM
% 0.12/0.33  % Computer : n013.cluster.edu
% 0.12/0.33  % Model    : x86_64 x86_64
% 0.12/0.33  % CPU      : Intel(R) Xeon(R) CPU E5-2620 v4 @ 2.10GHz
% 0.12/0.33  % Memory   : 8042.1875MB
% 0.12/0.33  % OS       : Linux 3.10.0-693.el7.x86_64
% 0.12/0.33  % CPULimit   : 300
% 0.12/0.33  % WCLimit    : 300
% 0.12/0.33  % DateTime   : Fri Sep 26 14:56:08 EDT 2025
% 0.12/0.34  % CPUTime    : 
% 0.21/0.48  Running first-order theorem proving
% 0.21/0.48  Running: /export/starexec/sandbox2/solver/bin/eprover --delete-bad-limit=2000000000 --definitional-cnf=24 -s --print-statistics -R --print-version --proof-object --auto-schedule=8 --cpu-limit=300 /export/starexec/sandbox2/benchmark/theBenchmark.p
% 0.21/0.50  # Version: 3.0.0
% 0.21/0.50  # Preprocessing class: FSSSSLSSSSSNFFN.
% 0.21/0.50  # Scheduled 4 strats onto 8 cores with 300 seconds (2400 total)
% 0.21/0.50  # Starting SAT001_MinMin_p005000_rr_RG with 1500s (5) cores
% 0.21/0.50  # Starting new_bool_3 with 300s (1) cores
% 0.21/0.50  # Starting new_bool_1 with 300s (1) cores
% 0.21/0.50  # Starting sh5l with 300s (1) cores
% 0.21/0.50  # new_bool_3 with pid 14408 completed with status 0
% 0.21/0.50  # Result found by new_bool_3
% 0.21/0.50  # Preprocessing class: FSSSSLSSSSSNFFN.
% 0.21/0.50  # Scheduled 4 strats onto 8 cores with 300 seconds (2400 total)
% 0.21/0.50  # Starting SAT001_MinMin_p005000_rr_RG with 1500s (5) cores
% 0.21/0.50  # Starting new_bool_3 with 300s (1) cores
% 0.21/0.50  # SinE strategy is GSinE(CountFormulas,hypos,1.5,,3,20000,1.0)
% 0.21/0.50  # Search class: FGHNF-FFSF00-SFFFFFNN
% 0.21/0.50  # Scheduled 5 strats onto 1 cores with 300 seconds (300 total)
% 0.21/0.50  # Starting SAT001_MinMin_p005000_rr_RG with 181s (1) cores
% 0.21/0.50  # SAT001_MinMin_p005000_rr_RG with pid 14414 completed with status 0
% 0.21/0.50  # Result found by SAT001_MinMin_p005000_rr_RG
% 0.21/0.50  # Preprocessing class: FSSSSLSSSSSNFFN.
% 0.21/0.50  # Scheduled 4 strats onto 8 cores with 300 seconds (2400 total)
% 0.21/0.50  # Starting SAT001_MinMin_p005000_rr_RG with 1500s (5) cores
% 0.21/0.50  # Starting new_bool_3 with 300s (1) cores
% 0.21/0.50  # SinE strategy is GSinE(CountFormulas,hypos,1.5,,3,20000,1.0)
% 0.21/0.50  # Search class: FGHNF-FFSF00-SFFFFFNN
% 0.21/0.50  # Scheduled 5 strats onto 1 cores with 300 seconds (300 total)
% 0.21/0.50  # Starting SAT001_MinMin_p005000_rr_RG with 181s (1) cores
% 0.21/0.50  # Preprocessing time       : 0.001 s
% 0.21/0.50  # Presaturation interreduction done
% 0.21/0.50  
% 0.21/0.50  # Proof found!
% 0.21/0.50  # SZS status Theorem
% 0.21/0.50  # SZS output start CNFRefutation
% See solution above
% 0.21/0.50  # Parsed axioms                        : 1
% 0.21/0.50  # Removed by relevancy pruning/SinE    : 0
% 0.21/0.50  # Initial clauses                      : 31
% 0.21/0.50  # Removed in clause preprocessing      : 0
% 0.21/0.50  # Initial clauses in saturation        : 31
% 0.21/0.50  # Processed clauses                    : 27
% 0.21/0.50  # ...of these trivial                  : 16
% 0.21/0.50  # ...subsumed                          : 6
% 0.21/0.50  # ...remaining for further processing  : 5
% 0.21/0.50  # Other redundant clauses eliminated   : 0
% 0.21/0.50  # Clauses deleted for lack of memory   : 0
% 0.21/0.50  # Backward-subsumed                    : 0
% 0.21/0.50  # Backward-rewritten                   : 1
% 0.21/0.50  # Generated clauses                    : 0
% 0.21/0.50  # ...of the previous two non-redundant : 0
% 0.21/0.50  # ...aggressively subsumed             : 0
% 0.21/0.50  # Contextual simplify-reflections      : 0
% 0.21/0.50  # Paramodulations                      : 0
% 0.21/0.50  # Factorizations                       : 0
% 0.21/0.50  # NegExts                              : 0
% 0.21/0.50  # Equation resolutions                 : 0
% 0.21/0.50  # Disequality decompositions           : 0
% 0.21/0.50  # Total rewrite steps                  : 19
% 0.21/0.50  # ...of those cached                   : 17
% 0.21/0.50  # Propositional unsat checks           : 0
% 0.21/0.50  #    Propositional check models        : 0
% 0.21/0.50  #    Propositional check unsatisfiable : 0
% 0.21/0.50  #    Propositional clauses             : 0
% 0.21/0.50  #    Propositional clauses after purity: 0
% 0.21/0.50  #    Propositional unsat core size     : 0
% 0.21/0.50  #    Propositional preprocessing time  : 0.000
% 0.21/0.50  #    Propositional encoding time       : 0.000
% 0.21/0.50  #    Propositional solver time         : 0.000
% 0.21/0.50  #    Success case prop preproc time    : 0.000
% 0.21/0.50  #    Success case prop encoding time   : 0.000
% 0.21/0.50  #    Success case prop solver time     : 0.000
% 0.21/0.50  # Current number of processed clauses  : 4
% 0.21/0.50  #    Positive orientable unit clauses  : 2
% 0.21/0.50  #    Positive unorientable unit clauses: 0
% 0.21/0.50  #    Negative unit clauses             : 2
% 0.21/0.50  #    Non-unit-clauses                  : 0
% 0.21/0.50  # Current number of unprocessed clauses: 4
% 0.21/0.50  # ...number of literals in the above   : 16
% 0.21/0.50  # Current number of archived formulas  : 0
% 0.21/0.50  # Current number of archived clauses   : 1
% 0.21/0.50  # Clause-clause subsumption calls (NU) : 0
% 0.21/0.50  # Rec. Clause-clause subsumption calls : 0
% 0.21/0.50  # Non-unit clause-clause subsumptions  : 0
% 0.21/0.50  # Unit Clause-clause subsumption calls : 0
% 0.21/0.50  # Rewrite failures with RHS unbound    : 0
% 0.21/0.50  # BW rewrite match attempts            : 1
% 0.21/0.50  # BW rewrite match successes           : 1
% 0.21/0.50  # Condensation attempts                : 0
% 0.21/0.50  # Condensation successes               : 0
% 0.21/0.50  # Termbank termtop insertions          : 945
% 0.21/0.50  # Search garbage collected termcells   : 151
% 0.21/0.50  
% 0.21/0.50  # -------------------------------------------------
% 0.21/0.50  # User time                : 0.004 s
% 0.21/0.50  # System time              : 0.001 s
% 0.21/0.50  # Total time               : 0.005 s
% 0.21/0.50  # Maximum resident set size: 1760 pages
% 0.21/0.50  
% 0.21/0.50  # -------------------------------------------------
% 0.21/0.50  # User time                : 0.004 s
% 0.21/0.50  # System time              : 0.004 s
% 0.21/0.50  # Total time               : 0.008 s
% 0.21/0.50  # Maximum resident set size: 1680 pages
% 0.21/0.50  % E exiting
% 0.21/0.50  % E exiting
%------------------------------------------------------------------------------

