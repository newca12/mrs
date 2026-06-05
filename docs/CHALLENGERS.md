# Competitor Evaluation Strategy

From a strategic engineering perspective, installing and benchmarking against competitors like GDV, Nörgler, and VTV is **not recommended at this stage.** 

It is highly valuable to benchmark against them eventually, but installing them *before* fixing `mrs-proover`'s known flaws would be a premature optimization.

Here is the technical rationale:

### 1. We already know we are losing
A comparison right now will only confirm what we already know: `mrs-proover` suffers from the `-10` point definition-laundering penalty and leaves `+3` points on the table due to AC-matching blindspots. Until the `introduced(definition)` structural check is patched to validate the formula body, `mrs-proover` is guaranteed to fail the competition benchmarks.

### 2. High Installation Friction
Academic theorem proving tools are notoriously difficult to build:
*   **VTV** requires setting up the Agda and Haskell ecosystems.
*   **GDV-LP / Dedukti** requires OCaml and complex translation toolchains.
*   **GDV** is a sprawling collection of Prolog and C tools that assumes a very specific UNIX environment.
*   **Nörgler** is the most modern, but still requires linking against specific ATP solver libraries.

Setting these up will consume significant time that is better spent closing the vulnerabilities we just discovered.

### Recommended Path Forward
1.  **Fix the Fatal Flaws First:** Patch the `introduced(definition)` bypass in `crates/mrs-proover/src/checks/introduced_definition.rs` to stop the `-10` point bleeding.
2.  **Harden the Structural Checks:** Change the overly conservative `Unknown` returns to `Unsound` to capture the `+2` points on malformed proofs.
3.  **Benchmark Later:** Once `mrs-proover` can successfully catch 100% of our `evil-proofs` suite, we should install **Nörgler** (the easiest modern competitor to compile) to serve as a baseline for wall-clock execution time and structural correctness.