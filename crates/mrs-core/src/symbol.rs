//! Symbol interning: efficient bidirectional mapping between string names and integer IDs.
//!
//! Symbols represent function names, predicate names, and other identifiers
//! in first-order logic. Interning converts strings to compact integer IDs
//! for efficient comparison and storage.

use crate::HashMap;

/// An interned symbol identifier.
///
/// This is a lightweight `Copy` handle that can be used to look up the
/// original string name via a [`SymbolTable`]. Two `SymbolId` values
/// are equal if and only if they refer to the same symbol name.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub struct SymbolId(pub(crate) u32);

impl SymbolId {
    /// Returns the raw integer index of this symbol.
    pub fn index(self) -> u32 {
        self.0
    }

    /// Reserved pseudo-symbol for ordering ground equality atoms as terms.
    ///
    /// The certified EPR+Eq path compares `Eq(l, r)` atoms by wrapping them
    /// as `RESERVED_EQ_ORDER(l, r)` terms inside ordering computations only
    /// (maximal-literal selection, totality validation). This id can never
    /// collide with an interned symbol (tables grow up from zero), and the
    /// pseudo-term must never be interned, rendered, collected into
    /// signatures, or stored in clauses: ordering configs resolve it
    /// through their unknown-symbol fallbacks (variable weight,
    /// index-derived precedence), which keeps it positive-weighted and
    /// precedence-distinct by construction. See `certified_eq`.
    pub const RESERVED_EQ_ORDER: SymbolId = SymbolId(u32::MAX);
}

/// Bidirectional mapping between symbol names and [`SymbolId`]s.
///
/// The symbol table owns all interned strings and provides O(1) lookup
/// in both directions.
///
/// # Examples
///
/// ```
/// use mrs_core::SymbolTable;
///
/// let mut syms = SymbolTable::new();
/// let f = syms.intern("f");
/// let g = syms.intern("g");
/// assert_ne!(f, g);
/// assert_eq!(syms.resolve(f), "f");
///
/// // Interning the same name returns the same ID
/// assert_eq!(syms.intern("f"), f);
/// ```
#[derive(Debug, Clone)]
pub struct SymbolTable {
    names: Vec<String>,
    ids: HashMap<String, SymbolId>,
    fresh_counters: HashMap<String, usize>,
}

impl SymbolTable {
    /// Creates an empty symbol table.
    pub fn new() -> Self {
        Self {
            names: Vec::new(),
            ids: HashMap::default(),
            fresh_counters: HashMap::default(),
        }
    }

    /// Generates a fresh symbol guaranteed to be globally unique in this symbol table.
    ///
    /// The resulting symbol will start with `base_prefix`, followed by an underscore
    /// and a monotonically increasing counter, continuing until an unused symbol name
    /// is found. This prevents collisions across formulas or passes.
    pub fn fresh_symbol(&mut self, base_prefix: &str) -> SymbolId {
        let prefix = if base_prefix.is_empty() {
            "fresh"
        } else {
            base_prefix
        };
        loop {
            let count = self.fresh_counters.entry(prefix.to_string()).or_insert(0);
            let name = format!("{prefix}_{count}");
            *count += 1;
            if !self.ids.contains_key(&name) {
                return self.intern(&name);
            }
        }
    }

    /// Interns a symbol name, returning its [`SymbolId`].
    ///
    /// If the name has been interned before, returns the existing ID.
    /// Otherwise, assigns a new ID.
    pub fn intern(&mut self, name: &str) -> SymbolId {
        use std::collections::hash_map::Entry;
        match self.ids.entry(name.to_string()) {
            Entry::Occupied(e) => *e.get(),
            Entry::Vacant(e) => {
                let id = SymbolId(self.names.len() as u32);
                self.names.push(name.to_string());
                *e.insert(id)
            }
        }
    }

    /// Looks up a symbol by name, returning its [`SymbolId`] if it exists.
    pub fn resolve_name(&self, name: &str) -> Option<SymbolId> {
        self.ids.get(name).copied()
    }

    /// Resolves a [`SymbolId`] back to its string name.
    ///
    /// # Panics
    ///
    /// Panics if the ID was not produced by this table.
    pub fn resolve(&self, id: SymbolId) -> &str {
        &self.names[id.0 as usize]
    }

    /// Returns the number of interned symbols.
    pub fn len(&self) -> usize {
        self.names.len()
    }

    /// Returns `true` if no symbols have been interned.
    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }

    /// Iterates over interned names in their [`SymbolId`] index order.
    ///
    /// The order is stable for the lifetime of this table and is useful when
    /// transferring symbol-bearing values to another symbol table.
    pub fn iter_names(&self) -> impl Iterator<Item = &str> {
        self.names.iter().map(|name| name.as_str())
    }
}

impl Default for SymbolTable {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intern_and_resolve() {
        let mut st = SymbolTable::new();
        let f = st.intern("f");
        let g = st.intern("g");
        assert_ne!(f, g);
        assert_eq!(st.resolve(f), "f");
        assert_eq!(st.resolve(g), "g");
    }

    #[test]
    fn intern_idempotent() {
        let mut st = SymbolTable::new();
        let a = st.intern("hello");
        let b = st.intern("hello");
        assert_eq!(a, b);
        assert_eq!(st.len(), 1);
    }

    #[test]
    fn fresh_symbol_monotonic_and_collision_free() {
        let mut st = SymbolTable::new();
        let s0 = st.fresh_symbol("def");
        let s1 = st.fresh_symbol("def");
        assert_ne!(s0, s1);
        assert_eq!(st.resolve(s0), "def_0");
        assert_eq!(st.resolve(s1), "def_1");

        // If def_2 is already interned, fresh_symbol must skip it
        let _s2_manual = st.intern("def_2");
        let s3 = st.fresh_symbol("def");
        assert_eq!(st.resolve(s3), "def_3");
    }
}
