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

/// One branch entry in an explicit AVATAR split annotation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AvatarSplitComponent<'a> {
    pub branch_index: usize,
    pub sat_var: &'a str,
    pub literal_indices: Vec<usize>,
}

/// Metadata carried by an `avatar_split_clause` step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AvatarSplitInfo<'a> {
    pub components: Vec<AvatarSplitComponent<'a>>,
    pub inherited: Vec<&'a str>,
}

/// Metadata carried by an `avatar_component_clause` step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AvatarComponentInfo<'a> {
    pub split_parent: &'a str,
    pub branch_index: usize,
    pub sat_var: &'a str,
}

/// Metadata carried by an `avatar_branch_refutation` step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AvatarBranchInfo<'a> {
    pub context: Vec<&'a str>,
}

/// Metadata carried by the final `avatar_sat_refutation` step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AvatarSatInfo<'a> {
    pub split_nodes: Vec<&'a str>,
    pub branch_roots: Vec<&'a str>,
    pub trace_format: Option<&'a str>,
    pub trace_variables: Option<usize>,
    pub trace_digest: Option<&'a str>,
    pub trace_original_ids: Vec<i64>,
    pub trace_cited_indices: Vec<usize>,
    pub trace_clauses: Vec<Vec<i32>>,
    pub trace_bytes: Option<&'a str>,
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
                    name: s,
                    negated: false,
                });
            }
            _ => {}
        }
        out
    }

    /// Return whether an inference pedigree contains only supported parent
    /// reference terms.
    ///
    /// `parent_refs` intentionally returns a flat list for callers that need
    /// the common case. It must not silently turn an unsupported term into an
    /// omitted citation, because that changes the logical premises of a step.
    pub fn parent_refs_well_formed(&self) -> bool {
        match &self.source {
            GeneralTerm::Function(AtomicWord::Lower("inference"), args) => {
                inference_parent_terms_well_formed(args)
            }
            GeneralTerm::Function(AtomicWord::SingleQuoted("inference"), _) => false,
            _ => true,
        }
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
            if let GeneralTerm::Function(AtomicWord::Lower("status"), inner) = it
                && let Some(g) = inner.first()
            {
                match g {
                    GeneralTerm::Word(AtomicWord::Lower(s)) => return Some(*s),
                    GeneralTerm::Word(AtomicWord::SingleQuoted(s)) => return Some(*s),
                    _ => {}
                }
            }
        }
        None
    }

    /// Extract `new_symbols(kind, [s1, s2, …])` symbol names if present.
    pub fn new_symbols(&self) -> Vec<&'a str> {
        for it in self.info_items() {
            if let GeneralTerm::Function(AtomicWord::Lower("new_symbols"), inner) = it
                && inner.len() == 2
                && let GeneralTerm::List(items) = &inner[1]
            {
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
        Vec::new()
    }

    /// Extract the SAT literals recorded by an AVATAR branch certificate.
    pub fn avatar_context(&self) -> Vec<&'a str> {
        for item in self.info_items() {
            if let GeneralTerm::Function(AtomicWord::Lower("avatar_context"), args) = item
                && let Some(GeneralTerm::List(items)) = args.first()
            {
                return items
                    .iter()
                    .filter_map(|item| match item {
                        GeneralTerm::Word(AtomicWord::Lower(name)) => Some(*name),
                        GeneralTerm::Word(AtomicWord::SingleQuoted(name)) => Some(*name),
                        _ => None,
                    })
                    .collect();
            }
        }
        Vec::new()
    }

    /// Parse `avatar_split([branch(0, spl0_1, [0]), ...], [spl0_9, ...])`.
    pub fn avatar_split(&self) -> Option<AvatarSplitInfo<'a>> {
        let args = self.info_function("avatar_split")?;
        if args.len() != 2 {
            return None;
        }
        let components = match args.first()? {
            GeneralTerm::List(items) => items
                .iter()
                .map(|item| {
                    let GeneralTerm::Function(name, args) = item else {
                        return None;
                    };
                    if !word_is(name, "branch") || args.len() != 3 {
                        return None;
                    }
                    Some(AvatarSplitComponent {
                        branch_index: number_value(args.first()?)?,
                        sat_var: word_value(args.get(1)?)?,
                        literal_indices: number_list(args.get(2)?)?,
                    })
                })
                .collect::<Option<Vec<_>>>()?,
            _ => return None,
        };
        let inherited = match args.get(1) {
            Some(GeneralTerm::List(items)) => items.iter().filter_map(word_value).collect(),
            _ => return None,
        };
        Some(AvatarSplitInfo {
            components,
            inherited,
        })
    }

    /// Parse `avatar_component(c123, 0, spl0_1)`.
    pub fn avatar_component(&self) -> Option<AvatarComponentInfo<'a>> {
        let args = self.info_function("avatar_component")?;
        if args.len() != 3 {
            return None;
        }
        Some(AvatarComponentInfo {
            split_parent: word_value(args.first()?)?,
            branch_index: number_value(args.get(1)?)?,
            sat_var: word_value(args.get(2)?)?,
        })
    }

    /// Parse `avatar_context([spl0_1, spl0_2])`.
    pub fn avatar_branch(&self) -> Option<AvatarBranchInfo<'a>> {
        let args = self.info_function("avatar_context")?;
        if args.len() != 1 {
            return None;
        }
        let context = match args.first()? {
            GeneralTerm::List(items) => items.iter().map(word_value).collect::<Option<Vec<_>>>()?,
            _ => return None,
        };
        Some(AvatarBranchInfo { context })
    }

    /// Parse the self-contained SAT payload carried by an AVATAR certificate.
    ///
    /// The supported shape is
    /// `sat_trace(frat-lrat, Variables, Digest, [Ids], [Cited], [[Clauses]], HexBytes)`.
    pub fn avatar_sat(&self) -> Option<AvatarSatInfo<'a>> {
        let args = self.info_function("avatar_sat_refutation")?;
        if args.len() < 2 || args.len() > 3 {
            return None;
        }
        let split_nodes = term_list(args.first()?)?;
        let branch_roots = term_list(args.get(1)?)?;
        let (
            trace_format,
            trace_variables,
            trace_digest,
            trace_original_ids,
            trace_cited_indices,
            trace_clauses,
            trace_bytes,
        ) = match args.get(2) {
            Some(GeneralTerm::Function(name, trace_args))
                if word_is(name, "sat_trace") && trace_args.len() == 7 =>
            {
                let original_ids = number_list_i64(trace_args.get(3)?)?;
                let cited_indices = number_list(trace_args.get(4)?)?;
                let clauses = clause_list(trace_args.get(5)?)?;
                (
                    Some(word_value(trace_args.first()?)?),
                    Some(number_value(trace_args.get(1)?)?),
                    Some(word_value(trace_args.get(2)?)?),
                    original_ids,
                    cited_indices,
                    clauses,
                    Some(word_value(trace_args.get(6)?)?),
                )
            }
            None => (None, None, None, Vec::new(), Vec::new(), Vec::new(), None),
            _ => return None,
        };
        Some(AvatarSatInfo {
            split_nodes,
            branch_roots,
            trace_format,
            trace_variables,
            trace_digest,
            trace_original_ids,
            trace_cited_indices,
            trace_clauses,
            trace_bytes,
        })
    }

    fn info_function(&self, name: &str) -> Option<&[GeneralTerm<'a>]> {
        self.info_items().iter().find_map(|item| match item {
            GeneralTerm::Function(function, args) if word_is(function, name) => {
                Some(args.as_slice())
            }
            _ => None,
        })
    }

    /// Extract `skolemize(Var, sk(args…))` if present.
    pub fn skolemize_info(&self) -> Option<SkolemizeInfo<'a>> {
        for it in self.info_items() {
            if let GeneralTerm::Function(AtomicWord::Lower("skolemize"), inner) = it
                && inner.len() == 2
            {
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

fn word_is(word: &AtomicWord<'_>, expected: &str) -> bool {
    matches!(
        word,
        AtomicWord::Lower(value) | AtomicWord::SingleQuoted(value) if *value == expected
    )
}

fn inference_parent_terms_well_formed(args: &[GeneralTerm<'_>]) -> bool {
    if args.len() != 3
        || !matches!(args[0], GeneralTerm::Word(_))
        || !matches!(args[1], GeneralTerm::List(_))
    {
        return false;
    }
    let GeneralTerm::List(parents) = &args[2] else {
        return false;
    };
    parents.iter().all(parent_term_well_formed)
}

fn parent_term_well_formed(term: &GeneralTerm<'_>) -> bool {
    match term {
        GeneralTerm::Word(_) | GeneralTerm::Number(_) => true,
        GeneralTerm::Function(AtomicWord::Lower("inference"), args) => {
            inference_parent_terms_well_formed(args)
        }
        GeneralTerm::Function(AtomicWord::SingleQuoted("inference"), _) => false,
        // Metis attaches an explicit substitution to the cited parent. The
        // verifier currently consumes the parent reference but not the
        // substitution payload, so require the standard list-shaped payload
        // rather than accepting arbitrary ignored syntax.
        GeneralTerm::ColonPair(left, right) => {
            matches!(right.as_ref(), GeneralTerm::List(_)) && parent_term_well_formed(left)
        }
        _ => false,
    }
}

fn word_value<'a>(term: &GeneralTerm<'a>) -> Option<&'a str> {
    match term {
        GeneralTerm::Word(AtomicWord::Lower(value) | AtomicWord::SingleQuoted(value)) => {
            Some(*value)
        }
        _ => None,
    }
}

fn number_value(term: &GeneralTerm<'_>) -> Option<usize> {
    match term {
        GeneralTerm::Number(value) => value.as_str().parse().ok(),
        _ => None,
    }
}

fn term_list<'a>(term: &GeneralTerm<'a>) -> Option<Vec<&'a str>> {
    match term {
        GeneralTerm::List(items) => items.iter().map(word_value).collect(),
        _ => None,
    }
}

