% Proof : Problems/SYN349+1.p
%------------------------------------------------------------------------------
% File     : E---3.3.0
% Problem  : SYN349+1 : TPTP v9.2.0. Released v2.0.0.
% Transfm  : none
% Format   : tptp:raw
% Command  : run_E /export/starexec/sandbox/benchmark/theBenchmark.p 300 THM

% Computer : n009.cluster.edu
% Model    : x86_64 x86_64
% CPU      : Intel(R) Xeon(R) CPU E5-2620 v4 2.10GHz
% Memory   : 8042.1875MB
% OS       : Linux 3.10.0-693.el7.x86_64
% CPULimit : 300s
% WCLimit  : 300s
% DateTime : Mon Sep 29 11:24:59 PM UTC 2025

% Result   : Theorem 0.19s 0.49s
% Output   : CNFRefutation 0.19s
% Verified : 
% SZS Type : Refutation
%            Derivation depth      :    9
%            Number of leaves      :    1
% Syntax   : Number of formulae    :   15 (   4 unt;   0 def)
%            Number of atoms       :   76 (   0 equ)
%            Maximal formula atoms :   36 (   5 avg)
%            Number of connectives :   95 (  34   ~;  42   |;   9   &)
%                                         (   8 <=>;   2  =>;   0  <=;   0 <~>)
%            Maximal formula depth :   16 (   5 avg)
%            Maximal term depth    :    3 (   1 avg)
%            Number of predicates  :    2 (   1 usr;   1 prp; 0-2 aty)
%            Number of functors    :    2 (   2 usr;   0 con; 1-2 aty)
%            Number of variables   :   27 (   0 sgn   6   !;   4   ?)

% Comments : 
%------------------------------------------------------------------------------
fof(church_46_17_5,conjecture,
    ? [X1] :
    ! [X2] :
    ? [X3] :
    ! [X4] :
      ( ( big_f(X1,X4)
      <=> big_f(X2,X4) )
     => ( ( ( big_f(X1,X4)
          <=> big_f(X4,X3) )
        <=> big_f(X3,X4) )
      <=> big_f(X4,X2) ) ),
    file('/export/starexec/sandbox/benchmark/theBenchmark.p',church_46_17_5) ).

fof(c_0_1,negated_conjecture,
    ~ ? [X1] :
      ! [X2] :
      ? [X3] :
      ! [X4] :
        ( ( big_f(X1,X4)
        <=> big_f(X2,X4) )
       => ( ( ( big_f(X1,X4)
            <=> big_f(X4,X3) )
          <=> big_f(X3,X4) )
        <=> big_f(X4,X2) ) ),
    inference(assume_negation,[status(cth)],[church_46_17_5]) ).

fof(c_0_2,negated_conjecture,
    ! [X5,X7] :
      ( ( ~ big_f(X5,esk2_2(X5,X7))
        | big_f(esk1_1(X5),esk2_2(X5,X7)) )
      & ( ~ big_f(esk1_1(X5),esk2_2(X5,X7))
        | big_f(X5,esk2_2(X5,X7)) )
      & ( ~ big_f(X5,esk2_2(X5,X7))
        | ~ big_f(esk2_2(X5,X7),X7)
        | ~ big_f(X7,esk2_2(X5,X7))
        | ~ big_f(esk2_2(X5,X7),esk1_1(X5)) )
      & ( big_f(X5,esk2_2(X5,X7))
        | big_f(esk2_2(X5,X7),X7)
        | ~ big_f(X7,esk2_2(X5,X7))
        | ~ big_f(esk2_2(X5,X7),esk1_1(X5)) )
      & ( ~ big_f(X5,esk2_2(X5,X7))
        | big_f(esk2_2(X5,X7),X7)
        | big_f(X7,esk2_2(X5,X7))
        | ~ big_f(esk2_2(X5,X7),esk1_1(X5)) )
      & ( ~ big_f(esk2_2(X5,X7),X7)
        | big_f(X5,esk2_2(X5,X7))
        | big_f(X7,esk2_2(X5,X7))
        | ~ big_f(esk2_2(X5,X7),esk1_1(X5)) )
      & ( ~ big_f(X5,esk2_2(X5,X7))
        | ~ big_f(esk2_2(X5,X7),X7)
        | big_f(X7,esk2_2(X5,X7))
        | big_f(esk2_2(X5,X7),esk1_1(X5)) )
      & ( big_f(X5,esk2_2(X5,X7))
        | big_f(esk2_2(X5,X7),X7)
        | big_f(X7,esk2_2(X5,X7))
        | big_f(esk2_2(X5,X7),esk1_1(X5)) )
      & ( ~ big_f(X5,esk2_2(X5,X7))
        | big_f(esk2_2(X5,X7),X7)
        | ~ big_f(X7,esk2_2(X5,X7))
        | big_f(esk2_2(X5,X7),esk1_1(X5)) )
      & ( ~ big_f(esk2_2(X5,X7),X7)
        | big_f(X5,esk2_2(X5,X7))
        | ~ big_f(X7,esk2_2(X5,X7))
        | big_f(esk2_2(X5,X7),esk1_1(X5)) ) ),
    inference(distribute,[status(thm)],[inference(fof_nnf,[status(thm)],[inference(skolemize,[status(esa)],[inference(variable_rename,[status(thm)],[inference(fof_nnf,[status(thm)],[c_0_1])])])])]) ).

