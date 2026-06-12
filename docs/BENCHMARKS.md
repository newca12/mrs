# Benchmark Log

Append-only log of CASC benchmark runs (`crates/mrs-bench/casc.sh`), newest first.
Each entry records the mrs commit and the exact command used.


commit 6ad04a4743507b27d91fba37b8b4efce864d6543 (HEAD -> feature/ml-guided-clause-selection, origin/feature/ml-guided-clause-selection)

ongoing
[root@mtsdev02 mrs]# INPUT_PROBLEMS_LIST=./casc_problem_lists/epr.list ./crates/mrs-bench/collect_ml_data.sh /mnt/sde1/TPTP-v9.2.1 ./ml_logs_epr 1 960 8

ongoing
hack@pve:~/mrs$ INPUT_PROBLEMS_LIST=./casc_problem_lists/ueq.list ./crates/mrs-bench/collect_ml_data.sh /home/hack/TPTP-v9.2.1 ./ml_logs_ueq 1 960 4

ongoing
[www@teenf9901 mrs]$ INPUT_PROBLEMS_LIST=./casc_problem_lists/fne.list ./crates/mrs-bench/collect_ml_data.sh /DATA/ai/TPTP-v9.2.1 ./ml_logs_fne 1 480 30

ongoing
[PPROD:fr22192@tlpnf9701:/DATA/ai/fr22192/mrs]$ INPUT_PROBLEMS_LIST=./casc_problem_lists/feq.list ./crates/mrs-bench/collect_ml_data.sh /DATA/ai/TPTP-v9.2.1 ./ml_logs_feq 1 480 30

