use mrs_core::{Atom, Clause, SymbolId, SymbolTable, Term};
use mrs_tptp::{AnnotatedFormula, TPTPProblem};
use rustc_hash::FxHashSet as HashSet;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Extracted statistics of a TPTP problem.
pub fn analyze_and_print(
    path: &str,
    problem: &TPTPProblem<'_>,
    symbols: &SymbolTable,
    all_clauses: &[Clause],
) {
    println!("================================================================================");
    println!("                                 TPTP ANALYSIS REPORT                           ");
    println!("================================================================================");

    // 1. File Info
    println!(" [ FILE INFORMATION ]");
    let name = if path == "-" {
        "stdin"
    } else {
        Path::new(path)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
    };
    println!("  Name: {}", name);
    if path != "-" {
        println!("  Path: {}", path);
        if let Ok(meta) = fs::metadata(path) {
            println!("  Size: {} bytes", meta.len());
        }
    } else {
        println!("  Source: standard input (stdin)");
    }
    println!();

    // 2. Syntactic / AST Dialect summary
    println!(" [ SYNTACTIC & DIALECT SUMMARY ]");
    println!("  Raw Formulas/Statements: {}", problem.formulas.len());
    println!("  Includes Resolved: {}", problem.includes.len());

    let mut dialects = HashMap::new();
    let mut roles = HashMap::new();
    for f in &problem.formulas {
        let dialect = match f {
            AnnotatedFormula::THF(_) => "THF",
            AnnotatedFormula::TFF(_) => "TFF",
            AnnotatedFormula::FOF(_) => "FOF",
            AnnotatedFormula::TCF(_) => "TCF",
            AnnotatedFormula::CNF(_) => "CNF",
            AnnotatedFormula::TPI(_) => "TPI",
        };
        *dialects.entry(dialect).or_insert(0) += 1;

        let role_str = format!("{:?}", f.role());
        *roles.entry(role_str).or_insert(0) += 1;
    }

    if !dialects.is_empty() {
        println!("  Formulas by Dialect:");
        let mut dialects_vec: Vec<_> = dialects.into_iter().collect();
        dialects_vec.sort_by_key(|d| d.0);
        for (d, count) in dialects_vec {
            println!("   - {}: {}", d, count);
        }
    }

    if !roles.is_empty() {
        println!("  Formula Roles:");
        let mut roles_vec: Vec<_> = roles.into_iter().collect();
        roles_vec.sort_by(|a, b| a.0.cmp(&b.0));
        for (r, count) in roles_vec {
            println!("   - {}: {}", r, count);
        }
    }
    println!();

    // 3. Clause stats
    println!(" [ CLAUSE STRUCTURE & COMPLEXITY ]");
    let total_clauses = all_clauses.len();
    println!("  Total Clausified Clauses: {}", total_clauses);

    if total_clauses > 0 {
        let mut unit_count = 0;
        let mut horn_count = 0;
        let mut max_len = 0;
        let mut sum_len = 0;
        let mut has_equality = false;

        for c in all_clauses {
            let len = c.literals.len();
            sum_len += len;
            max_len = max_len.max(len);

            if len == 1 {
                unit_count += 1;
            }

            let pos_lits = c.literals.iter().filter(|l| l.positive).count();
            if pos_lits <= 1 {
                horn_count += 1;
            }

            for lit in &c.literals {
                if matches!(lit.atom, Atom::Eq(_, _)) {
                    has_equality = true;
                }
            }
        }

        let is_ueq = total_clauses > 0
            && all_clauses
                .iter()
                .all(|c| c.literals.len() == 1 && matches!(c.literals[0].atom, Atom::Eq(_, _)));

        println!(
            "  Unit Clauses: {} ({:.2}%)",
            unit_count,
            (unit_count as f64 / total_clauses as f64) * 100.0
        );
        println!(
            "  Horn Clauses: {} ({:.2}%)",
            horn_count,
            (horn_count as f64 / total_clauses as f64) * 100.0
        );
        println!("  Max Clause Length: {} literals", max_len);
        println!(
            "  Avg Clause Length: {:.2} literals",
            sum_len as f64 / total_clauses as f64
        );
        println!(
            "  Has Equality: {}",
            if has_equality { "Yes" } else { "No" }
        );
        println!(
            "  Pure Unit Equality (UEQ): {}",
            if is_ueq { "Yes" } else { "No" }
        );
    } else {
        println!("  No clauses available (non-logical or skipped problem).");
    }
    println!();

    // 4. Term and Variable Stats
    println!(" [ DETAILED TERM & VARIABLE STATS ]");
    let mut total_vars = HashSet::default();
    let mut max_depth = 0;
    let mut sum_depth = 0;
    let mut term_count = 0;
    let mut clause_vars_sum = 0;

    // Symbol scanning maps
    let mut predicates: HashMap<SymbolId, HashSet<usize>> = HashMap::new();
    let mut functors: HashMap<SymbolId, HashSet<usize>> = HashMap::new();

    for c in all_clauses {
        let mut clause_vars = HashSet::default();
        for lit in &c.literals {
            lit.collect_vars(&mut clause_vars);
            lit.collect_vars(&mut total_vars);

            match &lit.atom {
                Atom::Pred(pred_id, args) => {
                    predicates.entry(*pred_id).or_default().insert(args.len());
                    for arg in args {
                        let (depth, count) = analyze_term(arg, &mut functors);
                        max_depth = max_depth.max(depth);
                        sum_depth += depth;
                        term_count += count;
                    }
                }
                Atom::Eq(l, r) => {
                    let (dl, cl) = analyze_term(l, &mut functors);
                    let (dr, cr) = analyze_term(r, &mut functors);
                    max_depth = max_depth.max(dl).max(dr);
                    sum_depth += dl + dr;
                    term_count += cl + cr;
                }
            }
        }
        clause_vars_sum += clause_vars.len();
    }

    println!("  Total Unique Variables: {}", total_vars.len());
    if total_clauses > 0 {
        println!(
            "  Avg Variables/Clause: {:.2}",
            clause_vars_sum as f64 / total_clauses as f64
        );
    }
    println!("  Max Term Depth: {}", max_depth);
    if term_count > 0 {
        println!(
            "  Avg Term Depth: {:.2}",
            sum_depth as f64 / term_count as f64
        );
    }
    println!();

    // 5. Symbol Table Summary
    println!(" [ SYMBOL TABLE SUMMARY ]");
    let mut skolem_count = 0;

    let mut sorted_preds = Vec::new();
    for (pred_id, arities) in predicates {
        if let Some(name) = get_symbol_name(pred_id, symbols) {
            if is_skolem(&name) {
                skolem_count += 1;
            }
            sorted_preds.push((name, arities));
        }
    }
    sorted_preds.sort_by(|a, b| a.0.cmp(&b.0));

    if !sorted_preds.is_empty() {
        println!("  Predicates:");
        for (name, arities) in sorted_preds {
            let arities_str: Vec<String> = arities.iter().map(|a| a.to_string()).collect();
            println!("   - {}/{}", name, arities_str.join(","));
        }
    } else {
        println!("  Predicates: None");
    }

    let mut sorted_funcs = Vec::new();
    let mut constants_count = 0;
    for (func_id, arities) in functors {
        if let Some(name) = get_symbol_name(func_id, symbols) {
            if is_skolem(&name) {
                skolem_count += 1;
            }
            if arities.contains(&0) {
                constants_count += 1;
            }
            sorted_funcs.push((name, arities));
        }
    }
    sorted_funcs.sort_by(|a, b| a.0.cmp(&b.0));

    if !sorted_funcs.is_empty() {
        println!("  Functors (Functions & Constants):");
        for (name, arities) in sorted_funcs {
            let is_const = arities.len() == 1 && arities.contains(&0);
            let arities_str: Vec<String> = arities.iter().map(|a| a.to_string()).collect();
            println!(
                "   - {}/{} {}",
                name,
                arities_str.join(","),
                if is_const { "[Constant]" } else { "" }
            );
        }
        println!("  Constants Count: {}", constants_count);
    } else {
        println!("  Functors: None");
    }

    println!("  Skolem Symbols: {}", skolem_count);
    println!();

    // 6. Routing Decision
    println!(" [ ROUTING DECISION ]");
    if total_clauses > 0 {
        let assigned = mrs_search::strategy::auto_schedule_name(all_clauses);
        println!("  CASC Division: {}", assigned);
        println!("  Suggested Schedule: {}", assigned);
    } else {
        println!("  CASC Division: Unknown (empty clause set)");
        println!("  Suggested Schedule: Default (casc)");
    }
    println!("================================================================================");
}

/// Recursively analyze term to extract depth, subterm count, and functor arities.
fn analyze_term(term: &Term, functors: &mut HashMap<SymbolId, HashSet<usize>>) -> (usize, usize) {
    match term {
        Term::Var(_) => (1, 1),
        Term::App(func_id, args) => {
            functors.entry(*func_id).or_default().insert(args.len());
            let mut max_depth = 0;
            let mut total_count = 1;
            for arg in args {
                let (depth, count) = analyze_term(arg, functors);
                max_depth = max_depth.max(depth);
                total_count += count;
            }
            (max_depth + 1, total_count)
        }
    }
}

fn get_symbol_name(id: SymbolId, symbols: &SymbolTable) -> Option<String> {
    if (id.index() as usize) < symbols.len() {
        Some(symbols.resolve(id).to_string())
    } else {
        None
    }
}

fn is_skolem(name: &str) -> bool {
    name.starts_with("sk_") || name.contains("sK")
}
