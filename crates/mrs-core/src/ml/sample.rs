#[derive(wincode::SchemaWrite, wincode::SchemaRead, Clone, Debug)]
pub struct LabeledSample {
    pub label: f32,
    pub feats: [f32; super::features::FEATURE_DIM],
}
