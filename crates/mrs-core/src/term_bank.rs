use crate::{HashMap, HashSet};
use smallvec::{SmallVec, smallvec};

use crate::Formula;
use crate::clause::{Clause, ClauseCertificate, ClauseId, ClauseSource, Literal};
use crate::formula::Atom;
use crate::symbol::SymbolId;
use crate::term::{Term, VarId};

/// A lightweight handle to an interned term.
/// Because terms are hash-consed, `TermId` equality implies deep structural equality.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct TermId(pub u32);

/// The internal representation of a term node inside the `TermBank`.
/// By using `TermId` instead of `Term`, we eliminate deep recursive allocations.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum TermNode {
    Var(VarId),
    App(SymbolId, SmallVec<[TermId; 4]>),
}

/// An atomic formula operating on `TermId`s.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum IdAtom {
    Pred(SymbolId, SmallVec<[TermId; 4]>),
    Eq(TermId, TermId),
}

impl IdAtom {
    pub fn collect_vars(&self, bank: &TermBank, vars: &mut HashSet<VarId>) {
        match self {
            IdAtom::Pred(_, args) => {
                for &arg in args {
                    bank.collect_vars(arg, vars);
                }
            }
            IdAtom::Eq(l, r) => {
                bank.collect_vars(*l, vars);
                bank.collect_vars(*r, vars);
            }
        }
    }
}

/// A literal operating on `TermId`s.
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct IdLiteral {
    pub positive: bool,
    pub atom: IdAtom,
}

impl IdLiteral {
    pub fn collect_vars(&self, bank: &TermBank, vars: &mut HashSet<VarId>) {
        self.atom.collect_vars(bank, vars);
    }
}

/// A clause operating on `TermId`s.
#[derive(Clone, Debug)]
pub struct IdClause {
    pub id: ClauseId,
    pub literals: SmallVec<[IdLiteral; 4]>,
    pub source: ClauseSource,
    pub avatar: Vec<u32>,
    pub distance: u32,
    /// Mirrors [`Clause::formula`]: when set, this is a non-clausal FOF-level
    /// proof step (not a real search clause). `Formula` is symbol-table-based
    /// (not `TermId`-interned), so it round-trips through
    /// `clause_to_legacy`/`clause_from_legacy` unchanged, with no
    /// interning/uninterning needed.
    ///
    /// **Never insert an `IdClause` with `formula: Some(_)` into
    /// `processed`/`unprocessed`** — only into `clause_store`, for proof
    /// provenance. Its empty `literals` would otherwise be misread as the
    /// empty clause (a refutation) by the given-clause loop.
    pub formula: Option<Box<Formula>>,
    pub certificate: Option<ClauseCertificate>,
}

impl IdClause {
    pub fn new<L>(id: ClauseId, literals: L, source: ClauseSource) -> Self
    where
        L: Into<SmallVec<[IdLiteral; 4]>>,
    {
        Self {
            id,
            literals: literals.into(),
            source,
            avatar: Vec::new(),
            distance: 1000,
            formula: None,
            certificate: None,
        }
    }

    pub fn new_avatar<L>(id: ClauseId, literals: L, source: ClauseSource, avatar: Vec<u32>) -> Self
    where
        L: Into<SmallVec<[IdLiteral; 4]>>,
    {
        let mut avatar = avatar;
        avatar.sort_unstable();
        avatar.dedup();
        Self {
            id,
            literals: literals.into(),
            source,
            avatar,
            distance: 1000,
            formula: None,
            certificate: None,
        }
    }

    pub fn free_vars(&self, bank: &TermBank) -> HashSet<VarId> {
        let mut vars = HashSet::default();
        for lit in &self.literals {
            lit.collect_vars(bank, &mut vars);
        }
        vars
    }

    /// Returns `true` if this is the empty clause (contradiction).
    pub fn is_empty(&self) -> bool {
        self.literals.is_empty()
    }

    /// Returns the number of literals in this clause.
    pub fn len(&self) -> usize {
        self.literals.len()
    }

