# mrs-cadical-sys

Workspace-owned CaDiCaL 3.0.1 C ABI for MRS.

The vendored source is CaDiCaL `rel-3.0.1` at commit
`c60730422e758ef1cebe7aeddf2dda31c996bf04`. The build compiles the solver
library, proof tracers, the `kitten` C helper, and the MRS-specific C++ wrapper;
standalone CaDiCaL applications are excluded.

Use `mrs-cadical` instead of this crate unless implementing or auditing the
FFI boundary.
