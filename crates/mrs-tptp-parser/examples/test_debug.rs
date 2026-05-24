use mrs_tptp::parse_tptp;
fn main() {
    let input = r#"thf(is_symmetric_property,conjecture,
    ( is_symmetric @ ( (@=) @ ( $i > a_type ) ) ))."#;
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
