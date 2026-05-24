use mrs_tptp::parse_tptp;
use std::env;
use std::fs;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() > 1 {
        // Read and parse a file
        let path = &args[1];
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Error reading {}: {}", path, e);
                return;
            }
        };

        match parse_tptp(&content) {
            Ok(problem) => {
                println!(
                    "OK: {} formulas, {} includes",
                    problem.formulas.len(),
                    problem.includes.len()
                );
                for f in &problem.formulas {
                    println!("  {}", f.name());
                }
            }
            Err(e) => println!("FAIL: {:?}", e),
        }
        return;
    }

    // Default: Test the problematic formula
    let test = r#"thf(fact_38_split__paired__All,axiom,
    ! [P_10: produc652964533on_val > $o] :
      ( ( !! @ produc652964533on_val @ P_10 )
    <=> ! [A_13: produc1746408499on_val,B_3: produc1746408499on_val] : ( P_10 @ ( produc345758123on_val @ A_13 @ B_3 ) ) ) )."#;

    match parse_tptp(test) {
        Ok(problem) => println!("OK: {} formulas", problem.formulas.len()),
        Err(e) => println!("FAIL: {:?}", e),
    }

    // Also test the simpler !! construct
    let test2 = r#"thf(test, axiom, !! @ $i @ p)."#;

    match parse_tptp(test2) {
        Ok(problem) => println!("OK test2: {} formulas", problem.formulas.len()),
        Err(e) => println!("FAIL test2: {:?}", e),
    }
}
