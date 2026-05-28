//! ProoVer-specific helpers for inspecting TSTP inference annotations.
//!
//! These helpers parse the common shape:
//!
//! ```text
//! inference(rule_name, [status(thm), new_symbols(skolem, [sK0]),
//!                       skolemize(Var, sk(args))],
//!           [parent1, parent2])
//! ```
//!
//! as well as `file('path', name)` source records on leaf nodes.

use crate::ast::Annotations;
use crate::ast::common::{AtomicWord, GeneralTerm};

/// Parsed shape of a `skolemize(Var, sk(args))` annotation entry.
#[derive(Debug, Clone)]
pub struct SkolemizeInfo<'a> {
    /// Name of the existential variable being eliminated (e.g. `"Bride"`).
    pub var: &'a str,
    /// Name of the Skolem symbol (e.g. `"sK0"`).
    pub skolem_symbol: &'a str,
    /// Names of the variables passed as arguments to the Skolem term.
    /// Each is expected to be an uppercase TPTP variable name.
    pub args: Vec<&'a str>,
}

impl<'a> Annotations<'a> {
    /// Extract the inference rule name from `inference(rule, …, …)` source.
    pub fn inference_rule(&self) -> Option<&'a str> {
        match &self.source {
            GeneralTerm::Function(AtomicWord::Lower("inference"), args) if args.len() == 3 => {
                match &args[0] {
                    GeneralTerm::Word(AtomicWord::Lower(s)) => Some(*s),
                    GeneralTerm::Word(AtomicWord::SingleQuoted(s)) => Some(*s),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// Extract the parent name list from `inference(rule, info, [parents])`.
    ///
    /// Each entry in the parent list may itself be a nested `inference(...)`
    /// term (E and Vampire both nest inferences inside parents to record the
    /// derivation pedigree). We recursively flatten such nestings, returning
    /// the *atomic* parent names actually referenced as proof DAG nodes.
    pub fn parent_names(&self) -> Vec<&'a str> {
        let mut out = Vec::new();
        if let GeneralTerm::Function(AtomicWord::Lower("inference"), args) = &self.source
            && args.len() == 3
            && let GeneralTerm::List(items) = &args[2]
        {
            for it in items {
                collect_parents(it, &mut out);
            }
        }
        out
    }

    /// Iterate the inference-info list (the second arg of `inference/3`).
    fn info_items(&self) -> &[GeneralTerm<'a>] {
        match &self.source {
            GeneralTerm::Function(AtomicWord::Lower("inference"), args) if args.len() == 3 => {
                match &args[1] {
                    GeneralTerm::List(items) => items.as_slice(),
                    _ => &[],
                }
            }
            _ => &[],
        }
    }

    /// Extract `status(...)` value if present, e.g. `"thm"`, `"cth"`, `"esa"`.
    pub fn status(&self) -> Option<&'a str> {
        for it in self.info_items() {
            if let GeneralTerm::Function(AtomicWord::Lower("status"), inner) = it {
                if let Some(g) = inner.first() {
                    match g {
                        GeneralTerm::Word(AtomicWord::Lower(s)) => return Some(*s),
                        GeneralTerm::Word(AtomicWord::SingleQuoted(s)) => return Some(*s),
                        _ => {}
                    }
                }
            }
        }
        None
    }

    /// Extract `new_symbols(kind, [s1, s2, …])` symbol names if present.
    pub fn new_symbols(&self) -> Vec<&'a str> {
        for it in self.info_items() {
            if let GeneralTerm::Function(AtomicWord::Lower("new_symbols"), inner) = it {
                if inner.len() == 2 {
                    if let GeneralTerm::List(items) = &inner[1] {
                        return items
                            .iter()
                            .filter_map(|g| match g {
                                GeneralTerm::Word(AtomicWord::Lower(s)) => Some(*s),
                                GeneralTerm::Word(AtomicWord::SingleQuoted(s)) => Some(*s),
                                _ => None,
                            })
                            .collect();
                    }
                }
            }
        }
        Vec::new()
    }