fof(c_0_3,negated_conjecture,
    ( big_f(esk2_2(X1,X2),X2)
    | big_f(esk2_2(X1,X2),esk1_1(X1))
    | ~ big_f(X1,esk2_2(X1,X2))
    | ~ big_f(X2,esk2_2(X1,X2)) ),
    inference(split_conjunct,[status(thm)],[c_0_2]) ).

fof(c_0_4,negated_conjecture,
    ( big_f(esk1_1(X1),esk2_2(X1,X2))
    | ~ big_f(X1,esk2_2(X1,X2)) ),
    inference(split_conjunct,[status(thm)],[c_0_2]) ).

fof(c_0_5,negated_conjecture,
    ( ~ big_f(X1,esk2_2(X1,X2))
    | ~ big_f(esk2_2(X1,X2),X2)
    | ~ big_f(X2,esk2_2(X1,X2))
    | ~ big_f(esk2_2(X1,X2),esk1_1(X1)) ),
    inference(split_conjunct,[status(thm)],[c_0_2]) ).

fof(c_0_6,negated_conjecture,
    ( big_f(esk2_2(X1,esk1_1(X1)),esk1_1(X1))
    | ~ big_f(X1,esk2_2(X1,esk1_1(X1))) ),
    inference(spm,[status(thm)],[c_0_3,c_0_4]) ).

fof(c_0_7,negated_conjecture,
    ( big_f(X1,esk2_2(X1,X2))
    | ~ big_f(esk1_1(X1),esk2_2(X1,X2)) ),
    inference(split_conjunct,[status(thm)],[c_0_2]) ).

fof(c_0_8,negated_conjecture,
    ( ~ big_f(esk1_1(X1),esk2_2(X1,esk1_1(X1)))
    | ~ big_f(esk2_2(X1,esk1_1(X1)),esk1_1(X1)) ),
    inference(csr,[status(thm)],[inference(spm,[status(thm)],[c_0_5,c_0_6]),c_0_7]) ).

fof(c_0_9,negated_conjecture,
    ~ big_f(esk1_1(X1),esk2_2(X1,esk1_1(X1))),
    inference(csr,[status(thm)],[inference(spm,[status(thm)],[c_0_8,c_0_6]),c_0_7]) ).

fof(c_0_10,negated_conjecture,
    ( big_f(X1,esk2_2(X1,X2))
    | big_f(esk2_2(X1,X2),X2)
    | big_f(X2,esk2_2(X1,X2))
    | big_f(esk2_2(X1,X2),esk1_1(X1)) ),
    inference(split_conjunct,[status(thm)],[c_0_2]) ).

fof(c_0_11,negated_conjecture,
    ~ big_f(X1,esk2_2(X1,esk1_1(X1))),
    inference(spm,[status(thm)],[c_0_9,c_0_4]) ).

fof(c_0_12,negated_conjecture,
    ( big_f(X1,esk2_2(X1,X2))
    | big_f(X2,esk2_2(X1,X2))
    | ~ big_f(esk2_2(X1,X2),X2)
    | ~ big_f(esk2_2(X1,X2),esk1_1(X1)) ),
    inference(split_conjunct,[status(thm)],[c_0_2]) ).

