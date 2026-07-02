use crate::symbol::SymbolTable;
use crate::term_bank::{IdAtom, IdClause, TermBank, TermId, TermNode};
use std::collections::HashSet;
use std::hash::{Hash, Hasher};

pub const PREMISE_FEATURE_DIM: usize = 24;
const HASH_BUCKETS: usize = 12; // Used for pairwise bucket overlap

pub struct ConjectureContext {
    predicates: HashSet<crate::SymbolId>,
    functors: HashSet<crate::SymbolId>,
    symbol_counts: [f32; HASH_BUCKETS],
}

impl ConjectureContext {
    pub fn new(conjectures: &[IdClause], bank: &TermBank, symbols: &SymbolTable) -> Self {
        let mut predicates = HashSet::new();
        let mut functors = HashSet::new();
        let mut symbol_counts = [0.0; HASH_BUCKETS];

        for c in conjectures {
            extract_symbols(c, bank, &mut predicates, &mut functors);
            count_symbols(c, bank, symbols, &mut symbol_counts);
        }

        Self {
            predicates,
            functors,
            symbol_counts,
        }
    }
}

pub fn extract_premise_features(
    axiom: &IdClause,
    ctx: &ConjectureContext,
    bank: &TermBank,
    symbols: &SymbolTable,
) -> [f32; PREMISE_FEATURE_DIM] {
    let mut f = [0.0; PREMISE_FEATURE_DIM];

    // 1. Structural Simplicity [0..8]
    let lit_count = axiom.literals.len() as f32;
    f[0] = (lit_count / 10.0).clamp(0.0, 1.0);

    let mut pos_lits = 0.0;
    let mut has_eq = false;
    let mut max_depth = 0;
    let mut total_size = 0;
    let mut total_vars = 0;

    for lit in &axiom.literals {
        if lit.positive {
            pos_lits += 1.0;
        }
        match &lit.atom {
            IdAtom::Pred(_, args) => {
                for &arg in args {
                    let (d, s, v) = measure_term(arg, bank);
                    max_depth = max_depth.max(d);
                    total_size += s;
                    total_vars += v;
                }
            }
            IdAtom::Eq(l, r) => {
                has_eq = true;
                let (dl, sl, vl) = measure_term(*l, bank);
                let (dr, sr, vr) = measure_term(*r, bank);
                max_depth = max_depth.max(dl).max(dr);
                total_size += sl + sr;
                total_vars += vl + vr;
            }
        }
    }

    f[1] = (max_depth as f32 / 10.0).clamp(0.0, 1.0);
    f[2] = if pos_lits <= 1.0 { 1.0 } else { 0.0 }; // is_horn
    f[3] = if lit_count == 1.0 { 1.0 } else { 0.0 }; // is_unit
    f[4] = (total_vars as f32 / 10.0).clamp(0.0, 1.0);
    f[5] = (total_size as f32 / 50.0).clamp(0.0, 1.0);
    f[6] = if lit_count > 0.0 {
        pos_lits / lit_count
    } else {
        0.0
    };
    f[7] = if has_eq { 1.0 } else { 0.0 };

    // 2. Pairwise Symbol Overlap [8..24]
    let mut ax_preds = HashSet::new();
    let mut ax_funcs = HashSet::new();
    extract_symbols(axiom, bank, &mut ax_preds, &mut ax_funcs);

    // Predicate overlap
    let pred_inter = ax_preds.intersection(&ctx.predicates).count() as f32;
    let pred_union = ax_preds.union(&ctx.predicates).count() as f32;
    f[8] = if pred_union > 0.0 {
        pred_inter / pred_union
    } else {
        0.0
    };

    // Functor overlap
    let func_inter = ax_funcs.intersection(&ctx.functors).count() as f32;
    let func_union = ax_funcs.union(&ctx.functors).count() as f32;
    f[9] = if func_union > 0.0 {
        func_inter / func_union
    } else {
        0.0
    };

    // TF-IDF / Cosine similarity over hashed buckets
    let mut ax_counts = [0.0; HASH_BUCKETS];
    count_symbols(axiom, bank, symbols, &mut ax_counts);

    let mut dot = 0.0;
    let mut norm_ax = 0.0;
    let mut norm_ctx = 0.0;
    for i in 0..HASH_BUCKETS {
        dot += ax_counts[i] * ctx.symbol_counts[i];
        norm_ax += ax_counts[i] * ax_counts[i];
        norm_ctx += ctx.symbol_counts[i] * ctx.symbol_counts[i];
    }

    let sim = if norm_ax > 0.0 && norm_ctx > 0.0 {
        dot / (norm_ax.sqrt() * norm_ctx.sqrt())
    } else {
        0.0
    };
    f[10] = sim;

    // SInE distance
    f[11] = 1.0 / (1.0 + axiom.distance as f32);

    // Remaining 12 features: Element-wise bucket overlap (Jaccard on buckets)
    for i in 0..HASH_BUCKETS {
        let min_c = ax_counts[i].min(ctx.symbol_counts[i]);
        let max_c = ax_counts[i].max(ctx.symbol_counts[i]);
        f[12 + i] = if max_c > 0.0 { min_c / max_c } else { 0.0 };
    }

    f
}

