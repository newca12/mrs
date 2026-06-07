% Proof : Problems/SYN351+1.p
%------------------------------------------------------------------------------
% File     : E---3.3.0
% Problem  : SYN351+1 : TPTP v9.2.0. Released v2.0.0.
% Transfm  : none
% Format   : tptp:raw
% Command  : run_E /export/starexec/sandbox2/benchmark/theBenchmark.p 300 THM

% Computer : n007.cluster.edu
% Model    : x86_64 x86_64
% CPU      : Intel(R) Xeon(R) CPU E5-2620 v4 2.10GHz
% Memory   : 8042.1875MB
% OS       : Linux 3.10.0-693.el7.x86_64
% CPULimit : 300s
% WCLimit  : 300s
% DateTime : Mon Sep 29 11:25:00 PM UTC 2025

% Result   : Theorem 0.15s 0.41s
% Output   : CNFRefutation 0.15s
% Verified : 
% SZS Type : Refutation
%            Derivation depth      :    5
%            Number of leaves      :    1
% Syntax   : Number of formulae    :    7 (   2 unt;   0 def)
%            Number of atoms       :   47 (   0 equ)
%            Maximal formula atoms :   22 (   6 avg)
%            Number of connectives :   55 (  15   ~;  15   |;  11   &)
%                                         (   4 <=>;  10  =>;   0  <=;   0 <~>)
%            Maximal formula depth :   15 (   7 avg)
%            Maximal term depth    :    2 (   1 avg)
%            Number of predicates  :    2 (   1 usr;   1 prp; 0-4 aty)
%            Number of functors    :    3 (   3 usr;   2 con; 0-2 aty)
%            Number of variables   :   18 (   2 sgn   8   !;   4   ?)

% Comments : 
%------------------------------------------------------------------------------
fof(church_46_18_3,conjecture,
    ! [X1,X2] :
    ? [X3,X4] :
    ! [X5] :
      ( big_f(X1,X4,X1,X5)
     => ( ( big_f(X1,X3,X1,X4)
        <=> big_f(X3,X2,X3,X4) )
       => ( big_f(X1,X3,X1,X4)
         => ( ( big_f(X1,X4,X3,X4)
             => big_f(X1,X5,X3,X5) )
            & ( big_f(X1,X5,X3,X5)
             => ( big_f(X1,X3,X1,X4)
              <=> big_f(X1,X4,X3,X4) ) ) ) ) ) ),
    file('/export/starexec/sandbox2/benchmark/theBenchmark.p',church_46_18_3) ).

fof(c_0_1,negated_conjecture,
    ~ ! [X1,X2] :
      ? [X3,X4] :
      ! [X5] :
        ( big_f(X1,X4,X1,X5)
       => ( ( big_f(X1,X3,X1,X4)
          <=> big_f(X3,X2,X3,X4) )
         => ( big_f(X1,X3,X1,X4)
           => ( ( big_f(X1,X4,X3,X4)
               => big_f(X1,X5,X3,X5) )
              & ( big_f(X1,X5,X3,X5)
               => ( big_f(X1,X3,X1,X4)
                <=> big_f(X1,X4,X3,X4) ) ) ) ) ) ),
    inference(assume_negation,[status(cth)],[church_46_18_3]) ).

fof(c_0_2,negated_conjecture,
    ! [X8,X9] :
      ( big_f(esk1_0,X9,esk1_0,esk3_2(X8,X9))
      & ( ~ big_f(esk1_0,X8,esk1_0,X9)
        | big_f(X8,esk2_0,X8,X9) )
      & ( ~ big_f(X8,esk2_0,X8,X9)
        | big_f(esk1_0,X8,esk1_0,X9) )
      & big_f(esk1_0,X8,esk1_0,X9)
      & ( big_f(esk1_0,esk3_2(X8,X9),X8,esk3_2(X8,X9))
        | big_f(esk1_0,X9,X8,X9) )
      & ( ~ big_f(esk1_0,X8,esk1_0,X9)
        | ~ big_f(esk1_0,X9,X8,X9)
        | big_f(esk1_0,X9,X8,X9) )
      & ( big_f(esk1_0,X8,esk1_0,X9)
        | big_f(esk1_0,X9,X8,X9)
        | big_f(esk1_0,X9,X8,X9) )
      & ( big_f(esk1_0,esk3_2(X8,X9),X8,esk3_2(X8,X9))
        | ~ big_f(esk1_0,esk3_2(X8,X9),X8,esk3_2(X8,X9)) )
      & ( ~ big_f(esk1_0,X8,esk1_0,X9)
        | ~ big_f(esk1_0,X9,X8,X9)
        | ~ big_f(esk1_0,esk3_2(X8,X9),X8,esk3_2(X8,X9)) )
      & ( big_f(esk1_0,X8,esk1_0,X9)
        | big_f(esk1_0,X9,X8,X9)
        | ~ big_f(esk1_0,esk3_2(X8,X9),X8,esk3_2(X8,X9)) ) ),
    inference(distribute,[status(thm)],[inference(fof_nnf,[status(thm)],[inference(skolemize,[status(esa)],[inference(variable_rename,[status(thm)],[inference(fof_nnf,[status(thm)],[c_0_1])])])])]) ).

