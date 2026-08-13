use crate::{HashMap, HashSet};
use smallvec::SmallVec;

use mrs_cadical::Solver;

use mrs_core::SymbolTable;
use mrs_core::clause::{
    AvatarComponent, Clause, ClauseCertificate, ClauseIdGen, ClauseSource, Literal,
};
use mrs_core::formula::Atom;
use mrs_core::term::{Term, VarId};
use mrs_core::term_bank::{IdAtom, IdClause, IdLiteral, TermBank, TermNode};

pub struct AvatarContext {
    pub solver: Solver,
    // Mapping from normalized split component to AVATAR propositional variable (u32).
    // Using string representation as a simple normalization for now.
    pub component_vars: HashMap<String, u32>,
    pub next_var: u32,

    // The current SAT model (true variables)
    pub current_model: HashSet<u32>,

    /// Exact clauses submitted to the SAT solver, in insertion order.
    /// This manifest is replayed by proof-mode CaDiCaL after search.
    pub sat_manifest: Vec<Vec<i32>>,
    /// Clause IDs for the split constraints in `sat_manifest`.
    ///
    /// Branch-exclusion clauses are also present in `sat_manifest`, so the
    /// split IDs are tracked separately instead of reconstructing them by
    /// scanning the mutable clause store during proof export.
    pub sat_split_ids: Vec<mrs_core::clause::ClauseId>,
}

impl AvatarContext {
    pub fn new() -> Self {
        Self {
            solver: Solver::new(),
            component_vars: HashMap::default(),
            next_var: 1, // cadical variables start from 1
            current_model: HashSet::default(),
            sat_manifest: Vec::new(),
            sat_split_ids: Vec::new(),
        }
    }

    pub fn add_sat_clause(&mut self, clause: Vec<i32>) {
        self.solver.add_clause(&clause);
        self.sat_manifest.push(clause);
    }

    /// Splits a clause into variable-disjoint components.
    /// If the clause cannot be split (only 1 component), returns None.
    pub fn split_clause(
        &mut self,
        clause: &Clause,
        id_gen: &mut ClauseIdGen,
    ) -> Option<Vec<Clause>> {
        if clause.len() <= 1 {
            return None;
        }

        // Find connected components of literals by variable sharing.
        // Also group all ground literals into a single component, because splitting ground
        // literals into separate components is valid, but usually we split into variable-disjoint parts.
        // Actually, Vampire puts all ground literals into one component if possible?
        // Let's do a simple disjoint-set over literals.
        let n = clause.literals.len();
        let mut parent = (0..n).collect::<Vec<_>>();

        fn find(parent: &mut [usize], i: usize) -> usize {
            if parent[i] == i {
                i
            } else {
                let p = parent[i];
                parent[i] = find(parent, p);
                parent[i]
            }
        }

        fn union(parent: &mut [usize], i: usize, j: usize) {
            let pi = find(parent, i);
            let pj = find(parent, j);
            if pi != pj {
                parent[pi] = pj;
            }
        }

        // Map VarId to literal index
        let mut var_to_lit: HashMap<VarId, usize> = HashMap::default();

        for (i, lit) in clause.literals.iter().enumerate() {
            let vars = literal_vars(lit);
            for v in vars {
                if let Some(&first_i) = var_to_lit.get(&v) {
                    union(&mut parent, i, first_i);
                } else {
                    var_to_lit.insert(v, i);
                }
            }
        }

        // Group literals by component
        let mut components: HashMap<usize, Vec<Literal>> = HashMap::default();
        let mut ground_lits: Vec<Literal> = Vec::new();

        for (i, lit) in clause.literals.iter().enumerate() {
            let p = find(&mut parent, i);
            let vars = literal_vars(lit);
            if vars.is_empty() {
                ground_lits.push(lit.clone());
            } else {
                components.entry(p).or_default().push(lit.clone());
            }
        }

        // Each ground literal becomes its own component!
        let num_components = components.len() + ground_lits.len();

        if num_components <= 1 {
            return None; // Cannot be split
        }

        let mut parts_raw: Vec<(usize, Vec<Literal>)> = components.into_iter().collect();
        parts_raw.sort_unstable_by_key(|(k, _)| *k);
        let mut parts: Vec<Vec<Literal>> = parts_raw.into_iter().map(|(_, lits)| lits).collect();
        for lit in ground_lits {
            parts.push(vec![lit]);
        }

        // We have successfully split the clause into `parts`.
        let split_id = id_gen.next();
        let mut split_clauses = Vec::new();
        let mut sat_clause = Vec::new();

        for (i, lits) in parts.into_iter().enumerate() {
            // For each part, canonicalize the component by renaming variables
            // 0..N in DFS order through the literals.  Two alpha-equivalent
            // components (identical up to variable renaming) then get the same
            // key, which lets the SAT solver share AVATAR variables between them
            // and prunes redundant case splits.
            let comp_str = canonical_component_key(&lits);

            let var = if let Some(&v) = self.component_vars.get(&comp_str) {
                v
            } else {
                let v = self.next_var;
                self.next_var += 1;
                self.component_vars.insert(comp_str, v);
                v
            };

            sat_clause.push(var as i32);

            let mut new_avatar = clause.avatar.clone();
            new_avatar.push(var);

            let mut new_clause = Clause::new_avatar(
                id_gen.next(),
                lits,
                ClauseSource::Inference {
                    rule: "avatar_component_clause",
                    parents: vec![split_id].into(),
                },
                new_avatar,
            );
            new_clause.certificate = Some(ClauseCertificate::AvatarComponent {
                split_parent: split_id,
                branch_index: i,
                sat_var: var,
            });
            split_clauses.push(new_clause);
        }

        // Add AVATAR assertion for the original clause's context.
        // The original clause was true under its avatar assertions.
        // So `A1 & A2 ... -> (S1 | S2 ...)`
        // `~A1 | ~A2 ... | S1 | S2 ...`
        for &a in &clause.avatar {
            sat_clause.push(-(a as i32));
        }

        self.add_sat_clause(sat_clause);
        self.sat_split_ids.push(split_id);

        Some(split_clauses)
    }
}