    /// Returns `true` if this clause is a tautology.
    ///
    /// Detects two kinds:
    /// - Positive `s = s` (equality reflexivity): `TermId` equality is structural.
    /// - Complementary literals: `L` and `¬L` for the same `IdAtom`.
    pub fn is_tautology(&self) -> bool {
        for lit in &self.literals {
            if lit.positive
                && let IdAtom::Eq(l, r) = &lit.atom
                && l == r
            {
                return true;
            }
        }
        for (i, lit1) in self.literals.iter().enumerate() {
            for lit2 in &self.literals[i + 1..] {
                if lit1.positive != lit2.positive && lit1.atom == lit2.atom {
                    return true;
                }
            }
        }
        false
    }

    /// Removes duplicate literals in place, keeping the first occurrence of each.
    pub fn deduplicate(&mut self) {
        let mut seen: Vec<IdLiteral> = Vec::new();
        self.literals.retain(|lit| {
            if seen.contains(lit) {
                false
            } else {
                seen.push(lit.clone());
                true
            }
        });
    }

    /// Returns `true` if `self.avatar` is a subset of `other.avatar`.
    pub fn avatar_is_subset_of(&self, other: &IdClause) -> bool {
        let mut i = 0;
        let mut j = 0;
        while i < self.avatar.len() && j < other.avatar.len() {
            if self.avatar[i] < other.avatar[j] {
                return false;
            } else if self.avatar[i] == other.avatar[j] {
                i += 1;
                j += 1;
            } else {
                j += 1;
            }
        }
        i == self.avatar.len()
    }
}

/// An index-based, hash-consing arena for first-order terms.
///
/// Ensures that structurally identical terms are only stored once in memory,
/// and allows representing complex terms as lightweight `TermId` handles.
#[derive(Clone, Default, Debug)]
pub struct TermBank {
    nodes: Vec<TermNode>,
    dedup: HashMap<TermNode, TermId>,
}

impl TermBank {
    /// Creates a new, empty term bank.
    pub fn new() -> Self {
        Self::default()
    }

    /// Interns a variable into the term bank.
    pub fn intern_var(&mut self, var: VarId) -> TermId {
        let node = TermNode::Var(var);
        if let Some(&id) = self.dedup.get(&node) {
            return id;
        }
        let id = TermId(self.nodes.len() as u32);
        self.nodes.push(node.clone());
        self.dedup.insert(node, id);
        id
    }

    /// Interns a function application into the term bank.
    pub fn intern_app(&mut self, sym: SymbolId, args: impl Into<SmallVec<[TermId; 4]>>) -> TermId {
        let node = TermNode::App(sym, args.into());
        if let Some(&id) = self.dedup.get(&node) {
            return id;
        }
        let id = TermId(self.nodes.len() as u32);
        self.nodes.push(node.clone());
        self.dedup.insert(node, id);
        id
    }

    /// Retrieves the internal node representation for a given `TermId`.
    #[inline]
    pub fn get(&self, id: TermId) -> &TermNode {
        &self.nodes[id.0 as usize]
    }

    pub fn collect_vars(&self, id: TermId, vars: &mut HashSet<VarId>) {
        match self.get(id) {
            TermNode::Var(v) => {
                vars.insert(*v);
            }
            TermNode::App(_, args) => {
                for &arg in args {
                    self.collect_vars(arg, vars);
                }
            }
        }
    }

    pub fn subterm_at(&self, term: TermId, pos: &[usize]) -> Option<TermId> {
        if pos.is_empty() {
            return Some(term);
        }
        let head = pos[0];
        let tail = &pos[1..];
        match self.get(term) {
            TermNode::Var(_) => None,
            TermNode::App(_, args) => {
                if head < args.len() {
                    self.subterm_at(args[head], tail)
                } else {
                    None
                }
            }
        }
    }

    pub fn replace_at(&mut self, term: TermId, pos: &[usize], to: TermId) -> TermId {
        if pos.is_empty() {
            return to;
        }
        let head = pos[0];
        let tail = &pos[1..];
        let node = self.get(term).clone();
        match node {
            TermNode::Var(_) => term, // Shouldn't happen if pos is valid
            TermNode::App(sym, mut args) => {
                if head < args.len() {
                    args[head] = self.replace_at(args[head], tail, to);
                }
                self.intern_app(sym, args)
            }
        }
    }

