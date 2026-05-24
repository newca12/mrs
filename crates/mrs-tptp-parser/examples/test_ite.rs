use mrs_tptp::parse_tptp;
use std::fmt::Write;
use std::fs;

fn main() {
    // Test simple $ite as value
    let test1 = r#"tff(test, axiom, p = $ite(q, a, b))."#;
    match parse_tptp(test1) {
        Ok(_) => println!("test1 OK - $ite as value"),
        Err(e) => println!("test1 FAIL: {}", e),
    }

    // Test $ite as left-hand side of equality (this is the actual failing case)
    let test2 = r#"tff(test, axiom, $ite(q, a, b) = c)."#;
    match parse_tptp(test2) {
        Ok(_) => println!("test2 OK - $ite = value"),
        Err(e) => println!("test2 FAIL: {}", e),
    }

    // Simplest $ite formula
    let test3 = r#"tff(test, axiom, $ite($true, a, b))."#;
    match parse_tptp(test3) {
        Ok(_) => println!("test3 OK - bare $ite"),
        Err(e) => println!("test3 FAIL: {}", e),
    }

    // Parenthesized $ite = value
    let test4 = r#"tff(test, axiom, ($ite(q, a, b)) = c)."#;
    match parse_tptp(test4) {
        Ok(_) => println!("test4 OK - ($ite) = value"),
        Err(e) => println!("test4 FAIL: {}", e),
    }

    // The exact failing formula pattern from the file
    let test5 = r#"tff(test, axiom, ( $ite(p,a,b) = c ) )."#;
    match parse_tptp(test5) {
        Ok(_) => println!("test5 OK - ( $ite = c )"),
        Err(e) => println!("test5 FAIL: {}", e),
    }

    // Now test the actual file
    let path = "/mnt/c/Users/fr22192/tmp/TPTP-v9.2.1/Problems/ITP/ITP232_3.p";
    let content = fs::read_to_string(path).expect("Failed to read file");

    match parse_tptp(&content) {
        Ok(problem) => {
            println!("\nITP232_3.p: OK ({} formulas)", problem.formulas.len());

            // Roundtrip test
            let mut output = String::new();
            for f in &problem.formulas {
                writeln!(output, "{}", f).unwrap();
            }
            match parse_tptp(&output) {
                Ok(reparsed) => {
                    if reparsed.formulas.len() == problem.formulas.len() {
                        println!("Roundtrip OK");
                    } else {
                        println!(
                            "Roundtrip FAIL: {} vs {}",
                            problem.formulas.len(),
                            reparsed.formulas.len()
                        );
                    }
                }
                Err(e) => println!("Roundtrip FAIL: {}", e),
            }
        }
        Err(e) => println!("\nITP232_3.p FAIL: {}", e),
    }
}
