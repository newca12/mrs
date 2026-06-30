#[derive(wincode::SchemaWrite, wincode::SchemaRead, Clone, Debug)]
pub struct LabeledSample {
    pub label: f32,
    pub feats: [f32; super::features::FEATURE_DIM],
}

#[derive(wincode::SchemaWrite, wincode::SchemaRead, Clone, Debug)]
pub struct ScheduleSample {
    pub label_idx: u32,
    pub feats: [f32; super::schedule_classifier::SCHEDULE_FEATURE_DIM],
}

#[derive(wincode::SchemaWrite, wincode::SchemaRead, Clone, Debug)]
pub struct PremiseSample {
    pub label: f32,
    pub feats: [f32; super::premise_selector::PREMISE_FEATURE_DIM],
}
