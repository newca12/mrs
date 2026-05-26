use std::collections::{HashMap, HashSet};

use varisat::{ExtendFormula, Solver};

use mrs_core::clause::{Clause, ClauseIdGen, Literal};
use mrs_core::formula::Atom;
use mrs_core::term::{Term, VarId};

pub struct AvatarContext {
    pub solver: Solver<'static>,
    // Mapping from normalized split component to AVATAR propositional variable (u32).
    // Using string representation as a simple normalization for now.
    pub component_vars: HashMap<String, u32>,
    pub next_var: u32,

    // The current SAT model (true variables)
    pub current_model: HashSet<u32>,
}

impl AvatarContext {
    pub fn new() -> Self {
        Self {
            solver: Solver::new(),
            component_vars: HashMap::new(),
            next_var: 1, // varisat variables start from 1
            current_model: HashSet::new(),
        }
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
        let mut var_to_lit: HashMap<VarId, usize> = HashMap::new();

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
        let mut components: HashMap<usize, Vec<Literal>> = HashMap::new();
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

        // Decide if we should split ground literals from the rest.
        // If there's 0 or 1 variable components and some ground literals, we can split them.
        // For maximum splitting, treat the ground literals as one component.
        let num_components = components.len() + if ground_lits.is_empty() { 0 } else { 1 };

        if num_components <= 1 {
            return None; // Cannot be split
        }

        let mut parts = Vec::new();
        for (_, lits) in components {
            parts.push(lits);
        }
        if !ground_lits.is_empty() {
            parts.push(ground_lits);
        }

        // We have successfully split the clause into `parts`.
        let mut split_clauses = Vec::new();
        let mut sat_clause = Vec::new();

        for lits in parts {
            // For each part, we need an AVATAR variable.
            // Normalize component (e.g. rename vars from 0 to N) to maximize sharing.
            // For now, simple format string.
            // TODO: Proper canonicalization
            let comp_str = format!("{:?}", lits); // Hacky normalization

            let var = if let Some(&v) = self.component_vars.get(&comp_str) {
                v
            } else {
                let v = self.next_var;
                self.next_var += 1;
                self.component_vars.insert(comp_str, v);
                v
            };

            sat_clause.push(varisat::Lit::from_var(
                varisat::Var::from_dimacs(var as isize),
                true,
            ));

            let mut new_avatar = clause.avatar.clone();
            new_avatar.push(var);

            let new_clause =
                Clause::new_avatar(id_gen.next(), lits, clause.source.clone(), new_avatar);
            split_clauses.push(new_clause);
        }

        // Add AVATAR assertion for the original clause's context.
        // The original clause was true under its avatar assertions.
        // So `A1 & A2 ... -> (S1 | S2 ...)`
        // `~A1 | ~A2 ... | S1 | S2 ...`
        for &a in &clause.avatar {
            sat_clause.push(varisat::Lit::from_var(
                varisat::Var::from_dimacs(a as isize),
                false,
            ));
        }

        self.solver.add_clause(&sat_clause);

        Some(split_clauses)
    }
}

impl Default for AvatarContext {
    fn default() -> Self {
        Self::new()
    }
}

fn literal_vars(lit: &Literal) -> HashSet<VarId> {
    let mut vars = HashSet::new();
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
