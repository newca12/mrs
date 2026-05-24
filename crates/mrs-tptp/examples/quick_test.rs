use mrs_tptp::parse_tptp;
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <tptp_input>", args[0]);
        return;
    }
    let input = &args[1];
    match parse_tptp(input) {
        Ok(problem) => {
            for f in &problem.formulas {
                println!("Parsed: {:?}", f);
                println!("Display: {}", f);
            }
        }
        Err(e) => eprintln!("Error: {}", e),
    }
}
