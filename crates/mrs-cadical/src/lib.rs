//! Safe, MRS-specific interface to the workspace-owned CaDiCaL 3.0.1 build.
//!
//! The crate intentionally exposes only the operations needed by search,
//! ProoVer, and independent SAT-proof replay. The solver is movable between
//! threads but must not be accessed concurrently.

#![allow(unsafe_op_in_unsafe_fn)]

use std::ffi::{CStr, CString, NulError};
use std::ptr::NonNull;

use mrs_cadical_sys as sys;

/// Result of a CaDiCaL solve call.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SolveResult {
    Sat,
    Unsat,
    Unknown,
}

/// Proof trace format understood by CaDiCaL 3.0.1.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProofFormat {
    Drat,
    Lrat,
    FratLrat,
    FratDrat,
}

impl ProofFormat {
    fn raw(self) -> (i32, i32) {
        match self {
            Self::Drat => (0, 0),
            Self::Lrat => (1, 0),
            Self::FratLrat => (2, 0),
            Self::FratDrat => (3, 0),
        }
    }
}

/// Configuration for callback-based proof event collection.
#[derive(Clone, Copy, Debug)]
pub struct TraceConfig {
    pub antecedents: bool,
    pub finalize_clauses: bool,
    pub max_events: usize,
}

impl Default for TraceConfig {
    fn default() -> Self {
        Self {
            antecedents: true,
            finalize_clauses: true,
            max_events: 1_000_000,
        }
    }
}

/// Owned proof event emitted by CaDiCaL's tracer interface.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProofEvent {
    BeginProof {
        first_derived_id: i64,
    },
    OriginalClause {
        id: i64,
        redundant: bool,
        clause: Vec<i32>,
        restored: bool,
    },
    DerivedClause {
        id: i64,
        redundant: bool,
        witness: i32,
        clause: Vec<i32>,
        antecedents: Vec<i64>,
    },
    DeleteClause {
        id: i64,
        redundant: bool,
        clause: Vec<i32>,
    },
    DemoteClause {
        id: i64,
        clause: Vec<i32>,
    },
    WeakenMinus {
        id: i64,
        clause: Vec<i32>,
    },
    Strengthen {
        id: i64,
    },
    FinalizeClause {
        id: i64,
        clause: Vec<i32>,
    },
    ReportStatus {
        status: i32,
        id: i64,
    },
    SolveQuery,
    AddAssumption {
        literal: i32,
    },
    AddConstraint {
        clause: Vec<i32>,
    },
    ResetAssumptions,
    AddAssumptionClause {
        id: i64,
        clause: Vec<i32>,
        antecedents: Vec<i64>,
    },
    ConcludeUnsat {
        conclusion: i32,
        clause_ids: Vec<i64>,
    },
    ConcludeSat {
        model: Vec<i32>,
    },
    ConcludeUnknown {
        trail: Vec<i32>,
    },
    NotifyEquivalence {
        first: i32,
        second: i32,
    },
}

/// Owned callback trace.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProofTrace {
    pub events: Vec<ProofEvent>,
}

/// Encode callback events in the ASCII FRAT dialect used by CaDiCaL 3.0.1.
pub fn encode_frat_ascii(trace: &ProofTrace) -> Result<Vec<u8>, TraceError> {
    let mut output = String::new();
    for event in &trace.events {
        match event {
            ProofEvent::OriginalClause { id, clause, .. } => {
                write_frat_clause(&mut output, 'o', *id, clause, &[]);
            }
            ProofEvent::DerivedClause {
                id,
                witness,
                clause,
                antecedents,
                ..
            } => {
                if *witness != 0 {
                    return Err(TraceError::Malformed(
                        "RAT witness encoding is not implemented".into(),
                    ));
                }
                write_frat_clause(&mut output, 'a', *id, clause, antecedents);
            }
            ProofEvent::DeleteClause { id, clause, .. } => {
                write_frat_clause(&mut output, 'd', *id, clause, &[]);
            }
            ProofEvent::FinalizeClause { id, clause } => {
                write_frat_clause(&mut output, 'f', *id, clause, &[]);
            }
            ProofEvent::BeginProof { .. }
            | ProofEvent::ReportStatus { .. }
            | ProofEvent::SolveQuery
            | ProofEvent::ConcludeUnsat { .. } => {}
            ProofEvent::ConcludeSat { .. }
            | ProofEvent::ConcludeUnknown { .. }
            | ProofEvent::AddAssumption { .. }
            | ProofEvent::AddConstraint { .. }
            | ProofEvent::ResetAssumptions
            | ProofEvent::AddAssumptionClause { .. }
            | ProofEvent::NotifyEquivalence { .. }
            | ProofEvent::WeakenMinus { .. }
            | ProofEvent::Strengthen { .. }
            | ProofEvent::DemoteClause { .. } => {
                return Err(TraceError::Malformed(
                    "incremental or clause-transformation event cannot be encoded as strict FRAT"
                        .into(),
                ));
            }
        }
    }
    Ok(output.into_bytes())
}

