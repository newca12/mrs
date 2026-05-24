//! Debug THF parsing issues
use mrs_tptp::parse_tptp;
use std::fs;
use std::path::PathBuf;

fn main() {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests");
    path.push("resources");
    path.push("SYN000");
    path.push("SYN000^2.p");

    let content = fs::read_to_string(&path).unwrap();

    // Extract formulas manually
    let mut current_formula = String::new();
    let mut in_formula = false;
    let mut formula_start_line = 0;
    let mut paren_depth = 0;

    for (i, line) in content.lines().enumerate() {
        let line_num = i + 1;
        let trimmed = line.trim();

        // Skip comments and empty lines when not in a formula
        if !in_formula && (trimmed.is_empty() || trimmed.starts_with('%')) {
            continue;
        }

        // Start of a formula
        if !in_formula && (trimmed.starts_with("thf(") || trimmed.starts_with("include(")) {
            in_formula = true;
            formula_start_line = line_num;
            current_formula.clear();
        }

        if in_formula {
            current_formula.push_str(line);
            current_formula.push('\n');

            // Track parenthesis depth
            for c in line.chars() {
                if c == '(' {
                    paren_depth += 1;
                }
                if c == ')' {
                    paren_depth -= 1;
                }
            }

            // Check if formula is complete
            if paren_depth == 0 && trimmed.ends_with('.') {
                // Try parsing
                match parse_tptp(&current_formula) {
                    Ok(problem) => {
                        let preview = current_formula.lines().next().unwrap_or("");
                        let preview = &preview[..preview.len().min(50)];
                        println!(
                            "Line {}: OK - {} ({} formulas)",
                            formula_start_line,
                            preview,
                            problem.formulas.len()
                        );
                    }
                    Err(e) => {
                        let preview = current_formula.lines().next().unwrap_or("");
                        let preview = &preview[..preview.len().min(50)];
                        println!("Line {}: FAIL - {}", formula_start_line, preview);
                        println!("  Formula:\n{}", current_formula);
                        println!("  Error: {:?}\n", e);
                    }
                }
                in_formula = false;
                current_formula.clear();
            }
        }
    }

    println!("\n--- Full Parse ---");
    match parse_tptp(&content) {
        Ok(problem) => {
            println!(
                "OK: {} formulas, {} includes",
                problem.formulas.len(),
                problem.includes.len()
            );
        }
        Err(e) => {
            println!("FAIL: {:?}", e);
        }
    }
}