    /// Recursively normalizes terms modulo AC-symbols.
    /// Canonicalizes nested associative applications of `sym` into a binary
    /// right-associated tree with sorted leaves.
    pub fn ac_normalize(&mut self, term: TermId, ac_syms: &HashSet<SymbolId>) -> TermId {
        let (sym, args) = match self.get(term) {
            TermNode::Var(_) => return term,
            TermNode::App(sym, args) => (*sym, args.clone()),
        };

        let norm_args: SmallVec<[TermId; 4]> = args
            .iter()
            .map(|&arg| self.ac_normalize(arg, ac_syms))
            .collect();

        if ac_syms.contains(&sym) {
            let mut leaves = Vec::new();
            let mut stack = norm_args.into_iter().collect::<Vec<_>>();
            stack.reverse();
            while let Some(current) = stack.pop() {
                match self.get(current) {
                    TermNode::App(g, child_args) if *g == sym => {
                        for &child in child_args.iter().rev() {
                            stack.push(child);
                        }
                    }
                    _ => {
                        leaves.push(current);
                    }
                }
            }

            leaves.sort_unstable_by_key(|t| t.0);

            if leaves.is_empty() {
                self.intern_app(sym, smallvec![])
            } else if leaves.len() == 1 {
                self.intern_app(sym, smallvec![leaves[0]])
            } else {
                let mut rebuilt = *leaves.last().unwrap();
                for &leaf in leaves.iter().rev().skip(1) {
                    rebuilt = self.intern_app(sym, smallvec![leaf, rebuilt]);
                }
                rebuilt
            }
        } else {
            self.intern_app(sym, norm_args)
        }
    }

    pub fn non_variable_positions(&self, term: TermId) -> Vec<Vec<usize>> {
        let mut positions = Vec::new();
        let mut current_path = Vec::new();
        self.collect_non_var_positions(term, &mut current_path, &mut positions);
        positions
    }

    fn collect_non_var_positions(
        &self,
        term: TermId,
        current_path: &mut Vec<usize>,
        positions: &mut Vec<Vec<usize>>,
    ) {
        match self.get(term) {
            TermNode::Var(_) => {}
            TermNode::App(_, args) => {
                positions.push(current_path.clone());
                for (i, &arg) in args.iter().enumerate() {
                    current_path.push(i);
                    self.collect_non_var_positions(arg, current_path, positions);
                    current_path.pop();
                }
            }
        }
    }

    /// Collects all non-variable subterms rooted at `term`.
    pub fn non_variable_subterms(&self, term: TermId) -> Vec<TermId> {
        let mut subterms = Vec::new();
        self.collect_non_var_subterms(term, &mut subterms);
        subterms
    }

    fn collect_non_var_subterms(&self, term: TermId, out: &mut Vec<TermId>) {
        match self.get(term) {
            TermNode::Var(_) => {}
            TermNode::App(_, args) => {
                out.push(term);
                for &arg in args.iter() {
                    self.collect_non_var_subterms(arg, out);
                }
            }
        }
    }

    /// Converts an interned `TermId` back into a legacy, deeply-allocated `Term`.
    pub fn to_legacy(&self, id: TermId) -> Term {
        match self.get(id) {
            TermNode::Var(v) => Term::Var(*v),
            TermNode::App(sym, args) => {
                let legacy_args = args.iter().map(|&arg| self.to_legacy(arg)).collect();
                Term::App(*sym, legacy_args)
            }
        }
    }

    /// Deeply inserts a legacy `Term` into the bank and returns its `TermId`.
    pub fn from_legacy(&mut self, term: &Term) -> TermId {
        match term {
            Term::Var(v) => self.intern_var(*v),
            Term::App(sym, args) => {
                let mut arg_ids = Vec::with_capacity(args.len());
                for arg in args {
                    arg_ids.push(self.from_legacy(arg));
                }
                self.intern_app(*sym, arg_ids)
            }
        }
    }

    pub fn atom_to_legacy(&self, atom: &IdAtom) -> Atom {
        match atom {
            IdAtom::Pred(sym, args) => {
                Atom::Pred(*sym, args.iter().map(|&a| self.to_legacy(a)).collect())
            }
            IdAtom::Eq(l, r) => Atom::Eq(self.to_legacy(*l), self.to_legacy(*r)),
        }
    }

