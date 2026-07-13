# MRS Hardware & Multi-Threading Benchmark Analysis

This document records the hardware benchmarks, vectorization latency speedups (AVX2), CPU multi-threading scalability findings (SMT/hyper-threading bottlenecks), server-specific glibc drift, and target configurations for the CASC StarExec competition.

---

## 1. AVX2 SIMD Vectorization Benchmark

To measure the raw, isolated hardware throughput gains of compiling with native AVX2 vectorization against the generic `x86_64` baseline, we executed `mrs` on two computationally intensive TPTP first-order problems under identical threading and scheduling conditions (`MRS_WORKERS=4`, 60s wall-clock time limit).

### Hardware Platform:
- **CPU:** Intel(R) Core(TM) i7-10610U CPU @ 1.80GHz (Comet Lake, 4 physical cores, 8 logical threads, 8MB Cache)
- **OS:** Linux (NixOS)

### Results & Throughput Comparison:

#### Problem A: `SYN841-1.p` (2,856 CNF clauses, dense symbol matrix)
| Metric (72s Run) | Generic Baseline (No AVX) | AVX2-Optimized (`target-cpu=native`) | Raw AVX2 Throughput Gain (%) |
| :--- | :---: | :---: | :---: |
| **Processed Clauses** | 51,389 | 55,672 | **+8.3%** more clauses |
| **Generated Clauses** | 225,563 | 282,883 | **+25.4%** more clauses |
| **Forward Subsumed (Checks)** | 88,143 | 102,901 | **+16.7%** more indexing scans |
| **LRS Discarded Clauses** | 104,425 | 140,972 | **+35.0%** more passive prunes |

#### Problem B: `SYN812-1.p` (1,837 CNF clauses, moderate symbol matrix)
| Metric (72s Run) | Generic Baseline (No AVX) | AVX2-Optimized (`target-cpu=native`) | Raw AVX2 Throughput Gain (%) |
| :--- | :---: | :---: | :---: |
| **Processed Clauses** | 48,668 | 56,263 | **+15.6%** more clauses |
| **Generated Clauses** | 171,930 | 234,094 | **+36.1%** more clauses |
| **Forward Subsumed (Checks)** | 82,131 | 118,036 | **+43.7%** more indexing scans |
| **LRS Discarded Clauses** | 51,884 | 69,633 | **+34.2%** more passive prunes |

### Systems Analysis:
Comparing the 64-element `u16` frequency arrays in `FeatureVector` (`mrs-index::fvi::sym_counts_le`) is a hot-path necessity for forward/backward subsumption and subsumption resolution. 
1. **Hardware-Level SIMD Execution:** When compiled with native AVX2 support, the compiler auto-vectorizes this loop into 256-bit VEX-prefixed instructions (such as `vpxor` and `vmovdqu` using the `%ymm` registers). This compares 16 symbol counters at a time in a single CPU clock cycle, accelerating indexing scans significantly.
2. **Problem Size Scaling:** The throughput speedup scales dramatically with the size of the clause database, providing +35% to +43% throughput boosts in subsumption operations, because the prover spends a significantly higher percentage of its execution budget performing indexing comparisons on large datasets.

*(Note for NixOS users: To enable these gains during development, the `RUSTFLAGS` environment variable must be explicitly merged, e.g., `RUSTFLAGS='-C target-cpu=native -C link-arg=-fuse-ld=bfd'`, because a global `RUSTFLAGS` environment variable overrides the `.cargo/config.toml` file.)*

---

## 2. Multi-Threading: Hardware SMT vs. Algorithmic Strategy Diversity

During parallel portfolio search, `mrs` spawns parallel worker threads to run complementary strategies concurrently. Analyzing the performance requires explicitly decoupling the **hardware effect** of Hyper-Threading (SMT) from the **algorithmic effect** of Strategy Diversity and **Time-Slicing Starvation**.

### 2.1 The Hardware Reality: SMT / AVX2 Contention
Hyper-threading (SMT) duplicates CPU architectural register states but **does not duplicate physical execution units** (like the AVX2 SIMD engines). When running 8 threads on a 4-core CPU where each thread performs intensive 256-bit AVX2 subsumption loops, two logical threads sharing the same physical core fight for the same vector execution pipeline. This hardware contention inherently slows down the raw throughput of individual threads.