/// Replays a complete CNF manifest in proof mode and returns an ASCII
/// FRAT-with-LRAT-antecedents trace. This creates a fresh solver so the
/// competition instance can remain optimized and mutable during search.
pub fn trace_manifest(clauses: &[Vec<i32>], max_variable: u32) -> Result<Vec<u8>, TraceError> {
    let mut solver = Solver::new();
    solver
        .configure("plain")
        .map_err(|_| TraceError::FfiFailure)?;
    solver
        .set_option("factor", 0)
        .map_err(|_| TraceError::FfiFailure)?;
    if max_variable > 0 {
        solver.declare_variables(max_variable as i32);
    }
    solver.connect_trace(TraceConfig::default())?;
    for clause in clauses {
        solver.add_clause(clause);
    }
    if solver.solve() != SolveResult::Unsat {
        let _ = solver.disconnect_trace();
        return Err(TraceError::Malformed(
            "SAT manifest did not replay to UNSAT".into(),
        ));
    }
    let trace = solver.disconnect_trace()?;
    check_proof_trace(&trace)?;
    encode_frat_ascii(&trace)
}

fn write_frat_clause(output: &mut String, kind: char, id: i64, clause: &[i32], hints: &[i64]) {
    output.push(kind);
    output.push(' ');
    output.push_str(&id.to_string());
    output.push_str("  ");
    for literal in clause {
        output.push_str(&literal.to_string());
        output.push(' ');
    }
    output.push_str("0");
    if !hints.is_empty() {
        output.push_str("  l ");
        for hint in hints {
            output.push_str(&hint.to_string());
            output.push(' ');
        }
        output.push('0');
    }
    output.push('\n');
}

/// A compact checker for the antecedent-bearing event stream emitted by
/// CaDiCaL's callback tracer. This is intentionally independent of the SAT
/// solver: it validates clause identity and RUP-style antecedent chains, but
/// it does not parse a binary/text FRAT file yet.
pub fn check_proof_trace(trace: &ProofTrace) -> Result<(), TraceError> {
    use std::collections::HashMap;

    let mut clauses: HashMap<i64, Vec<i32>> = HashMap::new();
    let mut finalized_empty = false;
    for event in &trace.events {
        match event {
            ProofEvent::BeginProof { .. }
            | ProofEvent::ReportStatus { .. }
            | ProofEvent::SolveQuery
            | ProofEvent::AddAssumption { .. }
            | ProofEvent::AddConstraint { .. }
            | ProofEvent::ResetAssumptions
            | ProofEvent::ConcludeUnsat { .. }
            | ProofEvent::ConcludeSat { .. }
            | ProofEvent::ConcludeUnknown { .. }
            | ProofEvent::NotifyEquivalence { .. } => {}
            ProofEvent::WeakenMinus { .. }
            | ProofEvent::Strengthen { .. }
            | ProofEvent::DemoteClause { .. }
            | ProofEvent::AddAssumptionClause { .. } => {
                return Err(TraceError::Malformed(
                    "unsupported incremental or clause-transformation event".into(),
                ));
            }
            ProofEvent::OriginalClause { id, clause, .. } => {
                if *id <= 0 || clauses.insert(*id, clause.clone()).is_some() {
                    return Err(TraceError::FfiFailure);
                }
            }
            ProofEvent::DerivedClause {
                id,
                witness,
                clause,
                antecedents,
                ..
            } => {
                if *id <= 0 || clauses.contains_key(id) {
                    return Err(TraceError::FfiFailure);
                }
                if *witness != 0 {
                    return Err(TraceError::Malformed(
                        "RAT witness replay is not implemented".into(),
                    ));
                }
                if antecedents.is_empty() && !is_tautology(clause) {
                    return Err(TraceError::Malformed(
                        "non-tautological derived clause has no antecedent chain".into(),
                    ));
                }
                if !antecedents.is_empty() && !rup_check(&clauses, clause, antecedents) {
                    return Err(TraceError::FfiFailure);
                }
                clauses.insert(*id, clause.clone());
            }
            ProofEvent::DeleteClause { id, .. } => {
                clauses.remove(id);
            }
            ProofEvent::FinalizeClause { id, clause } => {
                if clauses.get(id) != Some(clause) {
                    return Err(TraceError::FfiFailure);
                }
                finalized_empty |= clause.is_empty();
            }
        }
    }
    if finalized_empty {
        Ok(())
    } else {
        Err(TraceError::FfiFailure)
    }
}

