fn main() {
    let content = std::fs::read_to_string("tests/resources/SYN000/SYN000_4.p").unwrap();

    let lines: Vec<&str> = content.lines().collect();
    let mut current_input = String::new();

    for (i, line) in lines.iter().enumerate() {
        current_input.push_str(line);
        current_input.push('\n');

        if line.ends_with(").") || line.ends_with("). ") {
            match mrs_tptp::parse_tptp(&current_input) {
                Ok(p) => {
                    println!("Line {}: OK - {} formulas", i + 1, p.formulas.len());
                }
                Err(e) => {
                    println!("Line {}: FAIL at:", i + 1);
                    println!("  Content: {}", line.trim());
                    println!("  Error: {:?}", e);
                    let start = if i > 5 { i - 5 } else { 0 };
                    for j in start..=i {
                        println!("    {}: {}", j + 1, lines[j]);
                    }
                    return;
                }
            }
        }
    }

    match mrs_tptp::parse_tptp(&content) {
        Ok(p) => println!("SUCCESS: {} formulas", p.formulas.len()),
        Err(e) => println!("FINAL FAIL: {:?}", e),
    }
}
