# Wire v1 and MRS integration

## Running the integration

Build the **same source verified by Verus** into a standalone checker:

```sh
nix develop .#verus -c bash formal/superposition-checker/build-runtime.sh /tmp/opencode/mrs-verus-checker
```

The command prints its SHA-256 and writes `OUTPUT.json` containing the binary
hash, source hash/commit/dirty state, pinned toolchain, support manifest and
verification flags. On ordinary Linux run the script with `bash` directly.
Use an existing output directory. Verification never downloads/builds a checker
implicitly. The caller explicitly supplies the expected artifact SHA-256.

```sh
nix develop -c cargo run -p mrs-proof-kernel --example verus_shadow -- \
    shadow /tmp/opencode/mrs-verus-checker SHA256 problem.p proof.p
# Replace shadow with gate to require BOTH independent checkers to accept.
```

The production ProoVer CLI also accepts these opt-in modes (both imply strict):

```sh
nix develop -c cargo run -p mrs-proover --bin mrs-proover -- \
    --verus-gate /tmp/opencode/mrs-verus-checker --verus-sha256 SHA256 \
    formal/superposition-checker/fixtures/superposition-refutation-proof.p
```

`--verus-shadow PATH` reports formal evidence while retaining the independent
strict verdict. `--verus-gate PATH` suppresses `VerifiedGood` unless both checks
accept. Both require `--verus-sha256 HASH`; `--time` sets the subprocess deadline
(10 s default, capped at 60 s). Formal telemetry is written to stderr.

APIs:

* `mrs_proof_kernel::verus::check_tptp_step`: parse a real TPTP artifact, discover
  a certificate for its named parents/conclusion, invoke local verified replay.
  The archived c502 fixture traverses this path in CI.
* `check_tptp_dag`: lower the problem and entire proof, bind input leaves to the
  problem, reconstruct inference certificates, invoke `certify_dag`.
* `verify_with_shadow`: return the independent strict verdict and formal IR
  evidence separately. Formal Unknown does not affect the strict verdict.
* `verify_with_verus_gate`: explicit opt-in gate requiring strict `Certified`
  **and** formal `CertifiedDagIr`. Any failure, mismatch, unsupported construct,
  timeout, process error or malformed response suppresses certification.
* `mrs_proover::strict::{verify_loaded_job_with_verus_shadow,
  verify_loaded_job_with_verus_gate}`: loaded-job integration, including linked
  problem provenance. The loader recursively resolves includes and applies each
  include selection only to formulas imported from that target file, then merges
  the selected closure and clears directives before formal-adapter entry. Loader
  tests cover nested selection. The resolver/parser remain trusted host code;
  the Verus bridge itself rejects residual include directives.

The CLI example reads standalone files; unresolved includes return Unknown.
These APIs do not change the default strict kernel or prover output behavior.

## Trust boundary and supported subset

The Verus theorem covers the **decoded IR**. Wire-v1 version/tag/field/count
validation, recursive term/clause/certificate/DAG decoding, command dispatch,
and ASCII whitespace/unsigned-decimal tokenization are Verus-verified by
`check_wire_tokens` and `wire_tokenize`, including digit limits and checked
`u64` accumulation. The host shim only acquires a bounded byte buffer and
performs process I/O/response formatting. TPTP parsing, bounded CNF lowering,
numeric symbol assignment, certificate discovery, equality orientation, ordered
alpha matching, and subprocess orchestration remain unverified. The opt-in
gate remains a trusted-adapter dual-check, not an end-to-end verified TPTP
certifier. Its positive result requires the independently implemented strict
kernel. Shadow results always carry `production_certification=false` to prevent
mistaking them for an independent formal certification of the original proof.

Supported TPTP inputs are CNF, uninterpreted first-order predicates/functions of
arity at most two, equality, variables and a singleton positive `$false` empty
clause. Literal order and multiplicity are preserved. Interpreted functions,
numbers, distinct objects, larger arities, non-CNF dialects, types, unimplemented
rules and conjecture transformations return Unknown. No unsupported literal is
silently dropped. A shared signature check rejects inconsistent symbol arities.

Input leaves require `file(...)` provenance, matching role/source in the strict
kernel and exact lowered membership in the supplied premise clauses. Proof
names must be unique; missing parents, cycles, nonempty roots and disconnected
nodes cannot obtain a formal positive result. The whole DAG is transmitted;
unused proof nodes are not silently deleted to obtain closure.

Rules include ordinary superposition; exact complementary-atom resolution;
standardized-apart first-order resolution; equality resolution; clause and
equality factoring; subsumption resolution; weakening; unit-equality
demodulation; clause instantiation; clause condensation; equality reflexivity;
equality transitivity; and equality symmetry. The executable checker validates each
certificate and its exact cited parents and conclusion. Superposition and
first-order resolution discovery independently freshen the parents and find a
bounded syntactic unifier. Superposition preserves both complete contexts and
compares ordered conclusions up to a bijective variable renaming. The local
superposition adapter can orient the selected source equality in either
direction; whole-DAG lowering currently requires the original left-to-right
orientation so its certificate still matches its cited parent exactly.
Alpha-normalized superposition conclusions become the representation used by
subsequent nodes. These trusted transformations are not advertised as
Verus-refined lowering. AC reasoning, ordered-calculus conditions and broader
transformations remain unsupported.

The archived artifact contains only c27/c39/c502, not the complete archived
refutation. Its local step is translated and replayed; **full SWV406+1 proof
certification remains outside this result**. CI also checks actual CNF complete
refutations, including one with an essential superposition followed by exact
unary-predicate resolution.

## Protocol grammar