    /// Extract `skolemize(Var, sk(args…))` if present.
    pub fn skolemize_info(&self) -> Option<SkolemizeInfo<'a>> {
        for it in self.info_items() {
            if let GeneralTerm::Function(AtomicWord::Lower("skolemize"), inner) = it {
                if inner.len() == 2 {
                    let var = match &inner[0] {
                        GeneralTerm::Variable(v) => *v,
                        GeneralTerm::Word(AtomicWord::Lower(s)) => *s,
                        GeneralTerm::Word(AtomicWord::SingleQuoted(s)) => *s,
                        _ => continue,
                    };
                    let (sk_sym, args) = match &inner[1] {
                        GeneralTerm::Function(AtomicWord::Lower(sym), a) => {
                            let args: Vec<&str> = a
                                .iter()
                                .filter_map(|g| match g {
                                    GeneralTerm::Variable(v) => Some(*v),
                                    _ => None,
                                })
                                .collect();
                            (*sym, args)
                        }
                        GeneralTerm::Function(AtomicWord::SingleQuoted(sym), a) => {
                            let args: Vec<&str> = a
                                .iter()
                                .filter_map(|g| match g {
                                    GeneralTerm::Variable(v) => Some(*v),
                                    _ => None,
                                })
                                .collect();
                            (*sym, args)
                        }
                        // Skolem may be a constant
                        GeneralTerm::Word(AtomicWord::Lower(sym)) => (*sym, Vec::new()),
                        GeneralTerm::Word(AtomicWord::SingleQuoted(sym)) => (*sym, Vec::new()),
                        _ => continue,
                    };
                    return Some(SkolemizeInfo {
                        var,
                        skolem_symbol: sk_sym,
                        args,
                    });
                }
            }
        }
        None
    }

    /// Extract `file('path', name)` from the source, if this annotation is a leaf
    /// source rather than an `inference(...)`.
    pub fn file_source(&self) -> Option<(&'a str, &'a str)> {
        match &self.source {
            GeneralTerm::Function(AtomicWord::Lower("file"), args) if args.len() == 2 => {
                let path = match &args[0] {
                    GeneralTerm::Word(AtomicWord::SingleQuoted(s)) => *s,
                    GeneralTerm::Word(AtomicWord::Lower(s)) => *s,
                    _ => return None,
                };
                let name = match &args[1] {
                    GeneralTerm::Word(AtomicWord::Lower(s)) => *s,
                    GeneralTerm::Word(AtomicWord::SingleQuoted(s)) => *s,
                    GeneralTerm::Number(n) => n.as_str(),
                    _ => return None,
                };
                Some((path, name))
            }
            _ => None,
        }
    }
}

/// Walk a general term, collecting atomic parent names. Nested
/// `inference(rule, info, [parents])` terms are descended into so that the
/// names of *original* DAG nodes are recovered even when they are wrapped
/// in a derivation pedigree.
fn collect_parents<'a>(t: &GeneralTerm<'a>, out: &mut Vec<&'a str>) {
    match t {
        GeneralTerm::Word(AtomicWord::Lower(s))
        | GeneralTerm::Word(AtomicWord::SingleQuoted(s)) => out.push(*s),
        GeneralTerm::Number(n) => out.push(n.as_str()),
        GeneralTerm::Function(AtomicWord::Lower("inference"), args) if args.len() == 3 => {
            if let GeneralTerm::List(items) = &args[2] {
                for it in items {
                    collect_parents(it, out);
                }
            }
        }
        _ => {}
    }
}

/// Scan an input for the `% Proof : path/to/problem.p` header line.
///
/// Returns the path portion (trimmed), or `None` if absent.
pub fn proof_header_link(input: &str) -> Option<&str> {
    for line in input.lines() {
        let l = line.trim_start();
        let Some(l) = l.strip_prefix('%') else {
            continue;
        };
        let l = l.trim_start();
        if let Some(rest) = l.strip_prefix("Proof") {
            let rest = rest.trim_start();
            if let Some(rest) = rest.strip_prefix(':') {
                return Some(rest.trim());
            }
        }
    }
    None
}
