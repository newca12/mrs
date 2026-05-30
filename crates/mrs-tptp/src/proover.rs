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

/// A reference from one proof step to one of its parents, recovered from
/// a (possibly nested) `inference(...)` pedigree.
///
/// The flat list of parent *names* is usually enough — but TSTP's
/// `inference(assume_negation, _, [parent])` wrapper changes the polarity
/// of the wrapped parent (it asserts `¬parent`, not `parent`). Callers
/// that intend to feed parents as premises to an ATP need this flag, or
/// they will hand the ATP an obligation that is genuinely unsound under
/// the wrong-polarity premise and get back a spurious `Unsound` verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParentRef<'a> {
    /// Name of the parent step (a DAG node in the host proof).
    pub name: &'a str,
    /// `true` iff the pedigree wraps this parent in `assume_negation`
    /// (so the asserted premise is `¬parent`, not `parent`).
    pub negated: bool,
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
        self.parent_refs().into_iter().map(|r| r.name).collect()
    }

    /// Like [`parent_names`](Self::parent_names) but also records whether
    /// each referenced parent appears inside an `assume_negation` wrapper
    /// in the pedigree.
    ///
    /// `assume_negation(co1)` is eprover's standard idiom for the
    /// refutation-style preamble: take a `conjecture`-role formula `co1`
    /// and assert its negation as the starting point of the proof. If a
    /// later step's pedigree contains `inference(assume_negation, _, [co1])`,
    /// the obligation should be checked against **¬co1**, not `co1`. The
    /// flat `parent_names()` cannot express this and so silently drops the
    /// polarity flip, producing spurious `Unsound` verdicts from the ATP
    /// (it can't derive the negated conjecture from the positive one).
    ///
    /// Other status-flipping inference wrappers found in the wild should
    /// be added here as we encounter them.
    pub fn parent_refs(&self) -> Vec<ParentRef<'a>> {
        let mut out = Vec::new();
        match &self.source {
            GeneralTerm::Function(AtomicWord::Lower("inference"), args) if args.len() == 3 => {
                if let GeneralTerm::List(items) = &args[2] {
                    for it in items {
                        collect_parent_refs(it, false, &mut out);
                    }
                }
            }
            // Bare-atom source: `fof(c_0_4, axiom, (p(a)), c1).` is TPTP's
            // "general_source -> name" form, meaning "this formula was
            // copied from `c1`". eprover emits these for trivial
            // rename/identity steps. Treat the atom as a single parent
            // reference so that the verifier checks `c1 ⊨ c_0_4` (which
            // is trivially true if the formulas match) instead of feeding
            // the ATP an empty-premise query that comes back unsound.
            //
            // We deliberately exclude `file(...)` (handled by
            // `file_source()`) and `inference(...)` (handled above).
            GeneralTerm::Word(AtomicWord::Lower(s))
            | GeneralTerm::Word(AtomicWord::SingleQuoted(s)) => {
                out.push(ParentRef {
                    name: *s,
                    negated: false,
                });
            }
            _ => {}
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

/// Walk a general term, collecting parent references with a polarity
/// flag.
///
/// `negated` tracks whether any ancestor in the pedigree was an
/// `inference(assume_negation, _, _)` wrapper. When we reach a leaf
/// (an atomic parent name), the leaf inherits that ancestor flag.
///
/// `assume_negation` toggles the polarity each time it is encountered,
/// so nested `assume_negation(assume_negation(X))` correctly cancels —
/// although that combination is not known to occur in practice, treating
/// the flag as an XOR is the safe definition.
fn collect_parent_refs<'a>(t: &GeneralTerm<'a>, negated: bool, out: &mut Vec<ParentRef<'a>>) {
    match t {
        GeneralTerm::Word(AtomicWord::Lower(s))
        | GeneralTerm::Word(AtomicWord::SingleQuoted(s)) => {
            out.push(ParentRef { name: *s, negated });
        }
        GeneralTerm::Number(n) => {
            out.push(ParentRef {
                name: n.as_str(),
                negated,
            });
        }
        GeneralTerm::Function(AtomicWord::Lower("inference"), args) if args.len() == 3 => {
            let next_negated = match &args[0] {
                GeneralTerm::Word(AtomicWord::Lower("assume_negation"))
                | GeneralTerm::Word(AtomicWord::SingleQuoted("assume_negation")) => !negated,
                _ => negated,
            };
            if let GeneralTerm::List(items) = &args[2] {
                for it in items {
                    collect_parent_refs(it, next_negated, out);
                }
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_tptp;

    fn parse_single<'a>(input: &'a str) -> Annotations<'a> {
        let problem = parse_tptp(input).expect("parse");
        let af = problem
            .formulas
            .into_iter()
            .next()
            .expect("at least one annotated formula");
        match af {
            crate::ast::AnnotatedFormula::FOF(f) => f.annotations.expect("annotations"),
            _ => panic!("expected FOF"),
        }
    }

    #[test]
    fn bare_atom_source_is_parent() {
        // eprover emits trivial copy/rename steps with a bare-atom
        // source, e.g. `fof(c_0_4, axiom, (p(a)), c1).`. The single
        // atom `c1` is the parent reference and must be reported as
        // such; otherwise downstream consumers feed the ATP an
        // empty-premise query and get back a spurious Unsound verdict.
        let ann = parse_single("fof(c_0_4, axiom, (p(a)), c1).");
        let refs = ann.parent_refs();
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].name, "c1");
        assert!(!refs[0].negated);
        // No `inference(...)` wrapper → no rule name.
        assert_eq!(ann.inference_rule(), None);
    }

    #[test]
    fn single_quoted_bare_atom_source_is_parent() {
        let ann = parse_single("fof(step, plain, ($true), 'parent_name').");
        let refs = ann.parent_refs();
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].name, "parent_name");
    }

    #[test]
    fn file_source_is_not_a_parent() {
        // `file(...)` leaf sources must not be misreported as parents
        // (they're handled by `file_source()`).
        let ann = parse_single("fof(c1, axiom, (p(a)), file('foo.p', c1)).");
        assert!(ann.parent_refs().is_empty());
        assert!(ann.file_source().is_some());
    }

    #[test]
    fn inference_source_still_works() {
        let ann = parse_single(
            "fof(c_0_5, plain, ($false), inference(cn,[status(thm)],[c_0_3, c_0_4])).",
        );
        let refs = ann.parent_refs();
        assert_eq!(refs.len(), 2);
        assert_eq!(refs[0].name, "c_0_3");
        assert_eq!(refs[1].name, "c_0_4");
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
