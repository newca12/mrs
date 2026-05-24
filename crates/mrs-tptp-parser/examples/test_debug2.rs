use mrs_tptp::parse_tptp;
use std::fs;

fn main() {
    let content =
        fs::read_to_string("/mnt/c/Users/fr22192/tmp/TPTP-v9.2.1/Problems/SWW/SWW972_1.p").unwrap();

    // Try to parse each line to find where it breaks
    let lines: Vec<&str> = content.lines().collect();
    let mut accum = String::new();

    for (i, line) in lines.iter().enumerate() {
        accum.push_str(line);
        accum.push('\n');

        // Only test when we have a complete formula (line ends with period and non-comment)
        let trimmed = line.trim();
        if trimmed.ends_with(").") && !trimmed.starts_with('%') {
            match parse_tptp(&accum) {
                Ok(p) => {
                    println!(
                        "Line {}: OK - {} formulas parsed so far",
                        i + 1,
                        p.formulas.len()
                    );
                }
                Err(e) => {
                    println!("Line {}: FAIL - {}", i + 1, e);
                    println!(
                        "Last few lines:\n{}",
                        lines[i.saturating_sub(5)..=i].join("\n")
                    );
                    break;
                }
            }
        }
    }
}
