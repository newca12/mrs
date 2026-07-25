//! Display implementations for core logic types.
//!
//! Provides human-readable formatting of terms, formulas, atoms, literals,
//! and clauses. Since terms and formulas reference symbols by [`SymbolId`],
//! display requires access to a [`SymbolTable`] to resolve names.
//!
//! The module provides a [`DisplayWithSymbols`] trait and a helper
//! [`Formatted`] wrapper for use with `format!` / `println!`.

use std::fmt;

use crate::clause::{Clause, ClauseSource, Literal};
use crate::formula::{Atom, Formula};
use crate::symbol::SymbolTable;
use crate::term::Term;

/// Trait for types that can be displayed given a symbol table.
pub trait DisplayWithSymbols {
    /// Formats this value using the given symbol table.
    fn fmt_with_symbols(&self, f: &mut fmt::Formatter<'_>, symbols: &SymbolTable) -> fmt::Result;

    /// Returns a wrapper that implements `Display` using the given symbol table.
    fn display<'a>(&'a self, symbols: &'a SymbolTable) -> Formatted<'a, Self>
    where
        Self: Sized,
    {
        Formatted {
            value: self,
            symbols,
        }
    }
}

/// A wrapper that implements [`fmt::Display`] for types implementing
/// [`DisplayWithSymbols`].
pub struct Formatted<'a, T: ?Sized> {
    value: &'a T,
    symbols: &'a SymbolTable,
}

impl<T: DisplayWithSymbols> fmt::Display for Formatted<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.value.fmt_with_symbols(f, self.symbols)
    }
}

fn fmt_identifier(name: &str) -> String {
    if name.starts_with('\'') && name.ends_with('\'') {
        return name.to_string();
    }
    let mut chars = name.chars();
    if let Some(first) = chars.next()
        && first.is_ascii_lowercase()
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return name.to_string();
    }
    format!("'{}'", name)
}

impl DisplayWithSymbols for Term {
    fn fmt_with_symbols(&self, f: &mut fmt::Formatter<'_>, symbols: &SymbolTable) -> fmt::Result {
        match self {
            Term::Var(v) => write!(f, "X{v}"),
            Term::App(sym, args) => {
                write!(f, "{}", fmt_identifier(symbols.resolve(*sym)))?;
                if !args.is_empty() {
                    write!(f, "(")?;
                    for (i, arg) in args.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        arg.fmt_with_symbols(f, symbols)?;
                    }
                    write!(f, ")")?;
                }
                Ok(())
            }
        }
    }
}

impl DisplayWithSymbols for Atom {
    fn fmt_with_symbols(&self, f: &mut fmt::Formatter<'_>, symbols: &SymbolTable) -> fmt::Result {
        match self {
            Atom::Pred(sym, args) => {
                write!(f, "{}", fmt_identifier(symbols.resolve(*sym)))?;
                if !args.is_empty() {
                    write!(f, "(")?;
                    for (i, arg) in args.iter().enumerate() {
                        if i > 0 {
                            write!(f, ", ")?;
                        }
                        arg.fmt_with_symbols(f, symbols)?;
                    }
                    write!(f, ")")?;
                }
                Ok(())
            }
            Atom::Eq(l, r) => {
                l.fmt_with_symbols(f, symbols)?;
                write!(f, " = ")?;
                r.fmt_with_symbols(f, symbols)
            }
        }
    }
}

impl DisplayWithSymbols for Literal {
    fn fmt_with_symbols(&self, f: &mut fmt::Formatter<'_>, symbols: &SymbolTable) -> fmt::Result {
        if !self.positive {
            // Use != for negative equality (TPTP convention)
            if let Atom::Eq(l, r) = &self.atom {
                l.fmt_with_symbols(f, symbols)?;
                write!(f, " != ")?;
                return r.fmt_with_symbols(f, symbols);
            }
            write!(f, "~")?;
        }
        self.atom.fmt_with_symbols(f, symbols)
    }
}

