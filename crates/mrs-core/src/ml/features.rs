use crate::symbol::SymbolTable;
use crate::term_bank::{IdAtom, IdClause, TermBank, TermId, TermNode};
use std::hash::{Hash, Hasher};

pub const FEATURE_DIM: usize = 128;
pub const HASH_BUCKETS: usize = 120; // buckets 8..128
pub const SCHEMA_VERSION: u32 = 1; // bump on any layout change

/// Layout (all values normalized to roughly [0, 1]):
/// - [0] num_literals
/// - [1] num_positive_literals
/// - [2] num_negative_literals
/// - [3] total symbol weight (clause_weight, normalized)
/// - [4] max term depth (traversed over the TermBank)
/// - [5] total term size
/// - [6] distance-to-conjecture bucket (IdClause.distance; <100 => near-goal)
/// - [7] is_unit / is_horn flags packed
/// - [8..128] symbol-NAME hash buckets: for each symbol occurrence,
///   bucket = fxhash(symbol_name) % HASH_BUCKETS; counts then log/L2-normalized.
pub fn extract(
    clause: &IdClause,
    bank: &TermBank,
    symbols: &SymbolTable,
    weight: f32,
) -> [f32; FEATURE_DIM] {
    let mut f = [0.0f32; FEATURE_DIM];

    let num_lits = clause.literals.len();
    let mut pos_lits = 0;
    let mut neg_lits = 0;

    let mut max_depth = 0;
    let mut total_size = 0;

    let mut hash_counts = [0.0f32; HASH_BUCKETS];

    for lit in &clause.literals {
        if lit.positive {
            pos_lits += 1;
        } else {
            neg_lits += 1;
        }

        match &lit.atom {
            IdAtom::Pred(sym, args) => {
                let name = symbols.resolve(*sym);
                let mut hasher = rustc_hash::FxHasher::default();
                name.as_bytes().hash(&mut hasher);
                let bucket = (hasher.finish() as usize) % HASH_BUCKETS;
                hash_counts[bucket] += 1.0;
                total_size += 1;

                for &arg in args {
                    let (depth, size) = measure_term(arg, bank, symbols, &mut hash_counts);
                    max_depth = max_depth.max(depth + 1);
                    total_size += size;
                }
            }
            IdAtom::Eq(l, r) => {
                let (depth_l, size_l) = measure_term(*l, bank, symbols, &mut hash_counts);
                let (depth_r, size_r) = measure_term(*r, bank, symbols, &mut hash_counts);
                max_depth = max_depth.max(depth_l).max(depth_r);
                total_size += size_l + size_r;
            }
        }
    }

    // Normalize structural features
    f[0] = (num_lits as f32 / 20.0).clamp(0.0, 1.0);
    f[1] = (pos_lits as f32 / 10.0).clamp(0.0, 1.0);
    f[2] = (neg_lits as f32 / 10.0).clamp(0.0, 1.0);
    f[3] = (weight / 200.0).clamp(0.0, 1.0);
    f[4] = (max_depth as f32 / 20.0).clamp(0.0, 1.0);
    f[5] = (total_size as f32 / 100.0).clamp(0.0, 1.0);
    f[6] = if clause.distance < 100 { 1.0 } else { 0.0 };

    let is_unit = if num_lits == 1 { 1.0 } else { 0.0 };
    let is_horn = if pos_lits <= 1 { 1.0 } else { 0.0 };
    f[7] = is_unit * 0.5 + is_horn * 0.5;

    // Log/L2 normalize hash buckets
    let mut sum_sq = 0.0;
    for count in hash_counts.iter_mut().take(HASH_BUCKETS) {
        if *count > 0.0 {
            *count = (*count + 1.0).ln();
            sum_sq += *count * *count;
        }
    }

    let norm = if sum_sq > 0.0 { sum_sq.sqrt() } else { 1.0 };
    for i in 0..HASH_BUCKETS {
        f[8 + i] = hash_counts[i] / norm;
    }

    f
}

fn measure_term(
    term: TermId,
    bank: &TermBank,
    symbols: &SymbolTable,
    hash_counts: &mut [f32; HASH_BUCKETS],
) -> (usize, usize) {
    match bank.get(term) {
        TermNode::Var(_) => (1, 1),
        TermNode::App(sym, args) => {
            let name = symbols.resolve(*sym);
            let mut hasher = rustc_hash::FxHasher::default();
            name.as_bytes().hash(&mut hasher);
            let bucket = (hasher.finish() as usize) % HASH_BUCKETS;
            hash_counts[bucket] += 1.0;

            let mut max_depth = 0;
            let mut total_size = 1;

            for &arg in args {
                let (depth, size) = measure_term(arg, bank, symbols, hash_counts);
                max_depth = max_depth.max(depth);
                total_size += size;
            }

            (max_depth + 1, total_size)
        }
    }
}
