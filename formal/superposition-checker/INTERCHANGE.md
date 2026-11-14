# Wire v1 and MRS integration

## Running the integration

Build the **same source verified by Verus** into a standalone checker:

```sh
nix develop .#verus -c bash formal/superposition-checker/build-runtime.sh /tmp/opencode/mrs-verus-checker
```

The command first requires the selected Verus binary to report the pinned Verus
version and effective Rust toolchain, then compiles with `--no-cheating` and the
same SMT resource limit as `verify.sh`. It prints the artifact SHA-256 and writes
`OUTPUT.json` containing the artifact/source hashes, source commit/dirty state,
Verus binary hash/version, toolchain, support manifest and exact source hash,
verified build flags, and the verified versus unverified boundary. The runtime
bridge requires this adjacent sidecar to match the executable hash; a missing or
inconsistent sidecar returns `Unknown`. On ordinary Linux run the script with
`bash` directly. Use an existing output directory. Verification never
downloads/builds a checker implicitly. The caller explicitly supplies the
expected artifact SHA-256.

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
Reports from an invoked runtime are serialized as JSON and include the
supported-rule/limits manifest, manifest SHA-256 and build-evidence SHA-256.
Adapter preflight `Unknown` results may precede any runtime report. These hashes
and manifest are trusted build metadata, not additional verified theorem
premises.

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
  the Verus bridge itself rejects residual include directives. Before lowering,
  both problem and proof snapshots now check formula count, name, role, and full
  canonical formula text (including inference annotations) after AST serialization
  and reparsing. This is a fail-closed host-side consistency check, not a verified
  refinement from original input bytes.

The CLI example reads standalone files; unresolved includes return Unknown.
These APIs do not change the default strict kernel or prover output behavior.

## Trust boundary and supported subset

The Verus theorem covers the **decoded IR**. Wire-v1 version/tag/field/count
validation, recursive term/clause/certificate/DAG decoding, command dispatch,
and ASCII whitespace/unsigned-decimal tokenization are Verus-verified by
`check_wire_tokens` and `wire_tokenize`, including digit limits and checked
`u64` accumulation. The host shim only acquires a bounded byte buffer and
performs process I/O/response formatting. TPTP parsing, bounded CNF lowering,
numeric symbol assignment, wire request encoding, certificate discovery,
equality orientation, ordered alpha matching, and subprocess orchestration
remain unverified. Build-time evidence records the pinned Verus executable and
labels the verified decoder separately from these host-side boundaries. The opt-in
gate remains a trusted-adapter dual-check, not an end-to-end verified TPTP
certifier. Its positive result requires the independently implemented strict
kernel. Shadow results always carry `production_certification=false` to prevent
mistaking them for an independent formal certification of the original proof.

Problem inputs remain CNF with uninterpreted first-order predicates/functions of
arity at most two, equality, variables and a singleton positive `$false` empty
clause. Proof nodes may also use a restricted FOF clause representation: an outer
universal prefix followed by a disjunction of ordinary predicate/equality
literals. Existentials, other connectives, FOF sequents, defined/system symbols,
interpreted functions, numbers, distinct objects, larger arities, typed
declarations, unimplemented rules and conjecture transformations return
Unknown. Literal order and multiplicity are preserved except at the explicitly
checked input, clausify, resolution and superposition normalizations described
below. No
unsupported literal is silently dropped. A shared signature check rejects
inconsistent symbol arities.

Input leaves require `file(...)` provenance and a strict-kernel match on source,
formula name, compatible role and formula. A producer may label a source
`negated_conjecture` clause as an axiom/premise; an unnegated `conjecture` is
never admitted as a premise. The trusted adapter then normalizes a matching
leaf to the exact supplied input clause under universal closure, variable
alpha-renaming and literal permutation. A `clausify` edge is supported only
when its restricted FOF clause and CNF conclusion are the same under those
equivalences; the emitted IR edge is an identity instance, while the strict
kernel independently checks the original clausification annotation. A
`resolve` and superposition/`paramod` conclusions can be normalized to the exact
parent-order inference result after a bounded, shared-fuel alpha/permutation
comparison; the strict kernel checks the unmodified TSTP step. The `paramod`
alias searches both equality orientations and then uses verified superposition
replay. A `copy` edge is limited to clause identity modulo alpha/literal
permutation or one equality-symmetry step with unchanged context. These
normalizations remain in the unverified adapter boundary. Proof
names must be unique; missing parents, cycles, nonempty roots and disconnected
nodes cannot obtain a formal positive result. The whole DAG is transmitted;
unused proof nodes are not silently deleted to obtain closure.