impl Default for AvatarContext {
    fn default() -> Self {
        Self::new()
    }
}

// ── IdClause variant ────────────────────────────────────────────────────────

impl AvatarContext {
    /// Splits an `IdClause` into variable-disjoint AVATAR components.
    /// Returns `None` if the clause cannot be split (only 1 component).
    pub fn split_clause_id(
        &mut self,
        clause: &IdClause,
        id_gen: &mut ClauseIdGen,
        bank: &TermBank,
        clause_store: &mut HashMap<mrs_core::clause::ClauseId, IdClause>,
        symbols: &SymbolTable,
    ) -> Option<Vec<IdClause>> {
        if clause.literals.len() <= 1 {
            return None;
        }

        // Insert the unsplit parent clause itself so it can be resolved during proof extraction
        clause_store.insert(clause.id, clause.clone());

        let n = clause.literals.len();
        let mut parent = (0..n).collect::<Vec<_>>();

        fn find(parent: &mut [usize], i: usize) -> usize {
            if parent[i] == i {
                i
            } else {
                let p = parent[i];
                parent[i] = find(parent, p);
                parent[i]
            }
        }

        fn union(parent: &mut [usize], i: usize, j: usize) {
            let pi = find(parent, i);
            let pj = find(parent, j);
            if pi != pj {
                parent[pi] = pj;
            }
        }

        let mut var_to_lit: HashMap<VarId, usize> = HashMap::default();
        for (i, lit) in clause.literals.iter().enumerate() {
            let vars = id_literal_vars(lit, bank);
            for v in vars {
                if let Some(&first_i) = var_to_lit.get(&v) {
                    union(&mut parent, i, first_i);
                } else {
                    var_to_lit.insert(v, i);
                }
            }
        }

        let mut components: HashMap<usize, Vec<(usize, IdLiteral)>> = HashMap::default();
        let mut ground_lits: Vec<(usize, IdLiteral)> = Vec::new();

        for (i, lit) in clause.literals.iter().enumerate() {
            let p = find(&mut parent, i);
            let vars = id_literal_vars(lit, bank);
            if vars.is_empty() {
                ground_lits.push((i, lit.clone()));
            } else {
                components.entry(p).or_default().push((i, lit.clone()));
            }
        }

        let num_components = components.len() + ground_lits.len();
        if num_components <= 1 {
            return None;
        }

        let mut parts_raw: Vec<(usize, Vec<(usize, IdLiteral)>)> = components.into_iter().collect();
        parts_raw.sort_unstable_by_key(|(k, _)| *k);
        let mut parts: Vec<Vec<(usize, IdLiteral)>> =
            parts_raw.into_iter().map(|(_, lits)| lits).collect();
        for lit in ground_lits {
            parts.push(vec![lit]);
        }

        let mut split_clauses = Vec::new();
        let mut sat_clause = Vec::new();
        let mut split_lits = Vec::new();

        // 1. Generate split component variable IDs and construct the parent split clause
        for entries in &parts {
            let lits: Vec<IdLiteral> = entries.iter().map(|(_, lit)| lit.clone()).collect();
            let comp_str = canonical_component_key_id(&lits, bank);

            let var = if let Some(&v) = self.component_vars.get(&comp_str) {
                v
            } else {
                let v = self.next_var;
                self.next_var += 1;
                self.component_vars.insert(comp_str, v);
                v
            };

            sat_clause.push(var as i32);

            let sym_name = format!("spl0_{}", var);
            let sym_id = symbols
                .resolve_name(&sym_name)
                .expect("spl0 symbol must exist");
            let atom = IdAtom::Pred(sym_id, SmallVec::new());
            let lit = IdLiteral {
                positive: true,
                atom,
            };
            split_lits.push(lit);
        }

        let split_id = id_gen.next();
        let mut split_c = IdClause::new_avatar(
            split_id,
            split_lits,
            ClauseSource::Inference {
                rule: "avatar_split_clause",
                parents: vec![clause.id].into(),
            },
            clause.avatar.clone(),
        );
        split_c.certificate = Some(ClauseCertificate::AvatarSplit {
            inherited: clause.avatar.clone(),
            components: sat_clause
                .iter()
                .enumerate()
                .map(|(branch_index, var)| AvatarComponent {
                    branch_index,
                    sat_var: *var as u32,
                    literal_indices: parts[branch_index]
                        .iter()
                        .map(|(index, _)| *index)
                        .collect(),
                })
                .collect(),
        });
        clause_store.insert(split_id, split_c);
        // 2. Construct each component clause derived from split_c
        for (i, entries) in parts.into_iter().enumerate() {
            let var = sat_clause[i] as u32;
            let lits: Vec<IdLiteral> = entries.into_iter().map(|(_, lit)| lit).collect();

            let mut new_avatar = clause.avatar.clone();
            new_avatar.push(var);

            let mut new_clause = IdClause::new_avatar(
                id_gen.next(),
                lits,
                ClauseSource::Inference {
                    rule: "avatar_component_clause",
                    parents: vec![split_id].into(),
                },
                new_avatar,
            );
            new_clause.certificate = Some(ClauseCertificate::AvatarComponent {
                split_parent: split_id,
                branch_index: i,
                sat_var: var,
            });
            split_clauses.push(new_clause);
        }

        for &a in &clause.avatar {
            sat_clause.push(-(a as i32));
        }

        self.add_sat_clause(sat_clause);
        self.sat_split_ids.push(split_id);
        Some(split_clauses)
    }
}

