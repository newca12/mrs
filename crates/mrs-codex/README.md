# mrs-codex

`mrs-codex` is a utility tool designed to process a large corpus of TPTP (`.p`) files using a theorem prover (like `mrs` or `vampire`) and store the results, execution times, hardware details, and parameters into a SQLite database.

## Features
* **Resumable Execution:** If the process is killed or interrupted, restarting the same command will safely skip already processed files matching the exact configuration and resume where it left off.
* **Parallel Processing:** Leverages `rayon` to evaluate multiple problems simultaneously across available CPU cores.
* **Strict Timeouts:** Wraps the prover execution in a wall-clock timeout using `wait-timeout` to prevent hanging on excessively hard problems.
* **Hardware Auto-detection:** Uses `sysinfo` to automatically detect and log CPU brand, core count, RAM, and OS details (overridable via the `--hardware` flag).
* **SZS Status Extraction:** Parses standard `% SZS status <Status>` output directly from the prover's stdout/stderr.
* **Explicit Proof Verification:** Use `--verify-mode kernel` for only the independent strict proof kernel, `--verify-mode competition` (the default) for the existing local and StarExec competition checks, or `--verify-mode none` to skip proof verification. Legacy validation columns remain populated for compatibility; new `kernel_*`, `mrs_*`, `competition_*`, and `external_atp_*` columns preserve separate verdicts and timings.
* **Deferred Proof Audits:** Import a later `audit_casc_proofs` report with `--import-proof-audit`. The importer verifies recorded artifact hashes, matches only existing CASC result rows, and updates verifier columns without replacing generation timings or statuses.
* **Normalized Database Schema:** Utilizes dedicated tables for `systems`, `hardware`, and `parameters` with foreign keys in the `results` table to ensure scalability and speed when dealing with millions of records.

## Example Usage

Here is the exact command to process the `problems/` directory in this repository using the `mrs` prover.

For the most accurate execution times, it is highly recommended to use the release build of the prover:

```bash
# 1. Build the prover in release mode first, along with mrs-proover
#    (used for the automatic proof-verification step, see above)
cargo build --release -p mrs -p mrs-proover

# 2. Run the codex tool to process the problems directory
cargo run --release -p mrs-codex -- /home/user/EDLA/git/mrs/problems \
  --db codex.db \
  --system mrs-0.2.3 \
  --timeout 30 \
  --cmd "./target/release/mrs {file}" \
  --verify-mode competition
```

## CASC results and deferred proof audits

`casc.sh` archives the exact prover stdout and stderr for every job under the
run directory's `raw/` tree. The CSV includes relative artifact paths and
SHA-256 hashes. This keeps CASC timing generation-only while allowing proof
verification to happen later without rerunning MRS.

Run and import the benchmark first:

```bash
RUN_DIR=crates/mrs-bench/results/casc-30/my-run

crates/mrs-bench/casc.sh \
  --edition casc-30 \
  --systems mrs \
  --divisions fne,feq,epu,eps,ueq,icu \
  --casc-times \
  --jobs 2 \
  --output "$RUN_DIR"

nix develop -c cargo run --release -p mrs-codex -- \
  --db codex-casc30.db \
  --import-casc "$RUN_DIR" \
  --problems-dir crates/mrs-bench/problems/casc-30 \
  --corpus casc-30
```

Replay the saved proofs later. The audit tool never invokes MRS. All selected
checks use the same normalized proof file:

```bash
nix develop -c cargo run --release -p mrs-bench --bin audit_casc_proofs -- \
  --run "$RUN_DIR" \
  --problems-dir crates/mrs-bench/problems/casc-30 \
  --checks strict,mrs,ladder \
  --strict-time 30 \
  --mrs-time 10 \
  --ladder-time 30 \
  --mrs-workers 1 \
  --ladder-workers 8 \
  --jobs 1 \
  --output "$RUN_DIR/proof-audit"
```

Checks are independently selectable, for example `--checks strict` or
`--checks ladder`. Re-running the audit with another selection reuses the
archived raw output and existing normalized proof.

Merge the audit later without creating duplicate CASC rows:

```bash
nix develop -c cargo run --release -p mrs-codex -- \
  --db codex-casc30.db \
  --import-proof-audit "$RUN_DIR/proof-audit/audit.csv"
```

The audit importer updates `kernel_*` for strict, `mrs_*` for MRS-only, and
`competition_*`/`external_atp_*` for the full ladder. It also stores raw/proof
paths and hashes. Re-importing the CASC CSV preserves those audit columns.

For a kernel-only audit:

```bash
cargo run --release -p mrs-codex -- /home/user/EDLA/git/mrs/problems \
  --db codex-kernel.db \
  --system mrs-0.2.3 \
  --cmd "./target/release/mrs {file}" \
  --verify-mode kernel
```

*(Note: The `--hardware` flag is omitted above, so it will automatically detect and log your system's hardware specs. You can also add `--schedule casc` or other parameters inside the `--cmd` string if you want to test specific strategies).*
