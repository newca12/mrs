% Proof : Problems/SYN333+1.p
%------------------------------------------------------------------------------
% File     : E---3.3.0
% Problem  : SYN333+1 : TPTP v9.2.0. Released v2.0.0.
% Transfm  : none
% Format   : tptp:raw
% Command  : run_E /export/starexec/sandbox2/benchmark/theBenchmark.p 300 THM

% Computer : n014.cluster.edu
% Model    : x86_64 x86_64
% CPU      : Intel(R) Xeon(R) CPU E5-2620 v4 2.10GHz
% Memory   : 8042.1875MB
% OS       : Linux 3.10.0-693.el7.x86_64
% CPULimit : 300s
% WCLimit  : 300s
% DateTime : Mon Sep 29 11:24:54 PM UTC 2025

% Result   : Theorem 0.16s 0.44s
% Output   : CNFRefutation 0.16s
% Verified : 
% SZS Type : Refutation
%            Derivation depth      :    5
%            Number of leaves      :    1
% Syntax   : Number of formulae    :    8 (   3 unt;   0 def)
%            Number of atoms       :   35 (   0 equ)
%            Maximal formula atoms :   11 (   4 avg)
%            Number of connectives :   42 (  15   ~;  12   |;  11   &)
%                                         (   0 <=>;   4  =>;   0  <=;   0 <~>)
%            Maximal formula depth :   12 (   5 avg)
%            Maximal term depth    :    2 (   1 avg)
%            Number of predicates  :    3 (   2 usr;   1 prp; 0-2 aty)
%            Number of functors    :    4 (   4 usr;   0 con; 2-2 aty)
%            Number of variables   :   18 (   4 sgn   6   !;   4   ?)