    pub fn atom_from_legacy(&mut self, atom: &Atom) -> IdAtom {
        match atom {
            Atom::Pred(sym, args) => {
                IdAtom::Pred(*sym, args.iter().map(|a| self.from_legacy(a)).collect())
            }
            Atom::Eq(l, r) => IdAtom::Eq(self.from_legacy(l), self.from_legacy(r)),
        }
    }

    pub fn literal_to_legacy(&self, lit: &IdLiteral) -> Literal {
        Literal {
            positive: lit.positive,
            atom: self.atom_to_legacy(&lit.atom),
        }
    }

    pub fn literal_from_legacy(&mut self, lit: &Literal) -> IdLiteral {
        IdLiteral {
            positive: lit.positive,
            atom: self.atom_from_legacy(&lit.atom),
        }
    }

    pub fn clause_to_legacy(&self, clause: &IdClause) -> Clause {
        let mut c = Clause::new(
            clause.id,
            clause
                .literals
                .iter()
                .map(|l| self.literal_to_legacy(l))
                .collect::<SmallVec<[Literal; 4]>>(),
            clause.source.clone(),
        );
        c.avatar = clause.avatar.clone();
        c.distance = clause.distance;
        c.formula = clause.formula.clone();
        c.certificate = clause.certificate.clone();
        c
    }

    pub fn clause_from_legacy(&mut self, clause: &Clause) -> IdClause {
        IdClause {
            id: clause.id,
            literals: clause
                .literals
                .iter()
                .map(|l| self.literal_from_legacy(l))
                .collect(),
            source: clause.source.clone(),
            avatar: clause.avatar.clone(),
            distance: clause.distance,
            formula: clause.formula.clone(),
            certificate: clause.certificate.clone(),
        }
    }
}

/// A fast substitution mapping variables to `TermId`s.
#[derive(Clone, Default, Debug)]
pub struct IdSubstitution {
    bindings: SmallVec<[Option<TermId>; 8]>,
}

impl IdSubstitution {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn bind(&mut self, var: VarId, term: TermId) {
        let var_idx = var as usize;
        if var_idx >= self.bindings.len() {
            self.bindings.resize(var_idx + 1, None);
        }
        self.bindings[var_idx] = Some(term);
    }

    pub fn get(&self, var: VarId) -> Option<TermId> {
        self.bindings.get(var as usize).copied().flatten()
    }

    /// Recursively applies the substitution to a term, returning a new `TermId`.
    pub fn apply_term(&self, mut term: TermId, bank: &mut TermBank) -> TermId {
        if self.bindings.is_empty() {
            return term;
        }

        // Dereference variables iteratively
        let mut steps = 0;
        loop {
            if let TermNode::Var(v) = bank.get(term)
                && let Some(next) = self.get(*v)
            {
                term = next;
                steps += 1;
                debug_assert!(steps < 100_000, "apply_term: cycle detected");
                continue;
            }
            break;
        }

        // Apply recursively to function arguments
        match bank.get(term).clone() {
            // Clone the node to decouple from bank borrow
            TermNode::Var(_) => term,
            TermNode::App(sym, args) => {
                let mut changed = false;
                let mut new_args = Vec::with_capacity(args.len());
                for &arg in &args {
                    let new_arg = self.apply_term(arg, bank);
                    if new_arg != arg {
                        changed = true;
                    }
                    new_args.push(new_arg);
                }

                if changed {
                    bank.intern_app(sym, new_args)
                } else {
                    term
                }
            }
        }
    }

    pub fn apply_atom(&self, atom: &IdAtom, bank: &mut TermBank) -> IdAtom {
        if self.bindings.is_empty() {
            return atom.clone();
        }
        match atom {
            IdAtom::Pred(sym, args) => {
                let new_args = args.iter().map(|&a| self.apply_term(a, bank)).collect();
                IdAtom::Pred(*sym, new_args)
            }
            IdAtom::Eq(l, r) => IdAtom::Eq(self.apply_term(*l, bank), self.apply_term(*r, bank)),
        }
    }

    pub fn apply_literal(&self, lit: &IdLiteral, bank: &mut TermBank) -> IdLiteral {
        IdLiteral {
            positive: lit.positive,
            atom: self.apply_atom(&lit.atom, bank),
        }
    }

