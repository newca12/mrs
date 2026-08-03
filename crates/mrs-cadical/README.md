# mrs-cadical

Safe MRS-specific interface to CaDiCaL 3.0.1.

The crate supports:

- SAT solving with assumptions and constraints;
- explicit CaDiCaL 3.x variable declaration;
- limits, phases, freezing, and termination;
- callback proof events with antecedents and finalization;
- ASCII DRAT, LRAT, and FRAT trace files;
- bounded owned callback-trace checking for the initial replay prototype.

`mrs-cadical` is used by `mrs-search` and `mrs-proover`. The independent
strict proof kernel must not depend on this crate: final SAT-trace replay will
be moved into the kernel-side verification boundary in a later milestone.
