//! Low-level C ABI for the workspace-owned CaDiCaL 3.0.1 build.
//!
//! The safe wrapper lives in `mrs-cadical`. This crate intentionally exposes
//! only opaque handles, scalar values, and borrowed callback slices; no
//! CaDiCaL C++ type crosses the boundary.

use std::ffi::{c_char, c_int, c_void};

#[repr(C)]
pub struct MrsCaDiCaL {
    _private: [u8; 0],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct TraceCallbacks {
    pub userdata: *mut c_void,
    pub begin_proof: Option<unsafe extern "C" fn(*mut c_void, i64)>,
    pub add_original_clause:
        Option<unsafe extern "C" fn(*mut c_void, i64, c_int, *const c_int, usize, c_int)>,
    pub add_derived_clause: Option<
        unsafe extern "C" fn(
            *mut c_void,
            i64,
            c_int,
            c_int,
            *const c_int,
            usize,
            *const i64,
            usize,
        ),
    >,
    pub delete_clause: Option<unsafe extern "C" fn(*mut c_void, i64, c_int, *const c_int, usize)>,
    pub demote_clause: Option<unsafe extern "C" fn(*mut c_void, i64, *const c_int, usize)>,
    pub weaken_minus: Option<unsafe extern "C" fn(*mut c_void, i64, *const c_int, usize)>,
    pub strengthen: Option<unsafe extern "C" fn(*mut c_void, i64)>,
    pub finalize_clause: Option<unsafe extern "C" fn(*mut c_void, i64, *const c_int, usize)>,
    pub report_status: Option<unsafe extern "C" fn(*mut c_void, c_int, i64)>,
    pub solve_query: Option<unsafe extern "C" fn(*mut c_void)>,
    pub add_assumption: Option<unsafe extern "C" fn(*mut c_void, c_int)>,
    pub add_constraint: Option<unsafe extern "C" fn(*mut c_void, *const c_int, usize)>,
    pub reset_assumptions: Option<unsafe extern "C" fn(*mut c_void)>,
    pub add_assumption_clause:
        Option<unsafe extern "C" fn(*mut c_void, i64, *const c_int, usize, *const i64, usize)>,
    pub conclude_unsat: Option<unsafe extern "C" fn(*mut c_void, c_int, *const i64, usize)>,
    pub conclude_sat: Option<unsafe extern "C" fn(*mut c_void, *const c_int, usize)>,
    pub conclude_unknown: Option<unsafe extern "C" fn(*mut c_void, *const c_int, usize)>,
    pub notify_equivalence: Option<unsafe extern "C" fn(*mut c_void, c_int, c_int)>,
}

unsafe extern "C" {
    pub fn mrs_cadical_version() -> *const c_char;
    pub fn mrs_cadical_init() -> *mut MrsCaDiCaL;
    pub fn mrs_cadical_release(solver: *mut MrsCaDiCaL);
    pub fn mrs_cadical_set_terminate(
        solver: *mut MrsCaDiCaL,
        state: *mut c_void,
        terminate: Option<unsafe extern "C" fn(*mut c_void) -> c_int>,
    );
    pub fn mrs_cadical_add_clause(solver: *mut MrsCaDiCaL, clause: *const c_int, len: usize);
    pub fn mrs_cadical_assume(solver: *mut MrsCaDiCaL, lit: c_int);
    pub fn mrs_cadical_add_constraint(solver: *mut MrsCaDiCaL, clause: *const c_int, len: usize);
    pub fn mrs_cadical_solve(solver: *mut MrsCaDiCaL) -> c_int;
    pub fn mrs_cadical_status(solver: *const MrsCaDiCaL) -> c_int;
    pub fn mrs_cadical_value(solver: *const MrsCaDiCaL, lit: c_int) -> c_int;
    pub fn mrs_cadical_failed(solver: *const MrsCaDiCaL, lit: c_int) -> c_int;
    pub fn mrs_cadical_vars(solver: *const MrsCaDiCaL) -> c_int;
    pub fn mrs_cadical_declare_more_variables(solver: *mut MrsCaDiCaL, count: c_int) -> c_int;
    pub fn mrs_cadical_declare_one_more_variable(solver: *mut MrsCaDiCaL) -> c_int;
    pub fn mrs_cadical_set_option(
        solver: *mut MrsCaDiCaL,
        name: *const c_char,
        value: c_int,
    ) -> c_int;
    pub fn mrs_cadical_get_option(solver: *const MrsCaDiCaL, name: *const c_char) -> c_int;
    pub fn mrs_cadical_set_limit(
        solver: *mut MrsCaDiCaL,
        name: *const c_char,
        value: c_int,
    ) -> c_int;
    pub fn mrs_cadical_configure(solver: *mut MrsCaDiCaL, name: *const c_char) -> c_int;
    pub fn mrs_cadical_phase(solver: *mut MrsCaDiCaL, lit: c_int);
    pub fn mrs_cadical_unphase(solver: *mut MrsCaDiCaL, lit: c_int);
    pub fn mrs_cadical_freeze(solver: *mut MrsCaDiCaL, lit: c_int);
    pub fn mrs_cadical_melt(solver: *mut MrsCaDiCaL, lit: c_int);
    pub fn mrs_cadical_frozen(solver: *const MrsCaDiCaL, lit: c_int) -> c_int;
    pub fn mrs_cadical_terminate(solver: *mut MrsCaDiCaL);
    pub fn mrs_cadical_trace_proof(
        solver: *mut MrsCaDiCaL,
        path: *const c_char,
        format: c_int,
        binary: c_int,
    ) -> c_int;
    pub fn mrs_cadical_flush_proof(solver: *mut MrsCaDiCaL);
    pub fn mrs_cadical_close_proof(solver: *mut MrsCaDiCaL);
    pub fn mrs_cadical_connect_trace(
        solver: *mut MrsCaDiCaL,
        callbacks: *const TraceCallbacks,
        antecedents: c_int,
        finalize_clauses: c_int,
    ) -> c_int;
    pub fn mrs_cadical_disconnect_trace(solver: *mut MrsCaDiCaL) -> c_int;
}

// Raw FFI is intentionally unsafe and is wrapped by `mrs-cadical`.
unsafe impl Send for MrsCaDiCaL {}
