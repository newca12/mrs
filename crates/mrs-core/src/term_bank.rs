use std::collections::HashMap;

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
    App(SymbolId, Vec<TermId>),
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
    pub fn intern_app(&mut self, sym: SymbolId, args: Vec<TermId>) -> TermId {
        let node = TermNode::App(sym, args);
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
}

/// A fast substitution mapping variables to `TermId`s.
#[derive(Clone, Default, Debug)]
pub struct IdSubstitution {
    bindings: Vec<Option<TermId>>,
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
            if let TermNode::Var(v) = bank.get(term) {
                if let Some(next) = self.get(*v) {
                    term = next;
                    steps += 1;
                    debug_assert!(steps < 100_000, "apply_term: cycle detected");
                    continue;
                }
            }
            break;
        }

        // Apply recursively to function arguments
        match bank.get(term).clone() { // Clone the node to decouple from bank borrow
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
        assert_eq!(a1, a2, "Structurally identical constants must map to the same ID");

        // Variables
        let v1 = bank.intern_var(0);
        let v2 = bank.intern_var(0);
        assert_eq!(v1, v2, "Structurally identical variables must map to the same ID");

        // Applications
        let app1 = bank.intern_app(f, vec![v1, a1]);
        let app2 = bank.intern_app(f, vec![v2, a2]);
        assert_eq!(app1, app2, "Structurally identical applications must map to the same ID");

        // Verify total number of unique nodes: a, v, f(v, a)
        assert_eq!(bank.nodes.len(), 3);
    }
}
