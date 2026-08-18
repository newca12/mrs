# Legacy TSTP Repair

`crates/mrs-bench/sanitize_tstp.sh` repairs old mrs proof output that contains
unquoted TPTP identifiers. It is intended for archived competition proofs, not
as a replacement for fixing the proof serializer.

The script preserves symbol identity: it quotes the original text instead of
renaming punctuation-containing symbols. It also quotes formula names in
`file(...)` source annotations and numeric literals in FOF/CNF formulas when
the legacy output requires that representation for the external checker.

The input files are never modified. Directory layout is preserved below the
output directory.

```bash
crates/mrs-bench/sanitize_tstp.sh \
  --input-dir /path/to/legacy/proofs \
  --output-dir /path/to/repaired/proofs
```

For Geoff's archive, after extracting `mrsErrors.tgz`:

```bash
tar -xzf /mnt/c/Users/fr22192/tmp/mrsErrors.tgz -C /tmp/mrsErrors
crates/mrs-bench/sanitize_tstp.sh \
  --input-dir /tmp/mrsErrors \
  --output-dir /tmp/mrsErrors-repaired
tar -czf /tmp/mrsErrors-repaired.tgz -C /tmp/mrsErrors-repaired FEQ UEQ
```

The repaired files should be checked with both parsers:

```bash
for proof in /tmp/mrsErrors-repaired/FEQ/mrs---0.2.0/*.e \
             /tmp/mrsErrors-repaired/UEQ/mrs---0.2.0/*.e; do
  nix develop -c cargo run -q -p mrs-tptp --example test_file -- "$proof"
done
```

For the official TPTP World syntax check, use the locally installed
`tptp4X` command supplied by TPTP World:

```bash
tptp4X -q1 -ftptp -umachine /tmp/mrsErrors-repaired/FEQ/mrs---0.2.0/NUM835+1.e
```

The command must exit successfully and report no `ERROR` line. Syntax
validation does not replace mathematical proof verification; it only ensures
that the proof can be read by the competition's TPTP tooling.

## StarExec Wrapper

The StarExec entry point `crates/mrs-bench/systems/mrs/starexec_run_default`
can use the sanitizer without modifying the `mrs` binary. The StarExec package
must place these files in the same directory:

```text
mrs
starexec_run_default
sanitize_tstp.sh
```

The wrapper reserves time before the StarExec deadline, captures the mrs
stdout, sanitizes a complete proof block, and then emits the repaired output.
If the sanitizer or Perl is unavailable, it emits the original mrs output
instead of suppressing the SZS result. The sanitizer is not needed for
problems that produce no proof.

To check a whole output directory in one command:

```bash
TPTP4X=/path/to/tptp4X \
  crates/mrs-bench/validate_tstp.sh \
  --proofs-dir /tmp/mrsErrors-repaired
```