commit 4252ef646b2bfeda3ef1baedb3bc47034fcc0776 (HEAD -> feature/ml-guided-clause-selection

[www@teenf9901 mrs]$ INPUT_PROBLEMS_LIST=./casc_problem_lists/fne.list ./crates/mrs-bench/collect_ml_data.sh /DATA/ai/TPTP-v9.2.1 ./ml_logs_fne 3 480

commit 369f48eccbe77c4cbd917a22d60d785dce9fd0a8 (HEAD -> feature/ml-guided-clause-selection, origin/feature/ml-guided-clause-selection)

ongoing
[PPROD:fr22192@tlpnf9701:/DATA/ai/fr22192/mrs]$ crates/mrs-bench/casc.sh --systems mrs --casc-times --jobs 3 --divisions fne

commit 3380e2d65a43eab76b3f37a20c39efb123896fb8 (HEAD -> feature/ml-guided-clause-selection

[PPROD:fr22192@tlpnf9701:/DATA/ai/fr22192/mrs]$ ./crates/mrs-bench/casc.sh --systems mrs-ml --casc-times --jobs 3 --divisions feq,fne,ueq
CASC-30 Results — 2026-06-12 11:55  (800 problems × 1 systems)
==============================================================

Division  Problems    mrs-ml
                      Solved  Avg (s)
------------------  --------------------
FEQ            400        43   37.395
FNE            100        27   18.097
UEQ            300        18   32.996
------------------  --------------------
TOTAL          800        88   30.574

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

293abae65eda29d45d2a40111dcbec641b8dbc89

[PPROD:fr22192@tlpnf9701:/DATA/ai/fr22192/mrs]$ ./crates/mrs-bench/collect_ml_data.sh /path/to/TPTP-v9.2.1 ./ml_logs 16 30



Results for mrs commit b0ca6c15f18a5561ac795690c96f191ef61f79d7

ongoing
[www@teenf9901 mrs]$ crates/mrs-bench/casc.sh --systems vampire --casc-times


hack@pve:~/mrs$ crates/mrs-bench/casc.sh --systems vampire  --casc-times --divisions eps

CASC-30 Results — 2026-06-09 06:11  (100 problems × 1 systems)
==============================================================

Division  Problems    vampire
                      Solved  Avg (s)
------------------  --------------------
EPS            100        86    5.027
------------------  --------------------
TOTAL          100        86    5.027

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.


[www@teenf9901 mrs]$ crates/mrs-bench/casc.sh --systems mrs --casc-times --jobs 2 --divisions ueq

CASC-30 Results — 2026-06-09 07:35  (300 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
UEQ            300        20   23.312
------------------  --------------------
TOTAL          300        20   23.312

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

[root@mtsdev02 mrs]# TPTP=/mnt/sda1/mrs/crates/mrs-bench/problems/casc-30 crates/mrs-bench/casc.sh --systems mrs --casc-times --divisions epu
CASC-30 Results — 2026-06-09 07:25  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
EPU            100         8    1.808
------------------  --------------------
TOTAL          100         8    1.808

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.


hack@pve:~/mrs$ crates/mrs-bench/casc.sh --systems mrs  --casc-times --divisions eps

CASC-30 Results — 2026-06-08 21:14  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
EPS            100        22   16.077
------------------  --------------------
TOTAL          100        22   16.077

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

Results for mrs commit 4ea73eb2d96e6364ca73ef455cfd52d1f62bdea2

partial ongoing
[PPROD:fr22192@tlpnf9701:/DATA/ai/fr22192/mrs]$ crates/mrs-bench/casc.sh --systems mrs --casc-times --jobs 2
------------------  --------------------
FNE            100        31   25.060
FEQ            400        48   41.741
EPU            100         9   12.152
EPS            100        26   13.328
UEQ            300        23   15.731
ICU             87         1   15.711
------------------  --------------------
TOTAL         1087       138   26.187

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — 2 case(s) of wrong SZS polarity:
  UEQ     GRP024-5                        mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  UEQ     GRP196-1                        mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND



Results for mrs commit a3cc272eddf2fd408b4705dd650025dbde44a1e0


crash ?
hack@pve:~/mrs$ crates/mrs-bench/casc.sh --systems mrs  --casc-times --divisions feq
CASC-30 Results — 2026-06-06 21:20  (376 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
FEQ            376        41   35.218
------------------  --------------------
TOTAL          376        41   35.218

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

Results for mrs commit 65a6aebe14d676ba8ab990a1397b4a340483c36f (HEAD -> casc-improvements, origin/fix-imperfect-indexing, origin/casc-improvements)

ongoing
[www@teenf9901 mrs]$ crates/mrs-bench/casc.sh --systems mrs --casc-times --jobs 2

Results for mrs commit 65a6aebe14d676ba8ab990a1397b4a340483c36f (HEAD -> fix-imperfect-indexing, origin/fix-imperfect-indexing)

interrupted ?
hack@pve:~/mrs$ crates/mrs-bench/casc.sh --systems mrs  --casc-times --divisions feq
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
FEQ            364        41   37.644
------------------  --------------------
TOTAL          364        41   37.644

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

Results for mrs commit b8005170ff45ef6e1507863a651f28b40328e0d9 (HEAD -> fix-epr-grounding, origin/fix-epr-grounding)

[www@teenf9901 mrs]$ crates/mrs-bench/casc.sh --systems mrs --casc-times --divisions epu --jobs 2
CASC-30 Results — 2026-06-05 06:53  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
EPU            100         8    2.251
------------------  --------------------
TOTAL          100         8    2.251

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

Results for mrs commit ee8aaf20cfbf7a9c6fefff21303c5eb038191e09 (HEAD -> fix-sine-over-pruning, origin/fix-sine-over-pruning)

[PPROD:fr22192@tlpnf9701:/DATA/ai/fr22192/mrs]$ crates/mrs-bench/casc.sh --systems mrs --casc-times --jobs 2

CASC-30 Results — 2026-06-06 06:35  (1101 problems × 1 systems)
===============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
FNE            100        32   18.431
FEQ            400        46   17.958
EPU            100         8    0.603
EPS            100        23   15.448
UEQ            300        36   44.200
ICU            101         1   10.395
------------------  --------------------
TOTAL         1101       146   23.134

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.


Results for mrs commit 34338df31d907db20e708e6dc4d74e63a29d2e9a

aborted to 212/1101 (very slow)
[www@teenf9901 mrs]$ crates/mrs-bench/casc.sh --systems mrs --casc-times

[PPROD:fr22192@tlpnf9701:/DATA/ai/fr22192/mrs]$ crates/mrs-bench/casc.sh --systems mrs --casc-times --jobs 3
CASC-30 Results — 2026-06-04 14:22  (1101 problems × 1 systems)
===============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
FNE            100        30   21.895
FEQ            400        45   23.244
EPU            100         8    0.901
EPS            100        23   20.575
UEQ            300        32   47.636
ICU            101         1   20.648
------------------  --------------------
TOTAL         1101       139   26.822

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

ongoing
hack@pve:~/mrs$ crates/mrs-bench/casc.sh --systems mrs  --casc-times --divisions feq
cancelled at 212/400 33 Solved

OOM ?
hack@pve:~/mrs$ crates/mrs-bench/casc.sh --systems mrs  --casc-times

Results for mrs commit c0816a7a24dfb287d0eccc3e23b75d00c54d2fc8

[PPROD:fr22192@tlpnf9701:/DATA/ai/fr22192/mrs]$ crates/mrs-bench/casc.sh --systems mrs --jobs 30
CASC-30 Results — 2026-06-03 08:13  (1101 problems × 1 systems)
===============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
FNE            100        24   12.129
FEQ            400        27   19.331
EPU            100         8    2.738
EPS            100        13    0.670
UEQ            300        13   51.488
ICU            101         1    0.289
------------------  --------------------
TOTAL         1101        86   17.596

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

check commit
hack@pve:~/mrs$ crates/mrs-bench/casc.sh --systems mrs  --jobs 4
CASC-30 Results — 2026-06-03 04:51  (1101 problems × 1 systems)
===============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
FNE            100        24   12.374
FEQ            400        27   19.354
EPU            100         8    2.914
EPS            100        13    0.731
UEQ            300        12   49.054
ICU            101         1    0.309
------------------  --------------------
TOTAL         1101        85   16.956

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.


Results for mrs commit      f0638f5013ee34319fece821c5979f01fcaaebae

[PPROD:fr22192@tlpnf9701:/DATA/ai/fr22192/mrs]$ crates/mrs-bench/casc.sh --systems mrs --jobs 30
CASC-30 Results — 2026-06-03 06:44  (1101 problems × 1 systems)
===============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
FNE            100        28   17.650
FEQ            400        20   16.206
EPU            100         8    3.203
EPS            100        14    2.101
UEQ            300         0    0.000
ICU            101         2    1.154
------------------  --------------------
TOTAL         1101        72   12.162

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.



Results for mrs commit  e2dc18b19564f85f98d6e0d0c9e054a642bbc4a1

ongoing
hack@pve:~/mrs$ crates/mrs-bench/casc.sh --systems mrs  --jobs 4
CASC-30 Results — 2026-06-02 06:13  (1101 problems × 1 systems)
===============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
FNE            100        25   13.686
FEQ            400        18   19.094
EPU            100         9    2.931
EPS            100        31    2.154
UEQ            300        26   38.725
ICU            101         1    0.285
------------------  --------------------
TOTAL         1101       110   16.237

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — 1 case(s) of wrong SZS polarity:
  EPU     SYN914-1                        mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND

ongoing
[PPROD:fr22192@tlpnf9701:/DATA/ai/fr22192/mrs]$ crates/mrs-bench/casc.sh --systems mrs --jobs 30
CASC-30 Results — 2026-06-02 06:16  (1101 problems × 1 systems)
===============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
FNE            100        26   12.955
FEQ            400        19   21.982
EPU            100         9    2.555
EPS            100        31    3.379
UEQ            300        26   38.657
ICU            101         1    0.279
------------------  --------------------
TOTAL         1101       112   16.853

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — 1 case(s) of wrong SZS polarity:
  EPU     SYN914-1                        mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND

Results for mrs commit 635834c3b5f6c2c15a7647e724a335a938a86f1b

[PPROD:fr22192@tlpnf9701:/DATA/ai/fr22192/mrs]$ crates/mrs-bench/casc.sh --systems mrs --jobs 30
CASC-30 Results — 2026-06-01 08:30  (1101 problems × 1 systems)
===============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
FNE            100        24   10.356
FEQ            400        18   22.705
EPU            100         8    3.132
EPS            100         0    0.000
UEQ            300        18   35.236
ICU            101         2    9.432
------------------  --------------------
TOTAL         1101        70   19.077

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

Verifying SYN355+1...
-> % SZS status NotVerified : step 4: leaf with anonymous provenance (file(_,unknown)) does not α-match any premise-role formula in the linked problem (may differ only by AC-rewriting of commutative operators)
Verifying SYN424+1...
-> % SZS status FailedVerified : structural: proof does not derive $false
Found solution for SYN325+1. Downloading...
Verifying SYN325+1...
-> % SZS status FailedVerified : structural: proof does not derive $false
Found solution for SYN516+1. Downloading...
Verifying SYN516+1...
-> % SZS status FailedVerified : structural: proof does not derive $false
Verifying SYN551+1...
-> % SZS status NotVerified : load error: cannot parse problem file /tmp/tmp.YsE8SFa4sX/Problems/SYN551+1.p: parse error at byte offset 1968: Cut(ContextError { context: [Label("fof_statement"), Label("FOF formula"), Label("annotated_formula"), Label("tptp_input")], cause: None })
Verifying SYN439+1...
-> % SZS status FailedVerified : structural: proof does not derive $false
Verifying SYN507+1...
-> % SZS status FailedVerified : structural: node c_49 is not FOF
Verifying SYN508+1...
Verifying SYN978+1...
-> % SZS status NotVerified : load error: cannot parse problem file /tmp/tmp.YsE8SFa4sX/Problems/SYN978+1.p: parse error at byte offset 1169: Cut(ContextError { context: [Label("fof_statement"), Label("FOF formula"), Label("annotated_formula"), Label("tptp_input")], cause: None })-> % SZS status FailedVerified : structural: node c_49 is not FOF

Results for mrs commit 635834c3b5f6c2c15a7647e724a335a938a86f1b



hack@pve:~/mrs$ cargo run --release --bin mrs -- --time 480 ~/TPTP-v9.2.1/Problems/GRP/GRP678-1.p
    Finished `release` profile [optimized] target(s) in 0.30s
     Running `target/release/mrs --time 480 /home/hack/TPTP-v9.2.1/Problems/GRP/GRP678-1.p`
% Problem: GRP678-1 (0 axioms, 0 conjectures, 13 cnf clauses)
% SZS status Timeout for GRP678-1
% ------------------------------
% Version: mrs 0.1.8
% Termination reason: Timeout
% Time elapsed: 480.382 s
% Peak memory usage: 258 MB
% ------------------------------

Results for mrs commit ec266ecb29b1b7db1c37341c137a41e7e9e11505

hack@pve:~/mrs$ crates/mrs-bench/casc.sh --systems mrs  --jobs 4
CASC-30 Results — 2026-06-01 06:05  (1101 problems × 1 systems)
===============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
FNE            100        24   13.099
FEQ            400        17   21.917
EPU            100         8    3.417
EPS            100         0    0.000
UEQ            300        17   32.421
ICU            101         1    0.323
------------------  --------------------
TOTAL         1101        67   18.892

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

Results for mrs commit 40b17e46103c71888d09be3ec59430a92c724008

[PPROD:fr22192@tlpnf9701:/DATA/ai/fr22192/mrs]$ crates/mrs-bench/casc.sh --systems mrs,vampire,eprover --jobs 30
CASC-30 Results — 2026-06-01 06:28  (1101 problems × 3 systems)
===============================================================

Division  Problems    eprover               mrs                   vampire
                      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)
------------------  --------------------  --------------------  --------------------
FNE            100        65   12.337          24   13.759          75   10.169
FEQ            400       223   12.913          16   18.979         348    7.484
EPU            100        22    5.084           8    3.622          67   20.728
EPS            100        63    4.745           0    0.000          86    7.600
UEQ            300       166   14.653          17   38.830         227   22.991
ICU            101        12   36.898           1    0.254          37   32.268
------------------  --------------------  --------------------  --------------------
TOTAL         1101       551   12.645          66   20.049         840   14.074

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

hack@pve:~/mrs$ crates/mrs-bench/casc.sh --systems mrs,vampire,eprover --jobs 4
     Running `target/debug/bench_report /home/hack/mrs/crates/mrs-bench/results/casc-30/20260529_195037/run.csv`
CASC-30 Results — 2026-05-30 12:40  (1101 problems × 3 systems)
===============================================================

Division  Problems    eprover               mrs                   vampire
                      Solved  Avg (s)      Solved  Avg (s)      Solved  Avg (s)
------------------  --------------------  --------------------  --------------------
FNE            100        63   11.175          23   14.392          71    7.591
FEQ            400       221   13.045          15   19.771         327    8.069
EPU            100        22    4.471           8    3.813          62   20.040
EPS            100        63    5.406           0    0.000          85    6.895
UEQ            300       166   14.856          16   38.827         195   17.546
ICU            101        12   36.522           1    0.400          27   16.116
------------------  --------------------  --------------------  --------------------
TOTAL         1101       547   12.670          63   20.313         767   11.555

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

Results for mrs commit 9f386fff39b43a4c77a631e066080524bc74231f

to be analyzed
hack@pve:~/mrs$ cargo run --release --bin mrs -- --time 480 ~/TPTP-v9.2.1/Problems/GRP/GRP751-1.p
    Finished `release` profile [optimized] target(s) in 0.27s
     Running `target/release/mrs --time 480 /home/hack/TPTP-v9.2.1/Problems/GRP/GRP751-1.p`
% Problem: GRP751-1 (0 axioms, 0 conjectures, 8 cnf clauses)
% SZS status Timeout for GRP751-1
% ------------------------------
% Version: mrs 0.1.10
% Termination reason: Timeout
% Time elapsed: 480.533 s
% Peak memory usage: 229 MB
% ------------------------------

Results for mrs commit 4345265f468dc6038471ada870239dbe9c8edec0

[PPROD:fr22192@tlpnf9701:/DATA/ai/fr22192/mrs]$ crates/mrs-bench/casc.sh --systems reference,mrs --jobs 30 --time 240
     Running `target/debug/bench_report /DATA/ai/fr22192/mrs/crates/mrs-bench/results/casc-30/20260529_153630/run.csv`
CASC-30 Results — 2026-05-29 15:49  (1101 problems × 2 systems)
===============================================================

Division  Problems    mrs                   reference
                      Solved  Avg (s)      Solved  Avg (s)
------------------  --------------------  --------------------
FNE            100        26   27.331         100    0.032
FEQ            400        16   41.223         399    0.031
EPU            100         8    5.083         100    0.032
EPS            100         0    0.000         100    0.033
UEQ            300        26   50.078         300    0.031
ICU            101         2   17.092          57    0.031
------------------  --------------------  --------------------
TOTAL         1101        78   35.219        1056    0.031

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

[PPROD:fr22192@tlpnf9701:/DATA/ai/fr22192/mrs]$ crates/mrs-bench/casc.sh --systems reference,mrs --jobs 30
CASC-30 Results — 2026-05-29 12:17  (1101 problems × 2 systems)
===============================================================

Division  Problems    mrs                   reference
                      Solved  Avg (s)      Solved  Avg (s)
------------------  --------------------  --------------------
FNE            100        24   13.303         100    0.032
FEQ            400        16   21.884         399    0.031
EPU            100         8    3.534         100    0.032
EPS            100         0    0.000         100    0.033
UEQ            300        17   37.019         300    0.031
ICU            101         1    0.363          57    0.032
------------------  --------------------  --------------------
TOTAL         1101        66   20.112        1056    0.032

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

hack@pve:~/mrs$ TPTP=/home/hack/mrs/crates/mrs-bench/problems/casc-30 crates/mrs-bench/casc.sh --systems reference,mrs --jobs 3 --divisions ueq
CASC-30 Results — 2026-05-29 12:21  (300 problems × 2 systems)
==============================================================

Division  Problems    mrs                   reference
                      Solved  Avg (s)      Solved  Avg (s)
------------------  --------------------  --------------------
UEQ            300        18   32.820         300    0.014
------------------  --------------------  --------------------
TOTAL          300        18   32.820         300    0.014

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

Results for mrs commit df21f4b06de8483420a0ff72aef7f2e3129dcc52

[root@mtsdev02 mrs]# TPTP=/mnt/sda1/mrs/crates/mrs-bench/problems/casc-30 crates/mrs-bench/casc.sh --systems reference,mrs
CASC-30 Results — 2026-06-01 06:13  (1101 problems × 2 systems)
===============================================================

Division  Problems    mrs                   reference
                      Solved  Avg (s)      Solved  Avg (s)
------------------  --------------------  --------------------
FNE            100        26   19.243           0    0.000
FEQ            400        16   20.713           0    0.000
EPU            100         8    3.019           0    0.000
EPS            100         0    0.000           0    0.000
UEQ            300        21   29.076           0    0.000
ICU            101         2   47.490           0    0.000
------------------  --------------------  --------------------
TOTAL         1101        73   21.390           0    0.000

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

Results for mrs commit b4b78ac62853b7903d64ab6654b2e7e86e426dd2

[www@teenf9901 mrs]$ TPTP=/DATA/ai/mrs/crates/mrs-bench/problems/casc-30 crates/mrs-bench/casc.sh --systems reference,mrs
CASC-30 Results — 2026-06-01 06:32  (1101 problems × 2 systems)
===============================================================

Division  Problems    mrs                   reference
                      Solved  Avg (s)      Solved  Avg (s)
------------------  --------------------  --------------------
FNE            100        25   15.355         100    0.019
FEQ            400        17   21.700         399    0.019
EPU            100         8    2.729         100    0.019
EPS            100         0    0.000         100    0.019
UEQ            300        21   25.095         300    0.019
ICU            101         2    9.045          57    0.019
------------------  --------------------  --------------------
TOTAL         1101        73   18.078        1056    0.019

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

Results for mrs commit a5b220380d6a24234c59275a188a4f6a948f7160

[www@teenf9901 mrs]$ TPTP=/DATA/ai/mrs/crates/mrs-bench/problems/casc-30 crates/mrs-bench/casc.sh --systems reference,mrs --divisions fne
Division  Problems    mrs                   reference
                      Solved  Avg (s)      Solved  Avg (s)
------------------  --------------------  --------------------
FNE            100        26   13.872         100    0.019
------------------  --------------------  --------------------
TOTAL          100        26   13.872         100    0.019

DISAGREEMENTS — 3 problem(s) where systems gave contradictory answers:
  FNE     LCL660+1.015                    mrs=CounterSatisfiable  reference=Theorem  ⚠ SOUNDNESS
  FNE     SYN938+1                        mrs=CounterSatisfiable  reference=Theorem  ⚠ SOUNDNESS
  FNE     SYN986+1.004                    mrs=CounterSatisfiable  reference=Theorem  ⚠ SOUNDNESS

POLARITY VIOLATIONS — none detected.

Results for mrs commit c6eb579c47b11726154c1eaa6c215c1edc21ea2c

[root@mtsdev02 mrs]# TPTP=/mnt/sda1/mrs/crates/mrs-bench/problems/casc-30 crates/mrs-bench/casc.sh --systems reference,mrs
CASC-30 Results — 2026-05-28 16:26  (1101 problems × 2 systems)
===============================================================

Division  Problems    mrs                   reference
                      Solved  Avg (s)      Solved  Avg (s)
------------------  --------------------  --------------------
FNE            100        25   11.442           0    0.000
FEQ            400       208   11.069           0    0.000
EPU            100        11   12.286           0    0.000
EPS            100        18    9.831           0    0.000
UEQ            300        26   12.263           0    0.000
ICU            101        40   15.654           0    0.000
------------------  --------------------  --------------------
TOTAL         1101       328   11.724           0    0.000

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — 13 case(s) of wrong SZS polarity:
  EPU     SYN885-1                        mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  EPS     NLP006-1                        mrs=Unsatisfiable  (expected one of ["CounterSatisfiable", "Satisfiable"])  ⚠ UNSOUND
  EPS     NLP008-1                        mrs=Unsatisfiable  (expected one of ["CounterSatisfiable", "Satisfiable"])  ⚠ UNSOUND
  EPS     NLP012-1                        mrs=Unsatisfiable  (expected one of ["CounterSatisfiable", "Satisfiable"])  ⚠ UNSOUND
  UEQ     LAT080-1                        mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  UEQ     LAT081-1                        mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  UEQ     LAT082-1                        mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  UEQ     LAT084-1                        mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  UEQ     LAT085-1                        mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  UEQ     LAT092-1                        mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  UEQ     LAT096-1                        mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  UEQ     LAT097-1                        mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  UEQ     LAT392-1                        mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND

Results for mrs commit 38f023ef0e0319226bf659fc5dc9f53511eb81c2

[www@teenf9901 mrs]$ TPTP=/DATA/ai/mrs/crates/mrs-bench/problems/casc-30 crates/mrs-bench/casc.sh --systems reference,mrs
CASC-30 Results — 2026-05-28 08:03  (1101 problems × 2 systems)
===============================================================

Division  Problems    mrs                   reference
                      Solved  Avg (s)      Solved  Avg (s)
------------------  --------------------  --------------------
FNE            100        16    9.517         100    0.019
FEQ            400        37    5.110         399    0.019
EPU            100        11   10.095         100    0.019
EPS            100        14    3.182         100    0.019
UEQ            300        19   34.888         300    0.019
ICU            101         7    0.058          57    0.019
------------------  --------------------  --------------------
TOTAL         1101       104   11.156        1056    0.019

DISAGREEMENTS — 3 problem(s) where systems gave contradictory answers:
  EPS     NLP006-1                        mrs=Unsatisfiable  reference=Satisfiable  ⚠ SOUNDNESS
  EPS     NLP008-1                        mrs=Unsatisfiable  reference=Satisfiable  ⚠ SOUNDNESS
  ICU     VVA001+1                        mrs=Theorem  reference=CounterSatisfiable  ⚠ SOUNDNESS

POLARITY VIOLATIONS — 2 case(s) of wrong SZS polarity:
  EPS     NLP006-1                        mrs=Unsatisfiable  (expected one of ["CounterSatisfiable", "Satisfiable"])  ⚠ UNSOUND
  EPS     NLP008-1                        mrs=Unsatisfiable  (expected one of ["CounterSatisfiable", "Satisfiable"])  ⚠ UNSOUND


hack@pve:~/mrs$ TPTP=/home/hack/mrs/crates/mrs-bench/problems/casc-30 crates/mrs-bench/casc.sh --systems reference,mrs --divisions eps,epu,feq
CASC-30 Results — 2026-05-28 10:44  (600 problems × 2 systems)
==============================================================

Division  Problems    mrs                   reference
                      Solved  Avg (s)      Solved  Avg (s)
------------------  --------------------  --------------------
EPS            100        15    3.939         100    0.011
EPU            100        11    4.952         100    0.011
FEQ            400        64    1.794         399    0.011
------------------  --------------------  --------------------
TOTAL          600        90    2.537         599    0.011

DISAGREEMENTS — 3 problem(s) where systems gave contradictory answers:
  EPS     NLP006-1                        mrs=Unsatisfiable  reference=Satisfiable  ⚠ SOUNDNESS
  EPS     NLP008-1                        mrs=Unsatisfiable  reference=Satisfiable  ⚠ SOUNDNESS
  EPS     NLP012-1                        mrs=Unsatisfiable  reference=Satisfiable  ⚠ SOUNDNESS

POLARITY VIOLATIONS — 3 case(s) of wrong SZS polarity:
  EPS     NLP006-1                        mrs=Unsatisfiable  (expected one of ["CounterSatisfiable", "Satisfiable"])  ⚠ UNSOUND
  EPS     NLP008-1                        mrs=Unsatisfiable  (expected one of ["CounterSatisfiable", "Satisfiable"])  ⚠ UNSOUND
  EPS     NLP012-1                        mrs=Unsatisfiable  (expected one of ["CounterSatisfiable", "Satisfiable"])  ⚠ UNSOUND

hack@pve:~/mrs$ TPTP=/home/hack/mrs/crates/mrs-bench/problems/casc-30 crates/mrs-bench/casc.sh --systems reference,mrs --divisions fne,ueq
[casc] Done. Results: /home/hack/mrs/crates/mrs-bench/results/casc-30/20260527_161327/run.csv
hack@pve:~/mrs$
────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────
==============================================================

Division  Problems    mrs                   reference
                      Solved  Avg (s)      Solved  Avg (s)
------------------  --------------------  --------------------
FNE            100        19    4.990         100    0.011
UEQ            300        15   21.399         300    0.011
------------------  --------------------  --------------------
TOTAL          400        34   12.229         400    0.011

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — none detected.

Results for mrs commit 4c7418c39df5b1ea489d560d4b5fb8a97f5836f0
[www@teenf9901 mrs]$ cargo run -p mrs-bench --bin bench_report -- /DATA/ai/mrs/crates/mrs-bench/results/casc-30/20260527_095507/run.csv
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.03s
     Running `target/debug/bench_report /DATA/ai/mrs/crates/mrs-bench/results/casc-30/20260527_095507/run.csv`
CASC-30 Results — 2026-05-27 09:43  (100 problems × 1 systems)
==============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
FNE            100         7    5.715
------------------  --------------------
TOTAL          100         7    5.715


Results for mrs commit d9dacb516c0cf5c17c5eda34d7b67436057ac138

mtsdev02 partial [casc] 586/1101 completed
CASC-30 Results — 2026-05-28 06:42  (1101 problems × 1 systems)
===============================================================

Division  Problems    mrs
                      Solved  Avg (s)
------------------  --------------------
FNE            100         7    5.954
FEQ            400        63    3.385
EPU            100         8    3.063
EPS            100        35    3.533
UEQ            300        20   18.862
ICU            101         9    1.570
------------------  --------------------
TOTAL         1101       142    5.595

DISAGREEMENTS — none detected.

POLARITY VIOLATIONS — 8 case(s) of wrong SZS polarity:
  EPU     MSC024-1                        mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  UEQ     LCL195-10                       mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  UEQ     LCL203-10                       mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  UEQ     LCL211-10                       mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  UEQ     LCL224-10                       mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  UEQ     LCL416-10                       mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  UEQ     NUM284-10.014                   mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  UEQ     RNG001-10                       mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND


[www@teenf9901 mrs]$ ./crates/mrs-bench/systems/vampire/bin/vampire --version
Vampire 5.0.1 (Release build, commit cb4838130 on 2026-05-26 10:04:53 +0200)
CaDiCaL: cadical-2.1.3
Linked to Z3 4.14.0.0 3c47fd96cf5645d0c42b2c819d9e9a84380aa721 z3-4.8.4-9178-g3c47fd96c
[www@teenf9901 mrs]$ TPTP=/DATA/ai/mrs/crates/mrs-bench/problems/casc-30 crates/mrs-bench/casc.sh --systems mrs,vampire --divisions fne,ueq
[www@teenf9901 mrs]$ cargo run -p mrs-bench --bin bench_report -- /DATA/ai/mrs/crates/mrs-bench/results/casc-30/20260526_191646/run.csv
   Compiling mrs-bench v0.1.1 (/DATA/ai/mrs/crates/mrs-bench)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.68s
     Running `target/debug/bench_report /DATA/ai/mrs/crates/mrs-bench/results/casc-30/20260526_191646/run.csv`
CASC-30 Results — 2026-05-27 06:16  (400 problems × 2 systems)
==============================================================

Division  Problems    mrs                   vampire
                      Solved  Avg (s)      Solved  Avg (s)
------------------  --------------------  --------------------
FNE            100         7    5.677          79   10.925
UEQ            300        22   28.067         233   16.129
------------------  --------------------  --------------------
TOTAL          400        29   22.663         312   14.812

DISAGREEMENTS — 8 problem(s) where systems gave contradictory answers:
  FNE     MGT067+1                        mrs=CounterSatisfiable  vampire=Theorem  ⚠ SOUNDNESS
  FNE     SYN457+1                        mrs=CounterSatisfiable  vampire=Theorem  ⚠ SOUNDNESS
  FNE     SYO606+1                        mrs=CounterSatisfiable  vampire=Theorem  ⚠ SOUNDNESS
  UEQ     LCL195-10                       mrs=Satisfiable  vampire=Unsatisfiable  ⚠ SOUNDNESS
  UEQ     LCL203-10                       mrs=Satisfiable  vampire=Unsatisfiable  ⚠ SOUNDNESS
  UEQ     LCL211-10                       mrs=Satisfiable  vampire=Unsatisfiable  ⚠ SOUNDNESS
  UEQ     LCL224-10                       mrs=Satisfiable  vampire=Unsatisfiable  ⚠ SOUNDNESS
  UEQ     RNG001-10                       mrs=Satisfiable  vampire=Unsatisfiable  ⚠ SOUNDNESS

POLARITY VIOLATIONS — 6 case(s) of wrong SZS polarity:
  UEQ     LCL195-10                       mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  UEQ     LCL203-10                       mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  UEQ     LCL211-10                       mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  UEQ     LCL224-10                       mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  UEQ     NUM284-10.014                   mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  UEQ     RNG001-10                       mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND

hack@pve:~/mrs$ ./crates/mrs-bench/systems/vampire/bin/vampire --version
Vampire 5.0.1 (Release build, commit 1b13eaf on 2026-01-18 12:14:50 +0000)
CaDiCaL: cadical-2.1.3
Linked to Z3 4.14.0.0 3c47fd96cf5645d0c42b2c819d9e9a84380aa721 NOTFOUND
hack@pve:~/mrs$ TPTP=/home/hack/mrs/crates/mrs-bench/problems/casc-30 crates/mrs-bench/casc.sh --systems mrs,vampire --divisions fne,ueq
hack@pve:~/mrs$ cargo run -p mrs-bench --bin bench_report -- /home/hack/mrs/crates/mrs-bench/results/casc-30/20260526_192617/run.csv
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.03s
     Running `target/debug/bench_report /home/hack/mrs/crates/mrs-bench/results/casc-30/20260526_192617/run.csv`
CASC-30 Results — 2026-05-27 06:08  (400 problems × 2 systems)
==============================================================

Division  Problems    mrs                   vampire
                      Solved  Avg (s)      Solved  Avg (s)
------------------  --------------------  --------------------
FNE            100        34    0.494          75    6.086
UEQ            300        21   21.547         216   16.426
------------------  --------------------  --------------------
TOTAL          400        55    8.533         291   13.761

DISAGREEMENTS — 32 problem(s) where systems gave contradictory answers:
  FNE     CSR026+3                        mrs=CounterSatisfiable  vampire=Theorem  ⚠ SOUNDNESS
  FNE     CSR027+3                        mrs=CounterSatisfiable  vampire=Theorem  ⚠ SOUNDNESS
  FNE     CSR033+3                        mrs=CounterSatisfiable  vampire=Theorem  ⚠ SOUNDNESS
  FNE     CSR034+2                        mrs=CounterSatisfiable  vampire=Theorem  ⚠ SOUNDNESS
  FNE     CSR036+2                        mrs=CounterSatisfiable  vampire=Theorem  ⚠ SOUNDNESS
  FNE     CSR036+3                        mrs=CounterSatisfiable  vampire=Theorem  ⚠ SOUNDNESS
  FNE     CSR039+2                        mrs=CounterSatisfiable  vampire=Theorem  ⚠ SOUNDNESS
  FNE     CSR040+3                        mrs=CounterSatisfiable  vampire=Theorem  ⚠ SOUNDNESS
  FNE     CSR052+3                        mrs=CounterSatisfiable  vampire=Theorem  ⚠ SOUNDNESS
  FNE     CSR056+3                        mrs=CounterSatisfiable  vampire=Theorem  ⚠ SOUNDNESS
  FNE     CSR060+2                        mrs=CounterSatisfiable  vampire=Theorem  ⚠ SOUNDNESS
  FNE     CSR061+2                        mrs=CounterSatisfiable  vampire=Theorem  ⚠ SOUNDNESS
  FNE     CSR073+2                        mrs=CounterSatisfiable  vampire=Theorem  ⚠ SOUNDNESS
  FNE     CSR073+3                        mrs=CounterSatisfiable  vampire=Theorem  ⚠ SOUNDNESS
  FNE     CSR115+31                       mrs=CounterSatisfiable  vampire=Theorem  ⚠ SOUNDNESS
  FNE     CSR115+6                        mrs=CounterSatisfiable  vampire=Theorem  ⚠ SOUNDNESS
  FNE     CSR115+91                       mrs=CounterSatisfiable  vampire=Theorem  ⚠ SOUNDNESS
  FNE     CSR115+98                       mrs=CounterSatisfiable  vampire=Theorem  ⚠ SOUNDNESS
  FNE     CSR116+27                       mrs=CounterSatisfiable  vampire=Theorem  ⚠ SOUNDNESS
  FNE     CSR116+39                       mrs=CounterSatisfiable  vampire=Theorem  ⚠ SOUNDNESS
  FNE     CSR116+6                        mrs=CounterSatisfiable  vampire=Theorem  ⚠ SOUNDNESS
  FNE     LCL642+1.010                    mrs=CounterSatisfiable  vampire=Theorem  ⚠ SOUNDNESS
  FNE     LCL642+1.015                    mrs=CounterSatisfiable  vampire=Theorem  ⚠ SOUNDNESS
  FNE     LCL660+1.015                    mrs=CounterSatisfiable  vampire=Theorem  ⚠ SOUNDNESS
  FNE     MGT067+1                        mrs=CounterSatisfiable  vampire=Theorem  ⚠ SOUNDNESS
  FNE     NLP262+1                        mrs=CounterSatisfiable  vampire=Theorem  ⚠ SOUNDNESS
  FNE     SYN457+1                        mrs=CounterSatisfiable  vampire=Theorem  ⚠ SOUNDNESS
  FNE     SYN938+1                        mrs=CounterSatisfiable  vampire=Theorem  ⚠ SOUNDNESS
  UEQ     LCL195-10                       mrs=Satisfiable  vampire=Unsatisfiable  ⚠ SOUNDNESS
  UEQ     LCL203-10                       mrs=Satisfiable  vampire=Unsatisfiable  ⚠ SOUNDNESS
  UEQ     LCL211-10                       mrs=Satisfiable  vampire=Unsatisfiable  ⚠ SOUNDNESS
  UEQ     LCL224-10                       mrs=Satisfiable  vampire=Unsatisfiable  ⚠ SOUNDNESS

POLARITY VIOLATIONS — 7 case(s) of wrong SZS polarity:
  UEQ     LCL195-10                       mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  UEQ     LCL203-10                       mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  UEQ     LCL211-10                       mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  UEQ     LCL224-10                       mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  UEQ     LCL416-10                       mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  UEQ     NUM284-10.014                   mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
  UEQ     RNG001-10                       mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND

[root@mtsdev02 mrs]# TPTP=/mnt/sda1/mrs/crates/mrs-bench/problems/casc-30 crates/mrs-bench/casc.sh --systems mrs
POLARITY VIOLATIONS — 1 case(s) of wrong SZS polarity:
  EPU     MSC024-1                        mrs=Satisfiable  (expected one of ["Unsatisfiable"])  ⚠ UNSOUND
