use crate::symbol::SymbolTable;
use crate::term_bank::{IdAtom, IdClause, TermBank, TermId, TermNode};
use std::collections::HashSet;

pub const SCHEDULE_FEATURE_DIM: usize = 16;

pub fn extract_schedule_features(
    clauses: &[IdClause],
    bank: &TermBank,
    symbols: &SymbolTable,
) -> [f32; SCHEDULE_FEATURE_DIM] {
    let mut f = [0.0; SCHEDULE_FEATURE_DIM];
    if clauses.is_empty() {
        return f;
    }

    let total_count = clauses.len() as f32;
    f[0] = (total_count / 1000.0).tanh();

    let mut unit_count = 0.0;
    let mut horn_count = 0.0;
    let mut eq_free_count = 0.0;
    let mut pure_eq_count = 0.0;
    let mut total_literals = 0.0;
    let mut max_literals = 0.0;
    let mut conjecture_clauses = 0.0;
    let mut total_vars = 0.0;

    let mut total_term_depth = 0.0;
    let mut max_term_depth = 0.0;
    let mut total_terms = 0.0;

    let mut has_eq = false;
    let mut is_ueq = true;

    let mut predicates = HashSet::new();
    let mut functors = HashSet::new();

    for clause in clauses {
        let lit_count = clause.literals.len() as f32;
        total_literals += lit_count;
        if lit_count > max_literals {
            max_literals = lit_count;
        }

        if lit_count == 1.0 {
            unit_count += 1.0;
        } else {
            is_ueq = false;
        }

        let mut pos_lits = 0;
        let mut has_equation = false;
        let mut has_predicate = false;
        let mut clause_vars = 0;

        if clause.distance == 0 {
            conjecture_clauses += 1.0;
        }

        for lit in &clause.literals {
            if lit.positive {
                pos_lits += 1;
            }

            match &lit.atom {
                IdAtom::Pred(sym, args) => {
                    has_predicate = true;
                    is_ueq = false;
                    predicates.insert(*sym);
                    for &arg in args {
                        let (depth, vars) =
                            measure_term_depth_vars_and_funcs(arg, bank, &mut functors);
                        total_term_depth += depth as f32;
                        if (depth as f32) > max_term_depth {
                            max_term_depth = depth as f32;
                        }
                        total_terms += 1.0;
                        clause_vars += vars;
                    }
                }
                IdAtom::Eq(l, r) => {
                    has_equation = true;
                    has_eq = true;
                    let (depth_l, vars_l) =
                        measure_term_depth_vars_and_funcs(*l, bank, &mut functors);
                    let (depth_r, vars_r) =
                        measure_term_depth_vars_and_funcs(*r, bank, &mut functors);
                    total_term_depth += depth_l as f32 + depth_r as f32;
                    if (depth_l as f32) > max_term_depth {
                        max_term_depth = depth_l as f32;
                    }
                    if (depth_r as f32) > max_term_depth {
                        max_term_depth = depth_r as f32;
                    }
                    total_terms += 2.0;
                    clause_vars += vars_l + vars_r;
                }
            }
        }

        if pos_lits <= 1 {
            horn_count += 1.0;
        }
        if !has_equation {
            eq_free_count += 1.0;
        }
        if has_equation && !has_predicate {
            pure_eq_count += 1.0;
        }

        total_vars += clause_vars as f32;
    }

    f[1] = unit_count / total_count;
    f[2] = horn_count / total_count;
    f[3] = eq_free_count / total_count;
    f[4] = pure_eq_count / total_count;
    f[5] = total_literals / total_count;
    f[6] = max_literals;

    if total_terms > 0.0 {
        f[7] = total_term_depth / total_terms;
    }
    f[8] = max_term_depth;

    let all_symbols_count = (predicates.len() + functors.len()) as f32;
    if all_symbols_count > 0.0 {
        f[9] = functors.len() as f32 / all_symbols_count;
        f[10] = predicates.len() as f32 / all_symbols_count;

        let mut skolem_count = 0.0;
        for &sym in predicates.iter().chain(functors.iter()) {
            if (sym.0 as usize) < symbols.len() {
                let name = symbols.resolve(sym);
                if name.starts_with("sk_") || name.contains("sK") {
                    skolem_count += 1.0;
                }
            }
        }
        f[11] = skolem_count / all_symbols_count;
    }

    f[12] = conjecture_clauses / total_count;
    f[13] = total_vars / total_count;
    f[14] = if has_eq { 1.0 } else { 0.0 };
    f[15] = if is_ueq && has_eq { 1.0 } else { 0.0 };

    f
}