fn rup_check(
    clauses: &std::collections::HashMap<i64, Vec<i32>>,
    conclusion: &[i32],
    antecedents: &[i64],
) -> bool {
    let mut assignment = std::collections::HashSet::new();
    for &literal in conclusion {
        assignment.insert(-literal);
    }
    for &id in antecedents {
        let Some(clause) = clauses.get(&id) else {
            return false;
        };
        let mut satisfied = false;
        let mut unassigned = None;
        let mut multiple_unassigned = false;
        for &literal in clause {
            if assignment.contains(&literal) {
                satisfied = true;
                break;
            }
            if !assignment.contains(&-literal) && unassigned.replace(literal).is_some() {
                multiple_unassigned = true;
            }
        }
        if satisfied {
            continue;
        }
        if clause.is_empty() {
            return true;
        }
        if multiple_unassigned {
            return false;
        }
        if let Some(unit) = unassigned {
            assignment.insert(unit);
        } else {
            return true;
        }
    }
    false
}

/// Parse the ASCII FRAT dialect emitted by [`ProofFormat::FratLrat`] or
/// [`ProofFormat::FratDrat`]. Unknown records are rejected so a future format
/// extension cannot be silently accepted as a valid proof.
pub fn parse_frat_ascii(input: &str) -> Result<ProofTrace, TraceError> {
    let mut events = Vec::new();
    for (line_number, line) in input.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('c') {
            continue;
        }
        let mut tokens = line.split_whitespace();
        let kind = tokens
            .next()
            .ok_or_else(|| malformed(line_number, "missing record kind"))?;
        let id = tokens
            .next()
            .ok_or_else(|| malformed(line_number, "missing clause ID"))?
            .parse::<i64>()
            .map_err(|_| malformed(line_number, "invalid clause ID"))?;
        let mut clause = Vec::new();
        loop {
            let token = tokens
                .next()
                .ok_or_else(|| malformed(line_number, "unterminated clause"))?;
            if token == "0" {
                break;
            }
            clause.push(
                token
                    .parse::<i32>()
                    .map_err(|_| malformed(line_number, "invalid literal"))?,
            );
        }
        let event = match kind {
            "o" => ProofEvent::OriginalClause {
                id,
                redundant: false,
                clause,
                restored: false,
            },
            "a" => {
                let mut antecedents = Vec::new();
                if let Some(marker) = tokens.next() {
                    if marker != "l" {
                        return Err(malformed(line_number, "expected FRAT antecedent marker"));
                    }
                    loop {
                        let token = tokens.next().ok_or_else(|| {
                            malformed(line_number, "unterminated antecedent list")
                        })?;
                        if token == "0" {
                            break;
                        }
                        antecedents.push(
                            token
                                .parse::<i64>()
                                .map_err(|_| malformed(line_number, "invalid antecedent ID"))?,
                        );
                    }
                }
                ProofEvent::DerivedClause {
                    id,
                    redundant: false,
                    witness: 0,
                    clause,
                    antecedents,
                }
            }
            "d" => ProofEvent::DeleteClause {
                id,
                redundant: false,
                clause,
            },
            "f" => ProofEvent::FinalizeClause { id, clause },
            other => {
                return Err(malformed(
                    line_number,
                    &format!("unsupported FRAT record `{other}`"),
                ));
            }
        };
        if tokens.next().is_some() {
            return Err(malformed(line_number, "unexpected trailing record data"));
        }
        events.push(event);
    }
    Ok(ProofTrace { events })
}

fn malformed(line: usize, reason: &str) -> TraceError {
    TraceError::Malformed(format!("line {}: {reason}", line + 1))
}