fn measure_term(term: TermId, bank: &TermBank) -> (usize, usize, usize) {
    match bank.get(term) {
        TermNode::Var(_) => (1, 1, 1),
        TermNode::App(_, args) => {
            let mut max_depth = 0;
            let mut size = 1;
            let mut vars = 0;
            for &arg in args {
                let (d, s, v) = measure_term(arg, bank);
                max_depth = max_depth.max(d);
                size += s;
                vars += v;
            }
            (max_depth + 1, size, vars)
        }
    }
}

fn extract_symbols(
    clause: &IdClause,
    bank: &TermBank,
    preds: &mut HashSet<crate::SymbolId>,
    funcs: &mut HashSet<crate::SymbolId>,
) {
    for lit in &clause.literals {
        match &lit.atom {
            IdAtom::Pred(sym, args) => {
                preds.insert(*sym);
                for &arg in args {
                    extract_term_funcs(arg, bank, funcs);
                }
            }
            IdAtom::Eq(l, r) => {
                extract_term_funcs(*l, bank, funcs);
                extract_term_funcs(*r, bank, funcs);
            }
        }
    }
}

fn extract_term_funcs(term: TermId, bank: &TermBank, funcs: &mut HashSet<crate::SymbolId>) {
    if let TermNode::App(sym, args) = bank.get(term) {
        funcs.insert(*sym);
        for &arg in args {
            extract_term_funcs(arg, bank, funcs);
        }
    }
}

fn count_symbols(
    clause: &IdClause,
    bank: &TermBank,
    symbols: &SymbolTable,
    counts: &mut [f32; HASH_BUCKETS],
) {
    for lit in &clause.literals {
        match &lit.atom {
            IdAtom::Pred(sym, args) => {
                hash_sym(*sym, symbols, counts);
                for &arg in args {
                    count_term_symbols(arg, bank, symbols, counts);
                }
            }
            IdAtom::Eq(l, r) => {
                count_term_symbols(*l, bank, symbols, counts);
                count_term_symbols(*r, bank, symbols, counts);
            }
        }
    }
}

fn count_term_symbols(
    term: TermId,
    bank: &TermBank,
    symbols: &SymbolTable,
    counts: &mut [f32; HASH_BUCKETS],
) {
    if let TermNode::App(sym, args) = bank.get(term) {
        hash_sym(*sym, symbols, counts);
        for &arg in args {
            count_term_symbols(arg, bank, symbols, counts);
        }
    }
}

fn hash_sym(sym: crate::SymbolId, symbols: &SymbolTable, counts: &mut [f32; HASH_BUCKETS]) {
    if (sym.0 as usize) < symbols.len() {
        let name = symbols.resolve(sym);
        let mut hasher = rustc_hash::FxHasher::default();
        name.as_bytes().hash(&mut hasher);
        let bucket = (hasher.finish() as usize) % HASH_BUCKETS;
        counts[bucket] += 1.0;
    }
}

#[cfg(feature = "ml")]
use burn::module::Module;
#[cfg(feature = "ml")]
use burn::nn::{Linear, LinearConfig};
#[cfg(feature = "ml")]
use burn::tensor::Tensor;
#[cfg(feature = "ml")]
use burn::tensor::backend::Backend;