impl DisplayWithSymbols for Formula {
    fn fmt_with_symbols(&self, f: &mut fmt::Formatter<'_>, symbols: &SymbolTable) -> fmt::Result {
        match self {
            Formula::Atom(a) => a.fmt_with_symbols(f, symbols),
            Formula::Neg(inner) => {
                write!(f, "~(")?;
                inner.fmt_with_symbols(f, symbols)?;
                write!(f, ")")
            }
            Formula::And(conjuncts) => {
                write!(f, "(")?;
                for (i, c) in conjuncts.iter().enumerate() {
                    if i > 0 {
                        write!(f, " & ")?;
                    }
                    c.fmt_with_symbols(f, symbols)?;
                }
                write!(f, ")")
            }
            Formula::Or(disjuncts) => {
                write!(f, "(")?;
                for (i, d) in disjuncts.iter().enumerate() {
                    if i > 0 {
                        write!(f, " | ")?;
                    }
                    d.fmt_with_symbols(f, symbols)?;
                }
                write!(f, ")")
            }
            Formula::Implies(a, b) => {
                write!(f, "(")?;
                a.fmt_with_symbols(f, symbols)?;
                write!(f, " => ")?;
                b.fmt_with_symbols(f, symbols)?;
                write!(f, ")")
            }
            Formula::Iff(a, b) => {
                write!(f, "(")?;
                a.fmt_with_symbols(f, symbols)?;
                write!(f, " <=> ")?;
                b.fmt_with_symbols(f, symbols)?;
                write!(f, ")")
            }
            Formula::Forall(v, body) => {
                write!(f, "![X{v}]: (")?;
                body.fmt_with_symbols(f, symbols)?;
                write!(f, ")")
            }
            Formula::Exists(v, body) => {
                write!(f, "?[X{v}]: (")?;
                body.fmt_with_symbols(f, symbols)?;
                write!(f, ")")
            }
            Formula::True => write!(f, "$true"),
            Formula::False => write!(f, "$false"),
        }
    }
}

impl DisplayWithSymbols for Clause {
    fn fmt_with_symbols(&self, f: &mut fmt::Formatter<'_>, symbols: &SymbolTable) -> fmt::Result {
        if self.literals.is_empty() {
            write!(f, "$false")?;
        } else {
            for (i, lit) in self.literals.iter().enumerate() {
                if i > 0 {
                    write!(f, " | ")?;
                }
                lit.fmt_with_symbols(f, symbols)?;
            }
        }
        Ok(())
    }
}

impl fmt::Display for ClauseSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClauseSource::Input { name, role } => write!(f, "input({name}, {role})"),
            ClauseSource::Inference { rule, parents } => {
                write!(f, "{rule}(")?;
                for (i, p) in parents.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "c{}", p.0)?;
                }
                write!(f, ")")
            }
            ClauseSource::Introduced { symbol } => {
                write!(
                    f,
                    "introduced(definition, [new_symbols(definition, [s{}])])",
                    symbol.0
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_term() {
        let mut syms = SymbolTable::new();
        let f = syms.intern("f");
        let a = syms.intern("a");

        let t = Term::app(f, vec![Term::var(0), Term::constant(a)]);
        let s = format!("{}", t.display(&syms));
        assert_eq!(s, "f(X0, a)");
    }

    #[test]
    fn display_formula() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");

        let f = Formula::forall(0, Formula::atom(Atom::pred(p, vec![Term::var(0)])));
        let s = format!("{}", f.display(&syms));
        assert_eq!(s, "![X0]: (p(X0))");
    }

    #[test]
    fn display_clause() {
        let mut syms = SymbolTable::new();
        let p = syms.intern("p");
        let q = syms.intern("q");

        use crate::clause::{Clause, ClauseId, ClauseSource};
        let c = Clause::new(
            ClauseId(0),
            vec![
                Literal::pos(Atom::pred(p, vec![Term::var(0)])),
                Literal::neg(Atom::pred(q, vec![Term::var(1)])),
            ],
            ClauseSource::Input {
                name: "test".into(),
                role: "axiom".into(),
            },
        );
        let s = format!("{}", c.display(&syms));
        assert_eq!(s, "p(X0) | ~q(X1)");
    }

    #[test]
    fn display_empty_clause() {
        use crate::clause::{Clause, ClauseId, ClauseSource};
        let c = Clause::new(
            ClauseId(0),
            vec![],
            ClauseSource::Input {
                name: "empty".into(),
                role: "axiom".into(),
            },
        );
        let syms = SymbolTable::new();
        let s = format!("{}", c.display(&syms));
        assert_eq!(s, "$false");
    }
}
