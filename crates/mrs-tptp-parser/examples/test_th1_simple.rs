use mrs_tptp::parse_tptp;
use mrs_tptp::parser::thf::{thf_top_level_type, thf_type};

fn main() {
    // Test the problematic formula
    let input1 = r#"thf(pt_type,type,(
    pt: 
      ( [ tt, 
          $i ]
      > $o ) ))."#;
    println!("Test 1: pt_type with tuple");
    match parse_tptp(input1) {
        Ok(p) => println!("  OK: {} formulas\n", p.formulas.len()),
        Err(e) => println!("  FAIL: {:?}\n", e),
    }

    // Simpler test
    let input2 = r#"thf(test,type,(f: ( [ tt, $i ] > $o ) ))."#;
    println!("Test 2: simpler");
    match parse_tptp(input2) {
        Ok(p) => println!("  OK: {} formulas\n", p.formulas.len()),
        Err(e) => println!("  FAIL: {:?}\n", e),
    }

    // Even simpler - just the type in a formula
    let input3 = r#"thf(test,type,(f: [ $i, $o ]))."#;
    println!("Test 3: tuple type");
    match parse_tptp(input3) {
        Ok(p) => println!("  OK: {} formulas\n", p.formulas.len()),
        Err(e) => println!("  FAIL: {:?}\n", e),
    }

    // Debug type parsing directly
    println!("\n\n=== Debug Type Parsing ===");

    let type_tests = [
        ("tuple", "[ $i, $o ]"),
        ("tuple arrow", "[ $i, $o ] > $o"),
        ("paren tuple arrow", "( [ $i, $o ] > $o )"),
    ];

    for (name, input) in type_tests {
        println!("\nType test: {}", name);
        println!("Input: '{}'", input);

        let mut s = input;
        println!("  thf_type result: {:?}", thf_type(&mut s));
        println!("  remaining: '{}'", s);

        let mut s2 = input;
        println!(
            "  thf_top_level_type result: {:?}",
            thf_top_level_type(&mut s2)
        );
        println!("  remaining: '{}'", s2);
    }
}
