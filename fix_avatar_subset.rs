fn is_subset(a: &[u32], b: &[u32]) -> bool {
    a.iter().all(|x| b.contains(x))
}
