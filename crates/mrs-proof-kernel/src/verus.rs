//! Bounded shadow bridge to the compiled Verus checker.
//!
//! The parser, lowering, alpha/orientation normalization, certificate discovery,
//! wire decoder and process runner are trusted/unverified. A positive result is
//! evidence about the decoded IR only and never upgrades a kernel verdict.

use super::*;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

const MAX_BYTES: usize = 4 * 1024 * 1024;
const MAX_NODES: usize = 512;
const WORK: u64 = 10_000_000;
const CHILD_MEMORY_BYTES: u64 = 768 * 1024 * 1024;
const CHILD_CPU_SECONDS: u64 = 60;
const CHILD_FILE_BYTES: u64 = 1024 * 1024;
static ARTIFACT_SEQUENCE: AtomicUsize = AtomicUsize::new(0);

/// Explicitly selected, hash-pinned artifact. No binary is downloaded or built
/// by verification. Build with formal/superposition-checker/build-runtime.sh.
#[derive(Debug, Clone)]
pub struct VerusConfig {
    pub executable: PathBuf,
    pub expected_sha256: String,
    pub timeout: Duration,
}

/// Shadow results deliberately cannot be converted into `KernelVerdict`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShadowVerdict {
    AcceptedStepIr,
    CertifiedDagIr,
    RejectedIr,
    Unknown(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShadowReport {
    pub verdict: ShadowVerdict,
    pub artifact_sha256: Option<String>,
    pub request_sha256: Option<String>,
    pub protocol_version: u32,
    pub checked_nodes: usize,
    /// All reports remain shadow-only while the input bridge is unverified.
    pub production_certification: bool,
}

fn unknown(reason: impl Into<String>) -> ShadowReport {
    ShadowReport {
        verdict: ShadowVerdict::Unknown(reason.into()),
        artifact_sha256: None,
        request_sha256: None,
        protocol_version: 1,
        checked_nodes: 0,
        production_certification: false,
    }
}

fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Hash and execute one private snapshot, never the mutable caller path. This
/// removes the obvious hash-then-replace race between validation and spawn.
struct ArtifactSnapshot(PathBuf);
impl Drop for ArtifactSnapshot {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn snapshot_artifact(source: &std::path::Path) -> Result<(ArtifactSnapshot, String), String> {
    let mut bytes = Vec::new();
    std::fs::File::open(source)
        .map_err(|e| format!("checker artifact: {e}"))?
        .take(128 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 128 * 1024 * 1024 {
        return Err("checker artifact size ceiling".into());
    }
    let hash = digest(&bytes);
    let (file, path) = loop {
        let candidate = std::env::temp_dir().join(format!(
            "mrs-verus-{}-{}",
            std::process::id(),
            ARTIFACT_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(file) => break (file, candidate),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(format!("checker snapshot: {e}")),
        }
    };
    let snapshot = path;
    let mut file = file;
    if let Err(error) = file.write_all(&bytes).and_then(|_| file.sync_all()) {
        let _ = std::fs::remove_file(&snapshot);
        return Err(format!("checker snapshot write: {error}"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&snapshot, std::fs::Permissions::from_mode(0o500)).map_err(
            |e| {
                let _ = std::fs::remove_file(&snapshot);
                format!("checker snapshot mode: {e}")
            },
        )?;
    }
    #[cfg(not(unix))]
    {
        let permissions = std::fs::metadata(source)
            .map_err(|e| e.to_string())?
            .permissions();
        std::fs::set_permissions(&snapshot, permissions).map_err(|e| e.to_string())?;
    }
    Ok((ArtifactSnapshot(snapshot), hash))
}

fn configure_child(command: &mut Command) {
    command
        .env_clear()
        .current_dir("/")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
        // SAFETY: only async-signal-safe libc resource-limit and prctl calls
        // occur in the child setup before exec; errors abort spawning.
        unsafe {
            command.pre_exec(|| {
                set_limit(libc::RLIMIT_CPU, CHILD_CPU_SECONDS)?;
                set_limit(libc::RLIMIT_AS, CHILD_MEMORY_BYTES)?;
                set_limit(libc::RLIMIT_FSIZE, CHILD_FILE_BYTES)?;
                set_limit(libc::RLIMIT_CORE, 0)?;
                #[cfg(target_os = "linux")]
                if libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }
}

struct Wire {
    words: Vec<u64>,
}
impl Wire {
    fn new(command: u64) -> Self {
        Self {
            words: vec![1, command, WORK],
        }
    }
    fn n(&mut self, n: usize) -> Result<(), String> {
        self.words
            .push(u64::try_from(n).map_err(|_| "integer overflow")?);
        if self.words.len() > 400_000 {
            return Err("wire token ceiling".into());
        }
        Ok(())
    }
    fn term(&mut self, term: &Term, depth: usize) -> Result<(), String> {
        if depth >= 64 {
            return Err("term depth exceeds wire limit".into());
        }
        match term {
            Term::Var(v) => {
                self.n(0)?;
                self.n(*v as usize)?;
            }
            Term::App(s, args) => {
                self.n(match args.len() {
                    0 => 1,
                    1 => 2,
                    2 => 3,
                    _ => return Err("function arity exceeds two".into()),
                })?;
                self.n(s.index() as usize)?;
                for arg in args {
                    self.term(arg, depth + 1)?;
                }
            }
        }
        Ok(())
    }
    fn clause(&mut self, clause: &[Literal]) -> Result<(), String> {
        if clause.len() > 64 {
            return Err("clause width exceeds 64".into());
        }
        self.n(clause.len())?;
        for literal in clause {
            self.n(usize::from(literal.positive))?;
            match &literal.atom {
                Atom::Pred(s, args) => {
                    if args.len() > 2 {
                        return Err("predicate arity exceeds two".into());
                    }
                    self.n(args.len())?;
                    self.n(s.index() as usize)?;
                    for arg in args {
                        self.term(arg, 0)?;
                    }
                }
                Atom::Eq(a, b) => {
                    self.n(3)?;
                    self.term(a, 0)?;
                    self.term(b, 0)?;
                }
            }
        }
        Ok(())
    }
    fn bindings(&mut self, bindings: &[(VarId, Term)]) -> Result<(), String> {
        if bindings.len() > 256 {
            return Err("substitution exceeds 256 bindings".into());
        }
        self.n(bindings.len())?;
        for (v, t) in bindings {
            self.n(*v as usize)?;
            self.term(t, 0)?;
        }
        Ok(())
    }
    fn finish(self) -> Result<Vec<u8>, String> {
        let text = self
            .words
            .iter()
            .map(u64::to_string)
            .collect::<Vec<_>>()
            .join(" ");
        if text.len() > MAX_BYTES {
            Err("wire byte ceiling".into())
        } else {
            Ok(text.into_bytes())
        }
    }
}

type Bindings = Vec<(VarId, Term)>;

struct Certificate {
    source: Vec<Literal>,
    target: Vec<Literal>,
    conclusion: Vec<Literal>,
    source_map: Vec<(VarId, Term)>,
    target_map: Vec<(VarId, Term)>,
    unifier: Vec<(VarId, Term)>,
    si: usize,
    ti: usize,
    slot: usize,
    path: Vec<usize>,
}

struct ResolutionCertificate {
    left: Vec<Literal>,
    right: Vec<Literal>,
    conclusion: Vec<Literal>,
    left_map: Vec<(VarId, Term)>,
    right_map: Vec<(VarId, Term)>,
    unifier: Vec<(VarId, Term)>,
    lp: usize,
    rp: usize,
}

struct EqualityResolutionCertificate {
    clause: Vec<Literal>,
    pivot: usize,
    unifier: Vec<(VarId, Term)>,
    conclusion: Vec<Literal>,
}

struct FactoringCertificate {
    clause: Vec<Literal>,
    first: usize,
    second: usize,
    unifier: Vec<(VarId, Term)>,
    conclusion: Vec<Literal>,
}

struct EqualityFactoringCertificate {
    clause: Vec<Literal>,
    first: usize,
    second: usize,
    unifier: Vec<(VarId, Term)>,
    conclusion: Vec<Literal>,
}

struct SubsumptionResolutionCertificate {
    target: Vec<Literal>,
    active: Vec<Literal>,
    conclusion: Vec<Literal>,
    active_freshening: Vec<(VarId, Term)>,
    unifier: Vec<(VarId, Term)>,
    target_pivot: usize,
    active_pivot: usize,
    matches: Vec<usize>,
}

fn instance(
    term: &Term,
    subst: &HashMap<VarId, Term>,
    fuel: &mut usize,
    depth: usize,
) -> Result<Term, String> {
    *fuel = fuel.checked_sub(1).ok_or("substitution work ceiling")?;
    if depth >= 64 {
        return Err("substitution depth ceiling".into());
    }
    match term {
        Term::Var(v) => match subst.get(v) {
            Some(t) => instance(t, subst, fuel, depth + 1),
            None => Ok(term.clone()),
        },
        Term::App(s, args) => Ok(Term::App(
            *s,
            args.iter()
                .map(|t| instance(t, subst, fuel, depth + 1))
                .collect::<Result<_, _>>()?,
        )),
    }
}

fn instantiate(
    literal: &Literal,
    subst: &HashMap<VarId, Term>,
    fuel: &mut usize,
) -> Result<Literal, String> {
    let atom = match &literal.atom {
        Atom::Pred(s, args) => Atom::Pred(
            *s,
            args.iter()
                .map(|t| instance(t, subst, fuel, 0))
                .collect::<Result<_, _>>()?,
        ),
        Atom::Eq(a, b) => Atom::Eq(instance(a, subst, fuel, 0)?, instance(b, subst, fuel, 0)?),
    };
    Ok(Literal {
        positive: literal.positive,
        atom,
    })
}

fn bounded_unify(
    a: &Term,
    b: &Term,
    fuel: &mut usize,
) -> Result<Option<HashMap<VarId, Term>>, String> {
    let mut subst = HashMap::new();
    let mut pending = vec![(a.clone(), b.clone())];
    while let Some((a, b)) = pending.pop() {
        let a = instance(&a, &subst, fuel, 0)?;
        let b = instance(&b, &subst, fuel, 0)?;
        if a == b {
            continue;
        }
        match (a, b) {
            (Term::Var(v), t) | (t, Term::Var(v)) => {
                if t.free_vars().contains(&v) {
                    return Ok(None);
                }
                subst.insert(v, t);
            }
            (Term::App(f, a), Term::App(g, b)) => {
                if f != g || a.len() != b.len() {
                    return Ok(None);
                }
                pending.extend(a.into_iter().zip(b));
            }
        }
    }
    Ok(Some(subst))
}
impl Certificate {
    fn encode(&self, wire: &mut Wire) -> Result<(), String> {
        wire.clause(&self.source)?;
        wire.clause(&self.target)?;
        wire.clause(&self.conclusion)?;
        wire.bindings(&self.source_map)?;
        wire.bindings(&self.target_map)?;
        wire.bindings(&self.unifier)?;
        wire.n(self.si)?;
        wire.n(self.ti)?;
        wire.n(self.slot)?;
        for _ in 0..2 {
            wire.n(self.path.len())?;
            for child in &self.path {
                wire.n(*child)?;
            }
        }
        Ok(())
    }
}

impl ResolutionCertificate {
    fn encode(&self, wire: &mut Wire) -> Result<(), String> {
        wire.clause(&self.left)?;
        wire.clause(&self.right)?;
        wire.clause(&self.conclusion)?;
        wire.bindings(&self.left_map)?;
        wire.bindings(&self.right_map)?;
        wire.bindings(&self.unifier)?;
        wire.n(self.lp)?;
        wire.n(self.rp)?;
        Ok(())
    }
}

impl EqualityResolutionCertificate {
    fn encode(&self, wire: &mut Wire) -> Result<(), String> {
        wire.clause(&self.clause)?;
        wire.clause(&self.conclusion)?;
        wire.bindings(&self.unifier)?;
        wire.n(self.pivot)?;
        Ok(())
    }
}

impl FactoringCertificate {
    fn encode(&self, wire: &mut Wire) -> Result<(), String> {
        wire.clause(&self.clause)?;
        wire.clause(&self.conclusion)?;
        wire.bindings(&self.unifier)?;
        wire.n(self.first)?;
        wire.n(self.second)?;
        Ok(())
    }
}

impl EqualityFactoringCertificate {
    fn encode(&self, wire: &mut Wire) -> Result<(), String> {
        wire.clause(&self.clause)?;
        wire.clause(&self.conclusion)?;
        wire.bindings(&self.unifier)?;
        wire.n(self.first)?;
        wire.n(self.second)?;
        Ok(())
    }
}

impl SubsumptionResolutionCertificate {
    fn encode(&self, wire: &mut Wire) -> Result<(), String> {
        wire.clause(&self.target)?;
        wire.clause(&self.active)?;
        wire.clause(&self.conclusion)?;
        wire.bindings(&self.active_freshening)?;
        wire.bindings(&self.unifier)?;
        wire.n(self.target_pivot)?;
        wire.n(self.active_pivot)?;
        wire.n(self.matches.len())?;
        for m in &self.matches {
            wire.n(*m)?;
        }
        Ok(())
    }
}

fn freshen(clause: &[Literal], next: &mut VarId) -> Result<(Vec<Literal>, Bindings), String> {
    let mut vars: Vec<_> = clause.iter().flat_map(|l| atom_vars(&l.atom)).collect();
    vars.sort_unstable();
    vars.dedup();
    let mut map = Vec::new();
    for v in vars {
        let fresh = *next;
        *next = next.checked_add(1).ok_or("variable-id overflow")?;
        map.push((v, Term::Var(fresh)));
    }
    let subst = map.iter().cloned().collect();
    let fresh = clause
        .iter()
        .map(|l| apply_substitution_literal(l, &subst))
        .collect();
    Ok((fresh, map))
}

/// Discover an unordered, exact-context step. Conclusion alpha conversion and
/// source equality symmetry are explicitly part of the unverified bridge.
fn certificate(
    source: &[Literal],
    target: &[Literal],
    goal: &[Literal],
    reverse: bool,
    limits: VerificationLimits,
) -> Result<Certificate, String> {
    let mut next = max_var_clause(source)
        .max(max_var_clause(target))
        .checked_add(1)
        .ok_or("variable-id overflow")?;
    let (fresh_source, source_map) = freshen(source, &mut next)?;
    let (fresh_target, target_map) = freshen(target, &mut next)?;
    let mut attempts = 0usize;
    let mut fuel = limits.max_comparison_steps.min(200_000);
    for (si, eq) in fresh_source.iter().enumerate() {
        if !eq.positive {
            continue;
        }
        let Atom::Eq(a, b) = &eq.atom else {
            continue;
        };
        for swapped in [false, true] {
            if swapped && !reverse {
                continue;
            }
            let (from, to) = if swapped { (b, a) } else { (a, b) };
            if matches!(from, Term::Var(_)) {
                continue;
            }
            for (ti, literal) in fresh_target.iter().enumerate() {
                for (slot, path) in atom_term_positions(&literal.atom) {
                    attempts += 1;
                    if attempts > limits.max_comparison_steps.min(10_000) {
                        return Err("certificate search budget exhausted".into());
                    }
                    let base = match &literal.atom {
                        Atom::Pred(_, args) => &args[slot],
                        Atom::Eq(a, b) => {
                            if slot == 0 {
                                a
                            } else {
                                b
                            }
                        }
                    };
                    let selected = term_at_position(base, &path).ok_or("invalid internal path")?;
                    let Some(subst) = bounded_unify(from, selected, &mut fuel)? else {
                        continue;
                    };
                    let replacement = instance(to, &subst, &mut fuel, 0)?;
                    // Replace *before* simultaneous substitution; this is
                    // equivalent on a valid raw position for the normalized unifier.
                    let atom = replace_atom_side(
                        &literal.atom,
                        slot,
                        replace_term_at(base, &path, replacement),
                    );
                    let mut expected: Vec<_> = fresh_source
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| *i != si)
                        .map(|(_, l)| instantiate(l, &subst, &mut fuel))
                        .collect::<Result<_, _>>()?;
                    expected.extend(
                        fresh_target
                            .iter()
                            .enumerate()
                            .map(|(i, l)| {
                                instantiate(
                                    &Literal {
                                        positive: l.positive,
                                        atom: if i == ti {
                                            atom.clone()
                                        } else {
                                            l.atom.clone()
                                        },
                                    },
                                    &subst,
                                    &mut fuel,
                                )
                            })
                            .collect::<Result<Vec<_>, _>>()?,
                    );
                    // Bounded ordered alpha matching: permutations, contraction,
                    // and folded demodulation are outside this bridge's scope.
                    if expected.len() != goal.len() {
                        continue;
                    }
                    let (mut forward, mut backward) = (HashMap::new(), HashMap::new());
                    if !expected.iter().zip(goal).all(|(a, b)| {
                        a.positive == b.positive
                            && atom_alpha_equiv(&a.atom, &b.atom, &mut forward, &mut backward)
                    }) {
                        continue;
                    }
                    let mut unifier: Vec<_> = subst
                        .iter()
                        .map(|(v, t)| Ok((*v, instance(t, &subst, &mut fuel, 0)?)))
                        .collect::<Result<_, String>>()?;
                    unifier.sort_by_key(|(v, _)| *v);
                    let mut oriented_source = source.to_vec();
                    if swapped {
                        let Atom::Eq(a, b) = &source[si].atom else {
                            unreachable!()
                        };
                        oriented_source[si].atom = Atom::Eq(b.clone(), a.clone());
                    }
                    return Ok(Certificate {
                        source: oriented_source,
                        target: target.to_vec(),
                        conclusion: expected,
                        source_map: source_map.clone(),
                        target_map: target_map.clone(),
                        unifier,
                        si,
                        ti,
                        slot,
                        path,
                    });
                }
            }
        }
    }
    Err("no supported exact ordered superposition certificate".into())
}

fn bounded_unify_atoms_into(
    a: &Atom,
    b: &Atom,
    subst: &mut HashMap<VarId, Term>,
    fuel: &mut usize,
) -> Result<bool, String> {
    let mut pending = Vec::new();
    match (a, b) {
        (Atom::Pred(p1, args1), Atom::Pred(p2, args2)) => {
            if p1 != p2 || args1.len() != args2.len() {
                return Ok(false);
            }
            pending.extend(args1.iter().cloned().zip(args2.iter().cloned()));
        }
        (Atom::Eq(a1, a2), Atom::Eq(b1, b2)) => {
            pending.push((a1.clone(), b1.clone()));
            pending.push((a2.clone(), b2.clone()));
        }
        _ => return Ok(false),
    }
    while let Some((a, b)) = pending.pop() {
        let a = instance(&a, subst, fuel, 0)?;
        let b = instance(&b, subst, fuel, 0)?;
        if a == b {
            continue;
        }
        match (a, b) {
            (Term::Var(v), t) | (t, Term::Var(v)) => {
                if t.free_vars().contains(&v) {
                    return Ok(false);
                }
                subst.insert(v, t);
            }
            (Term::App(f, a), Term::App(g, b)) => {
                if f != g || a.len() != b.len() {
                    return Ok(false);
                }
                pending.extend(a.into_iter().zip(b));
            }
        }
    }
    Ok(true)
}

fn bounded_unify_atoms(
    a: &Atom,
    b: &Atom,
    fuel: &mut usize,
) -> Result<Option<HashMap<VarId, Term>>, String> {
    let mut subst = HashMap::new();
    if bounded_unify_atoms_into(a, b, &mut subst, fuel)? {
        Ok(Some(subst))
    } else {
        Ok(None)
    }
}

fn resolution_certificate(
    left: &[Literal],
    right: &[Literal],
    goal: &[Literal],
    limits: VerificationLimits,
) -> Result<ResolutionCertificate, String> {
    let mut next = max_var_clause(left)
        .max(max_var_clause(right))
        .checked_add(1)
        .ok_or("variable-id overflow")?;
    let (fresh_left, left_map) = freshen(left, &mut next)?;
    let (fresh_right, right_map) = freshen(right, &mut next)?;
    let mut attempts = 0usize;
    let mut fuel = limits.max_comparison_steps.min(200_000);
    for (lp, l) in fresh_left.iter().enumerate() {
        if !l.positive {
            continue;
        }
        for (rp, r) in fresh_right.iter().enumerate() {
            if r.positive {
                continue;
            }
            attempts += 1;
            if attempts > limits.max_comparison_steps.min(10_000) {
                return Err("resolution certificate search budget exhausted".into());
            }
            let Some(subst) = bounded_unify_atoms(&l.atom, &r.atom, &mut fuel)? else {
                continue;
            };
            let mut expected: Vec<_> = fresh_left
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != lp)
                .map(|(_, lit)| instantiate(lit, &subst, &mut fuel))
                .collect::<Result<_, _>>()?;
            expected.extend(
                fresh_right
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| *i != rp)
                    .map(|(_, lit)| instantiate(lit, &subst, &mut fuel))
                    .collect::<Result<Vec<_>, _>>()?,
            );
            if expected.len() != goal.len() {
                continue;
            }
            let (mut forward, mut backward) = (HashMap::new(), HashMap::new());
            if !expected.iter().zip(goal).all(|(a, b)| {
                a.positive == b.positive
                    && atom_alpha_equiv(&a.atom, &b.atom, &mut forward, &mut backward)
            }) {
                continue;
            }
            let mut unifier: Vec<_> = subst
                .iter()
                .map(|(v, t)| Ok((*v, instance(t, &subst, &mut fuel, 0)?)))
                .collect::<Result<_, String>>()?;
            unifier.sort_by_key(|(v, _)| *v);
            return Ok(ResolutionCertificate {
                left: left.to_vec(),
                right: right.to_vec(),
                conclusion: expected,
                left_map,
                right_map,
                unifier,
                lp,
                rp,
            });
        }
    }
    Err("no supported first-order resolution certificate".into())
}

fn equality_resolution_certificate(
    parent: &[Literal],
    goal: &[Literal],
    limits: VerificationLimits,
) -> Result<EqualityResolutionCertificate, String> {
    let mut fuel = limits.max_comparison_steps.min(200_000);
    for (pivot, lit) in parent.iter().enumerate() {
        if lit.positive {
            continue;
        }
        let Atom::Eq(left, right) = &lit.atom else {
            continue;
        };
        let Some(subst) = bounded_unify(left, right, &mut fuel)? else {
            continue;
        };
        let expected: Vec<_> = parent
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != pivot)
            .map(|(_, l)| instantiate(l, &subst, &mut fuel))
            .collect::<Result<_, _>>()?;
        if expected.len() != goal.len() {
            continue;
        }
        let (mut forward, mut backward) = (HashMap::new(), HashMap::new());
        if !expected.iter().zip(goal).all(|(a, b)| {
            a.positive == b.positive
                && atom_alpha_equiv(&a.atom, &b.atom, &mut forward, &mut backward)
        }) {
            continue;
        }
        let mut unifier: Vec<_> = subst
            .iter()
            .map(|(v, t)| Ok((*v, instance(t, &subst, &mut fuel, 0)?)))
            .collect::<Result<_, String>>()?;
        unifier.sort_by_key(|(v, _)| *v);
        return Ok(EqualityResolutionCertificate {
            clause: parent.to_vec(),
            pivot,
            unifier,
            conclusion: expected,
        });
    }
    Err("no supported equality resolution certificate".into())
}

fn factoring_certificate(
    parent: &[Literal],
    goal: &[Literal],
    limits: VerificationLimits,
) -> Result<FactoringCertificate, String> {
    let mut fuel = limits.max_comparison_steps.min(200_000);
    for first in 0..parent.len() {
        for second in (first + 1)..parent.len() {
            let (l1, l2) = (&parent[first], &parent[second]);
            if l1.positive != l2.positive {
                continue;
            }
            let Some(subst) = bounded_unify_atoms(&l1.atom, &l2.atom, &mut fuel)? else {
                continue;
            };
            let expected: Vec<_> = parent
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != second)
                .map(|(_, l)| instantiate(l, &subst, &mut fuel))
                .collect::<Result<_, _>>()?;
            if expected.len() != goal.len() {
                continue;
            }
            let (mut forward, mut backward) = (HashMap::new(), HashMap::new());
            if !expected.iter().zip(goal).all(|(a, b)| {
                a.positive == b.positive
                    && atom_alpha_equiv(&a.atom, &b.atom, &mut forward, &mut backward)
            }) {
                continue;
            }
            let mut unifier: Vec<_> = subst
                .iter()
                .map(|(v, t)| Ok((*v, instance(t, &subst, &mut fuel, 0)?)))
                .collect::<Result<_, String>>()?;
            unifier.sort_by_key(|(v, _)| *v);
            return Ok(FactoringCertificate {
                clause: parent.to_vec(),
                first,
                second,
                unifier,
                conclusion: expected,
            });
        }
    }
    Err("no supported clause factoring certificate".into())
}

fn equality_factoring_certificate(
    parent: &[Literal],
    goal: &[Literal],
    limits: VerificationLimits,
) -> Result<EqualityFactoringCertificate, String> {
    let mut fuel = limits.max_comparison_steps.min(200_000);
    for first in 0..parent.len() {
        for second in (first + 1)..parent.len() {
            let (l1, l2) = (&parent[first], &parent[second]);
            if !l1.positive || !l2.positive {
                continue;
            }
            let (Atom::Eq(s, t), Atom::Eq(u, v)) = (&l1.atom, &l2.atom) else {
                continue;
            };
            let Some(subst) = bounded_unify(s, u, &mut fuel)? else {
                continue;
            };
            let s_inst = instance(s, &subst, &mut fuel, 0)?;
            let t_inst = instance(t, &subst, &mut fuel, 0)?;
            let v_inst = instance(v, &subst, &mut fuel, 0)?;
            let ineq = Literal {
                positive: false,
                atom: Atom::Eq(t_inst.clone(), v_inst),
            };
            let mut expected = Vec::new();
            expected.push(Literal {
                positive: true,
                atom: Atom::Eq(s_inst, t_inst),
            });
            expected.push(ineq);
            for (i, l) in parent.iter().enumerate() {
                if i != first && i != second {
                    expected.push(instantiate(l, &subst, &mut fuel)?);
                }
            }
            if expected.len() != goal.len() {
                continue;
            }
            let (mut forward, mut backward) = (HashMap::new(), HashMap::new());
            if !expected.iter().zip(goal).all(|(a, b)| {
                a.positive == b.positive
                    && atom_alpha_equiv(&a.atom, &b.atom, &mut forward, &mut backward)
            }) {
                continue;
            }
            let mut unifier: Vec<_> = subst
                .iter()
                .map(|(v, t)| Ok((*v, instance(t, &subst, &mut fuel, 0)?)))
                .collect::<Result<_, String>>()?;
            unifier.sort_by_key(|(v, _)| *v);
            return Ok(EqualityFactoringCertificate {
                clause: parent.to_vec(),
                first,
                second,
                unifier,
                conclusion: expected,
            });
        }
    }
    Err("no supported equality factoring certificate".into())
}

#[allow(clippy::too_many_arguments)]
fn match_active_literals(
    active: &[Literal],
    target: &[Literal],
    active_idx: usize,
    active_pivot: usize,
    target_pivot: usize,
    subst: &HashMap<VarId, Term>,
    matches: &mut Vec<usize>,
    fuel: &mut usize,
) -> Result<Option<HashMap<VarId, Term>>, String> {
    if active_idx == active.len() {
        return Ok(Some(subst.clone()));
    }
    if active_idx == active_pivot {
        matches.push(target_pivot);
        let res = match_active_literals(
            active,
            target,
            active_idx + 1,
            active_pivot,
            target_pivot,
            subst,
            matches,
            fuel,
        )?;
        if res.is_some() {
            return Ok(res);
        }
        matches.pop();
        return Ok(None);
    }
    let act_lit = &active[active_idx];
    for tgt_idx in 0..target.len() {
        if tgt_idx == target_pivot {
            continue;
        }
        let tgt_lit = &target[tgt_idx];
        if act_lit.positive != tgt_lit.positive {
            continue;
        }
        let mut branch_subst = subst.clone();
        if bounded_unify_atoms_into(&act_lit.atom, &tgt_lit.atom, &mut branch_subst, fuel)? {
            matches.push(tgt_idx);
            let res = match_active_literals(
                active,
                target,
                active_idx + 1,
                active_pivot,
                target_pivot,
                &branch_subst,
                matches,
                fuel,
            )?;
            if res.is_some() {
                return Ok(res);
            }
            matches.pop();
        }
    }
    Ok(None)
}

fn subsumption_resolution_certificate(
    p0: &[Literal],
    p1: &[Literal],
    goal: &[Literal],
    limits: VerificationLimits,
) -> Result<(SubsumptionResolutionCertificate, bool), String> {
    let mut fuel = limits.max_comparison_steps.min(200_000);
    for (target, active, flipped) in [(p0, p1, false), (p1, p0, true)] {
        if target.is_empty() || active.is_empty() {
            continue;
        }
        for target_pivot in 0..target.len() {
            let mut expected = target.to_vec();
            expected.remove(target_pivot);
            if expected.len() != goal.len() {
                continue;
            }
            let (mut forward, mut backward) = (HashMap::new(), HashMap::new());
            if !expected.iter().zip(goal).all(|(a, b)| {
                a.positive == b.positive
                    && atom_alpha_equiv(&a.atom, &b.atom, &mut forward, &mut backward)
            }) {
                continue;
            }
            let mut next = max_var_clause(target)
                .max(max_var_clause(active))
                .checked_add(1)
                .ok_or("variable-id overflow")?;
            let (fresh_active, active_freshening) = freshen(active, &mut next)?;
            for active_pivot in 0..fresh_active.len() {
                if fresh_active[active_pivot].positive == target[target_pivot].positive {
                    continue;
                }
                let mut base_subst = HashMap::new();
                if !bounded_unify_atoms_into(
                    &fresh_active[active_pivot].atom,
                    &target[target_pivot].atom,
                    &mut base_subst,
                    &mut fuel,
                )? {
                    continue;
                }
                let mut matches = Vec::new();
                if let Some(final_subst) = match_active_literals(
                    &fresh_active,
                    target,
                    0,
                    active_pivot,
                    target_pivot,
                    &base_subst,
                    &mut matches,
                    &mut fuel,
                )? {
                    let mut unifier: Vec<_> = final_subst
                        .iter()
                        .map(|(v, t)| Ok((*v, instance(t, &final_subst, &mut fuel, 0)?)))
                        .collect::<Result<_, String>>()?;
                    unifier.sort_by_key(|(v, _)| *v);
                    return Ok((
                        SubsumptionResolutionCertificate {
                            target: target.to_vec(),
                            active: active.to_vec(),
                            conclusion: expected,
                            active_freshening,
                            unifier,
                            target_pivot,
                            active_pivot,
                            matches,
                        },
                        flipped,
                    ));
                }
            }
        }
    }
    Err("no supported subsumption resolution certificate".into())
}

fn supported_clause(
    symbols: &mut SymbolTable,
    formula: &AnnotatedFormula<'_>,
    limits: VerificationLimits,
) -> Result<Vec<Literal>, String> {
    if !formula.is_cnf() {
        return Err("only CNF clauses are supported by the shadow bridge".into());
    }
    fn term(
        t: &FOFTerm<'_>,
        symbols: &mut SymbolTable,
        vars: &mut HashMap<String, VarId>,
        depth: usize,
        work: &mut usize,
    ) -> Result<Term, String> {
        *work = work.checked_sub(1).ok_or("syntax work ceiling")?;
        if depth >= 64 {
            return Err("term depth ceiling".into());
        }
        match t {
            FOFTerm::Variable(name) => {
                let next = u32::try_from(vars.len()).map_err(|_| "variable ceiling")?;
                Ok(Term::Var(*vars.entry((*name).into()).or_insert(next)))
            }
            FOFTerm::Function(name, args) if args.len() <= 2 => Ok(Term::App(
                symbols.intern(name.as_str()),
                args.iter()
                    .map(|t| term(t, symbols, vars, depth + 1, work))
                    .collect::<Result<_, _>>()?,
            )),
            _ => {
                Err("unsupported interpreted, distinct-object, numeric or larger-arity term".into())
            }
        }
    }
    let AnnotatedFormula::CNF(f) = formula else {
        unreachable!()
    };
    let CNFStatement::Logical(body) = &f.formula;
    let mut body = body;
    let mut depth = 0;
    while let CNFFormula::Parens(inner) = body {
        depth += 1;
        if depth >= 64 {
            return Err("clause parentheses depth ceiling".into());
        }
        body = inner;
    }
    let CNFFormula::Disjunction(literals) = body else {
        unreachable!()
    };
    if literals.len() > limits.max_clause_literals.min(64) {
        return Err("clause width ceiling".into());
    }
    let mut vars = HashMap::new();
    let mut work = limits.max_formula_nodes.min(100_000);
    let mut clause = Vec::new();
    for literal in literals {
        let (positive, atom) = match literal {
            CNFLiteral::Positive(mrs_tptp::CNFAtomicFormula::False) if literals.len() == 1 => {
                continue;
            }
            CNFLiteral::Positive(mrs_tptp::CNFAtomicFormula::Plain(name, args))
            | CNFLiteral::Negative(mrs_tptp::CNFAtomicFormula::Plain(name, args)) => {
                if args.len() > 2 {
                    return Err("predicate arity ceiling".into());
                }
                (
                    matches!(literal, CNFLiteral::Positive(_)),
                    Atom::Pred(
                        symbols.intern(name.as_str()),
                        args.iter()
                            .map(|t| term(t, symbols, &mut vars, 0, &mut work))
                            .collect::<Result<_, _>>()?,
                    ),
                )
            }
            CNFLiteral::Equality(a, b) | CNFLiteral::Inequality(a, b) => (
                matches!(literal, CNFLiteral::Equality(_, _)),
                Atom::Eq(
                    term(a, symbols, &mut vars, 0, &mut work)?,
                    term(b, symbols, &mut vars, 0, &mut work)?,
                ),
            ),
            _ => return Err("unsupported CNF atom; no literal may be silently dropped".into()),
        };
        clause.push(Literal { positive, atom });
    }
    // Check all constructors before certificate discovery traverses them.
    Wire::new(0).clause(&clause)?;
    Ok(clause)
}

/// Parse actual TPTP parents/conclusion and check one archived/local step.
/// This makes no proof-DAG or problem-provenance claim.
pub fn check_tptp_step(
    text: &str,
    source: &str,
    target: &str,
    conclusion: &str,
    config: &VerusConfig,
) -> ShadowReport {
    let result = (|| {
        if text.len() > MAX_BYTES {
            return Err("TPTP byte ceiling".into());
        }
        let parsed = mrs_tptp::parse_tptp(text).map_err(|e| e.to_string())?;
        if parsed.formulas.len() > MAX_NODES {
            return Err("TPTP formula ceiling".into());
        }
        if !parsed.includes.is_empty() {
            return Err("unresolved includes".into());
        }
        let mut names = HashSet::new();
        for f in &parsed.formulas {
            if !names.insert(f.name()) {
                return Err("duplicate formula name".into());
            }
        }
        let mut symbols = SymbolTable::new();
        let mut get = |name| {
            let f = parsed
                .formulas
                .iter()
                .find(|f| f.name() == name)
                .ok_or("missing step formula")?;
            supported_clause(&mut symbols, f, VerificationLimits::default())
        };
        let (s, t, c) = (get(source)?, get(target)?, get(conclusion)?);
        let mut signatures = HashMap::new();
        for l in s.iter().chain(&t).chain(&c) {
            register_signatures(&Formula::Atom(l.atom.clone()), &mut signatures)?;
        }
        let cert = certificate(&s, &t, &c, true, VerificationLimits::default())?;
        let mut wire = Wire::new(1);
        cert.encode(&mut wire)?;
        wire.finish()
    })();
    match result {
        Ok(bytes) => run(config, bytes, 1, false),
        Err(e) => unknown(e),
    }
}

enum Kind {
    Input,
    Superposition(Certificate),
    Resolution(usize, usize),
    FirstOrderResolution(ResolutionCertificate),
    EqualityResolution(EqualityResolutionCertificate),
    Factoring(FactoringCertificate),
    EqualityFactoring(EqualityFactoringCertificate),
    SubsumptionResolution(SubsumptionResolutionCertificate),
}
struct WireNode {
    clause: Vec<Literal>,
    parents: Vec<usize>,
    kind: Kind,
}

fn dag_request(
    problem: &mrs_tptp::TPTPProblem<'_>,
    proof: &mrs_tptp::TPTPProblem<'_>,
    source: Option<&str>,
    limits: VerificationLimits,
) -> Result<(Vec<u8>, usize), String> {
    if !problem.includes.is_empty() || !proof.includes.is_empty() {
        return Err("unresolved includes".into());
    }
    if problem.formulas.len() > MAX_NODES || proof.formulas.len() > MAX_NODES {
        return Err("DAG node ceiling".into());
    }
    let mut signatures = HashMap::new();
    let dag = build_dag(proof, limits).map_err(|v| v.to_string())?;
    let mut symbols = SymbolTable::new();
    let mut inputs = Vec::new();
    for f in &problem.formulas {
        if !f.role().is_premise() && f.role() != FormulaRole::NegatedConjecture {
            return Err("unsupported problem role or conjecture transformation".into());
        }
        let clause = supported_clause(&mut symbols, f, limits)?;
        for l in &clause {
            register_signatures(&Formula::Atom(l.atom.clone()), &mut signatures)?;
        }
        inputs.push(clause);
    }
    let mut nodes: Vec<WireNode> = Vec::new();
    let mut indices = HashMap::new();
    for index in &dag.topo {
        let node = &dag.nodes[*index];
        // The parser-owned input is immutable for the duration of this call;
        // Rust's shared borrow prevents concurrent mutation while the adapter
        // validates and lowers this node.
        let node_formula = node.formula;
        if node
            .formula
            .annotations()
            .is_some_and(|a| !a.parent_refs_well_formed())
        {
            return Err("unsupported parent pedigree".into());
        }
        if !node.parents.is_empty() {
            let annotation = node_formula
                .annotations()
                .ok_or("missing inference annotation")?;
            let GeneralTerm::Function(_, args) = &annotation.source else {
                return Err("unsupported inference source".into());
            };
            let Some(GeneralTerm::List(parents)) = args.get(2) else {
                return Err("malformed inference parents".into());
            };
            if !parents.iter().all(|p| matches!(p, GeneralTerm::Word(_))) {
                return Err("nested/annotated inference parents require explicit replay".into());
            }
        }
        let raw = supported_clause(&mut symbols, node_formula, limits)?;
        for l in &raw {
            register_signatures(&Formula::Atom(l.atom.clone()), &mut signatures)?;
        }
        let parents: Vec<usize> = node
            .parents
            .iter()
            .map(|p| {
                if p.negated {
                    return Err("negated pedigree is unsupported".into());
                }
                indices
                    .get(p.name)
                    .copied()
                    .ok_or_else(|| "missing/forward parent".into())
            })
            .collect::<Result<_, String>>()?;
        let (kind, clause, parents) = if parents.is_empty() {
            verify_leaf(node, problem, source, &mut symbols, limits).map_err(|v| v.to_string())?;
            // Exact raw syntax, no AC/alpha leaf relaxation in this bridge.
            if !inputs.contains(&raw) {
                return Err("leaf differs from exact problem clause".into());
            }
            (Kind::Input, raw, parents)
        } else if parents.len() == 2 {
            let (left, right) = (&nodes[parents[0]].clause, &nodes[parents[1]].clause);
            match node.rule {
                Some("superposition") | Some("paramodulation") | Some("demodulation") => {
                    let (c, flipped) = if let Ok(c) = certificate(left, right, &raw, false, limits)
                    {
                        (c, false)
                    } else if let Ok(c) = certificate(right, left, &raw, false, limits) {
                        (c, true)
                    } else {
                        return Err(
                            "no supported superposition/paramodulation/demodulation certificate"
                                .into(),
                        );
                    };
                    let conclusion = c.conclusion.clone();
                    let parents = if flipped {
                        vec![parents[1], parents[0]]
                    } else {
                        parents
                    };
                    (Kind::Superposition(c), conclusion, parents)
                }
                Some("resolution") => {
                    let mut exact_pivot = None;
                    for (lp, l) in left.iter().enumerate() {
                        for (rp, r) in right.iter().enumerate() {
                            if l.positive && !r.positive && l.atom == r.atom {
                                let expected: Vec<_> = left
                                    .iter()
                                    .enumerate()
                                    .filter(|(i, _)| *i != lp)
                                    .chain(right.iter().enumerate().filter(|(i, _)| *i != rp))
                                    .map(|(_, l)| l.clone())
                                    .collect();
                                if expected == raw {
                                    exact_pivot = Some(((lp, rp), false));
                                }
                            }
                        }
                    }
                    if exact_pivot.is_none() {
                        for (rp, r) in right.iter().enumerate() {
                            for (lp, l) in left.iter().enumerate() {
                                if r.positive && !l.positive && r.atom == l.atom {
                                    let expected: Vec<_> = right
                                        .iter()
                                        .enumerate()
                                        .filter(|(i, _)| *i != rp)
                                        .chain(left.iter().enumerate().filter(|(i, _)| *i != lp))
                                        .map(|(_, l)| l.clone())
                                        .collect();
                                    if expected == raw {
                                        exact_pivot = Some(((rp, lp), true));
                                    }
                                }
                            }
                        }
                    }
                    if let Some(((lp, rp), flipped)) = exact_pivot {
                        let parents = if flipped {
                            vec![parents[1], parents[0]]
                        } else {
                            parents
                        };
                        (Kind::Resolution(lp, rp), raw, parents)
                    } else if let Ok(c) = resolution_certificate(left, right, &raw, limits) {
                        let conclusion = c.conclusion.clone();
                        (Kind::FirstOrderResolution(c), conclusion, parents)
                    } else if let Ok(c) = resolution_certificate(right, left, &raw, limits) {
                        let conclusion = c.conclusion.clone();
                        (
                            Kind::FirstOrderResolution(c),
                            conclusion,
                            vec![parents[1], parents[0]],
                        )
                    } else {
                        return Err("no supported first-order resolution certificate".into());
                    }
                }
                Some("subsumption_resolution") => {
                    let (c, flipped) =
                        subsumption_resolution_certificate(left, right, &raw, limits)?;
                    let conclusion = c.conclusion.clone();
                    let parents = if flipped {
                        vec![parents[1], parents[0]]
                    } else {
                        parents
                    };
                    (Kind::SubsumptionResolution(c), conclusion, parents)
                }
                _ => return Err("unsupported inference rule".into()),
            }
        } else if parents.len() == 1 {
            let parent = &nodes[parents[0]].clause;
            match node.rule {
                Some("equality_resolution")
                | Some("destructive_equality_resolution")
                | Some("trivial_inequality_removal") => {
                    let c = equality_resolution_certificate(parent, &raw, limits)?;
                    let conclusion = c.conclusion.clone();
                    (Kind::EqualityResolution(c), conclusion, parents)
                }
                Some("factoring") | Some("condensation") => {
                    let c = factoring_certificate(parent, &raw, limits)?;
                    let conclusion = c.conclusion.clone();
                    (Kind::Factoring(c), conclusion, parents)
                }
                Some("equality_factoring") => {
                    let c = equality_factoring_certificate(parent, &raw, limits)?;
                    let conclusion = c.conclusion.clone();
                    (Kind::EqualityFactoring(c), conclusion, parents)
                }
                _ => return Err("unsupported inference rule".into()),
            }
        } else {
            return Err("unsupported parent count".into());
        };
        indices.insert(node.name, nodes.len());
        nodes.push(WireNode {
            clause,
            parents,
            kind,
        });
    }
    let roots: Vec<_> = nodes
        .iter()
        .enumerate()
        .filter(|(i, n)| n.clause.is_empty() && !nodes.iter().any(|n| n.parents.contains(i)))
        .map(|(i, _)| i)
        .collect();
    if roots.len() != 1 {
        return Err("expected exactly one empty root".into());
    }
    let mut wire = Wire::new(2);
    wire.n(nodes.len())?;
    wire.n(roots[0])?;
    wire.n(inputs.len())?;
    for clause in &inputs {
        wire.clause(clause)?;
    }
    for (i, node) in nodes.iter().enumerate() {
        wire.n(i)?;
        wire.clause(&node.clause)?;
        wire.n(match node.kind {
            Kind::Input => 0,
            Kind::Superposition(_) => 1,
            Kind::Resolution(_, _) => 3,
            Kind::FirstOrderResolution(_) => 4,
            Kind::EqualityResolution(_) => 5,
            Kind::Factoring(_) => 6,
            Kind::EqualityFactoring(_) => 7,
            Kind::SubsumptionResolution(_) => 8,
        })?;
        wire.n(node.parents.len())?;
        for p in &node.parents {
            wire.n(*p)?;
        }
        match &node.kind {
            Kind::Input => {}
            Kind::Superposition(c) => c.encode(&mut wire)?,
            Kind::Resolution(lp, rp) => {
                wire.n(*lp)?;
                wire.n(*rp)?;
            }
            Kind::FirstOrderResolution(c) => c.encode(&mut wire)?,
            Kind::EqualityResolution(c) => c.encode(&mut wire)?,
            Kind::Factoring(c) => c.encode(&mut wire)?,
            Kind::EqualityFactoring(c) => c.encode(&mut wire)?,
            Kind::SubsumptionResolution(c) => c.encode(&mut wire)?,
        }
    }
    Ok((wire.finish()?, nodes.len()))
}

/// Run the compiled verified DAG checker as an independent shadow check.
pub fn check_tptp_dag(
    problem: &mrs_tptp::TPTPProblem<'_>,
    proof: &mrs_tptp::TPTPProblem<'_>,
    source: Option<&str>,
    limits: VerificationLimits,
    config: &VerusConfig,
) -> ShadowReport {
    let (problem, proof) = match (resolved_problem_snapshot(problem), owned_snapshot(proof)) {
        (Ok(problem), Ok(proof)) => (problem, proof),
        (Err(error), _) | (_, Err(error)) => return unknown(error),
    };
    match dag_request(problem.problem(), proof.problem(), source, limits) {
        Ok((bytes, count)) => run(config, bytes, count, true),
        Err(e) => unknown(e),
    }
}

/// Make the included-premise closure explicit and owned before lowering.
/// Unresolved includes are rejected. Formula-level annotations are preserved
/// exactly, and duplicate formula names fail closed.
fn resolved_problem_snapshot(
    problem: &mrs_tptp::TPTPProblem<'_>,
) -> Result<mrs_tptp::owned::OwnedTPTPProblem, String> {
    if !problem.includes.is_empty() {
        return Err("unresolved problem includes; load through mrs-proover first".into());
    }
    let mut source = String::new();
    for formula in &problem.formulas {
        source.push_str(&formula.to_string());
        source.push('\n');
        if source.len() > MAX_BYTES {
            return Err("resolved problem snapshot byte ceiling".into());
        }
    }
    let owned = mrs_tptp::owned::parse_tptp_owned(source).map_err(|e| e.to_string())?;
    if owned.problem().formulas.len() != problem.formulas.len() {
        return Err("problem snapshot did not preserve every formula".into());
    }
    for (original, reparsed) in problem.formulas.iter().zip(&owned.problem().formulas) {
        if original.name() != reparsed.name()
            || original.role() != reparsed.role()
            || original.to_string() != reparsed.to_string()
        {
            return Err("problem snapshot changed formula identity or syntax".into());
        }
    }
    let mut names = HashSet::new();
    if !owned
        .problem()
        .formulas
        .iter()
        .all(|f| names.insert(f.name()))
    {
        return Err("duplicate formula name in resolved problem closure".into());
    }
    Ok(owned)
}

fn owned_snapshot(
    problem: &mrs_tptp::TPTPProblem<'_>,
) -> Result<mrs_tptp::owned::OwnedTPTPProblem, String> {
    if !problem.includes.is_empty() {
        return Err("proof includes are unsupported".into());
    }
    let mut source = String::new();
    for formula in &problem.formulas {
        source.push_str(&formula.to_string());
        source.push('\n');
        if source.len() > MAX_BYTES {
            return Err("proof snapshot byte ceiling".into());
        }
    }
    let owned = mrs_tptp::owned::parse_tptp_owned(source).map_err(|e| e.to_string())?;
    if owned.problem().formulas.len() != problem.formulas.len() {
        return Err("proof snapshot did not preserve every formula".into());
    }
    Ok(owned)
}

fn run(config: &VerusConfig, bytes: Vec<u8>, nodes: usize, dag: bool) -> ShadowReport {
    let mut report = unknown("checker did not respond");
    report.request_sha256 = Some(digest(&bytes));
    report.checked_nodes = nodes;
    let result = (|| -> Result<ShadowVerdict, String> {
        if config.timeout.is_zero() || config.timeout > Duration::from_secs(60) {
            return Err("invalid checker timeout".into());
        }
        let (artifact, hash) = snapshot_artifact(&config.executable)?;
        report.artifact_sha256 = Some(hash.clone());
        if config.expected_sha256.len() != 64 || hash != config.expected_sha256 {
            return Err("checker artifact hash mismatch".into());
        }
        let mut command = Command::new(&artifact.0);
        command.arg("--wire-v1");
        configure_child(&mut command);
        let mut child = command.spawn().map_err(|e| format!("checker spawn: {e}"))?;
        let child_id = child.id();
        let mut stdin = child.stdin.take().ok_or("checker stdin missing")?;
        let stdout = child.stdout.take().ok_or("checker stdout missing")?;
        // Both pipes are drained concurrently; the deadline also covers blocked
        // input writes and a peer that floods stdout instead of reading stdin.
        let writer = std::thread::spawn(move || stdin.write_all(&bytes));
        let reader = std::thread::spawn(move || {
            let mut output = Vec::new();
            stdout.take(1025).read_to_end(&mut output).map(|_| output)
        });
        let deadline = Instant::now() + config.timeout;
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break Ok(status),
                Ok(None) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(5))
                }
                Ok(None) => break Err("checker timeout".to_owned()),
                Err(e) => break Err(format!("checker wait: {e}")),
            }
        };
        if status.is_err() {
            let _ = child.kill();
            let _ = child.wait();
            #[cfg(unix)]
            kill_process_group(child_id);
        }
        // An exited child may have forked peers holding pipe ends. Never block
        // waiting on their threads; unfinished pipe work is an unknown result.
        while (!writer.is_finished() || !reader.is_finished()) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        if !writer.is_finished() || !reader.is_finished() {
            #[cfg(unix)]
            kill_process_group(child_id);
            if !status.as_ref().is_ok_and(|exit| exit.success()) {
                return Err("checker process failed".into());
            }
            return Err("checker pipe timeout".into());
        }
        writer
            .join()
            .map_err(|_| "checker writer panicked")?
            .map_err(|e| e.to_string())?;
        let output = reader
            .join()
            .map_err(|_| "checker reader panicked")?
            .map_err(|e| e.to_string())?;
        if !status?.success() || output.len() > 1024 {
            return Err("checker failed or exceeded response bound".into());
        }
        match output.as_slice() {
            b"MRS_VERUS 1 Accepted\n" if !dag => Ok(ShadowVerdict::AcceptedStepIr),
            b"MRS_VERUS 1 CertifiedIR\n" if dag => Ok(ShadowVerdict::CertifiedDagIr),
            b"MRS_VERUS 1 Rejected\n" => Ok(ShadowVerdict::RejectedIr),
            b"MRS_VERUS 1 Unknown\n" => Err("verified checker returned Unknown".into()),
            _ => Err("checker version or protocol mismatch".into()),
        }
    })();
    report.verdict = result.unwrap_or_else(ShadowVerdict::Unknown);
    report
}