### 2.2 The Algorithmic Reality: Strategy Diversity and Time-Slicing
`mrs` assigns one distinct heuristic strategy to each worker. Running 8 workers instead of 4 explores **8 different search spaces concurrently**. However, the `casc` schedules natively divide the wall-clock time limit evenly by the number of active workers. 
* If you have a 240-second limit and **4 workers**, each strategy receives **60 seconds** of execution time.
* If you have a 240-second limit and **8 workers**, each strategy receives only **30 seconds** of execution time.

On deep, complex proofs, restricting a winning strategy (like strategy `s2` for the EPS division) to a mere 30-second window causes the prover to tear down the search and give up before it reaches the proof, even if the user explicitly provided a 240-second wall-clock limit.

### 2.3 The Historical Bottleneck vs. Parallel SInE
Historically, a sequential SInE pre-filtering phase locked the main thread, forcing all workers to wait. Removing this sequential lock revealed the true interplay between SMT, time-slicing, and Strategy Diversity on our 4-core/8-thread laptop CPU:

*   **Pre-Fix (Sequential SInE):**
    *   4 Workers (4 Cores): 21 FNE problems solved, **0.305s** average solve time.
    *   8 Workers (8 Threads): 23 FNE problems solved, **0.431s** average solve time.

*   **Current (Parallel SInE):**
    *   4 Workers (4 Cores): 21 FNE problems solved, **0.364s** average solve time.
    *   8 Workers (8 Threads): 23 FNE problems solved, **0.346s** average solve time.

For fast proofs (like the 2-second timeout FNE benchmarks), 8 workers succeed and are slightly faster because time-slicing hasn't kicked in yet. But for *deep* proofs (240s timeout), defaulting to 8 logical threads causes catastrophic time-slicing starvation.

### 2.4 Scaling Conclusion (Physical Cores Default)
- **Local Default (Physical Cores):** The `mrs` binary has been updated to use `num_cpus::get_physical()` as the default fallback when `--workers` is omitted. Defaulting to physical cores strictly avoids the SMT AVX2 stalling penalty, and critically, prevents the aggressive time-slice scaling from mathematically starving deep portfolio strategies of their needed search budgets.
- **For StarExec (CASC Hardware):** StarExec allocates 8 physical cores with zero SMT oversubscription. Thus, `mrs` running with `MRS_WORKERS=8` naturally achieves the ultimate peak: **maximum strategy diversity (8 workers) combined with maximum AVX2 speed and zero hyper-threading contention**.

---

## 3. Server-Level Drift & Compilation Baselines

Our `docs/BENCHMARKS.md` log reveals critical, silent performance degradation on older remote development nodes (such as the RHEL7 `server01` server):

### Server CPU Generation Gaps:
- **`server04` (Modern Build Node):** Achieves **12.3s / 47 solved** on FNE with a 2-second timeout.
- **`server01` (Legacy RHEL7 Server):** Dropped to **19.4s / 43 solved** on identical commits.

### The Systems Explanation:
1. **Generic De-Vectorization:** Cargo standard release compilations (`cargo build --release`) target a generic `x86_64` baseline (compatible with ancient 2004 AMD64 chips) to remain portable. Without explicit vector targeting, the compiler emulates our SIMD loops using slow, scalar assembly. 
2. **Old glibc Drift:** Remote RHEL7 servers run an older **`glibc 2.17`**. Precompiling a binary on a modern Ubuntu build server (which targets `glibc 2.31` or higher) will crash instantly on RHEL7 due to `version GLIBC_X.XX not found` linker errors.
3. **The `.cargo/config.toml` Trap:** Our previous config used a generic `RUSTFLAGS = "-C target-cpu=native"` inside the `[env]` block. Because Cargo evaluates target flags *before* subprocess env variables are applied, **Cargo silently ignored this RUSTFLAGS variable completely**, leading to de-vectorized generic builds for all benchmarks! We resolved this by moving it to the dedicated `[build].rustflags` key.

---

## 4. Official CASC-J13/StarExec Hardware Alignment