fn is_tautology(clause: &[i32]) -> bool {
    clause.iter().any(|literal| clause.contains(&-*literal))
}

#[derive(Debug, Eq, PartialEq)]
pub enum TraceError {
    AlreadyConnected,
    NotConnected,
    EventLimitExceeded,
    FfiFailure,
    Malformed(String),
}

#[derive(Debug)]
pub enum OptionError {
    InteriorNul(NulError),
    InvalidOption,
}

impl std::fmt::Display for OptionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InteriorNul(_) => f.write_str("CaDiCaL option contains an interior NUL"),
            Self::InvalidOption => f.write_str("invalid CaDiCaL option or configuration"),
        }
    }
}

impl std::error::Error for OptionError {}

impl std::fmt::Display for TraceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AlreadyConnected => f.write_str("CaDiCaL proof trace already connected"),
            Self::NotConnected => f.write_str("CaDiCaL proof trace is not connected"),
            Self::EventLimitExceeded => f.write_str("CaDiCaL proof trace event limit exceeded"),
            Self::FfiFailure => f.write_str("CaDiCaL proof trace FFI operation failed"),
            Self::Malformed(reason) => write!(f, "malformed CaDiCaL proof trace: {reason}"),
        }
    }
}

impl std::error::Error for TraceError {}

struct TraceState {
    events: Vec<ProofEvent>,
    max_events: usize,
    overflowed: bool,
}

impl TraceState {
    fn push(&mut self, event: ProofEvent) {
        if self.events.len() >= self.max_events {
            self.overflowed = true;
        } else {
            self.events.push(event);
        }
    }
}

/// Safe owner of one CaDiCaL solver instance.
pub struct Solver {
    raw: NonNull<sys::MrsCaDiCaL>,
    trace_state: Option<Box<TraceState>>,
}

impl Solver {
    pub fn new() -> Self {
        let raw = unsafe { sys::mrs_cadical_init() };
        let raw = NonNull::new(raw).expect("CaDiCaL initialization returned null");
        Self {
            raw,
            trace_state: None,
        }
    }

