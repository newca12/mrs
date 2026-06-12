use mrs_tptp::ast::cnf::*;
use mrs_tptp::ast::fof::*;
use mrs_tptp::ast::*;
use mrs_tptp::parse_tptp;
use std::env;
use std::fs;
use std::path::Path;
use walkdir::WalkDir;

#[derive(Debug, PartialEq, Eq)]
#[allow(clippy::upper_case_acronyms)]
enum Category {
    EPR,
    UEQ,
    FEQ,
    FNE,
    Other,
}

fn determine_category(ast: &TPTPProblem) -> Category {
    let mut has_equality = false;
    let mut has_functions = false;
    let mut all_unit = true;
    let mut all_equalities = true;
    let mut is_cnf = true;

    for input in &ast.formulas {
        let role = input.role();
        if role == FormulaRole::Type || role == FormulaRole::Definition {
            continue; // Skip types
        }

        match input {
            AnnotatedFormula::CNF(cnf) => {
                let lits = match &cnf.formula {
                    CNFStatement::Logical(CNFFormula::Disjunction(lits)) => lits.clone(),
                    CNFStatement::Logical(CNFFormula::Parens(inner)) => {
                        if let CNFFormula::Disjunction(lits) = &**inner {
                            lits.clone()
                        } else {
                            return Category::Other;
                        }
                    }
                };

                if lits.len() != 1 {
                    all_unit = false;
                }

                for lit in lits {
                    match lit {
                        CNFLiteral::Equality(..) | CNFLiteral::Inequality(..) => {
                            has_equality = true;
                        }
                        CNFLiteral::Positive(_) | CNFLiteral::Negative(_) => {
                            all_equalities = false;
                        }
                    }

                    // Check for functions arity > 0
                    check_cnf_literal(&lit, &mut has_functions);
                }
            }
            AnnotatedFormula::FOF(fof) => {
                is_cnf = false;
                all_unit = false;
                all_equalities = false;
                match &fof.formula {
                    FOFStatement::Logical(f) => {
                        check_fof_formula(f, &mut has_equality, &mut has_functions)
                    }
                    FOFStatement::Sequent(..) => return Category::Other,
                }
            }
            _ => return Category::Other, // TFF, THF, etc.
        }
    }

    if !has_functions {
        return Category::EPR;
    }

    if is_cnf && all_unit && all_equalities {
        return Category::UEQ;
    }

    if has_equality {
        Category::FEQ
    } else {
        Category::FNE
    }
}

fn check_cnf_literal(lit: &CNFLiteral, has_functions: &mut bool) {
    match lit {
        CNFLiteral::Positive(CNFAtomicFormula::Plain(_, args))
        | CNFLiteral::Negative(CNFAtomicFormula::Plain(_, args)) => {
            for arg in args {
                check_term(arg, has_functions);
            }
        }
        CNFLiteral::Equality(t1, t2) | CNFLiteral::Inequality(t1, t2) => {
            check_term(t1, has_functions);
            check_term(t2, has_functions);
        }
        _ => {}
    }
}

fn check_fof_formula(formula: &FOFFormula, has_eq: &mut bool, has_funcs: &mut bool) {
    match formula {
        FOFFormula::Atomic(FOFAtomicFormula::Plain(_, args)) => {
            for arg in args {
                check_term(arg, has_funcs);
            }
        }
        FOFFormula::Equality(t1, t2) | FOFFormula::Inequality(t1, t2) => {
            *has_eq = true;
            check_term(t1, has_funcs);
            check_term(t2, has_funcs);
        }
        FOFFormula::Atomic(_) => {}
        FOFFormula::Negation(f) => check_fof_formula(f, has_eq, has_funcs),
        FOFFormula::Binary { left, right, .. } => {
            check_fof_formula(left, has_eq, has_funcs);
            check_fof_formula(right, has_eq, has_funcs);
        }
        FOFFormula::Quantified { formula: f, .. } => check_fof_formula(f, has_eq, has_funcs),
        FOFFormula::Parens(f) => check_fof_formula(f, has_eq, has_funcs),
    }
}

fn check_term(term: &FOFTerm, has_funcs: &mut bool) {
    if let FOFTerm::Function(_, args) = term {
        if !args.is_empty() {
            *has_funcs = true;
        }
        for arg in args {
            check_term(arg, has_funcs);
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: {} <TPTP_DIR> <OUTPUT_DIR>", args[0]);
        std::process::exit(1);
    }

    let tptp_dir = &args[1];
    let out_dir = &args[2];
    fs::create_dir_all(out_dir).unwrap();

    let mut feq_list = fs::File::create(Path::new(out_dir).join("feq.list")).unwrap();
    let mut fne_list = fs::File::create(Path::new(out_dir).join("fne.list")).unwrap();
    let mut ueq_list = fs::File::create(Path::new(out_dir).join("ueq.list")).unwrap();
    let mut epr_list = fs::File::create(Path::new(out_dir).join("epr.list")).unwrap();

    use std::io::Write;

    let mut count = 0;
    for entry in WalkDir::new(tptp_dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("p") {
            let content = match fs::read_to_string(path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            match parse_tptp(&content) {
                Ok(ast) => {
                    let cat = determine_category(&ast);
                    let line = format!("{}\n", path.display());
                    match cat {
                        Category::FEQ => {
                            let _ = feq_list.write_all(line.as_bytes());
                        }
                        Category::FNE => {
                            let _ = fne_list.write_all(line.as_bytes());
                        }
                        Category::UEQ => {
                            let _ = ueq_list.write_all(line.as_bytes());
                        }
                        Category::EPR => {
                            let _ = epr_list.write_all(line.as_bytes());
                        }
                        Category::Other => {}
                    }
                    count += 1;
                    if count % 1000 == 0 {
                        println!("Categorized {} problems...", count);
                    }
                }
                Err(_) => {
                    // Ignore parse errors (TFF/THF)
                }
            }
        }
    }

    println!("Total successfully categorized: {}", count);
}