    /// Converts this `IdSubstitution` into a legacy `Substitution` for incremental refactoring.
    pub fn to_legacy(&self, bank: &TermBank) -> crate::subst::Substitution {
        let mut legacy = crate::subst::Substitution::new();
        for (v, opt_term) in self.bindings.iter().enumerate() {
            if let Some(term) = opt_term {
                legacy.bind(v as u32, bank.to_legacy(*term));
            }
        }
        legacy
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::symbol::SymbolTable;

    #[test]
    fn test_term_bank_hash_consing() {
        let mut syms = SymbolTable::new();
        let f = syms.intern("f");
        let a = syms.intern("a");

        let mut bank = TermBank::new();

        // Constants (arity 0)
        let a1 = bank.intern_app(a, vec![]);
        let a2 = bank.intern_app(a, vec![]);
        assert_eq!(
            a1, a2,
            "Structurally identical constants must map to the same ID"
        );

        // Variables
        let v1 = bank.intern_var(0);
        let v2 = bank.intern_var(0);
        assert_eq!(
            v1, v2,
            "Structurally identical variables must map to the same ID"
        );

        // Applications
        let app1 = bank.intern_app(f, vec![v1, a1]);
        let app2 = bank.intern_app(f, vec![v2, a2]);
        assert_eq!(
            app1, app2,
            "Structurally identical applications must map to the same ID"
        );

        // Verify total number of unique nodes: a, v, f(v, a)
        assert_eq!(bank.nodes.len(), 3);
    }

    #[test]
    fn test_ac_normalize() {
        let mut syms = SymbolTable::new();
        let f = syms.intern("f"); // Treated as AC
        let g = syms.intern("g"); // Standard function symbol
        let a = syms.intern("a");
        let b = syms.intern("b");
        let c = syms.intern("c");

        let mut bank = TermBank::new();
        let a_term = bank.intern_app(a, vec![]);
        let b_term = bank.intern_app(b, vec![]);
        let c_term = bank.intern_app(c, vec![]);

        let mut ac_syms = HashSet::default();
        ac_syms.insert(f);

        // 1. Test binary commutativity: f(b, a) -> f(a, b)
        let f_b_a = bank.intern_app(f, vec![b_term, a_term]);
        let norm_f_b_a = bank.ac_normalize(f_b_a, &ac_syms);

        let f_a_b = bank.intern_app(f, vec![a_term, b_term]);
        let norm_f_a_b = bank.ac_normalize(f_a_b, &ac_syms);

        assert_eq!(
            norm_f_b_a, norm_f_a_b,
            "f(b, a) and f(a, b) must normalize to the same term"
        );

        // 2. Test associativity: f(f(a, b), c) -> f(a, f(b, c))
        let f_a_b_node = bank.intern_app(f, vec![a_term, b_term]);
        let f_f_a_b_c = bank.intern_app(f, vec![f_a_b_node, c_term]);

        let f_b_c_node = bank.intern_app(f, vec![b_term, c_term]);
        let f_a_f_b_c = bank.intern_app(f, vec![a_term, f_b_c_node]);

        let norm_f_f_a_b_c = bank.ac_normalize(f_f_a_b_c, &ac_syms);
        let norm_f_a_f_b_c = bank.ac_normalize(f_a_f_b_c, &ac_syms);

        assert_eq!(
            norm_f_f_a_b_c, norm_f_a_f_b_c,
            "f(f(a, b), c) and f(a, f(b, c)) must normalize to the same term"
        );

        // 3. Complex nesting and sorting: f(g(f(c, b)), a) -> f(a, g(f(b, c)))
        let f_c_b = bank.intern_app(f, vec![c_term, b_term]);
        let g_f_c_b = bank.intern_app(g, vec![f_c_b]);
        let term_complex = bank.intern_app(f, vec![g_f_c_b, a_term]);

        let norm_complex = bank.ac_normalize(term_complex, &ac_syms);

        let f_b_c = bank.intern_app(f, vec![b_term, c_term]);
        let norm_f_b_c = bank.ac_normalize(f_b_c, &ac_syms);
        let g_f_b_c = bank.intern_app(g, vec![norm_f_b_c]);

        if let TermNode::App(sym_out, args_out) = bank.get(norm_complex) {
            assert_eq!(*sym_out, f);
            assert_eq!(args_out[0], a_term);
            assert_eq!(args_out[1], g_f_b_c);
        } else {
            panic!("Expected App node");
        }
    }
}
