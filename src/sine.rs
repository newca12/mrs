// This module is shared across multiple binary targets via `#[path]`; not all
// binaries use every item, so suppress dead-code warnings here.
#![allow(dead_code)]

use std::collections::{HashMap, HashSet};

use mrs_core::clause::Clause;
use mrs_core::formula::{Atom, Formula};
use mrs_core::symbol::SymbolId;
use mrs_core::term::Term;

use crate::lowering::LoweredFormula;

/// Extracts all symbols from a clause.
fn clause_symbols(clause: &Clause, syms: &mut HashSet<SymbolId>) {
    for lit in &clause.literals {
        match &lit.atom {
            Atom::Pred(p, args) => {
                syms.insert(*p);
                for arg in args {
                    term_symbols(arg, syms);
                }
            }
            Atom::Eq(l, r) => {
                term_symbols(l, syms);
                term_symbols(r, syms);
            }
        }
    }
}

/// Extracts all symbols from a formula.
fn formula_symbols(formula: &Formula, syms: &mut HashSet<SymbolId>) {
    match formula {
        Formula::Atom(Atom::Pred(p, args)) => {
            syms.insert(*p);
            for arg in args {
                term_symbols(arg, syms);
            }
        }
        Formula::Atom(Atom::Eq(l, r)) => {
            term_symbols(l, syms);
            term_symbols(r, syms);
        }
        Formula::Neg(f) | Formula::Forall(_, f) | Formula::Exists(_, f) => {
            formula_symbols(f, syms);
        }
        Formula::And(fs) | Formula::Or(fs) => {
            for f in fs {
                formula_symbols(f, syms);
            }
        }
        Formula::Implies(l, r) | Formula::Iff(l, r) => {
            formula_symbols(l, syms);
            formula_symbols(r, syms);
        }
        Formula::True | Formula::False => {}
    }
}

fn term_symbols(term: &Term, syms: &mut HashSet<SymbolId>) {
    let mut stack = vec![term];
    while let Some(t) = stack.pop() {
        if let Term::App(f, args) = t {
            syms.insert(*f);
            stack.extend(args.iter());
        }
    }
}

/// A generic item that can be selected by SInE.
pub trait SineItem {
    fn symbols(&self) -> HashSet<SymbolId>;
    fn is_conjecture(&self) -> bool;
}

impl SineItem for LoweredFormula {
    fn symbols(&self) -> HashSet<SymbolId> {
        let mut syms = HashSet::new();
        formula_symbols(&self.formula, &mut syms);
        syms
    }
    fn is_conjecture(&self) -> bool {
        self.role == "conjecture" || self.role == "negated_conjecture"
    }
}

impl SineItem for Clause {
    fn symbols(&self) -> HashSet<SymbolId> {
        let mut syms = HashSet::new();
        clause_symbols(self, &mut syms);
        syms
    }
    fn is_conjecture(&self) -> bool {
        // For pre-clausified things, we assume false unless we know better.
        // If we need to know, we should wrap it. But in MRS, lowered conjectures
        // are lowered formulas, and lowered axioms are formulas.
        false
    }
}

#[derive(Clone)]
pub enum SineItemWrapper {
    Formula(LoweredFormula),
    Clause(Box<Clause>),
}

impl SineItem for SineItemWrapper {
    fn symbols(&self) -> HashSet<SymbolId> {
        match self {
            SineItemWrapper::Formula(f) => f.symbols(),
            SineItemWrapper::Clause(c) => c.symbols(),
        }
    }
    fn is_conjecture(&self) -> bool {
        match self {
            SineItemWrapper::Formula(f) => f.is_conjecture(),
            SineItemWrapper::Clause(c) => c.is_conjecture(),
        }
    }
}

pub fn filter_items<T: SineItem + Clone>(
    items: &[T],
    tolerance: f64,
    depth_limit: Option<usize>,
) -> Vec<T> {
    if items.is_empty() {
        return Vec::new();
    }

    let mut item_syms: Vec<HashSet<SymbolId>> = Vec::with_capacity(items.len());
    let mut sym_counts: HashMap<SymbolId, usize> = HashMap::new();

    let mut has_conjectures = false;

    for item in items {
        let syms = item.symbols();
        if item.is_conjecture() {
            has_conjectures = true;
        }
        for &s in &syms {
            *sym_counts.entry(s).or_insert(0) += 1;
        }
        item_syms.push(syms);
    }

    // If there are no conjectures, SInE can't start easily from the goal.
    // Return everything.
    if !has_conjectures {
        return items.to_vec();
    }

    // Map each symbol to the items it triggers
    let mut triggers: HashMap<SymbolId, Vec<usize>> = HashMap::new();

    for (i, syms) in item_syms.iter().enumerate() {
        if syms.is_empty() {
            continue;
        }
        // Find minimum generality in this item
        let min_g = syms.iter().map(|s| sym_counts[s]).min().unwrap() as f64;
        let threshold = min_g * tolerance;

        for &s in syms {
            if (sym_counts[&s] as f64) <= threshold {
                triggers.entry(s).or_default().push(i);
            }
        }
    }

    let mut active_items = HashSet::new();
    let mut active_syms = HashSet::new();
    let mut new_syms = HashSet::new();

    // Initialize with conjectures
    for (i, item) in items.iter().enumerate() {
        if item.is_conjecture() {
            active_items.insert(i);
            for &s in &item_syms[i] {
                if active_syms.insert(s) {
                    new_syms.insert(s);
                }
            }
        }
    }

    let mut depth = 0;
    while !new_syms.is_empty() {
        if let Some(dl) = depth_limit
            && depth >= dl
        {
            break;
        }
        depth += 1;

        let mut next_new_syms = HashSet::new();
        for s in new_syms {
            if let Some(triggered_items) = triggers.get(&s) {
                for &i in triggered_items {
                    if active_items.insert(i) {
                        // Added new item
                        for &new_s in &item_syms[i] {
                            if active_syms.insert(new_s) {
                                next_new_syms.insert(new_s);
                            }
                        }
                    }
                }
            }
        }
        new_syms = next_new_syms;
    }

    // Collect result
    let mut result = Vec::with_capacity(active_items.len());
    for (i, item) in items.iter().enumerate() {
        if active_items.contains(&i) {
            result.push(item.clone());
        }
    }

    result
}
