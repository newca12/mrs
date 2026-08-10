# StarExec & SystemOnTPTP Packaging Guide

This document explains how to build, package, and deploy `mrs` (superposition prover) and `mrs-proover` (proof verifier) as ZIP archives for **StarExec** and as installed wrapper/binary pairs for **SystemOnTPTP**.

---

## 1. Multi-Platform Compatibility Guidelines

### 1.1 The GLIBC Compatibility Trap
Both StarExec and SystemOnTPTP run Linux environments whose system libraries may differ from the build machine. Compiling on a newer Ubuntu system can produce a binary that requires a newer glibc than the deployment host. Deploying it to an older host can cause the dynamic linker to fail before the program starts, for example:
```text
/lib64/libc.so.6: version `GLIBC_2.38' not found
```
To prevent this compatibility issue, build on an Ubuntu machine or container with a glibc version no newer than the target host. For a fully static build, use a configured `x86_64-unknown-linux-musl` toolchain instead.

### 1.2 The AVX2 ISA Requirement
By default, this repository's development configurations set:
```toml
# .cargo/config.toml
[build]
rustflags = ["-C", "target-cpu=native"]
```
This allows the compiler to use the AVX2/FMA/BMI instruction set available on Haswell-class CPUs, including 256-bit SIMD instructions in the superposition and term-indexing code.

However, if the host CPU or VM hypervisor on the target platform (e.g. SystemOnTPTP's container) does not expose AVX2 instructions, the binary can crash with a `SIGILL` (Illegal Instruction) signal and exit status `132` when execution reaches code paths compiled with those instructions, typically during search.

**To produce a portable and safe production release on Ubuntu, explicitly compile with a conservative target CPU:**
```bash
# Use the AVX2-capable Haswell baseline (see below for justification)
env RUSTFLAGS="-C target-cpu=haswell" cargo build --release --bin mrs

# Force a fully generic x86_64 target (works on essentially all 64-bit AMD/Intel chips)
env RUSTFLAGS="-C target-cpu=x86-64" cargo build --release --bin mrs
```

#### Why Use `target-cpu=haswell` instead of `target-cpu=broadwell`?
While the official CASC-J13 server nodes use **Broadwell** CPUs, compiling with `target-cpu=haswell` is the recommended and safest practice for several reasons:

1.  **Instruction Set Compatibility:** Broadwell includes the Haswell AVX, AVX2, FMA3, and BMI instruction baseline. LLVM models Broadwell as Haswell's features plus `ADX`, `RDSEED`, and `PRFCHW` (the `PREFETCHW` instruction). `mrs` does not require those Broadwell-only extensions.
2.  **The Relevant SIMD Code Is Available:** In LLVM's x86 target descriptions, Broadwell features are defined as:
    ```tablegen
    list<SubtargetFeature> BDWFeatures = !listconcat(HSWFeatures, BDWAdditionalFeatures);
    ```
    A Haswell target therefore enables the SIMD instruction set needed by the indexing and subsumption loops and runs on Broadwell without relying on Broadwell-only instructions. The two LLVM targets still have different processor scheduling models, so generated code and performance are not guaranteed to be byte-for-byte or cycle-for-cycle identical.
3.  **Maximum Portability:** Using `haswell` avoids generating Broadwell-only instructions, so the binary also remains compatible with Haswell-generation evaluation servers. It is a conservative performance/portability compromise, not a claim that the two microarchitectures are identical.

The LLVM target definitions are available at:
<https://github.com/llvm/llvm-project/blob/main/llvm/lib/Target/X86/X86.td>

---

## 2. Shared Calling Convention Support

The repository's official Bash wrappers support **both** StarExec and SystemOnTPTP invocation formats automatically. StarExec uses the filename `starexec_run_default`; SystemOnTPTP's registered command is normally named `run_mrs` and should be a copy or rename of that same MRS wrapper, not the separate tcsh metadata/postprocessing launcher.

### 2.1 The Two Execution Environments
1.  **StarExec:** Calls `starexec_run_default` with **1** positional argument (problem path) and exports the time limit via an environment variable:
    ```bash
    ./starexec_run_default '/starexec/sandbox/problem_123.p'  # STAREXEC_WALLCLOCK_LIMIT=240
    ```
2.  **SystemOnTPTP:** The registered command should be `run_mrs %s %d`; it calls the copied wrapper with **2** positional arguments (problem path and time limit as an integer):
    ```bash
     ./run_mrs '/tmp/SOT_file' 60
    ```

### 2.2 The Unified Wrapper Fallback Logic
The wrappers use the following Bash parameter expansion to resolve the final wall-clock limit:
```bash
WALLCLOCK="${2:-${STAREXEC_WALLCLOCK_LIMIT:-240}}"
```
This evaluates to:
*   `$2` (the second positional argument) if provided by SystemOnTPTP.
*   `STAREXEC_WALLCLOCK_LIMIT` if provided by StarExec.
*   A conservative default (`240` for `mrs`, `30` for `mrs-proover`) if neither is present.

---

## 3. Package Structures

### 3.1 `mrs` (Superposition Prover) Package
The StarExec ZIP contains these files in a flat structure under `bin/`:

```text
bin/
├── mrs                     # Compiled superposition binary (release target)
└── starexec_run_default    # The official Bash wrapper script (with unified WALLCLOCK fallback)
```

The `starexec_run_default` wrapper raises the main-thread stack limit and configures `RUST_MIN_STACK` to reduce recursive search-thread stack-overflow risk. Current proof formatting quotes generated non-word identifiers in the Rust serializer, so `sanitize_tstp.sh` is not required for a current package. The tracked script remains available at `crates/mrs-bench/sanitize_tstp.sh` for repairing archived legacy proofs.

For SystemOnTPTP, install the same two files beside one another and name the wrapper `run_mrs` because that is the registered command:

```text
/home/tptp/Systems/mrs---0.2.0/
├── mrs
└── run_mrs
```

### 3.2 `mrs-proover` (Proof Verifier) Package
The StarExec verifier ZIP contains the main verifier binary along with its optional external ATP backends:

```text
bin/
├── mrs-proover             # Compiled verifier binary (release target)
├── starexec_run_default    # The official Bash wrapper script (with unified WALLCLOCK fallback)
├── eprover/
│   └── bin/
│       └── eprover         # Optional bundled eprover binary
└── vampire/
    └── bin/
        └── vampire         # Optional bundled vampire binary
