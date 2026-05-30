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
                    let start = i.saturating_sub(5);
                    for (j, ctx_line) in lines.iter().enumerate().take(i + 1).skip(start) {
                        println!("    {}: {}", j + 1, ctx_line);
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