The executable accepts exactly `--wire-v1`. One request is read from stdin until
EOF, then exactly one response is written. Tokens are unsigned decimal `u64`
integers separated by whitespace; no negative/signed integers, nondecimal data,
unknown tags, trailing fields or partially decoded requests are accepted.

```text
request       := 1 command work payload
command       := 0 (identity) | 1 (superposition step) | 2 (DAG)
               | 3 (first-order resolution step) | 4 (equality resolution step)
               | 5 (clause factoring step) | 6 (equality factoring step)
               | 7 (subsumption resolution step) | 8 (clause weakening step)
               | 9 (demodulation step) | 10 (instantiation step) | 11 (condensation step)
               | 12 (reflexivity step) | 13 (transitivity step)
step payload  := certificate | resolution_certificate | eq_resolution_certificate
               | factoring_certificate | eq_factoring_certificate
               | subsumption_resolution_certificate | weakening_certificate
               | demodulation_certificate | instantiation_certificate | condensation_certificate
               | clause | clause clause clause
DAG payload   := node_count root input_count clause* node*
node          := id clause kind parent_count parent_index* rule_payload
kind          := 0 (input) | 1 (superposition) | 2 (nullary resolution)
               | 3 (exact atom resolution) | 4 (first-order resolution)
               | 5 (equality resolution) | 6 (clause factoring)
               | 7 (equality factoring) | 8 (subsumption resolution)
               | 9 (clause weakening) | 10 (demodulation) | 11 (instantiation)
               | 12 (condensation) | 13 (reflexivity) | 14 (transitivity)
rule_payload  := empty | certificate | resolution_certificate | eq_resolution_certificate
               | factoring_certificate | eq_factoring_certificate
               | subsumption_resolution_certificate | weakening_certificate
               | demodulation_certificate | instantiation_certificate | condensation_certificate
               | left_pivot right_pivot
certificate   := source_clause target_clause conclusion_clause
                 source_freshening target_freshening unifier
                 source_equality_index target_literal_index argument_slot
                 raw_path fresh_path
resolution_certificate := left_clause right_clause conclusion_clause
                          left_freshening right_freshening unifier
                          left_pivot right_pivot
eq_resolution_certificate := clause conclusion unifier pivot
factoring_certificate := clause conclusion unifier first second
eq_factoring_certificate := clause conclusion unifier first second
subsumption_resolution_certificate := target_clause active_clause conclusion_clause
                                      active_freshening unifier
                                      target_pivot active_pivot matches_count match_index*
weakening_certificate := parent_clause conclusion_clause
                         inclusions_count inclusion_index*
demodulation_certificate := target_clause equality_clause conclusion_clause
                            equality_freshening unifier
                            target_literal_index argument_slot
                            raw_path fresh_path reversed
instantiation_certificate := parent_clause substitution conclusion_clause
condensation_certificate := parent_clause substitution conclusion_clause
                            inclusions_count inclusion_index*
clause        := literal_count (polarity atom)*
polarity      := 0 (negative) | 1 (positive)
atom          := 0 symbol | 1 symbol term | 2 symbol term term | 3 term term
term          := 0 variable | 1 constant | 2 function term | 3 function term term
bindings      := count (variable term)*
path          := count child_index*
```

Each `*` repeats its preceding count; all collections and fields are positional.
Both raw/fresh paths are supplied and independently checked by replay. Symbols
and variables are separate numeric namespaces. DAG references are zero-based
vector indices, not producer IDs. Duplicate node IDs and forward/self parents
are rejected by verified structural checks. A valid empty-input clause is not
implicit; it must occur in the input problem and match an input node.

Limits: 4 MiB request, 500,000 decoded numeric tokens (the producer uses at most
400,000), 512 nodes/inputs, two parents, 64 literals per clause, term depth below
64, 256 substitution bindings, 64 path entries, work at most 10,000,000. These
are ceilings, not completeness claims or exact cumulative replay accounting.
The tokenizer's Verus postcondition proves both the output token-count ceiling
and the quantified byte-span-to-token-value correspondence (`wire_tokens_span_correspondence`).
The MRS subprocess runner bounds output to 1 KiB and the wall clock to the
explicit configuration (maximum 60 s). Exceeded limits cannot certify.
On Unix, the runner snapshots and hashes the binary, clears the child's
environment, sets its cwd to `/`, starts a private process group, and installs
CPU (60 s), address-space (768 MiB), file-size (1 MiB), and zero core-dump
limits before exec. Linux additionally sets `NO_NEW_PRIVS`. Timeouts and
descendants holding pipes trigger process-group termination. Startup or limit
setup failures return Unknown.

Responses:

```text
MRS_VERUS 1 Ready
MRS_VERUS 1 Accepted
MRS_VERUS 1 CertifiedIR
MRS_VERUS 1 Rejected
MRS_VERUS 1 Unknown
```

`Accepted` is only a local step result; `CertifiedIR` is only a DAG result. The
caller checks the exact version/response shape and requested command. Artifact
and request SHA-256 values, protocol version and checked node count accompany
the report. A hash supplied by the user identifies a trusted artifact; hashing
alone does not prove provenance. Keep the artifact immutable between hash
validation and launch.

## Verification

```sh
nix develop .#verus -c env MRS_VERUS_BRIDGE_TESTS=1 formal/superposition-checker/verify.sh
```

CI enables this bridge gate. It verifies the core with `--no-cheating`, compiles
it twice, compares hashes, runs runtime regressions and wire malformed/truncated
request tests, then runs Rust integration tests against that exact temporary
artifact/hash. Rust tests requiring the artifact are explicitly ignored in
ordinary `cargo test`; they are mandatory in the formal CI job.
The compiled-runtime mutation gate weakens the tokenizer's digit guard and maps
a non-ASCII byte into a digit, then requires the malformed request to remain
rejected.
