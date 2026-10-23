fn main() {
    let clauses = vec![vec![1, 2], vec![-1], vec![-2]];
    let trace = mrs_cadical::trace_manifest(&clauses, 2).expect("manifest is UNSAT");
    print!("{}", String::from_utf8(trace).expect("ASCII FRAT"));
}
