# mrs-codex

`mrs-codex` is a utility tool designed to process a large corpus of TPTP (`.p`) files using a theorem prover (like `mrs` or `vampire`) and store the results, execution times, hardware details, and parameters into a SQLite database.

## Features
* **Resumable Execution:** If the process is killed or interrupted, restarting the same command will safely skip already processed files matching the exact configuration and resume where it left off.
* **Parallel Processing:** Leverages `rayon` to evaluate multiple problems simultaneously across available CPU cores.
* **Strict Timeouts:** Wraps the prover execution in a wall-clock timeout using `wait-timeout` to prevent hanging on excessively hard problems.
* **Hardware Auto-detection:** Uses `sysinfo` to automatically detect and log CPU brand, core count, RAM, and OS details (overridable via the `--hardware` flag).
* **SZS Status Extraction:** Parses standard `% SZS status <Status>` output directly from the prover's stdout/stderr.
* **Independent Proof Verification:** Whenever a run reports `Theorem` or `Unsatisfiable`, the prover's stdout (the TSTP proof) is handed to `mrs-proover --only-mrs` for an independent soundness check. The result is stored in the `proover_validated` column and shown inline as `[Verified]` / `[FAILED Verif]`. Requires a `mrs-proover` binary next to the `mrs-codex` executable (built automatically as part of the workspace); verification is skipped (`proover_validated` stays `NULL`) if it cannot be found or times out after 10s.
* **Normalized Database Schema:** Utilizes dedicated tables for `systems`, `hardware`, and `parameters` with foreign keys in the `results` table to ensure scalability and speed when dealing with millions of records.

## Example Usage

Here is the exact command to process the `problems/` directory in this repository using the `mrs` prover.

For the most accurate execution times, it is highly recommended to use the release build of the prover:

```bash
# 1. Build the prover in release mode first, along with mrs-proover
#    (used for the automatic proof-verification step, see above)
cargo build --release -p mrs -p mrs-proover

# 2. Run the codex tool to process the problems directory
cargo run --release -p mrs-codex -- /home/fr22192/EDLA/git/mrs/problems \
  --db codex.db \
  --system mrs-0.1.9 \
  --timeout 30 \
  --cmd "./target/release/mrs {file}"
```

*(Note: The `--hardware` flag is omitted above, so it will automatically detect and log your system's hardware specs. You can also add `--schedule casc` or other parameters inside the `--cmd` string if you want to test specific strategies).*