fn measure_term_depth_vars_and_funcs(
    term: TermId,
    bank: &TermBank,
    functors: &mut HashSet<crate::SymbolId>,
) -> (usize, usize) {
    match bank.get(term) {
        TermNode::Var(_) => (1, 1),
        TermNode::App(sym, args) => {
            functors.insert(*sym);
            let mut max_depth = 0;
            let mut vars = 0;
            for &arg in args {
                let (d, v) = measure_term_depth_vars_and_funcs(arg, bank, functors);
                max_depth = max_depth.max(d);
                vars += v;
            }
            (max_depth + 1, vars)
        }
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
pub struct ScheduleModel<B: Backend> {
    layer1: Linear<B>,
    layer2: Linear<B>,
    output: Linear<B>,
}

#[cfg(feature = "ml")]
impl<B: Backend> ScheduleModel<B> {
    pub fn new(device: &B::Device) -> Self {
        Self {
            layer1: LinearConfig::new(SCHEDULE_FEATURE_DIM, 32).init(device),
            layer2: LinearConfig::new(32, 16).init(device),
            output: LinearConfig::new(16, 5).init(device),
        }
    }

    pub fn forward(&self, x: Tensor<B, 2>) -> Tensor<B, 2> {
        let x = burn::tensor::activation::relu(self.layer1.forward(x));
        let x = burn::tensor::activation::relu(self.layer2.forward(x));
        self.output.forward(x) // Raw logits for Cross-Entropy loss
    }
}

#[cfg(feature = "ml")]
pub struct ScheduleClassifier<B: Backend> {
    model: ScheduleModel<B>,
    device: B::Device,
}

#[cfg(feature = "ml")]
impl<B: Backend> ScheduleClassifier<B> {
    pub fn new(device: B::Device) -> Self {
        Self {
            model: ScheduleModel::new(&device),
            device,
        }
    }

    pub fn with_model(model: ScheduleModel<B>, device: B::Device) -> Self {
        Self { model, device }
    }

    pub fn classify(&self, features: [f32; SCHEDULE_FEATURE_DIM]) -> &'static str {
        let tensor =
            Tensor::<B, 1>::from_data(features, &self.device).reshape([1, SCHEDULE_FEATURE_DIM]);
        let logits = self.model.forward(tensor);
        let selected_idx = logits.argmax(1).into_data().as_slice::<i64>().unwrap()[0] as usize;
        match selected_idx {
            0 => "casc_fne",
            1 => "casc_feq",
            2 => "casc_ueq",
            3 => "casc_epr",
            4 => "casc_icu",
            _ => "casc",
        }
    }
}

#[cfg(all(test, feature = "ml"))]
mod tests {
    use super::*;
    use burn::backend::NdArray;

    #[test]
    fn test_schedule_model_shape() {
        let device = burn::backend::ndarray::NdArrayDevice::Cpu;
        let model = ScheduleModel::<NdArray>::new(&device);
        let x = Tensor::<NdArray, 2>::zeros([2, SCHEDULE_FEATURE_DIM], &device);
        let out = model.forward(x);
        assert_eq!(out.shape().dims(), [2, 5]);
    }
}
