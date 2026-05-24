use mrs_tptp::parser::tff;
use winnow::Parser;

fn main() {
    let input = "$let(c: $int, c:= 27, pl1(c))";
    let mut input_str = input;

    println!("Parsing: {}", input);

    // Try tff_formula directly
    match tff::tff_formula.parse_next(&mut input_str) {
        Ok(result) => {
            println!("OK: {:?}", result);
            println!("Remaining: {:?}", input_str);
        }
        Err(e) => println!("Error: {:?}, remaining: {:?}", e, input_str),
    }
}