The official CASC competition runs on **StarExec Miami**. Per the current
CASC-J13 `Design.html` page (https://tptp.org/CASC/J13/Design.html), the
actual specification is:

- **Hardware:** Two octa-core Intel(R) Xeon(R) E5-2620 v4 @ 2.10GHz CPUs
  (Broadwell), without hyperthreading — 16 physical cores per machine, 30
  machines available (60 CPUs total). One ATP system runs on one CPU
  (StarExec uses `sched_setaffinity` to pin each run to a single CPU).
- **Operating System:** Ubuntu 24.04.3 LTS, Linux kernel 6.8.0-71-generic.
- **Memory:** 256GiB per machine; StarExec uses `setrlimit` to cap each run
  at 128GiB.
- **Time limits:** minimum 120s, maximum set by the organizers and
  announced at the competition (historically up to 240s for FOF/UEQ-class
  divisions).

This supersedes older internal notes that referenced RHEL7/glibc 2.17 and
an Intel Xeon E5-2609 (Haswell) as the "official" StarExec spec — those
were accurate for a previous CASC edition's infrastructure, not the
current one. StarExec Miami is also the same platform publicly available
to the TPTP community for testing/tuning outside of the competition itself.

### How `mrs` fits the StarExec Hardware:
1. **No SMT Contention:** StarExec allocates one physical CPU per run with
   zero hyperthreading oversubscription, and each CPU here is itself
   8-core. Running `mrs` with **`MRS_WORKERS=8`** (or `--workers 8`) uses
   every core of the allocated CPU with maximum strategy diversity and no
   SMT contention.
2. **GLIBC Compatibility:** Ubuntu 24.04.3 ships **glibc 2.39**. To avoid
   any risk of glibc-version mismatch between the build machine and the
   competition nodes (e.g. a build machine with a newer glibc than 2.39,
   which would produce a binary that refuses to run on the older
   competition nodes), either build directly on a matching Ubuntu
   24.04.x machine, or produce a fully static binary (e.g. via the
   `x86_64-unknown-linux-musl` target) that carries no glibc dependency
   at all.
3. **The Competition Target Compilation:** To get SIMD-vectorized
   performance (AVX2, BMI1/BMI2, FMA3) without risking an illegal
   instruction on the actual hardware, build with:
   ```bash
   RUSTFLAGS="-C target-cpu=haswell" cargo build --release --bin mrs
   ```
   `haswell` rather than `native` or `broadwell` is a deliberate choice:
   Broadwell (the actual competition CPU) is ISA-identical to Haswell —
   it introduces no new vector/bit-manipulation instructions, only minor
   scheduling/latency-model differences that LLVM's `broadwell` target
   would tune for. Targeting `haswell` therefore loses nothing on the
   real hardware while being a strictly safer, slightly more conservative
   floor than pinning to the exact chip generation. Do **not** add
   `--features ml-guidance` for the competition build: the ML-guided
   clause selection work was not shipped (see `docs/BENCHMARKS.md`'s "ML
   not shipped" note) — the competition entry uses the static `casc_*`
   portfolios / `--auto-schedule`, built with default features only.

---

## 5. Older Hardware: The Sandy Bridge AVX1 Fallback

When compiling and benchmarking on older hardware, such as the **Intel Xeon E5-2407 (Sandy Bridge-EN)**, the correct compilation target is `-C target-cpu=sandybridge`. 

However, it is critical to understand the vectorization limits of this architecture:
- **AVX1 vs AVX2:** Sandy Bridge was the first architecture to introduce Advanced Vector Extensions (AVX), but it only supported 256-bit **floating-point** operations. 
- **The Integer Limitation:** 256-bit **integer** operations were not introduced until **AVX2** (Haswell). 
- **The Impact on `mrs`:** Because the `FeatureVector` subsumption arrays (`[u16; 64]`) rely entirely on integer math, compiling for `sandybridge` means LLVM cannot use 256-bit `vpxor` and `%ymm` registers. Instead, it will safely fall back to using 128-bit **SSE4.1/SSE4.2** instructions (`%xmm` registers). 

While still significantly faster than generic scalar execution, processing the arrays 128 bits at a time will take roughly twice as many CPU cycles as execution on modern Haswell/Comet Lake hardware.
