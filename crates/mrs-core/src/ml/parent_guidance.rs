use crate::symbol::SymbolTable;
use crate::term_bank::{IdAtom, IdClause, TermBank, TermId, TermNode};
use std::hash::{Hash, Hasher};

/// Fixed inference feature vector: operation metadata plus hashed parent
/// vocabulary. No clause IDs or problem-local symbol indices enter the model.
pub const PARENT_FEATURE_DIM: usize = 32;
#[allow(dead_code)]
const HASH_BUCKETS: usize = PARENT_FEATURE_DIM - 12;
pub const PARENT_GUIDANCE_SCHEMA: u32 = 1;
pub type ParentPairSample = (
    [f32; PARENT_FEATURE_DIM],
    [crate::clause::ClauseId; 2],
    InferenceKind,
    Option<bool>,
);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum InferenceKind {
    Resolution = 0,
    GivenAsEqualitySource = 1,
    GivenAsTarget = 2,
    SelfSuperposition = 3,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ParentGuidanceModel {
    pub schema_version: u32,
    pub inference_kind: u8,
    pub weights: [f32; PARENT_FEATURE_DIM],
    pub bias: f32,
}

impl ParentGuidanceModel {
    pub fn valid(&self) -> bool {
        self.schema_version == PARENT_GUIDANCE_SCHEMA
            && self.inference_kind <= InferenceKind::SelfSuperposition as u8
            && self.bias.is_finite()
            && self.weights.iter().all(|weight| weight.is_finite())
    }

    pub fn score(&self, features: &[f32; PARENT_FEATURE_DIM]) -> f32 {
        self.bias
            + self
                .weights
                .iter()
                .zip(features)
                .map(|(w, x)| w * x)
                .sum::<f32>()
    }
}

pub fn extract_parent_features(
    given: &IdClause,
    partner: &IdClause,
    kind: InferenceKind,
    selected_literals: usize,
    bank: &TermBank,
    symbols: &SymbolTable,
) -> [f32; PARENT_FEATURE_DIM] {
    let mut features = [0.0; PARENT_FEATURE_DIM];
    let given_shape = clause_shape(given, bank, symbols, &mut features[12..]);
    let partner_shape = clause_shape(partner, bank, symbols, &mut features[12..]);

    features[0] = kind as u8 as f32;
    features[1] = (given.literals.len() as f32 / 20.0).clamp(0.0, 1.0);
    features[2] = (partner.literals.len() as f32 / 20.0).clamp(0.0, 1.0);
    features[3] = ratio(given_shape.0, given.literals.len());
    features[4] = ratio(partner_shape.0, partner.literals.len());
    features[5] = f32::from(given_shape.1);
    features[6] = f32::from(partner_shape.1);
    features[7] = (given_shape.2 as f32 / 20.0).clamp(0.0, 1.0);
    features[8] = (partner_shape.2 as f32 / 20.0).clamp(0.0, 1.0);
    features[9] = (given_shape.3 as f32 / 100.0).clamp(0.0, 1.0);
    features[10] = (partner_shape.3 as f32 / 100.0).clamp(0.0, 1.0);
    features[11] = (selected_literals as f32 / 16.0).clamp(0.0, 1.0);

    let norm = features[12..]
        .iter_mut()
        .map(|bucket| {
            if *bucket > 0.0 {
                *bucket = (1.0 + *bucket).ln();
            }
            *bucket * *bucket
        })
        .sum::<f32>()
        .sqrt()
        .max(1.0);
    for bucket in &mut features[12..] {
        *bucket /= norm;
    }
    features
}

fn ratio(value: usize, total: usize) -> f32 {
    value as f32 / total.max(1) as f32
}

fn clause_shape(
    clause: &IdClause,
    bank: &TermBank,
    symbols: &SymbolTable,
    buckets: &mut [f32],
) -> (usize, bool, usize, usize) {
    let mut positive = 0;
    let mut equality = false;
    let mut depth = 0;
    let mut size = 0;
    for literal in &clause.literals {
        positive += usize::from(literal.positive);
        match &literal.atom {
            IdAtom::Pred(symbol, args) => {
                hash_symbol(*symbol, symbols, buckets);
                size += 1;
                for &arg in args {
                    let (d, s) = term_shape(arg, bank, symbols, buckets);
                    depth = depth.max(d);
                    size += s;
                }
            }
            IdAtom::Eq(left, right) => {
                equality = true;
                let (dl, sl) = term_shape(*left, bank, symbols, buckets);
                let (dr, sr) = term_shape(*right, bank, symbols, buckets);
                depth = depth.max(dl).max(dr);
                size += sl + sr;
            }
        }
    }
    (positive, equality, depth, size)
}

fn term_shape(
    term: TermId,
    bank: &TermBank,
    symbols: &SymbolTable,
    buckets: &mut [f32],
) -> (usize, usize) {
    match bank.get(term) {
        TermNode::Var(_) => (1, 1),
        TermNode::App(symbol, args) => {
            hash_symbol(*symbol, symbols, buckets);
            let mut depth = 0;
            let mut size = 1;
            for &arg in args {
                let (d, s) = term_shape(arg, bank, symbols, buckets);
                depth = depth.max(d);
                size += s;
            }
            (depth + 1, size)
        }
    }
}

fn hash_symbol(symbol: crate::SymbolId, symbols: &SymbolTable, buckets: &mut [f32]) {
    if (symbol.0 as usize) >= symbols.len() || buckets.is_empty() {
        return;
    }
    let mut hasher = rustc_hash::FxHasher::default();
    symbols.resolve(symbol).as_bytes().hash(&mut hasher);
    buckets[hasher.finish() as usize % buckets.len()] += 1.0;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clause::{Clause, ClauseId, ClauseSource, Literal};
    use crate::{Atom, SymbolTable, Term};

    #[test]
    fn features_are_finite_stable_and_encode_inference_kind() {
        let mut symbols = SymbolTable::new();
        let p = symbols.intern("p");
        let q = symbols.intern("q");
        let a = symbols.intern("a");
        let left = Clause::new(
            ClauseId(1),
            vec![Literal::pos(Atom::pred(p, vec![Term::constant(a)]))],
            ClauseSource::Input {
                name: "a".into(),
                role: "axiom".into(),
            },
        );
        let right = Clause::new(
            ClauseId(2),
            vec![Literal::neg(Atom::pred(q, vec![Term::var(0)]))],
            ClauseSource::Input {
                name: "b".into(),
                role: "axiom".into(),
            },
        );
        let mut bank = TermBank::new();
        let left = bank.clause_from_legacy(&left);
        let right = bank.clause_from_legacy(&right);
        let a =
            extract_parent_features(&left, &right, InferenceKind::Resolution, 2, &bank, &symbols);
        let b =
            extract_parent_features(&left, &right, InferenceKind::Resolution, 2, &bank, &symbols);
        let c = extract_parent_features(
            &left,
            &right,
            InferenceKind::GivenAsTarget,
            2,
            &bank,
            &symbols,
        );
        assert!(a.iter().all(|feature| feature.is_finite()));
        assert_eq!(a, b);
        assert_ne!(a[0], c[0]);
    }

    #[test]
    fn model_validation_checks_schema_kind_and_finiteness() {
        let model = ParentGuidanceModel {
            schema_version: PARENT_GUIDANCE_SCHEMA,
            inference_kind: InferenceKind::Resolution as u8,
            weights: [0.0; PARENT_FEATURE_DIM],
            bias: 0.0,
        };
        assert!(model.valid());
        let bad = ParentGuidanceModel {
            inference_kind: 99,
            ..model.clone()
        };
        assert!(!bad.valid());
        let bad = ParentGuidanceModel {
            bias: f32::NAN,
            ..model
        };
        assert!(!bad.valid());
    }
}