    pub fn version() -> &'static str {
        let ptr = unsafe { sys::mrs_cadical_version() };
        unsafe { CStr::from_ptr(ptr) }.to_str().unwrap_or("unknown")
    }

    pub fn add_clause(&mut self, clause: impl AsRef<[i32]>) {
        let clause = clause.as_ref();
        unsafe { sys::mrs_cadical_add_clause(self.raw.as_ptr(), clause.as_ptr(), clause.len()) };
    }

    pub fn assume(&mut self, literal: i32) {
        unsafe { sys::mrs_cadical_assume(self.raw.as_ptr(), literal) };
    }

    pub fn add_constraint(&mut self, clause: &[i32]) {
        unsafe {
            sys::mrs_cadical_add_constraint(self.raw.as_ptr(), clause.as_ptr(), clause.len())
        };
    }

    pub fn solve(&mut self) -> SolveResult {
        match unsafe { sys::mrs_cadical_solve(self.raw.as_ptr()) } {
            10 => SolveResult::Sat,
            20 => SolveResult::Unsat,
            _ => SolveResult::Unknown,
        }
    }

    pub fn status(&self) -> SolveResult {
        match unsafe { sys::mrs_cadical_status(self.raw.as_ptr()) } {
            10 => SolveResult::Sat,
            20 => SolveResult::Unsat,
            _ => SolveResult::Unknown,
        }
    }

    pub fn value(&self, literal: i32) -> Option<bool> {
        match unsafe { sys::mrs_cadical_value(self.raw.as_ptr(), literal) } {
            value if value == literal.abs() => Some(true),
            value if value == -literal.abs() => Some(false),
            _ => None,
        }
    }

    pub fn failed(&self, literal: i32) -> bool {
        unsafe { sys::mrs_cadical_failed(self.raw.as_ptr(), literal) != 0 }
    }

    pub fn vars(&self) -> i32 {
        unsafe { sys::mrs_cadical_vars(self.raw.as_ptr()) }
    }

    /// Declares and returns one fresh user variable.
    pub fn declare_variable(&mut self) -> i32 {
        unsafe { sys::mrs_cadical_declare_one_more_variable(self.raw.as_ptr()) }
    }

    pub fn declare_variables(&mut self, count: i32) -> i32 {
        unsafe { sys::mrs_cadical_declare_more_variables(self.raw.as_ptr(), count) }
    }

    pub fn set_option(&mut self, name: &str, value: i32) -> Result<(), OptionError> {
        let name = CString::new(name).map_err(OptionError::InteriorNul)?;
        let ok = unsafe { sys::mrs_cadical_set_option(self.raw.as_ptr(), name.as_ptr(), value) };
        if ok != 0 {
            Ok(())
        } else {
            Err(OptionError::InvalidOption)
        }
    }

    pub fn option(&self, name: &str) -> Result<i32, NulError> {
        let name = CString::new(name)?;
        Ok(unsafe { sys::mrs_cadical_get_option(self.raw.as_ptr(), name.as_ptr()) })
    }

    pub fn configure(&mut self, name: &str) -> Result<(), OptionError> {
        let name = CString::new(name).map_err(OptionError::InteriorNul)?;
        let ok = unsafe { sys::mrs_cadical_configure(self.raw.as_ptr(), name.as_ptr()) };
        if ok != 0 {
            Ok(())
        } else {
            Err(OptionError::InvalidOption)
        }
    }

    pub fn set_limit(&mut self, name: &str, value: i32) -> Result<(), OptionError> {
        let name = CString::new(name).map_err(OptionError::InteriorNul)?;
        let ok = unsafe { sys::mrs_cadical_set_limit(self.raw.as_ptr(), name.as_ptr(), value) };
        if ok != 0 {
            Ok(())
        } else {
            Err(OptionError::InvalidOption)
        }
    }

    pub fn phase(&mut self, literal: i32) {
        unsafe { sys::mrs_cadical_phase(self.raw.as_ptr(), literal) };
    }

    pub fn unphase(&mut self, literal: i32) {
        unsafe { sys::mrs_cadical_unphase(self.raw.as_ptr(), literal) };
    }

    pub fn freeze(&mut self, literal: i32) {
        unsafe { sys::mrs_cadical_freeze(self.raw.as_ptr(), literal) };
    }

    pub fn melt(&mut self, literal: i32) {
        unsafe { sys::mrs_cadical_melt(self.raw.as_ptr(), literal) };
    }

    pub fn frozen(&self, literal: i32) -> bool {
        unsafe { sys::mrs_cadical_frozen(self.raw.as_ptr(), literal) != 0 }
    }

    pub fn terminate(&mut self) {
        unsafe { sys::mrs_cadical_terminate(self.raw.as_ptr()) };
    }

    pub fn start_file_trace(
        &mut self,
        path: &std::path::Path,
        format: ProofFormat,
    ) -> Result<(), TraceError> {
        let path = path.to_str().ok_or(TraceError::FfiFailure)?;
        let path = CString::new(path).map_err(|_| TraceError::FfiFailure)?;
        let (format, binary) = format.raw();
        let ok = unsafe {
            sys::mrs_cadical_trace_proof(self.raw.as_ptr(), path.as_ptr(), format, binary)
        };
        if ok != 0 {
            Ok(())
        } else {
            Err(TraceError::FfiFailure)
        }
    }

    pub fn flush_file_trace(&mut self) {
        unsafe { sys::mrs_cadical_flush_proof(self.raw.as_ptr()) };
    }

    pub fn close_file_trace(&mut self) {
        unsafe { sys::mrs_cadical_close_proof(self.raw.as_ptr()) };
    }

    pub fn connect_trace(&mut self, config: TraceConfig) -> Result<(), TraceError> {
        if self.trace_state.is_some() {
            return Err(TraceError::AlreadyConnected);
        }
        let mut state = Box::new(TraceState {
            events: Vec::new(),
            max_events: config.max_events,
            overflowed: false,
        });
        let mut callbacks = callback_table();
        callbacks.userdata = (&mut *state) as *mut TraceState as *mut _;
        let ok = unsafe {
            sys::mrs_cadical_connect_trace(
                self.raw.as_ptr(),
                &callbacks,
                config.antecedents as i32,
                config.finalize_clauses as i32,
            )
        };
        if ok == 0 {
            return Err(TraceError::FfiFailure);
        }
        self.trace_state = Some(state);
        Ok(())
    }

    pub fn disconnect_trace(&mut self) -> Result<ProofTrace, TraceError> {
        if self.trace_state.is_none() {
            return Err(TraceError::NotConnected);
        }
        let ok = unsafe { sys::mrs_cadical_disconnect_trace(self.raw.as_ptr()) };
        if ok == 0 {
            return Err(TraceError::FfiFailure);
        }
        let state = self.trace_state.take().expect("trace state checked above");
        if state.overflowed {
            return Err(TraceError::EventLimitExceeded);
        }
        Ok(ProofTrace {
            events: state.events,
        })
    }
}