fn number_list(term: &GeneralTerm<'_>) -> Option<Vec<usize>> {
    match term {
        GeneralTerm::List(items) => items.iter().map(number_value).collect(),
        _ => None,
    }
}

fn number_list_i64(term: &GeneralTerm<'_>) -> Option<Vec<i64>> {
    match term {
        GeneralTerm::List(items) => items
            .iter()
            .map(|item| match item {
                GeneralTerm::Number(value) => value.as_str().parse().ok(),
                _ => None,
            })
            .collect(),
        _ => None,
    }
}

fn clause_list(term: &GeneralTerm<'_>) -> Option<Vec<Vec<i32>>> {
    match term {
        GeneralTerm::List(clauses) => clauses
            .iter()
            .map(|clause| match clause {
                GeneralTerm::List(literals) => literals
                    .iter()
                    .map(|literal| match literal {
                        GeneralTerm::Number(value) => value.as_str().parse().ok(),
                        _ => None,
                    })
                    .collect(),
                _ => None,
            })
            .collect(),
        _ => None,
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
            out.push(ParentRef { name: s, negated });
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
        // Metis (and other TSTP producers) write parents with an attached
        // substitution as a colon-pair: `parent_name : [bind(X, $fot(t)), …]`.
        // The parent is the LEFT side; the right side is the binding list
        // (an instantiation that is applied to the parent), not a parent
        // itself. Recurse only into the left so the real parent name is
        // extracted. Without this, a `inference(subst, [], [p:[bind…]])`
        // step is seen as having no parent, the entailment query gets empty
        // premises, and a sound instantiation step is wrongly refuted.
        GeneralTerm::ColonPair(left, _bindings) => {
            collect_parent_refs(left, negated, out);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_tptp;

    fn parse_single(input: &str) -> Annotations<'_> {
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
        assert!(ann.parent_refs_well_formed());
        let refs = ann.parent_refs();
        assert_eq!(refs.len(), 2);
        assert_eq!(refs[0].name, "c_0_3");
        assert_eq!(refs[1].name, "c_0_4");
    }

    #[test]
    fn unsupported_parent_term_is_not_well_formed() {
        let ann = parse_single(
            "fof(bot, plain, $false, inference(consequence, [status(thm)], [S0, s6])).",
        );
        assert!(!ann.parent_refs_well_formed());
        assert_eq!(ann.parent_refs().len(), 1);
        assert_eq!(ann.parent_refs()[0].name, "s6");
    }

    #[test]
    fn nested_inference_parent_is_well_formed() {
        let ann = parse_single(
            "fof(bot, plain, $false, inference(consequence, [status(thm)], [inference(assume_negation, [], [c])])).",
        );
        assert!(ann.parent_refs_well_formed());
        let refs = ann.parent_refs();
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].name, "c");
        assert!(refs[0].negated);
    }

    #[test]
    fn metis_colon_pair_parent_is_extracted() {
        // Metis writes substitution steps with the parent attached to a
        // binding list as a colon-pair: `parent : [bind(X, $fot(t))]`.
        // The parent name is the LEFT side of the colon-pair; the binding
        // list is an instantiation, not a parent. We must extract
        // `refute_0_8` and ignore the bindings.
        let ann = parse_single(
            "fof(refute_0_11, plain, big_g(z, z), \
             inference(subst, [], \
               [refute_0_8 : [bind(X, $fot(z)), bind(Y, $fot(z))]])).",
        );
        let refs = ann.parent_refs();
        assert_eq!(refs.len(), 1, "expected exactly one parent, got {refs:?}");
        assert_eq!(refs[0].name, "refute_0_8");
        assert!(!refs[0].negated);
    }

    #[test]
    fn avatar_certificate_metadata_is_parsed() {
        let ann = parse_single(
            "fof(split, plain, spl0_1 | spl0_2, \
             inference(avatar_split, [status(thm), \
               avatar_split([branch(0, spl0_1, [0]), branch(1, spl0_2, [1])], [])], [top])).",
        );
        let split = ann.avatar_split().expect("split metadata");
        assert_eq!(split.components.len(), 2);
        assert_eq!(split.components[1].branch_index, 1);
        assert_eq!(split.components[1].literal_indices, vec![1]);
    }

    #[test]
    fn avatar_sat_trace_metadata_is_parsed() {
        let ann = parse_single(
            "fof(bot, plain, $false, inference(avatar_sat_refutation, [status(thm), avatar_sat_refutation([split], [branch], sat_trace('frat-lrat', 2, '00', [1], [0], [[1]], 'aa'))], [split, branch])).",
        );
        let sat = ann.avatar_sat().expect("SAT metadata");
        assert_eq!(sat.trace_format, Some("frat-lrat"));
        assert_eq!(sat.trace_variables, Some(2));
        assert_eq!(sat.trace_original_ids, vec![1]);
        assert_eq!(sat.trace_cited_indices, vec![0]);
        assert_eq!(sat.trace_clauses, vec![vec![1]]);
        assert_eq!(sat.trace_bytes, Some("aa"));
    }
}