fof(c_0_3,negated_conjecture,
    ( ~ big_f(esk1_0,X1,esk1_0,X2)
    | ~ big_f(esk1_0,X2,X1,X2)
    | ~ big_f(esk1_0,esk3_2(X1,X2),X1,esk3_2(X1,X2)) ),
    inference(split_conjunct,[status(thm)],[c_0_2]) ).

fof(c_0_4,negated_conjecture,
    big_f(esk1_0,X1,esk1_0,X2),
    inference(split_conjunct,[status(thm)],[c_0_2]) ).

fof(c_0_5,negated_conjecture,
    ( ~ big_f(esk1_0,esk3_2(X1,X2),X1,esk3_2(X1,X2))
    | ~ big_f(esk1_0,X2,X1,X2) ),
    inference(cn,[status(thm)],[inference(rw,[status(thm)],[c_0_3,c_0_4])]) ).

fof(c_0_6,negated_conjecture,
    $false,
    inference(cn,[status(thm)],[inference(rw,[status(thm)],[inference(spm,[status(thm)],[c_0_5,c_0_4]),c_0_4])]),
    [proof] ).

%------------------------------------------------------------------------------
%----ORIGINAL SYSTEM OUTPUT
% 0.05/0.10  % Problem    : SYN351+1 : TPTP v9.2.0. Released v2.0.0.
% 0.05/0.10  % Command    : run_E /export/starexec/sandbox2/benchmark/theBenchmark.p 300 THM
% 0.09/0.29  % Computer : n007.cluster.edu
% 0.09/0.29  % Model    : x86_64 x86_64
% 0.09/0.29  % CPU      : Intel(R) Xeon(R) CPU E5-2620 v4 @ 2.10GHz
% 0.09/0.29  % Memory   : 8042.1875MB
% 0.09/0.29  % OS       : Linux 3.10.0-693.el7.x86_64
% 0.09/0.29  % CPULimit   : 300
% 0.09/0.29  % WCLimit    : 300
% 0.09/0.29  % DateTime   : Fri Sep 26 14:42:38 EDT 2025
% 0.09/0.29  % CPUTime    : 
% 0.15/0.41  Running first-order theorem proving
% 0.15/0.41  Running: /export/starexec/sandbox2/solver/bin/eprover --delete-bad-limit=2000000000 --definitional-cnf=24 -s --print-statistics -R --print-version --proof-object --auto-schedule=8 --cpu-limit=300 /export/starexec/sandbox2/benchmark/theBenchmark.p
% 0.15/0.41  # Version: 3.0.0
% 0.15/0.41  # Preprocessing class: FSSSSMSSSSSNFFN.
% 0.15/0.41  # Scheduled 4 strats onto 8 cores with 300 seconds (2400 total)
% 0.15/0.41  # Starting G-E--_302_C18_F1_URBAN_RG_S04BN with 1500s (5) cores
% 0.15/0.41  # Starting new_bool_3 with 300s (1) cores
% 0.15/0.41  # Starting new_bool_1 with 300s (1) cores
% 0.15/0.41  # Starting sh5l with 300s (1) cores
% 0.15/0.41  # new_bool_3 with pid 25855 completed with status 0
% 0.15/0.41  # Result found by new_bool_3
% 0.15/0.41  # Preprocessing class: FSSSSMSSSSSNFFN.
% 0.15/0.41  # Scheduled 4 strats onto 8 cores with 300 seconds (2400 total)
% 0.15/0.41  # Starting G-E--_302_C18_F1_URBAN_RG_S04BN with 1500s (5) cores
% 0.15/0.41  # Starting new_bool_3 with 300s (1) cores
% 0.15/0.41  # SinE strategy is GSinE(CountFormulas,hypos,1.5,,3,20000,1.0)
% 0.15/0.41  # Search class: FGHNS-FFSF22-SFFFFFNN
% 0.15/0.41  # partial match(1): FGHNF-FFSF22-SFFFFFNN
% 0.15/0.41  # Scheduled 5 strats onto 1 cores with 300 seconds (300 total)
% 0.15/0.41  # Starting SAT001_MinMin_p005000_rr_RG with 181s (1) cores
% 0.15/0.41  # SAT001_MinMin_p005000_rr_RG with pid 25861 completed with status 0
% 0.15/0.41  # Result found by SAT001_MinMin_p005000_rr_RG
% 0.15/0.41  # Preprocessing class: FSSSSMSSSSSNFFN.
% 0.15/0.41  # Scheduled 4 strats onto 8 cores with 300 seconds (2400 total)
% 0.15/0.41  # Starting G-E--_302_C18_F1_URBAN_RG_S04BN with 1500s (5) cores
% 0.15/0.41  # Starting new_bool_3 with 300s (1) cores
% 0.15/0.41  # SinE strategy is GSinE(CountFormulas,hypos,1.5,,3,20000,1.0)
% 0.15/0.41  # Search class: FGHNS-FFSF22-SFFFFFNN
% 0.15/0.41  # partial match(1): FGHNF-FFSF22-SFFFFFNN
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
% 0.15/0.41  # Initial clauses                      : 10
% 0.15/0.41  # Removed in clause preprocessing      : 2
% 0.15/0.41  # Initial clauses in saturation        : 8
% 0.15/0.41  # Processed clauses                    : 10
% 0.15/0.41  # ...of these trivial                  : 4
% 0.15/0.41  # ...subsumed                          : 0
% 0.15/0.41  # ...remaining for further processing  : 6
% 0.15/0.41  # Other redundant clauses eliminated   : 0
% 0.15/0.41  # Clauses deleted for lack of memory   : 0
% 0.15/0.41  # Backward-subsumed                    : 0
% 0.15/0.41  # Backward-rewritten                   : 0
% 0.15/0.41  # Generated clauses                    : 1
% 0.15/0.41  # ...of the previous two non-redundant : 0
% 0.15/0.41  # ...aggressively subsumed             : 0
% 0.15/0.41  # Contextual simplify-reflections      : 0
% 0.15/0.41  # Paramodulations                      : 1
% 0.15/0.41  # Factorizations                       : 0
% 0.15/0.41  # NegExts                              : 0
% 0.15/0.41  # Equation resolutions                 : 0
% 0.15/0.41  # Disequality decompositions           : 0
% 0.15/0.41  # Total rewrite steps                  : 7
% 0.15/0.41  # ...of those cached                   : 4
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
% 0.15/0.41  # Current number of processed clauses  : 2
% 0.15/0.41  #    Positive orientable unit clauses  : 1
% 0.15/0.41  #    Positive unorientable unit clauses: 0
% 0.15/0.41  #    Negative unit clauses             : 0
% 0.15/0.41  #    Non-unit-clauses                  : 1
% 0.15/0.41  # Current number of unprocessed clauses: 2
% 0.15/0.41  # ...number of literals in the above   : 3
% 0.15/0.41  # Current number of archived formulas  : 0
% 0.15/0.41  # Current number of archived clauses   : 4
% 0.15/0.41  # Clause-clause subsumption calls (NU) : 0
% 0.15/0.41  # Rec. Clause-clause subsumption calls : 0
% 0.15/0.41  # Non-unit clause-clause subsumptions  : 0
% 0.15/0.41  # Unit Clause-clause subsumption calls : 0
% 0.15/0.41  # Rewrite failures with RHS unbound    : 0
% 0.15/0.41  # BW rewrite match attempts            : 0
% 0.15/0.41  # BW rewrite match successes           : 0
% 0.15/0.41  # Condensation attempts                : 0
% 0.15/0.41  # Condensation successes               : 0
% 0.15/0.41  # Termbank termtop insertions          : 611
% 0.15/0.41  # Search garbage collected termcells   : 161
% 0.15/0.41  
% 0.15/0.41  # -------------------------------------------------
% 0.15/0.41  # User time                : 0.002 s
% 0.15/0.41  # System time              : 0.001 s
% 0.15/0.41  # Total time               : 0.003 s
% 0.15/0.41  # Maximum resident set size: 1788 pages
% 0.15/0.41  
% 0.15/0.41  # -------------------------------------------------
% 0.15/0.41  # User time                : 0.004 s
% 0.15/0.41  # System time              : 0.001 s
% 0.15/0.41  # Total time               : 0.005 s
% 0.15/0.41  # Maximum resident set size: 1696 pages
% 0.15/0.41  % E exiting
% 0.15/0.42  % E exiting
%------------------------------------------------------------------------------