fof(c_0_13,negated_conjecture,
    big_f(esk2_2(X1,esk1_1(X1)),esk1_1(X1)),
    inference(sr,[status(thm)],[inference(sr,[status(thm)],[inference(ef,[status(thm)],[c_0_10]),c_0_9]),c_0_11]) ).

fof(c_0_14,negated_conjecture,
    $false,
    inference(sr,[status(thm)],[inference(sr,[status(thm)],[inference(cn,[status(thm)],[inference(rw,[status(thm)],[inference(spm,[status(thm)],[c_0_12,c_0_13]),c_0_13])]),c_0_9]),c_0_11]),
    [proof] ).

%------------------------------------------------------------------------------
%----ORIGINAL SYSTEM OUTPUT
% 0.06/0.12  % Problem    : SYN349+1 : TPTP v9.2.0. Released v2.0.0.
% 0.06/0.12  % Command    : run_E /export/starexec/sandbox/benchmark/theBenchmark.p 300 THM
% 0.12/0.33  % Computer : n009.cluster.edu
% 0.12/0.33  % Model    : x86_64 x86_64
% 0.12/0.33  % CPU      : Intel(R) Xeon(R) CPU E5-2620 v4 @ 2.10GHz
% 0.12/0.33  % Memory   : 8042.1875MB
% 0.12/0.33  % OS       : Linux 3.10.0-693.el7.x86_64
% 0.12/0.33  % CPULimit   : 300
% 0.12/0.33  % WCLimit    : 300
% 0.12/0.33  % DateTime   : Fri Sep 26 14:55:38 EDT 2025
% 0.12/0.33  % CPUTime    : 
% 0.19/0.47  Running first-order theorem proving
% 0.19/0.47  Running: /export/starexec/sandbox/solver/bin/eprover --delete-bad-limit=2000000000 --definitional-cnf=24 -s --print-statistics -R --print-version --proof-object --auto-schedule=8 --cpu-limit=300 /export/starexec/sandbox/benchmark/theBenchmark.p
% 0.19/0.49  # Version: 3.0.0
% 0.19/0.49  # Preprocessing class: FSSSSMSSSSSNFFN.
% 0.19/0.49  # Scheduled 4 strats onto 8 cores with 300 seconds (2400 total)
% 0.19/0.49  # Starting G-E--_302_C18_F1_URBAN_RG_S04BN with 1500s (5) cores
% 0.19/0.49  # Starting new_bool_3 with 300s (1) cores
% 0.19/0.49  # Starting new_bool_1 with 300s (1) cores
% 0.19/0.49  # Starting sh5l with 300s (1) cores
% 0.19/0.49  # new_bool_3 with pid 30349 completed with status 0
% 0.19/0.49  # Result found by new_bool_3
% 0.19/0.49  # Preprocessing class: FSSSSMSSSSSNFFN.
% 0.19/0.49  # Scheduled 4 strats onto 8 cores with 300 seconds (2400 total)
% 0.19/0.49  # Starting G-E--_302_C18_F1_URBAN_RG_S04BN with 1500s (5) cores
% 0.19/0.49  # Starting new_bool_3 with 300s (1) cores
% 0.19/0.49  # SinE strategy is GSinE(CountFormulas,hypos,1.5,,3,20000,1.0)
% 0.19/0.49  # Search class: FGHNF-FFSF21-SFFFFFNN
% 0.19/0.49  # Scheduled 5 strats onto 1 cores with 300 seconds (300 total)
% 0.19/0.49  # Starting G-E--_208_C18_F1_SE_CS_SP_PI_PS_S5PRR_S032N with 181s (1) cores
% 0.19/0.49  # G-E--_208_C18_F1_SE_CS_SP_PI_PS_S5PRR_S032N with pid 30353 completed with status 0
% 0.19/0.49  # Result found by G-E--_208_C18_F1_SE_CS_SP_PI_PS_S5PRR_S032N
% 0.19/0.49  # Preprocessing class: FSSSSMSSSSSNFFN.
% 0.19/0.49  # Scheduled 4 strats onto 8 cores with 300 seconds (2400 total)
% 0.19/0.49  # Starting G-E--_302_C18_F1_URBAN_RG_S04BN with 1500s (5) cores
% 0.19/0.49  # Starting new_bool_3 with 300s (1) cores
% 0.19/0.49  # SinE strategy is GSinE(CountFormulas,hypos,1.5,,3,20000,1.0)
% 0.19/0.49  # Search class: FGHNF-FFSF21-SFFFFFNN
% 0.19/0.49  # Scheduled 5 strats onto 1 cores with 300 seconds (300 total)
% 0.19/0.49  # Starting G-E--_208_C18_F1_SE_CS_SP_PI_PS_S5PRR_S032N with 181s (1) cores
% 0.19/0.49  # Preprocessing time       : 0.001 s
% 0.19/0.49  # Presaturation interreduction done
% 0.19/0.49  
% 0.19/0.49  # Proof found!
% 0.19/0.49  # SZS status Theorem
% 0.19/0.49  # SZS output start CNFRefutation
% See solution above
% 0.19/0.49  # Parsed axioms                        : 1
% 0.19/0.49  # Removed by relevancy pruning/SinE    : 0
% 0.19/0.49  # Initial clauses                      : 10
% 0.19/0.49  # Removed in clause preprocessing      : 0
% 0.19/0.49  # Initial clauses in saturation        : 10
% 0.19/0.49  # Processed clauses                    : 25
% 0.19/0.49  # ...of these trivial                  : 0
% 0.19/0.49  # ...subsumed                          : 0
% 0.19/0.49  # ...remaining for further processing  : 25
% 0.19/0.49  # Other redundant clauses eliminated   : 0
% 0.19/0.49  # Clauses deleted for lack of memory   : 0
% 0.19/0.49  # Backward-subsumed                    : 2
% 0.19/0.49  # Backward-rewritten                   : 0
% 0.19/0.49  # Generated clauses                    : 19
% 0.19/0.49  # ...of the previous two non-redundant : 7
% 0.19/0.49  # ...aggressively subsumed             : 0
% 0.19/0.49  # Contextual simplify-reflections      : 2
% 0.19/0.49  # Paramodulations                      : 17
% 0.19/0.49  # Factorizations                       : 2
% 0.19/0.49  # NegExts                              : 0
% 0.19/0.49  # Equation resolutions                 : 0
% 0.19/0.49  # Disequality decompositions           : 0
% 0.19/0.49  # Total rewrite steps                  : 1
% 0.19/0.49  # ...of those cached                   : 0
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
% 0.19/0.49  # Current number of processed clauses  : 13
% 0.19/0.49  #    Positive orientable unit clauses  : 1
% 0.19/0.49  #    Positive unorientable unit clauses: 0
% 0.19/0.49  #    Negative unit clauses             : 2
% 0.19/0.49  #    Non-unit-clauses                  : 10
% 0.19/0.49  # Current number of unprocessed clauses: 1
% 0.19/0.49  # ...number of literals in the above   : 3
% 0.19/0.49  # Current number of archived formulas  : 0
% 0.19/0.49  # Current number of archived clauses   : 12
% 0.19/0.49  # Clause-clause subsumption calls (NU) : 112
% 0.19/0.49  # Rec. Clause-clause subsumption calls : 48
% 0.19/0.49  # Non-unit clause-clause subsumptions  : 2
% 0.19/0.49  # Unit Clause-clause subsumption calls : 8
% 0.19/0.49  # Rewrite failures with RHS unbound    : 0
% 0.19/0.49  # BW rewrite match attempts            : 0
% 0.19/0.49  # BW rewrite match successes           : 0
% 0.19/0.49  # Condensation attempts                : 0
% 0.19/0.49  # Condensation successes               : 0
% 0.19/0.49  # Termbank termtop insertions          : 1737
% 0.19/0.49  # Search garbage collected termcells   : 244
% 0.19/0.49  
% 0.19/0.49  # -------------------------------------------------
% 0.19/0.49  # User time                : 0.005 s
% 0.19/0.49  # System time              : 0.001 s
% 0.19/0.49  # Total time               : 0.006 s
% 0.19/0.49  # Maximum resident set size: 1760 pages
% 0.19/0.49  
% 0.19/0.49  # -------------------------------------------------
% 0.19/0.49  # User time                : 0.008 s
% 0.19/0.49  # System time              : 0.001 s
% 0.19/0.49  # Total time               : 0.009 s
% 0.19/0.49  # Maximum resident set size: 1696 pages
% 0.19/0.49  % E exiting
% 0.19/0.49  % E exiting
%------------------------------------------------------------------------------