```

The `starexec_run_default` wrapper automatically discovers the bundled `eprover` and `vampire` binaries, configuring them on `mrs-proover`'s semantic verification ladder to achieve high-performance verification. If those optional backends are absent, the verifier still runs with its available backends.

---

## 4. Ubuntu Build And Packaging

Run these commands from the repository root on Ubuntu. Use the same target CPU and compatible Ubuntu/glibc baseline for every native binary in a package.

### 4.1 Build `mrs`

```bash
export RUSTFLAGS="-C target-cpu=haswell"
cargo build --release --bin mrs
```

The binary is written to `target/release/mrs`. Use `target-cpu=x86-64` instead when the package must run on CPUs without AVX2.

### 4.2 Build `mrs-proover`

```bash
export RUSTFLAGS="-C target-cpu=haswell"
cargo build --release -p mrs-proover --bin mrs-proover
```

The binary is written to `target/release/mrs-proover`. Copy compatible `eprover` and `vampire` binaries into the paths shown above if the full verification ladder is required.

### 4.3 Create And Check The `mrs` ZIP

```bash
rm -rf /tmp/mrs-starexec
mkdir -p /tmp/mrs-starexec/bin
cp target/release/mrs /tmp/mrs-starexec/bin/mrs
cp crates/mrs-bench/systems/mrs/starexec_run_default /tmp/mrs-starexec/bin/starexec_run_default
chmod 755 /tmp/mrs-starexec/bin/mrs /tmp/mrs-starexec/bin/starexec_run_default
ZIP_PATH="$PWD/mrs-starexec.zip"
(cd /tmp/mrs-starexec && zip -qr "$ZIP_PATH" bin)
unzip -l "$ZIP_PATH"
```

Smoke-test the extracted StarExec layout with a self-contained TPTP problem:

```bash
unzip -q "$ZIP_PATH" -d /tmp/mrs-starexec-test
STAREXEC_WALLCLOCK_LIMIT=10 \
  /tmp/mrs-starexec-test/bin/starexec_run_default /path/to/problem.p
```

### 4.4 Create And Check The `mrs-proover` ZIP

```bash
rm -rf /tmp/mrs-proover-starexec
mkdir -p /tmp/mrs-proover-starexec/bin
cp target/release/mrs-proover /tmp/mrs-proover-starexec/bin/mrs-proover
cp crates/mrs-bench/systems/mrs-proover/starexec_run_default \
  /tmp/mrs-proover-starexec/bin/starexec_run_default
chmod 755 /tmp/mrs-proover-starexec/bin/mrs-proover \
  /tmp/mrs-proover-starexec/bin/starexec_run_default

for backend in eprover vampire; do
  source="crates/mrs-bench/systems/${backend}/bin/${backend}"
  if [[ -x "$source" ]]; then
    mkdir -p "/tmp/mrs-proover-starexec/bin/${backend}/bin"
    cp "$source" "/tmp/mrs-proover-starexec/bin/${backend}/bin/${backend}"
    chmod 755 "/tmp/mrs-proover-starexec/bin/${backend}/bin/${backend}"
  fi
done

ZIP_PATH="$PWD/mrs-proover-starexec.zip"
(cd /tmp/mrs-proover-starexec && zip -qr "$ZIP_PATH" bin)
unzip -l "$ZIP_PATH"
```

Smoke-test the extracted verifier layout with a proof file:

```bash
unzip -q "$ZIP_PATH" -d /tmp/mrs-proover-starexec-test
STAREXEC_WALLCLOCK_LIMIT=30 \
  /tmp/mrs-proover-starexec-test/bin/starexec_run_default /path/to/proof.e
```

### 4.5 Install For SystemOnTPTP

SystemOnTPTP does not use the StarExec `bin/` name as its command. Install the same wrapper under `run_mrs`, beside `mrs`, and register/configure:

```text
run_mrs %s %d
```

The resulting command is:

```bash
/home/tptp/Systems/mrs---0.2.0/run_mrs /path/to/problem.p 60
```

### 4.6 Smoke-Test Both Calling Conventions

```bash
# StarExec convention
STAREXEC_WALLCLOCK_LIMIT=10 \
  ./bin/starexec_run_default /path/to/problem.p

# SystemOnTPTP convention
./bin/starexec_run_default /path/to/problem.p 10
```

Both commands should emit an SZS status line. A `SIGILL` exit status (`132`) indicates that the binary was built for ISA extensions unavailable on the host; rebuild with `target-cpu=x86-64` or another target supported by that host.