fn literal_vars(lit: &Literal) -> HashSet<VarId> {
    let mut vars = HashSet::default();
    match &lit.atom {
        Atom::Pred(_, args) => {
            for a in args {
                collect_vars(a, &mut vars);
            }
        }
        Atom::Eq(l, r) => {
            collect_vars(l, &mut vars);
            collect_vars(r, &mut vars);
        }
    }
    vars
}

fn collect_vars(term: &Term, vars: &mut HashSet<VarId>) {
    match term {
        Term::Var(v) => {
            vars.insert(*v);
        }
        Term::App(_, args) => {
            for a in args {
                collect_vars(a, vars);
            }
        }
    }
}

/// Returns a canonical string key for a set of literals that is invariant
/// under variable renaming.  Variables are assigned fresh names V0, V1, …
/// in the order they are first encountered during a left-to-right, DFS
/// traversal of the literals.  This means two alpha-equivalent components
/// produce the same key, enabling the SAT solver to share AVATAR variables.
fn canonical_component_key(lits: &[Literal]) -> String {
    let mut var_map: HashMap<VarId, u32> = HashMap::default();
    let mut next: u32 = 0;
    let mut s = String::new();
    for (i, lit) in lits.iter().enumerate() {
        if i > 0 {
            s.push('|');
        }
        if lit.is_negative() {
            s.push('~');
        }
        match &lit.atom {
            Atom::Pred(sym, args) => {
                s.push('P');
                push_u32(&mut s, sym.index());
                if !args.is_empty() {
                    s.push('(');
                    for (j, a) in args.iter().enumerate() {
                        if j > 0 {
                            s.push(',');
                        }
                        write_term_canonical(a, &mut s, &mut var_map, &mut next);
                    }
                    s.push(')');
                }
            }
            Atom::Eq(l, r) => {
                s.push_str("Eq(");
                write_term_canonical(l, &mut s, &mut var_map, &mut next);
                s.push(',');
                write_term_canonical(r, &mut s, &mut var_map, &mut next);
                s.push(')');
            }
        }
    }
    s
}