impl Default for Solver {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Solver {
    fn drop(&mut self) {
        unsafe { sys::mrs_cadical_release(self.raw.as_ptr()) };
    }
}

unsafe impl Send for Solver {}

fn callback_table() -> sys::TraceCallbacks {
    sys::TraceCallbacks {
        userdata: std::ptr::null_mut(),
        begin_proof: Some(begin_proof),
        add_original_clause: Some(add_original_clause),
        add_derived_clause: Some(add_derived_clause),
        delete_clause: Some(delete_clause),
        demote_clause: Some(demote_clause),
        weaken_minus: Some(weaken_minus),
        strengthen: Some(strengthen),
        finalize_clause: Some(finalize_clause),
        report_status: Some(report_status),
        solve_query: Some(solve_query),
        add_assumption: Some(add_assumption),
        add_constraint: Some(add_constraint),
        reset_assumptions: Some(reset_assumptions),
        add_assumption_clause: Some(add_assumption_clause),
        conclude_unsat: Some(conclude_unsat),
        conclude_sat: Some(conclude_sat),
        conclude_unknown: Some(conclude_unknown),
        notify_equivalence: Some(notify_equivalence),
    }
}

unsafe fn state<'a>(userdata: *mut std::ffi::c_void) -> &'a mut TraceState {
    &mut *(userdata as *mut TraceState)
}

unsafe fn copy_slice<T: Copy>(ptr: *const T, len: usize) -> Vec<T> {
    if len == 0 {
        Vec::new()
    } else {
        std::slice::from_raw_parts(ptr, len).to_vec()
    }
}

unsafe extern "C" fn begin_proof(userdata: *mut std::ffi::c_void, id: i64) {
    state(userdata).push(ProofEvent::BeginProof {
        first_derived_id: id,
    });
}

unsafe extern "C" fn add_original_clause(
    userdata: *mut std::ffi::c_void,
    id: i64,
    redundant: i32,
    clause: *const i32,
    len: usize,
    restored: i32,
) {
    let clause = copy_slice(clause, len);
    state(userdata).push(ProofEvent::OriginalClause {
        id,
        redundant: redundant != 0,
        clause,
        restored: restored != 0,
    });
}

unsafe extern "C" fn add_derived_clause(
    userdata: *mut std::ffi::c_void,
    id: i64,
    redundant: i32,
    witness: i32,
    clause: *const i32,
    clause_len: usize,
    antecedents: *const i64,
    antecedent_len: usize,
) {
    state(userdata).push(ProofEvent::DerivedClause {
        id,
        redundant: redundant != 0,
        witness,
        clause: copy_slice(clause, clause_len),
        antecedents: copy_slice(antecedents, antecedent_len),
    });
}

unsafe extern "C" fn delete_clause(
    userdata: *mut std::ffi::c_void,
    id: i64,
    redundant: i32,
    clause: *const i32,
    len: usize,
) {
    state(userdata).push(ProofEvent::DeleteClause {
        id,
        redundant: redundant != 0,
        clause: copy_slice(clause, len),
    });
}

unsafe extern "C" fn demote_clause(
    userdata: *mut std::ffi::c_void,
    id: i64,
    clause: *const i32,
    len: usize,
) {
    state(userdata).push(ProofEvent::DemoteClause {
        id,
        clause: copy_slice(clause, len),
    });
}

unsafe extern "C" fn weaken_minus(
    userdata: *mut std::ffi::c_void,
    id: i64,
    clause: *const i32,
    len: usize,
) {
    state(userdata).push(ProofEvent::WeakenMinus {
        id,
        clause: copy_slice(clause, len),
    });
}

unsafe extern "C" fn strengthen(userdata: *mut std::ffi::c_void, id: i64) {
    state(userdata).push(ProofEvent::Strengthen { id });
}

unsafe extern "C" fn finalize_clause(
    userdata: *mut std::ffi::c_void,
    id: i64,
    clause: *const i32,
    len: usize,
) {
    state(userdata).push(ProofEvent::FinalizeClause {
        id,
        clause: copy_slice(clause, len),
    });
}

unsafe extern "C" fn report_status(userdata: *mut std::ffi::c_void, status: i32, id: i64) {
    state(userdata).push(ProofEvent::ReportStatus { status, id });
}

unsafe extern "C" fn solve_query(userdata: *mut std::ffi::c_void) {
    state(userdata).push(ProofEvent::SolveQuery);
}

unsafe extern "C" fn add_assumption(userdata: *mut std::ffi::c_void, literal: i32) {
    state(userdata).push(ProofEvent::AddAssumption { literal });
}

unsafe extern "C" fn add_constraint(
    userdata: *mut std::ffi::c_void,
    clause: *const i32,
    len: usize,
) {
    state(userdata).push(ProofEvent::AddConstraint {
        clause: copy_slice(clause, len),
    });
}

unsafe extern "C" fn reset_assumptions(userdata: *mut std::ffi::c_void) {
    state(userdata).push(ProofEvent::ResetAssumptions);
}

unsafe extern "C" fn add_assumption_clause(
    userdata: *mut std::ffi::c_void,
    id: i64,
    clause: *const i32,
    clause_len: usize,
    antecedents: *const i64,
    antecedent_len: usize,
) {
    state(userdata).push(ProofEvent::AddAssumptionClause {
        id,
        clause: copy_slice(clause, clause_len),
        antecedents: copy_slice(antecedents, antecedent_len),
    });
}

unsafe extern "C" fn conclude_unsat(
    userdata: *mut std::ffi::c_void,
    conclusion: i32,
    clause_ids: *const i64,
    len: usize,
) {
    state(userdata).push(ProofEvent::ConcludeUnsat {
        conclusion,
        clause_ids: copy_slice(clause_ids, len),
    });
}

unsafe extern "C" fn conclude_sat(userdata: *mut std::ffi::c_void, model: *const i32, len: usize) {
    state(userdata).push(ProofEvent::ConcludeSat {
        model: copy_slice(model, len),
    });
}

unsafe extern "C" fn conclude_unknown(
    userdata: *mut std::ffi::c_void,
    trail: *const i32,
    len: usize,
) {
    state(userdata).push(ProofEvent::ConcludeUnknown {
        trail: copy_slice(trail, len),
    });
}

unsafe extern "C" fn notify_equivalence(userdata: *mut std::ffi::c_void, first: i32, second: i32) {
    state(userdata).push(ProofEvent::NotifyEquivalence { first, second });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_version_and_solves() {
        assert_eq!(Solver::version(), "3.0.1");
        let mut solver = Solver::new();
        solver.add_clause(&[1]);
        solver.add_clause(&[-1]);
        assert_eq!(solver.solve(), SolveResult::Unsat);
    }

    #[test]
    fn records_lrat_antecedent_events() {
        let mut solver = Solver::new();
        solver.configure("plain").expect("plain configuration");
        solver
            .connect_trace(TraceConfig::default())
            .expect("connect trace");
        solver.add_clause(&[1]);
        solver.add_clause(&[-1]);
        assert_eq!(solver.solve(), SolveResult::Unsat);
        let trace = solver.disconnect_trace().expect("disconnect trace");
        check_proof_trace(&trace).expect("check callback trace");
        assert!(trace.events.iter().any(|event| {
            matches!(event, ProofEvent::DerivedClause { clause, .. } if clause.is_empty())
        }));
    }

    #[test]
    fn writes_ascii_frat_trace() {
        let path = std::env::temp_dir().join(format!(
            "mrs-cadical-frat-{}-{}.proof",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        let mut solver = Solver::new();
        solver
            .start_file_trace(&path, ProofFormat::FratLrat)
            .expect("start FRAT trace");
        solver.add_clause(&[1]);
        solver.add_clause(&[-1]);
        assert_eq!(solver.solve(), SolveResult::Unsat);
        solver.close_file_trace();

        let trace = std::fs::read_to_string(&path).expect("read FRAT trace");
        assert!(trace.contains("a ") || trace.contains("o "));
        assert!(trace.contains("0"));
        let parsed = parse_frat_ascii(&trace).expect("parse FRAT trace");
        check_proof_trace(&parsed).expect("check FRAT trace");
        let _ = std::fs::remove_file(path);
    }
}
