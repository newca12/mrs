# Terminology

| Term | Meaning in this repository |
|---|---|
| TPTP | Thousands of Problems for Theorem Provers input language and library. |
| TSTP | TPTP proof/output syntax. |
| SZS | Standard status vocabulary used by TPTP tools. |
| FOF | Untyped first-order formulas. |
| FNE | Local taxonomy for first-order problems without equality. |
| FEQ | Local taxonomy for first-order problems with equality. |
| UEQ | Unit equality CNF problems. |
| EPR | Effectively propositional, function-free first-order structure. |
| EPS | Local EPR satisfiable benchmark division. |
| EPU | Local EPR unsatisfiable benchmark division. |
| ICU | Legacy local intensional unit-equality benchmark division. |
| PRV / ProoVer | Proof-verification competition and corpus terminology. |
| Given clause | The next passive clause selected for inference against processed clauses. |
| Passive set | Clauses waiting to be selected. |
| Processed set | Active clauses already selected and indexed. |
| Refutation | A derivation of the empty clause or `$false`. |
| Saturation | Exhaustion of a complete, certified search path; ordinary heuristic search does not establish it. |
| GaveUp | A safe inconclusive result caused by unsupported input, incomplete restrictions, or a bounded search path. |
| LRS | Limited Resource Strategy, the passive-queue pruning mechanism. |
| AVATAR | SAT-backed clause splitting architecture. |
| InstGen | SAT-guided first-order instance generation for EPR-shaped inputs. |
| CWA | Componentwise AVATAR, a narrow definitional-CNF split/refutation prepass. |