fn write_term_canonical(
    term: &Term,
    s: &mut String,
    var_map: &mut HashMap<VarId, u32>,
    next: &mut u32,
) {
    match term {
        Term::Var(v) => {
            let id = *var_map.entry(*v).or_insert_with(|| {
                let id = *next;
                *next += 1;
                id
            });
            s.push('V');
            push_u32(s, id);
        }
        Term::App(sym, args) => {
            s.push('F');
            push_u32(s, sym.index());
            if !args.is_empty() {
                s.push('(');
                for (i, a) in args.iter().enumerate() {
                    if i > 0 {
                        s.push(',');
                    }
                    write_term_canonical(a, s, var_map, next);
                }
                s.push(')');
            }
        }
    }
}

/// Append a `u32` to a `String` without going through the allocating
/// `format!` machinery.
fn push_u32(s: &mut String, mut n: u32) {
    if n == 0 {
        s.push('0');
        return;
    }
    let mut buf = [0u8; 10];
    let mut len = 0;
    while n > 0 {
        buf[len] = b'0' + (n % 10) as u8;
        len += 1;
        n /= 10;
    }
    buf[..len].reverse();
    s.push_str(std::str::from_utf8(&buf[..len]).unwrap());
}

fn id_literal_vars(lit: &IdLiteral, bank: &TermBank) -> HashSet<VarId> {
    let mut vars = HashSet::default();
    match &lit.atom {
        IdAtom::Pred(_, args) => {
            for &a in args {
                id_collect_vars(a, bank, &mut vars);
            }
        }
        IdAtom::Eq(l, r) => {
            id_collect_vars(*l, bank, &mut vars);
            id_collect_vars(*r, bank, &mut vars);
        }
    }
    vars
}

fn id_collect_vars(term: mrs_core::term_bank::TermId, bank: &TermBank, vars: &mut HashSet<VarId>) {
    match bank.get(term) {
        TermNode::Var(v) => {
            vars.insert(*v);
        }
        TermNode::App(_, args) => {
            for &a in args {
                id_collect_vars(a, bank, vars);
            }
        }
    }
}

/// Returns a canonical string key for a set of `IdLiteral`s, invariant
/// under variable renaming (same algorithm as `canonical_component_key`).
fn canonical_component_key_id(lits: &[IdLiteral], bank: &TermBank) -> String {
    let mut var_map: HashMap<VarId, u32> = HashMap::default();
    let mut next: u32 = 0;
    let mut s = String::new();
    for (i, lit) in lits.iter().enumerate() {
        if i > 0 {
            s.push('|');
        }
        if !lit.positive {
            s.push('~');
        }
        match &lit.atom {
            IdAtom::Pred(sym, args) => {
                s.push('P');
                push_u32(&mut s, sym.index());
                if !args.is_empty() {
                    s.push('(');
                    for (j, &a) in args.iter().enumerate() {
                        if j > 0 {
                            s.push(',');
                        }
                        write_id_term_canonical(a, bank, &mut s, &mut var_map, &mut next);
                    }
                    s.push(')');
                }
            }
            IdAtom::Eq(l, r) => {
                s.push_str("Eq(");
                write_id_term_canonical(*l, bank, &mut s, &mut var_map, &mut next);
                s.push(',');
                write_id_term_canonical(*r, bank, &mut s, &mut var_map, &mut next);
                s.push(')');
            }
        }
    }
    s
}

