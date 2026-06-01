use mrs_tptp::{AnnotatedFormula, parse_tptp};

fn main() {
    let s = "fof(f15, plain, \
             ( ! [X0] : ( ( ? [X1] : (op2(X1,X1) != X0 & sorti2(X1)) ) \
                          => (op2(sK1(X0),sK1(X0)) != X0 & sorti2(sK1(X0))) ) ), \
             introduced(definition,[],[skolem_symbol_introduction])).";
    let p = parse_tptp(s).expect("parse");
    if let AnnotatedFormula::FOF(f) = &p.formulas[0] {
        println!("{:#?}", f.formula);
    }
}