% Comments : 
%------------------------------------------------------------------------------
fof(church_46_14_5,conjecture,
    ? [X1,X2] :
    ! [X3] :
      ( big_f(X1,X2)
     => ( big_f(X2,X3)
        & big_f(X3,X3)
        & ( ( big_f(X1,X2)
            & big_g(X1,X2) )
         => ( big_g(X1,X3)
            & big_g(X3,X3) ) ) ) ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',church_46_14_5) ).

fof(c_0_1,negated_conjecture,
    ~ ? [X1,X2] :
      ! [X3] :
        ( big_f(X1,X2)
       => ( big_f(X2,X3)
          & big_f(X3,X3)
          & ( ( big_f(X1,X2)
              & big_g(X1,X2) )
           => ( big_g(X1,X3)
              & big_g(X3,X3) ) ) ) ),
    inference(assume_negation,[status(cth)],[church_46_14_5]) ).

fof(c_0_2,negated_conjecture,
    ! [X4,X5,X6,X7] :
      ( big_f(X4,X5)
      & ( big_f(X6,X7)
        | ~ big_f(X7,esk1_2(X6,X7))
        | ~ big_f(esk2_2(X6,X7),esk2_2(X6,X7)) )
      & ( big_g(X6,X7)
        | ~ big_f(X7,esk1_2(X6,X7))
        | ~ big_f(esk2_2(X6,X7),esk2_2(X6,X7)) )
      & ( ~ big_g(X6,esk3_2(X6,X7))
        | ~ big_g(esk4_2(X6,X7),esk4_2(X6,X7))
        | ~ big_f(X7,esk1_2(X6,X7))
        | ~ big_f(esk2_2(X6,X7),esk2_2(X6,X7)) ) ),
    inference(distribute,[status(thm)],[inference(fof_nnf,[status(thm)],[inference(shift_quantors,[status(thm)],[inference(skolemize,[status(esa)],[inference(variable_rename,[status(thm)],[inference(shift_quantors,[status(thm)],[inference(fof_nnf,[status(thm)],[c_0_1])])])])])])]) ).

fof(c_0_3,negated_conjecture,
    ( big_g(X1,X2)
    | ~ big_f(X2,esk1_2(X1,X2))
    | ~ big_f(esk2_2(X1,X2),esk2_2(X1,X2)) ),
    inference(split_conjunct,[status(thm)],[c_0_2]) ).

fof(c_0_4,negated_conjecture,
    big_f(X1,X2),
    inference(split_conjunct,[status(thm)],[c_0_2]) ).

fof(c_0_5,negated_conjecture,
    ( ~ big_g(X1,esk3_2(X1,X2))
    | ~ big_g(esk4_2(X1,X2),esk4_2(X1,X2))
    | ~ big_f(X2,esk1_2(X1,X2))
    | ~ big_f(esk2_2(X1,X2),esk2_2(X1,X2)) ),
    inference(split_conjunct,[status(thm)],[c_0_2]) ).

fof(c_0_6,negated_conjecture,
    big_g(X1,X2),
    inference(cn,[status(thm)],[inference(rw,[status(thm)],[inference(rw,[status(thm)],[c_0_3,c_0_4]),c_0_4])]) ).

fof(c_0_7,negated_conjecture,
    $false,
    inference(cn,[status(thm)],[inference(rw,[status(thm)],[inference(rw,[status(thm)],[inference(cn,[status(thm)],[inference(rw,[status(thm)],[inference(rw,[status(thm)],[c_0_5,c_0_4]),c_0_4])]),c_0_6]),c_0_6])]),
    [proof] ).

%------------------------------------------------------------------------------
%----ORIGINAL SYSTEM OUTPUT
% 0.00/0.10  % Problem    : SYN333+1 : TPTP v9.2.0. Released v2.0.0.
% 0.00/0.10  % Command    : run_E /export/starexec/sandbox2/benchmark/theBenchmark.p 300 THM
% 0.09/0.31  % Computer : n014.cluster.edu
% 0.09/0.31  % Model    : x86_64 x86_64
% 0.09/0.31  % CPU      : Intel(R) Xeon(R) CPU E5-2620 v4 @ 2.10GHz
% 0.09/0.31  % Memory   : 8042.1875MB
% 0.09/0.31  % OS       : Linux 3.10.0-693.el7.x86_64
% 0.09/0.31  % CPULimit   : 300
% 0.09/0.31  % WCLimit    : 300
% 0.09/0.31  % DateTime   : Fri Sep 26 14:39:08 EDT 2025
% 0.09/0.31  % CPUTime    : 
% 0.16/0.43  Running first-order theorem proving
% 0.16/0.43  Running: /export/starexec/sandbox2/solver/bin/eprover --delete-bad-limit=2000000000 --definitional-cnf=24 -s --print-statistics -R --print-version --proof-object --auto-schedule=8 --cpu-limit=300 /export/starexec/sandbox2/benchmark/theBenchmark.p
% 0.16/0.44  # Version: 3.0.0
% 0.16/0.44  # Preprocessing class: FSSSSMSSSSSNFFN.
% 0.16/0.44  # Scheduled 4 strats onto 8 cores with 300 seconds (2400 total)
% 0.16/0.44  # Starting G-E--_302_C18_F1_URBAN_RG_S04BN with 1500s (5) cores
% 0.16/0.44  # Starting new_bool_3 with 300s (1) cores
% 0.16/0.44  # Starting new_bool_1 with 300s (1) cores
% 0.16/0.44  # Starting sh5l with 300s (1) cores
% 0.16/0.44  # G-E--_302_C18_F1_URBAN_RG_S04BN with pid 28249 completed with status 0
% 0.16/0.44  # Result found by G-E--_302_C18_F1_URBAN_RG_S04BN
% 0.16/0.44  # Preprocessing class: FSSSSMSSSSSNFFN.
% 0.16/0.44  # Scheduled 4 strats onto 8 cores with 300 seconds (2400 total)
% 0.16/0.44  # Starting G-E--_302_C18_F1_URBAN_RG_S04BN with 1500s (5) cores
% 0.16/0.44  # No SInE strategy applied
% 0.16/0.44  # Search class: FUHPF-FFSF00-SFFFFFNN
% 0.16/0.44  # Scheduled 6 strats onto 5 cores with 1500 seconds (1500 total)
% 0.16/0.44  # Starting SAT001_MinMin_p005000_rr_RG with 811s (1) cores
% 0.16/0.44  # Starting G-E--_302_C18_F1_URBAN_RG_S04BN with 151s (1) cores
% 0.16/0.44  # Starting new_bool_3 with 136s (1) cores
% 0.16/0.44  # Starting new_bool_1 with 136s (1) cores
% 0.16/0.44  # Starting sh5l with 136s (1) cores
% 0.16/0.44  # SAT001_MinMin_p005000_rr_RG with pid 28253 completed with status 0
% 0.16/0.44  # Result found by SAT001_MinMin_p005000_rr_RG
% 0.16/0.44  # Preprocessing class: FSSSSMSSSSSNFFN.
% 0.16/0.44  # Scheduled 4 strats onto 8 cores with 300 seconds (2400 total)
% 0.16/0.44  # Starting G-E--_302_C18_F1_URBAN_RG_S04BN with 1500s (5) cores
% 0.16/0.44  # No SInE strategy applied
% 0.16/0.44  # Search class: FUHPF-FFSF00-SFFFFFNN
% 0.16/0.44  # Scheduled 6 strats onto 5 cores with 1500 seconds (1500 total)
% 0.16/0.44  # Starting SAT001_MinMin_p005000_rr_RG with 811s (1) cores
% 0.16/0.44  # Preprocessing time       : 0.001 s
% 0.16/0.44  # Presaturation interreduction done
% 0.16/0.44  
% 0.16/0.44  # Proof found!
% 0.16/0.44  # SZS status Theorem
% 0.16/0.44  # SZS output start CNFRefutation
% See solution above
% 0.16/0.44  # Parsed axioms                        : 1
% 0.16/0.44  # Removed by relevancy pruning/SinE    : 0
% 0.16/0.44  # Initial clauses                      : 4
% 0.16/0.44  # Removed in clause preprocessing      : 3
% 0.16/0.44  # Initial clauses in saturation        : 1
% 0.16/0.44  # Processed clauses                    : 1
% 0.16/0.44  # ...of these trivial                  : 0
% 0.16/0.44  # ...subsumed                          : 0
% 0.16/0.44  # ...remaining for further processing  : 0
% 0.16/0.44  # Other redundant clauses eliminated   : 0
% 0.16/0.44  # Clauses deleted for lack of memory   : 0
% 0.16/0.44  # Backward-subsumed                    : 0
% 0.16/0.44  # Backward-rewritten                   : 0
% 0.16/0.44  # Generated clauses                    : 0
% 0.16/0.44  # ...of the previous two non-redundant : 0
% 0.16/0.44  # ...aggressively subsumed             : 0
% 0.16/0.44  # Contextual simplify-reflections      : 0
% 0.16/0.44  # Paramodulations                      : 0
% 0.16/0.44  # Factorizations                       : 0
% 0.16/0.44  # NegExts                              : 0
% 0.16/0.44  # Equation resolutions                 : 0
% 0.16/0.44  # Disequality decompositions           : 0
% 0.16/0.44  # Total rewrite steps                  : 0
% 0.16/0.44  # ...of those cached                   : 0
% 0.16/0.44  # Propositional unsat checks           : 0
% 0.16/0.44  #    Propositional check models        : 0
% 0.16/0.44  #    Propositional check unsatisfiable : 0
% 0.16/0.44  #    Propositional clauses             : 0
% 0.16/0.44  #    Propositional clauses after purity: 0
% 0.16/0.44  #    Propositional unsat core size     : 0
% 0.16/0.44  #    Propositional preprocessing time  : 0.000
% 0.16/0.44  #    Propositional encoding time       : 0.000
% 0.16/0.44  #    Propositional solver time         : 0.000
% 0.16/0.44  #    Success case prop preproc time    : 0.000
% 0.16/0.44  #    Success case prop encoding time   : 0.000
% 0.16/0.44  #    Success case prop solver time     : 0.000
% 0.16/0.44  # Current number of processed clauses  : 0
% 0.16/0.44  #    Positive orientable unit clauses  : 0
% 0.16/0.44  #    Positive unorientable unit clauses: 0
% 0.16/0.44  #    Negative unit clauses             : 0
% 0.16/0.44  #    Non-unit-clauses                  : 0
% 0.16/0.44  # Current number of unprocessed clauses: 0
% 0.16/0.44  # ...number of literals in the above   : 0
% 0.16/0.44  # Current number of archived formulas  : 0
% 0.16/0.44  # Current number of archived clauses   : 2
% 0.16/0.44  # Clause-clause subsumption calls (NU) : 0
% 0.16/0.44  # Rec. Clause-clause subsumption calls : 0
% 0.16/0.44  # Non-unit clause-clause subsumptions  : 0
% 0.16/0.44  # Unit Clause-clause subsumption calls : 0
% 0.16/0.44  # Rewrite failures with RHS unbound    : 0
% 0.16/0.44  # BW rewrite match attempts            : 0
% 0.16/0.44  # BW rewrite match successes           : 0
% 0.16/0.44  # Condensation attempts                : 0
% 0.16/0.44  # Condensation successes               : 0
% 0.16/0.44  # Termbank termtop insertions          : 288
% 0.16/0.44  # Search garbage collected termcells   : 123
% 0.16/0.44  
% 0.16/0.44  # -------------------------------------------------
% 0.16/0.44  # User time                : 0.000 s
% 0.16/0.44  # System time              : 0.003 s
% 0.16/0.44  # Total time               : 0.003 s
% 0.16/0.44  # Maximum resident set size: 1704 pages
% 0.16/0.44  
% 0.16/0.44  # -------------------------------------------------
% 0.16/0.44  # User time                : 0.003 s
% 0.16/0.44  # System time              : 0.005 s
% 0.16/0.44  # Total time               : 0.008 s
% 0.16/0.44  # Maximum resident set size: 1692 pages
% 0.16/0.44  % E exiting
% 0.16/0.44  % E exiting
%------------------------------------------------------------------------------