fn write_id_term_canonical(
    term: mrs_core::term_bank::TermId,
    bank: &TermBank,
    s: &mut String,
    var_map: &mut HashMap<VarId, u32>,
    next: &mut u32,
) {
    match bank.get(term) {
        TermNode::Var(v) => {
            let id = *var_map.entry(*v).or_insert_with(|| {
                let id = *next;
                *next += 1;
                id
            });
            s.push('V');
            push_u32(s, id);
        }
        TermNode::App(sym, args) => {
            s.push('F');
            push_u32(s, sym.index());
            if !args.is_empty() {
                s.push('(');
                for (i, a) in args.iter().enumerate() {
                    if i > 0 {
                        s.push(',');
                    }
                    write_id_term_canonical(*a, bank, s, var_map, next);
                }
                s.push(')');
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mrs_core::{Atom, Literal, SymbolTable, Term};

    #[test]
    fn canonical_key_alpha_invariant() {
        // p(X0) and p(X5) should produce the same key
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");

        let lits_v0 = vec![Literal::pos(Atom::pred(p, vec![Term::var(0)]))];
        let lits_v5 = vec![Literal::pos(Atom::pred(p, vec![Term::var(5)]))];
        assert_eq!(
            canonical_component_key(&lits_v0),
            canonical_component_key(&lits_v5),
            "alpha-equivalent components must have the same canonical key"
        );
    }

    #[test]
    fn canonical_key_distinct_structures() {
        // p(X0, X1) and p(X0, X0) must have different keys
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");

        let lits_12 = vec![Literal::pos(Atom::pred(
            p,
            vec![Term::var(0), Term::var(1)],
        ))];
        let lits_11 = vec![Literal::pos(Atom::pred(
            p,
            vec![Term::var(0), Term::var(0)],
        ))];
        assert_ne!(
            canonical_component_key(&lits_12),
            canonical_component_key(&lits_11),
            "structurally different components must have different keys"
        );
    }

    #[test]
    fn split_clause_shares_avatar_var_for_alpha_equiv_components() {
        // Two clauses that split into the same component shape should get
        // the same AVATAR variable for that component.
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let q = syms.intern("q");
        let mut ctx = AvatarContext::new();
        let mut id_gen = mrs_core::clause::ClauseIdGen::new();

        // Clause 1: p(X0) | q(X1)  -- two variable-disjoint literals
        let c1 = Clause::new(
            id_gen.next(),
            vec![
                Literal::pos(Atom::pred(p, vec![Term::var(0)])),
                Literal::pos(Atom::pred(q, vec![Term::var(1)])),
            ],
            mrs_core::clause::ClauseSource::Input {
                name: "c1".into(),
                role: "axiom".into(),
            },
        );
        // Clause 2: p(X5) | q(X6)  -- same shape, different var IDs
        let c2 = Clause::new(
            id_gen.next(),
            vec![
                Literal::pos(Atom::pred(p, vec![Term::var(5)])),
                Literal::pos(Atom::pred(q, vec![Term::var(6)])),
            ],
            mrs_core::clause::ClauseSource::Input {
                name: "c2".into(),
                role: "axiom".into(),
            },
        );

        let splits1 = ctx.split_clause(&c1, &mut id_gen).expect("c1 must split");
        let splits2 = ctx.split_clause(&c2, &mut id_gen).expect("c2 must split");

        // Collect the AVATAR variables allocated for each split.
        // For alpha-equivalent components the variables must coincide.
        let vars1: HashSet<u32> = splits1
            .iter()
            .flat_map(|s| s.avatar.iter().copied())
            .collect();
        let vars2: HashSet<u32> = splits2
            .iter()
            .flat_map(|s| s.avatar.iter().copied())
            .collect();

        assert_eq!(
            vars1, vars2,
            "alpha-equivalent clauses must reuse the same AVATAR variables"
        );
        assert_eq!(ctx.sat_split_ids.len(), 2);
    }
}