#[cfg(unix)]
fn kill_process_group(child_id: u32) {
    // SAFETY: CommandExt assigned a fresh process group whose id is the child
    // pid. Kill only on timeout or leaked descendant pipe descriptors.
    unsafe {
        libc::kill(-(child_id as i32), libc::SIGKILL);
    }
}

#[cfg(unix)]
fn set_limit(resource: libc::__rlimit_resource_t, value: u64) -> std::io::Result<()> {
    let limit = libc::rlimit {
        rlim_cur: value as libc::rlim_t,
        rlim_max: value as libc::rlim_t,
    };
    // SAFETY: points to a properly initialized rlimit value for this syscall.
    if unsafe { libc::setrlimit(resource, &limit) } == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

/// Independent strict-kernel verdict plus optional formal-track shadow evidence.
/// A shadow result cannot turn an unsupported/invalid strict proof into a theorem.
pub fn verify_with_shadow(
    problem: &mrs_tptp::TPTPProblem<'_>,
    proof: &mrs_tptp::TPTPProblem<'_>,
    source: Option<&str>,
    limits: VerificationLimits,
    config: &VerusConfig,
) -> (KernelVerdict, ShadowReport) {
    let kernel = verify_strict_with_source(problem, proof, source, limits);
    let shadow = check_tptp_dag(problem, proof, source, limits, config);
    (kernel, shadow)
}

/// Explicit dual-check gate. Certification requires both independent strict
/// replay and a positive Verus IR-DAG result. This is an opt-in trusted-adapter
/// gate, not a proof that TPTP decoding/lowering is verified. Any disagreement,
/// unsupported shape, timeout or protocol failure suppresses certification.
pub fn verify_with_verus_gate(
    problem: &mrs_tptp::TPTPProblem<'_>,
    proof: &mrs_tptp::TPTPProblem<'_>,
    source: Option<&str>,
    limits: VerificationLimits,
    config: &VerusConfig,
) -> (KernelVerdict, ShadowReport) {
    let (kernel, shadow) = verify_with_shadow(problem, proof, source, limits, config);
    let verdict = match (&kernel, &shadow.verdict) {
        (KernelVerdict::Certified, ShadowVerdict::CertifiedDagIr) => KernelVerdict::Certified,
        (KernelVerdict::Rejected(_), _) | (KernelVerdict::Inconclusive(_), _) => kernel,
        _ => KernelVerdict::Inconclusive(format!("Verus dual-check gate: {:?}", shadow.verdict)),
    };
    (verdict, shadow)
}

#[cfg(all(test, unix))]
mod process_tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static SEQUENCE: AtomicUsize = AtomicUsize::new(0);

    #[test]
    fn failed_malformed_and_hanging_peers_cannot_certify() {
        let shell = Command::new("sh")
            .args(["-c", "command -v sh"])
            .output()
            .unwrap();
        assert!(shell.status.success());
        let shell = String::from_utf8(shell.stdout).unwrap();
        let dir = std::env::temp_dir().join(format!(
            "mrs-verus-peer-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&dir).unwrap();
        let binary = dir.join("peer");
        for body in [
            "printf 'MRS_VERUS 2 CertifiedIR\\n'",
            "printf 'MRS_VERUS 1 CertifiedIR\\nextra'",
            "printf 'MRS_VERUS 1 CertifiedIR\\n'; exit 1",
            "printf 'MRS_VERUS 1 Accepted\\n'",
            "sleep 10",
            "sleep 10 & exit 0",
            "sleep 10 >&1 & printf 'MRS_VERUS 2 CertifiedIR\\n'; exit 0",
            "while :; do printf 'xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx'; done",
            "pwd; printf '%s\\n' \"${LEAK_ME:-missing}\"",
        ] {
            let script = format!("#!{}\n{body}\n", shell.trim());
            std::fs::write(&binary, &script).unwrap();
            std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
            let config = VerusConfig {
                executable: binary.clone(),
                expected_sha256: digest(script.as_bytes()),
                timeout: Duration::from_millis(100),
            };
            let start = Instant::now();
            let report = run(&config, b"1 2 1000".to_vec(), 3, true);
            assert!(
                matches!(report.verdict, ShadowVerdict::Unknown(_)),
                "body={body:?} {report:?}"
            );
            assert!(start.elapsed() < Duration::from_secs(2));
        }
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn child_environment_directory_and_rlimits_are_sanitized() {
        let shell = Command::new("sh")
            .args(["-c", "command -v sh"])
            .output()
            .unwrap();
        assert!(shell.status.success());
        let shell = String::from_utf8(shell.stdout).unwrap();
        let dir = std::env::temp_dir().join(format!(
            "mrs-verus-limits-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&dir).unwrap();
        let binary = dir.join("peer");
        let script = format!(
            "#!{}\nprintf '%s\\n' \"${{LEAK_ME:-missing}}\"\npwd\nulimit -v\nulimit -t\nulimit -f\nulimit -c\nprintf 'MRS_VERUS 1 CertifiedIR\\n'\n",
            shell.trim()
        );
        std::fs::write(&binary, &script).unwrap();
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
        let config = VerusConfig {
            executable: binary.clone(),
            expected_sha256: digest(script.as_bytes()),
            timeout: Duration::from_secs(2),
        };
        let old_leak = std::env::var_os("LEAK_ME");
        unsafe {
            std::env::set_var("LEAK_ME", "must-not-be-inherited");
        }
        let report = run(&config, b"1 2 1000".to_vec(), 3, true);
        match old_leak {
            Some(value) => unsafe {
                std::env::set_var("LEAK_ME", value);
            },
            None => unsafe {
                std::env::remove_var("LEAK_ME");
            },
        }
        assert!(
            matches!(report.verdict, ShadowVerdict::Unknown(_)),
            "{report:?}"
        );
        // The runner's stdout parser does not expose peer output. Execute the
        // bounded policy check directly in a clean environment as a harness
        // sanity check, then exercise an output that fails the exact protocol.
        let mut command = Command::new(&binary);
        configure_child(&mut command);
        let direct = command.output().unwrap();
        assert!(direct.status.success());
        let output = String::from_utf8_lossy(&direct.stdout);
        let lines: Vec<_> = output.lines().collect();
        assert_eq!(lines[0], "missing");
        assert_eq!(lines[1], "/");
        assert_eq!(lines[2], (CHILD_MEMORY_BYTES / 1024).to_string());
        assert_eq!(lines[3], CHILD_CPU_SECONDS.to_string());
        // Linux shells round file-size limits to filesystem blocks; the
        // requested byte cap is 1 MiB (normally printed as 2048 512-byte
        // blocks), and core dumps remain disabled.
        assert_eq!(lines[4], "2048");
        assert_eq!(lines[5], "0");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn executable_snapshot_is_immutable_when_caller_path_changes() {
        let dir = std::env::temp_dir().join(format!("mrs-verus-snapshot-{}", std::process::id()));
        let _ = std::fs::create_dir(&dir);
        let original = dir.join("checker");
        std::fs::write(&original, b"first artifact").unwrap();
        let (snapshot, hash) = snapshot_artifact(&original).unwrap();
        std::fs::write(&original, b"replaced artifact").unwrap();
        assert_eq!(hash, digest(b"first artifact"));
        assert_eq!(std::fs::read(&snapshot.0).unwrap(), b"first artifact");
        drop(snapshot);
        assert!(!dir.join("unused").exists());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