Rules include ordinary superposition; exact complementary-atom resolution;
standardized-apart first-order resolution; equality resolution; clause and
equality factoring; subsumption resolution; weakening; unit-equality
demodulation; clause instantiation; clause condensation; equality reflexivity;
equality transitivity; equality symmetry; clause excluded middle; equality congruence; and clause tautology. The executable checker validates each
certificate and its exact cited parents and conclusion. Superposition and
first-order resolution discovery independently freshen the parents and find a
bounded syntactic unifier. Superposition preserves both complete contexts and
the wire certificate uses its exact ordered conclusion. The trusted adapter
matches a raw conclusion modulo literal permutation and a bijective variable
renaming before emitting that exact order. The `paramod` TPTP alias may orient
the selected source equality in either direction; ordinary whole-DAG
`superposition` still requires its original left-to-right orientation.
Alpha-normalized conclusions become the representation used by subsequent
nodes. These trusted transformations are not advertised as Verus-refined
lowering. AC reasoning, ordered-calculus conditions and broader transformations
remain unsupported.

Exact and standardized first-order resolution accept equality pivots in either
ordered or swapped orientation. Multi-parent Horn, consequence and contrapositive
citations are lowered to bounded binary resolution chains (at most six steps and
128 discovered states, within the caller's comparison budget). Only intermediate
steps on the final derivation are emitted; original proof nodes are retained, so
an unused cited parent still fails the whole-DAG root-closure check. Alpha-renamed
conclusions use the generated clause representation for subsequent nodes.

Multi-step demodulation and ground equality normalization are expanded into
binary demodulation nodes, each replayed by the verified checker. Discovery uses
a shared 50,000-comparison ceiling and at most six rewrites and 128 states,
further bounded by the caller's limits. Target variables remain rigid, every
cited equality must occur in the discovered chain, and the expanded DAG must
still fit the 512-node ceiling. Unsupported or exhausted searches return Unknown.

The full supplied MRS 0.2.3 stdout transcript is recorded by SHA-256 in
`supported-rules.json`; the repository keeps only the nine-node ancestor slice
for c502 in `fixtures/SWV406+1-c502-proof-context.p`. It contains the exact
input/NNF/skolemisation/CNF chains `c24 -> c25 -> c26 -> c27` and
`c36 -> c37 -> c38 -> c39`, followed by the archived `c27,c39 -> c502` step.
CI parses the slice and replays c502 through `check_tptp_step`, rejecting a forged
conclusion. This local-step adapter treats c27 and c39 as supplied clauses; it
does not certify their transformation ancestry or the full refutation. The
complete stdout ends in an AVATAR SAT refutation, whose branch/splitting rules
are outside the whitelist, so **full SWV406+1 proof certification remains
unsupported**. The transcript's reported CASC-J13 path is not independent
campaign provenance. A separate complete public Leo-III THF TSTP solution is preserved in
`fixtures/archived-unsupported-proofs.tsv` and must fail closed at the unsupported
type-declaration guard; it is not certified. CI also checks actual supported CNF
complete refutations, including one with an essential superposition followed by
exact unary-predicate resolution. The eleven-entry dual-check corpus now includes
three complete public Prover9 2026-6A proofs: seven-node `SYN343-1`, 31-node
`PUZ001-1`, and 42-node equality `PUZ001-2`. Their archive headers name TPTP
v9.3.1; each is checked against its vendored TPTP v10.0.0 problem fixture. The
vendored proofs remove only trailing horizontal whitespace; raw archive and
normalized fixture hashes are both recorded in the support manifest. These
examples do not form a representative competition corpus or verify the
FOF-to-clause adapter.

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
               | 14 (symmetry step) | 15 (excluded middle step) | 16 (congruence step)
               | 17 (tautology step)
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
               | 15 (symmetry) | 16 (excluded middle) | 17 (congruence)
               | 18 (tautology)
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
64, 256 substitution bindings, 64 path entries, work at most 10,000,000. The work
ceiling is cumulative across DAG structural preflight, root closure and nested
node replay in checker-defined work units; it is not an instruction-level cost
model or a completeness claim.
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
The bridge suite also reads `fixtures/supported-dag-corpus.tsv` and requires all
eleven indexed full TPTP refutations to pass the strict/Verus dual-check path with
their recorded node counts. The set includes four hand-authored smoke proofs, a
synthetic MRS 0.3.0 seven-node function-bearing Horn chain, three MRS 0.3.0
proofs generated from real TPTP v10.0.0 problems, and three complete archived
Prover9 2026-6A proofs: `SYN343-1` (7 nodes), `PUZ001-1` (31 nodes) and
`PUZ001-2` (42 nodes). Their archive headers name TPTP v9.3.1; their proof nodes
are checked against the vendored v10.0.0 problem fixtures. The equality proof
exercises the restricted `copy` symmetry and `paramod` adapter paths.
The generated cases are `PUZ001-1` (28 nodes,
instantiation/resolution), `SYN343-1` (5 nodes, condensation/subsumption
resolution), and `PUZ001-2` (29 nodes, equality and superposition rules).
`PUZ001-2` expands a symmetric source equality into an explicit symmetry node;
`PUZ001-1` normalizes one conclusion's literal order to the exact ordered-
resolvent contract. For every case, formal CI also changes the first cited
parent to a different existing input, replaces the first inference with an
unsupported rule, adds an otherwise-valid but disconnected input node, and
mutates the root to a nonempty clause; neither checker may certify these
variants. The TPTP problem and proof SHA-256 values are retained in the support
manifest. This is a small fixture set, not a representative competition archive;
three archived TSTP solutions do not establish campaign provenance or
representative archived coverage.
Both `verify.sh` and `build-runtime.sh` reject a Verus executable whose reported
release or effective Rust toolchain differs from `verus-toolchain.json`.
The compiled-runtime mutation gate weakens the tokenizer's digit guard and maps
a non-ASCII byte into a digit, then requires the malformed request to remain
rejected. It also removes the source-fresh variable images from both target
avoid sets; that mutant must pass Verus verification but fail the dedicated
superposition parent-variable collision regression. A second runtime mutant
omits the freshening binding-lookup charge and must fail the 4-unit/3-unit
freshening work-boundary regression. A third omits clause-literal collection
work and must fail the collection-plus-freshening boundary. A fourth omits the
freshened-clause materialization charge and must fail the 21-unit/20-unit
collection-plus-freshening regression. A fifth mutant omits the unifier's
term-comparison charge and must fail the 11-unit/10-unit unifier work regression.
A mutant bypasses the source-parent replay substitution meter and must fail the
12-unit/11-unit shared parent-instantiation regression. Three replay-output
mutants remove the target-replacement, conclusion-build or exact-comparison
structure charge; each must fail the 23-unit/22-unit output-work regression.
The path-sequence comparison and selected-term lookup mutants must fail their
4-unit/3-unit and 5-unit/4-unit preflight regressions.
A final mutant resets the standalone superposition meter before replay and must
fail the computed structural-plus-preflight cutoff regression.
A DAG-node mutant isolates local replay after the per-node parent-index,
node-clause and cited-parent checks; it must fail the exact composed
DAG-superposition work regression.
The first-order-resolution atom comparison charge has its own 4-unit/3-unit
cutoff regression and compiled charge-removal mutant.
The first-order-resolution DAG handoff mutant isolates replay after node and
cited-parent charges; it must fail the exact composed node-work regression.
Equality-resolution clause deletion is tested at 4 units/3 units, with a compiled
charge-removal mutant. Its DAG handoff also has a composed-cutoff reset mutant.
Clause factoring now shares work through substitution, pivot comparison, deletion
and exact conclusion matching; its DAG path has a composed-cutoff reset mutant too.
Equality factoring's selected left-term unification accepts at 5 units and rejects
at 4; a compiled mutant removes only the comparison charge. Its DAG parent checks
and replay share one counter, tested by an exact composed cutoff and reset mutant.
Subsumption-resolution literal comparison accepts at 3 units and rejects at 2; a
compiled mutant removes the literal charge. Its DAG parent comparisons and replay
also share one counter, guarded by an exact composed cutoff and reset mutant.
Weakening inclusion matching uses the metered literal comparator; its DAG parent
and conclusion charges share the replay counter and are covered by an exact composed
cutoff/reset mutant.
Instantiation and condensation each carry their DAG parent/conclusion charges into
replay; exact composed cutoffs and separate counter-reset mutants guard both paths.
Demodulation carries cited target/equality checks into replay and shares the meter
through freshening, selected-position comparison and replacement; its DAG path has
an exact composed cutoff and counter-reset mutation.
Nullary and exact-atom resolution DAG steps carry cited-parent and node charges into
resolvent construction and conclusion comparison; each has a composed-cutoff reset
mutant.
Reflexivity accepts at 5 units and rejects at 4; a compiled mutant removes the term
comparison charge. Its DAG wrapper also has an exact composed cutoff/reset mutant.
Transitivity's term-comparison helper accepts at 12 units and rejects at 11; a
compiled mutant removes one comparison charge. Its DAG parent and node work share
the replay meter under an exact composed cutoff/reset mutant.
Symmetry's orientation comparison accepts at 3 units and rejects at 2; a compiled
mutant removes one term charge. Its DAG work-sharing cutoff is computed independently
from the wrapper handoff, with a compiled counter-reset mutant.
Excluded-middle polarity and atom comparison accepts at 3 units and rejects at 2;
the compiled atom-charge and DAG counter-reset mutants must fail the exact regressions.
Congruence argument comparison accepts at 5 units and rejects at 4; its compiled
charge-removal mutant is paired with an independently measured DAG cutoff/reset test.
Verifier-negative mutations that bypass the unchanged binary sibling or nested unary
function-symbol check must fail the congruence proof.
Tautology's complementary-literal comparison accepts at 4 units and rejects at 3;
a compiled mutant removes the atom charge, while a separate DAG reset mutant checks
the node-meter handoff. Structural preflight uses the same bounded tautology scan.
`certify_dag` uses one mutable work counter across shape preflight, root closure and
the topological node loop. A three-node refutation tests the exact composed cutoff;
compiled mutants that reset the counter at each of those three handoffs must fail.
This is exact cumulative use of checker-defined work units, not an
instruction-count refinement. The claim is limited to the verified IR DAG
checker; TPTP lowering and the host bridge remain separate trust boundaries.