#[cfg(feature = "ml")]
#[derive(Module, Debug)]
pub struct PremiseModel<B: Backend> {
    layer1: Linear<B>,
    layer2: Linear<B>,
    layer3: Linear<B>,
    output: Linear<B>,
}

#[cfg(feature = "ml")]
impl<B: Backend> PremiseModel<B> {
    pub fn new(device: &B::Device) -> Self {
        Self {
            layer1: LinearConfig::new(PREMISE_FEATURE_DIM, 256).init(device),
            layer2: LinearConfig::new(256, 128).init(device),
            layer3: LinearConfig::new(128, 64).init(device),
            output: LinearConfig::new(64, 1).init(device),
        }
    }

    pub fn forward(&self, x: Tensor<B, 2>) -> Tensor<B, 2> {
        let x = burn::tensor::activation::gelu(self.layer1.forward(x));
        let x = burn::tensor::activation::gelu(self.layer2.forward(x));
        let x = burn::tensor::activation::gelu(self.layer3.forward(x));
        self.output.forward(x) // Raw logit for BCEWithLogitsLoss
    }
}

#[cfg(feature = "ml")]
pub struct PremiseSelector<B: Backend> {
    model: PremiseModel<B>,
    device: B::Device,
}

#[cfg(feature = "ml")]
impl<B: Backend> PremiseSelector<B> {
    pub fn new(device: B::Device) -> Self {
        Self {
            model: PremiseModel::new(&device),
            device,
        }
    }

    pub fn load_from_file(path: &str, device: &B::Device) -> Result<Self, burn::record::RecorderError> {
        use burn::record::{BinFileRecorder, Recorder};
        let recorder = BinFileRecorder::<burn::record::HalfPrecisionSettings>::default();
        let record = recorder.load(path.into(), device)?;
        let model = PremiseModel::new(device).load_record(record);
        Ok(Self { model, device: device.clone() })
    }

    pub fn with_model(model: PremiseModel<B>, device: B::Device) -> Self {
        Self { model, device }
    }

    pub fn evaluate_score(&self, features: [f32; PREMISE_FEATURE_DIM]) -> f32 {
        let tensor =
            Tensor::<B, 1>::from_data(features, &self.device).reshape([1, PREMISE_FEATURE_DIM]);
        let logit = self
            .model
            .forward(tensor)
            .into_data()
            .as_slice::<f32>()
            .unwrap()[0];
        // Apply sigmoid to return a relevance score in [0, 1]
        1.0 / (1.0 + (-logit).exp())
    }

    pub fn select_premises(
        &self,
        axioms: Vec<IdClause>,
        conjectures: &[IdClause],
        keep_ratio: f32,
        bank: &TermBank,
        symbols: &SymbolTable,
    ) -> Vec<IdClause> {
        let ctx = ConjectureContext::new(conjectures, bank, symbols);
        let mut scored_axioms = Vec::with_capacity(axioms.len());

        for axiom in axioms {
            let feats = extract_premise_features(&axiom, &ctx, bank, symbols);
            let score = self.evaluate_score(feats);
            scored_axioms.push((axiom, score));
        }

        scored_axioms.sort_by(|(_, s1), (_, s2)| s2.partial_cmp(s1).unwrap());

        let keep_count = ((scored_axioms.len() as f32) * keep_ratio).round() as usize;
        let keep_count = keep_count.max(10); // Keep at least 10

        scored_axioms
            .into_iter()
            .take(keep_count)
            .map(|(ax, _)| ax)
            .collect()
    }
}

#[cfg(all(test, feature = "ml"))]
mod tests {
    use super::*;
    use burn::backend::NdArray;

    #[test]
    fn test_premise_model_shape() {
        let device = burn::backend::ndarray::NdArrayDevice::Cpu;
        let model = PremiseModel::<NdArray>::new(&device);
        let x = Tensor::<NdArray, 2>::zeros([2, PREMISE_FEATURE_DIM], &device);
        let out = model.forward(x);
        assert_eq!(out.shape().dims(), [2, 1]);
    }
}
