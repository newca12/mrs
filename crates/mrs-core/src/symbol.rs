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
}

impl SymbolTable {
    /// Creates an empty symbol table.
    pub fn new() -> Self {
        Self {
            names: Vec::new(),
            ids: HashMap::default(),
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
}
